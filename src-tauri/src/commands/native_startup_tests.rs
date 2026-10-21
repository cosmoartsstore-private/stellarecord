mod startup {
    use std::ffi::{ c_void, OsString };
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::AsRawHandle;
    use std::fs::{self, File, OpenOptions};
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Mutex;
    use std::time::Duration;
    use std::os::windows::process::CommandExt;
    use sha2::Digest;

    // 専用portと同じsingle-instance名だけを直列化する。失敗時も各fixtureの所有資源はDropで終了する。
    static NATIVE_STARTUP_TEST_LOCK: Mutex<()> = Mutex::new(());
    type StartupHandle = *mut c_void;
    type StartupTestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
    #[repr(C)]
    struct StartupInformation { size: u32, reserved: *mut u16, desktop: *mut u16, title: *mut u16, x: u32, y: u32, width: u32, height: u32, columns: u32, rows: u32, fill: u32, flags: u32, show: u16, reserved_bytes: u16, reserved_data: *mut u8, input: StartupHandle, output: StartupHandle, error: StartupHandle }
    #[repr(C)]
    #[derive(Default)]
    struct StartupProcessInformation { process: StartupHandle, thread: StartupHandle, pid: u32, thread_id: u32 }
    #[repr(C)]
    #[derive(Default)]
    struct StartupJobBasicLimits { process_time: i64, job_time: i64, flags: u32, minimum: usize, maximum: usize, active_processes: u32, affinity: usize, priority: u32, scheduling: u32 }
    #[repr(C)]
    #[derive(Default)]
    struct StartupJobLimits { basic: StartupJobBasicLimits, io: [u64; 6], memory: [usize; 4] }
    #[repr(C)]
    #[derive(Default)]
    struct StartupJobAccounting { times: [i64; 4], counts: [u32; 4] }

    #[link(name = "kernel32")]
    extern "system" {
        fn CloseHandle(handle: StartupHandle) -> i32;
        fn GetCurrentThreadId() -> u32;
        fn GetLastError() -> u32;
        fn SetLastError(error: u32);
        fn SetHandleInformation(handle: StartupHandle, mask: u32, flags: u32) -> i32;
        fn CreateProcessW(application: *const u16, command: *mut u16, process_security: *const c_void, thread_security: *const c_void, inherit: i32, flags: u32, environment: *const c_void, directory: *const u16, startup: *const StartupInformation, information: *mut StartupProcessInformation) -> i32;
        fn WaitForSingleObject(handle: StartupHandle, milliseconds: u32) -> u32;
        fn GetExitCodeProcess(handle: StartupHandle, code: *mut u32) -> i32;
        fn TerminateProcess(handle: StartupHandle, code: u32) -> i32;
        fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> StartupHandle;
        fn SetInformationJobObject(job: StartupHandle, class: i32, information: *const c_void, bytes: u32) -> i32;
        fn AssignProcessToJobObject(job: StartupHandle, process: StartupHandle) -> i32;
        fn TerminateJobObject(job: StartupHandle, code: u32) -> i32;
        fn QueryInformationJobObject(job: StartupHandle, class: i32, information: *mut c_void, bytes: u32, returned: *mut u32) -> i32;
        fn ResumeThread(thread: StartupHandle) -> u32;
        fn CreateMutexW(attributes: *const c_void, owner: i32, name: *const u16) -> StartupHandle;
    }
    #[link(name = "user32")]
    extern "system" {
        fn CreateDesktopW(name: *const u16, device: *const u16, mode: *const c_void, flags: u32, access: u32, attributes: *const c_void) -> StartupHandle;
        fn CloseDesktop(desktop: StartupHandle) -> i32;
        fn OpenInputDesktop(flags: u32, inherit: i32, access: u32) -> StartupHandle;
        fn GetThreadDesktop(thread: u32) -> StartupHandle;
        fn GetUserObjectInformationW(object: StartupHandle, index: i32, information: *mut c_void, bytes: u32, required: *mut u32) -> i32;
        fn EnumDesktopWindows(desktop: StartupHandle, callback: Option<unsafe extern "system" fn(StartupHandle, isize) -> i32>, parameter: isize) -> i32;
        fn GetWindowThreadProcessId(window: StartupHandle, pid: *mut u32) -> u32;
        fn GetWindowTextW(window: StartupHandle, text: *mut u16, maximum: i32) -> i32;
        fn EnumChildWindows(window: StartupHandle, callback: Option<unsafe extern "system" fn(StartupHandle, isize) -> i32>, parameter: isize) -> i32;
        fn GetClassNameW(window: StartupHandle, name: *mut u16, maximum: i32) -> i32;
        fn GetDlgItem(dialog: StartupHandle, identifier: i32) -> StartupHandle;
        fn IsWindowVisible(window: StartupHandle) -> i32;
        fn SendMessageTimeoutW(window: StartupHandle, message: u32, wparam: usize, lparam: isize, flags: u32, timeout: u32, result: *mut usize) -> isize;
        fn PostMessageW(window: StartupHandle, message: u32, wparam: usize, lparam: isize) -> i32;
    }

    struct NativeStartupHandle(StartupHandle);
    impl NativeStartupHandle { fn new(handle: StartupHandle) -> Self { assert!(!handle.is_null(), "専用handleを作成できません: {}", std::io::Error::last_os_error()); Self(handle) } }
    impl Drop for NativeStartupHandle { fn drop(&mut self) { let _ = unsafe { CloseHandle(self.0) }; } }
    fn startup_wide(text: &str) -> Vec<u16> { text.encode_utf16().chain([0]).collect() }
    fn native_startup_desktop_name(handle: StartupHandle) -> String {
        assert!(!handle.is_null()); let mut buffer = [0_u16; 512]; let mut required = 0;
        assert_ne!(unsafe { GetUserObjectInformationW(handle, 2, buffer.as_mut_ptr().cast(), u32::try_from(std::mem::size_of_val(&buffer)).unwrap(), &raw mut required) }, 0);
        String::from_utf16(&buffer[..buffer.iter().position(|value| *value == 0).unwrap()]).unwrap()
    }
    fn native_startup_input_desktop() -> String {
        let handle = unsafe { OpenInputDesktop(0, 0, 1) }; let name = native_startup_desktop_name(handle);
        assert_ne!(unsafe { CloseDesktop(handle) }, 0); name
    }
    #[derive(Clone, Debug, serde::Serialize)]
    struct StartupWindowSnapshot { handle: String, pid: u32, title: String, class: String }
    #[derive(Debug, serde::Serialize)]
    struct StartupWindowCleanup { initial_windows: Vec<StartupWindowSnapshot>, remaining_windows: Vec<StartupWindowSnapshot>, waited_ms: u128 }
    struct NativeStartupDesktop { handle: StartupHandle, name: String }
    impl NativeStartupDesktop {
        fn new(root: &Path) -> Self {
            let name = root.file_name().unwrap().to_str().unwrap().to_owned();
            let current = native_startup_desktop_name(unsafe { GetThreadDesktop(GetCurrentThreadId()) });
            let handle = unsafe { CreateDesktopW(startup_wide(&name).as_ptr(), std::ptr::null(), std::ptr::null(), 0, 0xc3, std::ptr::null()) };
            assert_eq!(native_startup_desktop_name(handle), name); assert_ne!(name, native_startup_input_desktop());
            assert_eq!(native_startup_desktop_name(unsafe { GetThreadDesktop(GetCurrentThreadId()) }), current); Self { handle, name }
        }
        fn windows(&self) -> Vec<StartupHandle> {
            unsafe extern "system" fn collect(window: StartupHandle, parameter: isize) -> i32 { unsafe { &mut *(parameter as *mut Vec<StartupHandle>) }.push(window); 1 }
            let mut windows = Vec::new(); unsafe { SetLastError(0) };
            let result = unsafe { EnumDesktopWindows(self.handle, Some(collect), std::ptr::from_mut(&mut windows) as isize) }; let error = unsafe { GetLastError() };
            assert!(result != 0 || (error == 0 && windows.is_empty()), "専用desktopの列挙に失敗しました: {error}"); windows
        }
        fn window_snapshot(&self) -> Vec<StartupWindowSnapshot> {
            self.windows().into_iter().map(|window| {
                let mut pid = 0; unsafe { GetWindowThreadProcessId(window, &raw mut pid) };
                let mut title = [0_u16; 512]; let length = unsafe { GetWindowTextW(window, title.as_mut_ptr(), i32::try_from(title.len()).unwrap()) };
                StartupWindowSnapshot { handle: format!("{window:p}"), pid, title: String::from_utf16(&title[..usize::try_from(length).unwrap()]).unwrap(), class: startup_window_class(window) }
            }).collect()
        }
        // 所有jobのprocess終了後も、OSによるwindow解放の完了を独立して確認する。
        fn wait_for_empty_windows(&self) -> StartupWindowCleanup {
            let started = std::time::Instant::now(); let initial = self.window_snapshot(); let mut remaining = initial.clone();
            while !remaining.is_empty() && started.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(10)); remaining = self.window_snapshot();
            }
            let observation = StartupWindowCleanup { initial_windows: initial, remaining_windows: remaining, waited_ms: started.elapsed().as_millis() };
            assert!(observation.remaining_windows.is_empty(), "所有process終了後も専用desktopのwindowが残っています: {observation:?}"); observation
        }
        fn main_window(&self, pid: u32) -> StartupHandle {
            let windows: Vec<_> = self.windows().into_iter().filter(|window| {
                let mut owner = 0; unsafe { GetWindowThreadProcessId(*window, &raw mut owner) }; let mut title = [0_u16; 512];
                let length = unsafe { GetWindowTextW(*window, title.as_mut_ptr(), i32::try_from(title.len()).unwrap()) };
                owner == pid && String::from_utf16(&title[..usize::try_from(length).unwrap()]).ok().as_deref() == Some("STELLA RECORD")
            }).collect(); assert_eq!(windows.len(), 1, "所有する本体Windowを一意に確認できません"); windows[0]
        }
    }
    impl Drop for NativeStartupDesktop { fn drop(&mut self) { let _ = unsafe { CloseDesktop(self.handle) }; } }

    // exitでstackが破棄されなくても、親が専用keyを削除する。利用者のInstallLocationへ書かない。
    struct NativeStartupRegistry { original: winreg::RegKey, isolated: winreg::RegKey, path: String }
    impl NativeStartupRegistry {
        fn new(root: &Path) -> StartupTestResult<Self> {
            let original = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
            let path = format!("Software\\StellaRecord.Tests\\startup-{}", root.file_name().ok_or("専用領域名がありません")?.to_str().ok_or("専用領域名を読めません")?);
            assert!(original.open_subkey(&path).is_err()); let (isolated, _) = original.create_subkey(&path)?;
            isolated.set_value("Owner", &root.to_str().ok_or("専用領域を読めません")?)?;
            let (settings, _) = isolated.create_subkey("Software\\CosmoArtsStore\\StellaRecord")?;
            settings.set_value("InstallLocation", &root.to_str().ok_or("専用領域を読めません")?)?; Ok(Self { original, isolated, path })
        }
    }
    impl Drop for NativeStartupRegistry { fn drop(&mut self) { let _ = self.original.delete_subkey_all(&self.path); } }

    struct NativeStartupProcess { handle: NativeStartupHandle, job: NativeStartupHandle, pid: u32, log: PathBuf }
    impl NativeStartupProcess {
        fn start(executable: &Path, root: &Path, desktop: &NativeStartupDesktop, registry: &NativeStartupRegistry) -> Self {
            Self::start_with_args(executable, root, desktop, registry, "")
        }
        fn start_with_args(executable: &Path, root: &Path, desktop: &NativeStartupDesktop, registry: &NativeStartupRegistry, arguments: &str) -> Self {
            let job = NativeStartupHandle::new(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) });
            let limits = StartupJobLimits { basic: StartupJobBasicLimits { flags: 0x2000, ..Default::default() }, ..Default::default() };
            assert_ne!(unsafe { SetInformationJobObject(job.0, 9, std::ptr::from_ref(&limits).cast(), u32::try_from(std::mem::size_of_val(&limits)).unwrap()) }, 0);
            let keys = ["STELLA_STARTUP_TEST_ROOT", "STELLA_STARTUP_TEST_REGISTRY", "STELLA_STARTUP_TEST_DESKTOP", "WEBVIEW2_USER_DATA_FOLDER", "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER"];
            let mut values: Vec<_> = std::env::vars_os().filter(|(key, _)| !keys.iter().any(|name| key.to_string_lossy().eq_ignore_ascii_case(name))).collect();
            values.extend([(OsString::from(keys[0]), root.as_os_str().to_owned()), (OsString::from(keys[1]), OsString::from(&registry.path)), (OsString::from(keys[2]), OsString::from(&desktop.name))]);
            values.sort_by_key(|(key, _)| key.to_string_lossy().to_uppercase()); let mut environment = Vec::new();
            for (key, value) in values { environment.extend(key.encode_wide()); environment.push(u16::from(b'=')); environment.extend(value.encode_wide()); environment.push(0); } environment.push(0);
            let application: Vec<_> = executable.as_os_str().encode_wide().chain([0]).collect();
            let mut command = startup_wide(&format!("\"{}\" {arguments}", executable.display())); let mut name = startup_wide(&desktop.name);
            let log = root.join("startup.child.log"); let output = OpenOptions::new().create_new(true).write(true).open(&log).unwrap();
            let input = File::open("NUL").unwrap();
            let handles = [input.as_raw_handle(), output.as_raw_handle()];
            for handle in handles { assert_ne!(unsafe { SetHandleInformation(handle, 1, 1) }, 0); }
            let startup = StartupInformation { size: u32::try_from(std::mem::size_of::<StartupInformation>()).unwrap(), desktop: name.as_mut_ptr(), flags: 0x140, input: handles[0], output: handles[1], error: handles[1], ..unsafe { std::mem::zeroed() } };
            let mut information = StartupProcessInformation::default();
            let created = unsafe { CreateProcessW(application.as_ptr(), command.as_mut_ptr(), std::ptr::null(), std::ptr::null(), 1, 0x0800_0404, environment.as_ptr().cast(), std::ptr::null(), &raw const startup, &raw mut information) };
            let error = std::io::Error::last_os_error(); for handle in handles { assert_ne!(unsafe { SetHandleInformation(handle, 1, 0) }, 0); }
            assert_ne!(created, 0, "専用desktopへ起動できません: {error}");
            let process = Self { handle: NativeStartupHandle::new(information.process), job, pid: information.pid, log };
            let thread = NativeStartupHandle::new(information.thread);
            // 再開前にjobへ割り当て、異常時もこのchildとそのWebViewだけを終了する。
            assert_ne!(unsafe { AssignProcessToJobObject(process.job.0, process.handle.0) }, 0); assert_ne!(unsafe { ResumeThread(thread.0) }, u32::MAX); process
        }
        fn running(&self) -> bool { (unsafe { WaitForSingleObject(self.handle.0, 0) }) == 258 }
        fn finish(&self) -> u32 {
            assert_eq!(unsafe { WaitForSingleObject(self.handle.0, 15_000) }, 0, "本体が終了しません: {}", self.diagnostic());
            let mut code = 0; assert_ne!(unsafe { GetExitCodeProcess(self.handle.0, &raw mut code) }, 0);
            self.finish_children().unwrap(); code
        }
        fn finish_children(&self) -> std::io::Result<()> {
            if unsafe { TerminateJobObject(self.job.0, 1) } == 0 { return Err(std::io::Error::last_os_error()); }
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                let mut accounting = StartupJobAccounting::default();
                if unsafe { QueryInformationJobObject(self.job.0, 1, std::ptr::from_mut(&mut accounting).cast(), u32::try_from(std::mem::size_of_val(&accounting)).unwrap(), std::ptr::null_mut()) } == 0 { return Err(std::io::Error::last_os_error()); }
                if accounting.counts[2] == 0 { return Ok(()); }
                if std::time::Instant::now() >= deadline { return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "所有job内のprocessが残っています")); }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        fn diagnostic(&self) -> String {
            let mut code = 0; assert_ne!(unsafe { GetExitCodeProcess(self.handle.0, &raw mut code) }, 0);
            let root = self.log.parent().unwrap();
            let logs = root.join("Data/logs"); let mut messages = fs::read_to_string(&self.log).unwrap();
            if logs.is_dir() { for entry in fs::read_dir(logs).unwrap() { messages.push_str(&fs::read_to_string(entry.unwrap().path()).unwrap()); } }
            format!("exit={code:#x}, before={}: {messages}", root.join("startup.before.txt").is_file())
        }
        fn close(&self, desktop: &NativeStartupDesktop) {
            assert_ne!(unsafe { PostMessageW(desktop.main_window(self.pid), 0x10, 0, 0) }, 0); assert_eq!(self.finish(), 0);
        }
    }
    impl Drop for NativeStartupProcess {
        fn drop(&mut self) {
            if self.running() { eprintln!("専用起動fixtureの終了前診断: {}", self.diagnostic()); let _ = unsafe { TerminateProcess(self.handle.0, 1) }; let _ = unsafe { WaitForSingleObject(self.handle.0, 5_000) }; }
            if let Err(error) = self.finish_children() { eprintln!("専用起動jobの終了失敗: {error}"); }
        }
    }

    fn native_startup_single_instance_available() {
        unsafe { SetLastError(0) };
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, startup_wide("Local\\StellaRecord_SingleInstance").as_ptr()) }; let error = unsafe { GetLastError() };
        let handle = NativeStartupHandle::new(handle); assert_ne!(error, 183, "実StellaRecordが稼働しているため本体起動を確認できません"); drop(handle);
    }

    fn native_startup_compile(directory: &tempfile::TempDir) -> StartupTestResult<(PathBuf, PathBuf)> {
        let dependency = std::env::current_exe()?.parent().ok_or("test exeの親がありません")?.to_path_buf();
        let mut libraries = Vec::new();
        for entry in fs::read_dir(&dependency)? {
            let path = entry?.path(); let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            if name.starts_with("libstella_record_ui-") && name.ends_with(".rlib") { libraries.push((path.metadata()?.modified()?, path)); }
        }
        libraries.sort(); let library = libraries.pop().ok_or("同じbuildの本体libraryがありません")?.1;
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")); let root = manifest.parent().ok_or("Frontend rootがありません")?;
        let source = fs::read_to_string(manifest.join("src/main.rs"))?;
        let (attribute, body) = source.split_once('\n').ok_or("main.rsのcrate属性がありません")?;
        assert_eq!(attribute.trim_end_matches('\r'), "#![cfg_attr(not(debug_assertions), windows_subsystem = \"windows\")]");
        assert_eq!(source, include_str!("../main.rs"), "本体入口がtest build後に変更されています");
        let entry = directory.path().join("startup-entry.rs"); let generated = format!("\n{body}"); fs::write(&entry, &generated)?;
        assert_eq!(fs::read_to_string(&entry)?, generated);
        let executable = directory.path().join("owned-startup.exe"); let mut compiler = Command::new("rustc");
        compiler.args(["--edition=2021"]).arg(root.join("src/test/native-ipc/startup-runner.rs")).arg("--extern").arg(format!("stella_record_ui={}", library.display()))
            .arg("-L").arg(format!("dependency={}", dependency.display())).arg("-o").arg(&executable).env("STELLA_STARTUP_ENTRY_SOURCE", &entry)
            .arg("--remap-path-prefix").arg(format!("{}={}", entry.display(), manifest.join("src/main.rs").display())).creation_flags(0x0800_0000);
        let resource = Path::new(env!("OUT_DIR")).join("resource.lib"); assert!(resource.is_file()); compiler.arg("-C").arg(format!("link-arg={}", resource.display()));
        let build = dependency.parent().ok_or("debug領域がありません")?.join("build"); let mut directories = std::collections::BTreeSet::new();
        for entry in fs::read_dir(build)? {
            let path = entry?.path();
            // 古いアプリbuildのresource・CRT指定を現在の検索先へ混ぜない。
            if path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("stella_record_ui-")) && path.join("out") != Path::new(env!("OUT_DIR")) { continue; }
            let output = path.join("output"); if !output.is_file() { continue; }
            for line in fs::read_to_string(output)?.lines() {
                if let Some(path) = line.strip_prefix("cargo:rustc-link-search=native=").or_else(|| line.strip_prefix("cargo::rustc-link-search=native=")) { directories.insert(path.to_owned()); }
            }
        }
        for path in directories { assert!(Path::new(&path).is_dir()); compiler.arg("-L").arg(format!("native={path}")); }
        if std::env::var_os("LLVM_PROFILE_FILE").is_some() {
            compiler.args(["-C", "instrument-coverage"]);
            if std::env::var("STELLA_STARTUP_CONTINUOUS_PROFILE").ok().as_deref() == Some("1") { compiler.args(["-C", "llvm-args=-runtime-counter-relocation"]); }
        }
        let compilation = compiler.output()?; fs::write(directory.path().join("startup.compile.log"), &compilation.stderr)?;
        assert!(compilation.status.success(), "専用exeをbuildできません: {}", String::from_utf8_lossy(&compilation.stderr)); Ok((executable, library))
    }

    #[derive(serde::Serialize)]
    struct StartupProbeInputs { nonce: String, database_path: String, log_archive_path: String, owned_exe: String }
    #[derive(serde::Serialize, serde::Deserialize)]
    struct StartupProbeReport { ok: bool, error: String, checks: Vec<String> }
    fn startup_json<T: serde::Serialize>(value: T) -> StartupTestResult<String> {
        use tauri::ipc::IpcResponse;
        match value.body()? { tauri::ipc::InvokeResponseBody::Json(text) => Ok(text), tauri::ipc::InvokeResponseBody::Raw(_) => Err("検証記録がJSONではありません".into()) }
    }

    /// 要求なしで閉じた接続と受信途中の切断を分け、同じ全体期限で実Browserの結果を待つ。
    fn startup_http_request(stream: &mut std::net::TcpStream) -> StartupTestResult<Option<(String, Vec<u8>)>> {
        let mut request = Vec::new();
        let (header, start, bytes) = loop {
            let mut buffer = [0_u8; 4096]; let count = stream.read(&mut buffer)?;
            if count == 0 && request.is_empty() { return Ok(None); }
            if count == 0 { return Err(format!("専用HTTP要求のheaderが途中で終了しました: {} bytes受信済み", request.len()).into()); }
            request.extend_from_slice(&buffer[..count]); if request.len() > 65_536 { return Err("専用HTTP要求が既存の上限を超えています".into()); }
            if let Some(position) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let header = String::from_utf8(request[..position].to_vec())?;
                let bytes = header.lines().filter_map(|line| line.split_once(':')).find(|(key, _)| key.eq_ignore_ascii_case("content-length")).map(|(_, value)| value.trim().parse::<usize>()).transpose()?.unwrap_or(0);
                if bytes > 65_536 - (position + 4) { return Err("専用HTTP要求が既存の上限を超えています".into()); }
                break (header, position + 4, bytes);
            }
        };
        while request.len() < start + bytes {
            let mut buffer = [0_u8; 4096]; let count = stream.read(&mut buffer)?;
            if count == 0 { return Err(format!("専用HTTP要求のbodyが途中で終了しました: {} bytes受信済み", request.len() - start).into()); }
            request.extend_from_slice(&buffer[..count]); if request.len() > 65_536 { return Err("専用HTTP要求が既存の上限を超えています".into()); }
        }
        Ok(Some((header, request[start..start + bytes].to_vec())))
    }

    fn startup_http_fixture(request: &[u8]) -> StartupTestResult<Option<(String, Vec<u8>)>> {
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        let mut client = std::net::TcpStream::connect(listener.local_addr()?)?; client.write_all(request)?; client.shutdown(std::net::Shutdown::Write)?;
        let (mut stream, peer) = listener.accept()?; assert!(peer.ip().is_loopback()); stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        startup_http_request(&mut stream)
    }

    #[test]
    fn native_startup_http_reader_distinguishes_unused_connection_and_utf8_report() -> StartupTestResult {
        assert!(startup_http_fixture(b"")?.is_none());
        assert_eq!(startup_http_fixture(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")?, Some(("GET / HTTP/1.1\r\nHost: localhost".into(), Vec::new())));
        let body = "{\"ok\":true,\"error\":\"日本語と\\\"引用符\\\"\"}";
        let header = format!("POST /owned-startup-result/mutex HTTP/1.1\r\nContent-Length: {}", body.len());
        assert_eq!(startup_http_fixture(format!("{header}\r\n\r\n{body}").as_bytes())?, Some((header, body.as_bytes().to_vec()))); Ok(())
    }

    #[test]
    fn native_startup_http_reader_keeps_partial_requests_and_existing_size_limit_invalid() {
        assert_eq!(startup_http_fixture(b"GET / HTTP/1.1").unwrap_err().to_string(), "専用HTTP要求のheaderが途中で終了しました: 14 bytes受信済み");
        assert_eq!(startup_http_fixture(b"POST / HTTP/1.1\r\nContent-Length: 2\r\n\r\nx").unwrap_err().to_string(), "専用HTTP要求のbodyが途中で終了しました: 1 bytes受信済み");
        assert_eq!(startup_http_fixture(b"POST / HTTP/1.1\r\nContent-Length: 65536\r\n\r\n").unwrap_err().to_string(), "専用HTTP要求が既存の上限を超えています");
    }

    struct NativeStartupServer { listener: std::net::TcpListener, script: Vec<u8> }
    impl NativeStartupServer {
        fn new(directory: &tempfile::TempDir) -> StartupTestResult<Self> {
            let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 5173))?; listener.set_nonblocking(true)?;
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().ok_or("Frontend rootがありません")?;
            let output = directory.path().join("browser-probe.js");
            let compiled = Command::new("node").arg(root.join("src/test/native-ipc/build-probe.ts")).arg(&output).arg("startup").creation_flags(0x0800_0000).output()?;
            assert!(compiled.status.success(), "起動probeをbuildできません: {}", String::from_utf8_lossy(&compiled.stderr)); Ok(Self { listener, script: fs::read(output)? })
        }
        fn report(&self, child: &NativeStartupProcess, root: &Path, nonce: &str) -> StartupTestResult<StartupProbeReport> {
            let input = startup_json(StartupProbeInputs { nonce: nonce.into(), database_path: root.join("Data").join("db").join("stellarecord.db").display().to_string(),
                log_archive_path: root.join("Data").join("archive").display().to_string(), owned_exe: root.join("owned-catalog.exe").display().to_string() })?.replace('<', "\\u003c");
            let html = format!("<!doctype html><html lang=\"ja\"><head><meta charset=\"UTF-8\"><title>STELLA RECORD</title></head><body><script id=\"probe-input\" type=\"application/json\">{input}</script><script src=\"browser-probe.js\"></script></body></html>");
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            let started = std::time::Instant::now();
            let log_path = startup_evidence_directory()?.map_or_else(|| root.join("startup.http.log"), |folder| folder.join(format!("stellarecord-startup-{nonce}-http.log")));
            let mut log = OpenOptions::new().create_new(true).write(true).open(log_path)?;
            loop {
                assert!(child.running(), "本体commandへ接続する前に終了しました: {}", child.diagnostic()); assert!(std::time::Instant::now() < deadline, "本体起動結果を確認できません: {}", child.diagnostic());
                let (mut stream, address) = match self.listener.accept() { Ok(connection) => connection, Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => { std::thread::sleep(Duration::from_millis(10)); continue; }, Err(error) => return Err(error.into()) };
                assert!(address.ip().is_loopback()); stream.set_nonblocking(false)?; stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                let (header, request_body) = match startup_http_request(&mut stream) {
                    Ok(Some(request)) => request,
                    Ok(None) => { writeln!(log, "+{}ms empty-connection-closed: 0 bytes受信済み", started.elapsed().as_millis())?; continue; },
                    Err(error) => { writeln!(log, "+{}ms {error}", started.elapsed().as_millis())?; return Err(error); },
                };
                let line = header.lines().next().ok_or("専用HTTP要求の先頭がありません")?;
                writeln!(log, "+{}ms {line}", started.elapsed().as_millis())?;
                let (status, mime, body) = if matches!(line, "GET / HTTP/1.1" | "GET /index.html HTTP/1.1") { ("200 OK", "text/html; charset=utf-8", html.as_bytes()) }
                    else if line == "GET /browser-probe.js HTTP/1.1" { ("200 OK", "text/javascript; charset=utf-8", self.script.as_slice()) }
                    else if line == format!("POST /owned-startup-result/{nonce} HTTP/1.1") { ("200 OK", "text/plain", b"recorded".as_slice()) } else { ("404 Not Found", "text/plain", b"owned fixture only".as_slice()) };
                stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n", body.len()).as_bytes())?; stream.write_all(body)?;
                if line == format!("POST /owned-startup-result/{nonce} HTTP/1.1") {
                    let report = tauri::ipc::InvokeResponseBody::Json(String::from_utf8(request_body)?).deserialize::<StartupProbeReport>()?;
                    assert!(report.ok, "本体起動のBrowser検証が失敗しました: {}", report.error); assert_eq!(report.checks.len(), 6); return Ok(report);
                }
            }
        }
    }

    fn startup_fixture(root: &Path, registry: &NativeStartupRegistry) -> StartupTestResult<()> {
        let settings = registry.isolated.open_subkey_with_flags("Software\\CosmoArtsStore\\StellaRecord", winreg::enums::KEY_WRITE)?;
        settings.set_value("EnableStartup", &0_u32)?; settings.set_value("StartupPreferenceSet", &1_u32)?;
        settings.set_value("StartupImportEnabled", &0_u32)?; settings.set_value("StartupImportPreferenceSet", &1_u32)?;
        fs::create_dir_all(root.join("Data/db"))?; fs::create_dir_all(root.join("Data/archive"))?;
        fs::write(root.join("Data/archive/owned-preserved.bin"), b"owned archive preserved")?; fs::write(root.join("owned-catalog.exe"), b"not launched")?;
        let connection = rusqlite::Connection::open(root.join("Data/db/stellarecord.db"))?; crate::analyze::init_main_db(&connection)?;
        connection.execute("INSERT INTO apps (name, description, path) VALUES ('起動検証用アプリ', '起動中に保持', ?1)", [root.join("owned-catalog.exe").display().to_string()])?;
        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?; Ok(())
    }
    fn startup_preserved(root: &Path, database: &[u8]) -> StartupTestResult<()> {
        assert_eq!(fs::read(root.join("Data/db/stellarecord.db"))?, database); assert_eq!(fs::read(root.join("Data/archive/owned-preserved.bin"))?, b"owned archive preserved"); Ok(())
    }
    fn startup_log(root: &Path) -> StartupTestResult<String> {
        let directory = root.join("Data/logs"); let files = fs::read_dir(directory)?.map(|entry| entry.map(|value| value.path())).collect::<Result<Vec<_>, _>>()?;
        assert_eq!(files.len(), 1); Ok(fs::read_to_string(&files[0])?)
    }
    #[derive(serde::Deserialize)]
    struct StartupLocalState {}
    fn startup_profile(root: &Path) -> StartupTestResult<()> {
        let profile = root.join("Data/EBWebView"); assert!(profile.is_dir());
        // 起動時のPreferences生成を前提にせず、専用領域に生成された実cacheの形式を検証する。
        let state = fs::read_to_string(profile.join("EBWebView/Local State"))?;
        tauri::ipc::InvokeResponseBody::Json(state).deserialize::<StartupLocalState>()?;
        let history = fs::read(profile.join("EBWebView/Default/History"))?; assert!(history.starts_with(b"SQLite format 3\0")); Ok(())
    }
    #[derive(serde::Serialize)]
    // 証拠JSONの独立した観測項目をboolで保持する。production型の規則は緩和しない。
    #[allow(clippy::struct_excessive_bools)]
    struct StartupEvidence<'a> { kind: &'a str, actual_main_body: bool, actual_run: bool, browser: Option<StartupProbeReport>, error_dialog: Option<String>, exit_codes: Vec<u32>, window_cleanup: Option<StartupWindowCleanup>, picker: Option<NativePickerObservation>,
        database_bytes_preserved: bool, archive_bytes_preserved: bool, input_desktop_unchanged: bool, owned_registry_removed: bool, owned_roots_removed: Vec<String>, library_input: String, library_sha256: String }
    fn startup_evidence_directory() -> StartupTestResult<Option<PathBuf>> {
        let Some(folder) = std::env::var_os("STELLA_STARTUP_TEST_EVIDENCE_DIR") else { return Ok(None); };
        let folder = PathBuf::from(folder).canonicalize()?;
        let boundary = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).ok_or("software rootがありません")?.join(".claude/audit-work/checks").canonicalize()?;
        assert!(folder.starts_with(boundary)); Ok(Some(folder))
    }
    // assertion失敗時も、実起動した測定objectとその入力をfixture削除前に保持する。
    fn startup_archive_inputs(kind: &str, directory: &Path, executable: &Path, library: &Path) -> StartupTestResult<()> {
        #[derive(serde::Serialize)]
        struct Inputs { library_input: String, library_sha256: String, executable_sha256: String, entry_sha256: String }
        let Some(folder) = startup_evidence_directory()? else { return Ok(()); };
        let entry = directory.join("startup-entry.rs");
        for (source, suffix) in [(executable, ".exe"), (entry.as_path(), "-entry.rs")] {
            let target = folder.join(format!("stellarecord-startup-{kind}{suffix}")); assert!(!target.exists()); fs::copy(source, target)?;
        }
        let target = folder.join(format!("stellarecord-startup-{kind}-inputs.json")); assert!(!target.exists());
        fs::write(target, startup_json(Inputs { library_input: library.display().to_string(), library_sha256: format!("{:x}", sha2::Sha256::digest(fs::read(library)?)),
            executable_sha256: format!("{:x}", sha2::Sha256::digest(fs::read(executable)?)), entry_sha256: format!("{:x}", sha2::Sha256::digest(fs::read(entry)?)) })?)?; Ok(())
    }
    fn startup_finish(kind: &str, directory: tempfile::TempDir, executable: &Path, library: &Path, registry: NativeStartupRegistry, mut evidence: StartupEvidence<'_>) -> StartupTestResult<()> {
        let path = registry.path.clone(); drop(registry); assert!(winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(path).is_err()); evidence.owned_registry_removed = true;
        let folder = startup_evidence_directory()?;
        if let Some(folder) = &folder {
            let target = folder.join(format!("stellarecord-startup-{kind}.exe"));
            if target.exists() { assert_eq!(fs::read(&target)?, fs::read(executable)?); } else { fs::copy(executable, target)?; }
            let target = folder.join(format!("stellarecord-startup-{kind}-child.log")); assert!(!target.exists()); fs::copy(directory.path().join("startup.child.log"), target)?;
            if evidence.actual_main_body { assert_eq!(fs::read(directory.path().join("startup-entry.rs"))?, fs::read(folder.join(format!("stellarecord-startup-{kind}-entry.rs")))?); }
            let failure = directory.path().join("startup.failure.log"); if failure.is_file() { fs::copy(failure, folder.join(format!("stellarecord-startup-{kind}-failure.log")))?; }
        }
        if evidence.actual_run { evidence.library_input = library.display().to_string(); evidence.library_sha256 = format!("{:x}", sha2::Sha256::digest(fs::read(library)?)); }
        evidence.owned_roots_removed.push(startup_remove_fixture(directory)?);
        if let Some(folder) = folder { let target = folder.join(format!("stellarecord-startup-{kind}-evidence.json")); assert!(!target.exists()); fs::write(target, startup_json(evidence)?)?; }
        Ok(())
    }
    fn startup_remove_fixture(directory: tempfile::TempDir) -> StartupTestResult<String> {
        let root = directory.path().canonicalize()?; assert_eq!(root.parent(), Some(std::env::temp_dir().canonicalize()?.as_path()));
        assert!(root.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("stellarecord-native-startup-")));
        let deadline = std::time::Instant::now() + Duration::from_secs(5); let mut failures = Vec::new();
        loop {
            match fs::remove_dir_all(&root) {
                Ok(()) => break,
                Err(error) => { failures.push(error.to_string()); if std::time::Instant::now() >= deadline { return Err(format!("専用fixtureを削除できません: {}: {failures:?}", root.display()).into()); } std::thread::sleep(Duration::from_millis(20)); }
            }
        }
        assert!(!root.try_exists()?); drop(directory); Ok(root.display().to_string())
    }
    fn startup_evidence(kind: &str, browser: Option<StartupProbeReport>, error_dialog: Option<String>, exit_codes: Vec<u32>) -> StartupEvidence<'_> {
        StartupEvidence { kind, actual_main_body: true, actual_run: true, browser, error_dialog, exit_codes, window_cleanup: None, picker: None, database_bytes_preserved: true,
            archive_bytes_preserved: true, input_desktop_unchanged: true, owned_registry_removed: false, owned_roots_removed: Vec::new(), library_input: String::new(), library_sha256: String::new() }
    }

    #[test]
    fn native_startup_main_runs_owned_data_and_duplicate_exits_before_setup() -> StartupTestResult {
        let _lock = NATIVE_STARTUP_TEST_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner); native_startup_single_instance_available();
        let input = native_startup_input_desktop(); let directory = tempfile::Builder::new().prefix("stellarecord-native-startup-normal-").tempdir()?;
        let (executable, library) = native_startup_compile(&directory)?; startup_archive_inputs("normal", directory.path(), &executable, &library)?;
        let desktop = NativeStartupDesktop::new(directory.path()); let registry = NativeStartupRegistry::new(directory.path())?;
        startup_fixture(directory.path(), &registry)?; let database = fs::read(directory.path().join("Data/db/stellarecord.db"))?;
        let server = NativeStartupServer::new(&directory)?; let child = NativeStartupProcess::start(&executable, directory.path(), &desktop, &registry);
        let report = server.report(&child, directory.path(), "normal")?; startup_profile(directory.path())?; startup_preserved(directory.path(), &database)?;
        let duplicate = tempfile::Builder::new().prefix("stellarecord-native-startup-duplicate-").tempdir()?; let duplicate_registry = NativeStartupRegistry::new(duplicate.path())?;
        let second = NativeStartupProcess::start(&executable, duplicate.path(), &desktop, &duplicate_registry); let duplicate_pid = second.pid; assert_eq!(second.finish(), 0); drop(second);
        assert!(child.running()); assert!(!duplicate.path().join("Data").exists()); assert!(!duplicate.path().join("startup.returned.txt").exists());
        assert!(desktop.windows().iter().all(|window| { let mut pid = 0; unsafe { GetWindowThreadProcessId(*window, &raw mut pid) }; pid != duplicate_pid }));
        let duplicate_key = duplicate_registry.path.clone(); drop(duplicate_registry); assert!(winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(duplicate_key).is_err());
        let duplicate_root = startup_remove_fixture(duplicate)?; child.close(&desktop); drop(child); drop(server);
        assert!(!directory.path().join("startup.returned.txt").exists()); startup_preserved(directory.path(), &database)?;
        assert!(startup_log(directory.path())?.contains("[WARN] frontend: 起動検証専用の記録"));
        let window_cleanup = desktop.wait_for_empty_windows(); drop(desktop); assert_eq!(native_startup_input_desktop(), input); native_startup_single_instance_available();
        let mut evidence = startup_evidence("normal", Some(report), None, vec![0, 0]); evidence.owned_roots_removed.push(duplicate_root);
        evidence.window_cleanup = Some(window_cleanup);
        startup_finish("normal", directory, &executable, &library, registry, evidence)
    }

    fn startup_window_text(window: StartupHandle) -> Option<String> {
        let mut buffer = [0_u16; 4096]; let mut length = 0;
        let received = unsafe { SendMessageTimeoutW(window, 0x0d, buffer.len(), buffer.as_mut_ptr() as isize, 2, 1000, &raw mut length) };
        if received == 0 || length >= buffer.len() { return None; } String::from_utf16(&buffer[..length]).ok()
    }
    fn startup_window_class(window: StartupHandle) -> String {
        let mut buffer = [0_u16; 128]; let length = unsafe { GetClassNameW(window, buffer.as_mut_ptr(), i32::try_from(buffer.len()).unwrap()) };
        String::from_utf16(&buffer[..usize::try_from(length).unwrap()]).unwrap()
    }
    fn startup_message_dialog(desktop: &NativeStartupDesktop, child: &NativeStartupProcess, expected: &str) -> StartupHandle {
        unsafe extern "system" fn collect(window: StartupHandle, parameter: isize) -> i32 { unsafe { &mut *(parameter as *mut Vec<StartupHandle>) }.push(window); 1 }
        let deadline = std::time::Instant::now() + Duration::from_secs(15); let mut observed = Vec::new();
        loop {
            observed.clear();
            for window in desktop.windows() {
                let mut pid = 0; unsafe { GetWindowThreadProcessId(window, &raw mut pid) };
                let mut title = [0_u16; 512]; let length = unsafe { GetWindowTextW(window, title.as_mut_ptr(), i32::try_from(title.len()).unwrap()) };
                let title = String::from_utf16(&title[..usize::try_from(length).unwrap()]).unwrap(); let class = startup_window_class(window);
                observed.push(vec![format!("pid={pid}, title={title}, class={class}")]);
                if pid != child.pid || class != "#32770" || title != "StellaRecord" { continue; }
                let mut controls = Vec::new(); unsafe { EnumChildWindows(window, Some(collect), std::ptr::from_mut(&mut controls) as isize) };
                let texts: Vec<_> = controls.iter().filter_map(|control| startup_window_text(*control)).collect();
                let buttons: Vec<_> = controls.iter().copied().filter(|control| startup_window_class(*control) == "Button" && startup_window_text(*control).as_deref() == Some("OK") && unsafe { IsWindowVisible(*control) } != 0).collect();
                observed.push(texts.clone());
                if texts.iter().any(|text| text == expected) { assert_eq!(buttons.len(), 1); return buttons[0]; }
            }
            assert!(child.running(), "本体がdialog確認前に終了しました: {observed:?} {}", child.diagnostic());
            assert!(std::time::Instant::now() < deadline, "実MessageBoxの本文を確認できません: {observed:?}"); std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn native_startup_dialog_displays_exact_unicode_text_and_returns() -> StartupTestResult {
        const MESSAGE: &str = "Tauri アプリケーションの起動に失敗しました: 起動検証専用の原因\n日本語・Unicode（星★）を保持します。";
        if let Some(root) = std::env::var_os("STELLA_STARTUP_TEST_ROOT") {
            let root = PathBuf::from(root).canonicalize()?; assert_eq!(root.parent(), Some(std::env::temp_dir().canonicalize()?.as_path()));
            assert!(root.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("stellarecord-native-startup-dialog-")));
            assert_eq!(native_startup_desktop_name(unsafe { GetThreadDesktop(GetCurrentThreadId()) }), std::env::var("STELLA_STARTUP_TEST_DESKTOP")?);
            let path = std::env::var("STELLA_STARTUP_TEST_REGISTRY")?; assert_eq!(path, format!("Software\\StellaRecord.Tests\\startup-{}", root.file_name().unwrap().to_str().unwrap()));
            let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey_with_flags(path, winreg::enums::KEY_READ | winreg::enums::KEY_WRITE)?;
            assert_eq!(PathBuf::from(key.get_value::<String, _>("Owner")?).canonicalize()?, root);
            assert_eq!(unsafe { super::RegOverridePredefKey(winreg::enums::HKEY_CURRENT_USER, key.raw_handle()) }, 0);
            fs::write(root.join("startup.before.txt"), std::process::id().to_string())?;
            crate::show_error_dialog(MESSAGE); fs::write(root.join("startup.returned.txt"), b"returned")?;
            assert_eq!(unsafe { super::RegOverridePredefKey(winreg::enums::HKEY_CURRENT_USER, 0) }, 0); return Ok(());
        }
        let _lock = NATIVE_STARTUP_TEST_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let input = native_startup_input_desktop(); let directory = tempfile::Builder::new().prefix("stellarecord-native-startup-dialog-").tempdir()?;
        let executable = std::env::current_exe()?; let desktop = NativeStartupDesktop::new(directory.path()); let registry = NativeStartupRegistry::new(directory.path())?;
        startup_fixture(directory.path(), &registry)?; let database = fs::read(directory.path().join("Data/db/stellarecord.db"))?;
        let child = NativeStartupProcess::start_with_args(&executable, directory.path(), &desktop, &registry,
            "--exact commands::tests::startup::native_startup_dialog_displays_exact_unicode_text_and_returns --nocapture");
        let button = startup_message_dialog(&desktop, &child, MESSAGE); assert!(child.running());
        assert!(!directory.path().join("startup.returned.txt").exists()); startup_preserved(directory.path(), &database)?;
        assert_ne!(unsafe { PostMessageW(button, 0x00f5, 0, 0) }, 0); assert_eq!(child.finish(), 0); drop(child);
        assert_eq!(fs::read(directory.path().join("startup.returned.txt"))?, b"returned");
        assert!(fs::read_to_string(directory.path().join("startup.child.log"))?.contains("1 passed; 0 failed")); startup_preserved(directory.path(), &database)?;
        let window_cleanup = desktop.wait_for_empty_windows(); drop(desktop); assert_eq!(native_startup_input_desktop(), input);
        let mut evidence = startup_evidence("dialog", None, Some(MESSAGE.into()), vec![0]); evidence.actual_main_body = false; evidence.actual_run = false;
        evidence.window_cleanup = Some(window_cleanup);
        startup_finish("dialog", directory, &executable, &executable, registry, evidence)
    }

    #[test]
    fn native_startup_main_logs_mutex_type_collision_and_continues() -> StartupTestResult {
        use windows::Win32::System::Threading::CreateEventW;
        use windows::core::HSTRING;
        let _lock = NATIVE_STARTUP_TEST_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner); native_startup_single_instance_available();
        let collision = unsafe { CreateEventW(None, true, false, &HSTRING::from("Local\\StellaRecord_SingleInstance")) }?;
        let collision = NativeStartupHandle::new(collision.0);
        let input = native_startup_input_desktop(); let directory = tempfile::Builder::new().prefix("stellarecord-native-startup-mutex-").tempdir()?;
        let (executable, library) = native_startup_compile(&directory)?; startup_archive_inputs("mutex", directory.path(), &executable, &library)?;
        let desktop = NativeStartupDesktop::new(directory.path()); let registry = NativeStartupRegistry::new(directory.path())?;
        startup_fixture(directory.path(), &registry)?; let database = fs::read(directory.path().join("Data/db/stellarecord.db"))?;
        let server = NativeStartupServer::new(&directory)?;
        // 先に要求なしの専用接続を閉じても、後続の実Browser結果と同じ30秒の期限を要求する。
        drop(std::net::TcpStream::connect(server.listener.local_addr()?)?);
        let child = NativeStartupProcess::start(&executable, directory.path(), &desktop, &registry);
        let report = server.report(&child, directory.path(), "mutex")?; startup_profile(directory.path())?; startup_preserved(directory.path(), &database)?;
        let log = startup_log(directory.path())?; assert!(log.contains("[WARN] 単一起動ガードを初期化できませんでした。多重起動を防げない可能性があります: "));
        assert!(!log.contains("[ERROR]")); child.close(&desktop); drop(child); drop(server); drop(collision);
        let window_cleanup = desktop.wait_for_empty_windows(); drop(desktop); assert_eq!(native_startup_input_desktop(), input); native_startup_single_instance_available();
        let mut evidence = startup_evidence("mutex", Some(report), None, vec![0]); evidence.window_cleanup = Some(window_cleanup);
        startup_finish("mutex", directory, &executable, &library, registry, evidence)
    }

    /// childとjobの終了後、失敗時も専用領域内の診断だけをphaseへ残す。
    struct NativeUiPickerArchive<'a>(&'a Path, &'static str);
    impl Drop for NativeUiPickerArchive<'_> {
        fn drop(&mut self) {
            if let Ok(Some(folder)) = startup_evidence_directory() {
                for name in ["startup.child.log", "ui-picker.child-stage.txt", "ui-picker.last-check.txt", "ui-picker.progress.txt", "ui-picker.report.json", "ui-picker.completed.txt", "ui-picker.database-mismatch.json", "ui-picker.stream-lock-state.txt", "ui-picker.partial-before.tar.zst", "ui-picker.partial-after.tar.zst", "browser-probe-assets.json"] {
                    let source = self.0.join(name); let target = folder.join(format!("stellarecord-{}-{name}", self.1));
                    if source.is_file() && !target.exists() { if let Err(error) = fs::copy(source, target) { eprintln!("専用pickerの診断を保持できません: {error}"); } }
                }
                let target = folder.join(format!("stellarecord-{}-owned-root.txt", self.1));
                if !target.exists() { if let Err(error) = fs::write(target, self.0.display().to_string()) { eprintln!("専用pickerの所有rootを保持できません: {error}"); } }
            }
        }
    }

    #[derive(serde::Serialize)]
    #[allow(clippy::struct_excessive_bools, reason = "証拠JSONの独立した境界と解除の観測を保持するため")]
    struct NativeUiPickerEvidence { report: super::NativeProbeReport, stages: Vec<String>, window_cleanup: StartupWindowCleanup, external_file_hashes: Vec<(String, String)>, child_exit_code: u32,
        actual_frontend_main: bool, actual_run: bool, dialog_replaced: bool, core_sdk_facade_used: bool, input_desktop_unchanged: bool, owned_registry_removed: bool, owned_root_removed: String,
        stream_warning: String, stream_snapshots: Vec<NativeUiPickerDatabaseSnapshot>, lock_release_stage: Option<String>, initial_external_file_hashes: Vec<(String, String)>, repair_stage: Option<String>, physical_input_verified: bool, release_entry_verified: bool, verification_complete: bool }

    #[derive(serde::Serialize)]
    struct NativeUiPickerDatabaseSnapshot { stage: String, database_bytes: Vec<u8>, wal_bytes: Option<Vec<u8>> }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum NativeUiPickerCase { Normal, ReadFailure, StreamLocked, StreamPartial }
    impl NativeUiPickerCase {
        fn prefix(self) -> &'static str { match self { Self::Normal => "ui-picker", Self::ReadFailure => "ui-picker-read-failure", Self::StreamLocked => "ui-picker-stream-lock", Self::StreamPartial => "ui-picker-stream-partial" } }
    }

    fn native_ui_picker_child(root: &Path, case: NativeUiPickerCase) -> StartupTestResult {
        assert!(root.is_absolute()); let resolved_root = root.canonicalize()?; assert_eq!(resolved_root.parent(), Some(std::env::temp_dir().canonicalize()?.as_path()));
        assert!(root.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("stellarecord-native-startup-ui-picker-")));
        assert_eq!(native_startup_desktop_name(unsafe { GetThreadDesktop(GetCurrentThreadId()) }), std::env::var("STELLA_STARTUP_TEST_DESKTOP")?);
        let path = std::env::var("STELLA_STARTUP_TEST_REGISTRY")?; assert_eq!(path, format!("Software\\StellaRecord.Tests\\startup-{}", root.file_name().unwrap().to_str().unwrap()));
        let owner = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(path)?;
        assert_eq!(PathBuf::from(owner.get_value::<String, _>("Owner")?).canonicalize()?, resolved_root);
        fs::write(root.join("ui-picker.child-stage.txt"), "validated-owned-root")?;
        let registry = super::NativeRegistryFixture::new()?; let path = registry.path.clone();
        // canonicalizeは所有確認だけへ使い、SDKとpickerのpathは親が渡した絶対path表記で照合する。
        super::initialize_native_command_directory(root, &registry)?; fs::write(root.join("ui-picker.child-stage.txt"), "configured-owned-data")?;
        super::verify_native_ui_probe(root, "ui-picker", true, case == NativeUiPickerCase::ReadFailure, case == NativeUiPickerCase::StreamLocked, case == NativeUiPickerCase::StreamPartial)?;
        drop(registry); assert!(winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(path).is_err());
        fs::write(root.join("ui-picker.completed.txt"), b"registry removed and verification completed")?; Ok(())
    }

    fn native_ui_picker_fixture(root: &Path, case: NativeUiPickerCase) -> StartupTestResult<Vec<(String, Vec<u8>)>> {
        let raw = root.join("output_log_選択 ★ 直接.txt"); let compressed = root.join("output_log_選択 ★ 圧縮.tar.zst");
        let raw_bytes = (0..501).map(|index| format!("2026.10.08 13:00:00 Log - 外部ログ{index} 日本語と\"引用符\"")).collect::<Vec<_>>().join("\n").into_bytes();
        fs::write(&raw, &raw_bytes)?;
        if case == NativeUiPickerCase::StreamPartial {
            let contents = format!("{}\n", String::from_utf8(raw_bytes.clone())?);
            let compressed_bytes = super::native_viewer_archive_fixture("output_log_選択 ★ 直接.txt", &contents, true)?;
            fs::write(&compressed, &compressed_bytes)?;
            fs::write(root.join("ui-picker.partial-before.tar.zst"), &compressed_bytes)?;
            return Ok(vec![(raw.display().to_string(), raw_bytes), (compressed.display().to_string(), compressed_bytes)]);
        }
        // 実decoderの入力を独立したtar writerで作り、productionの圧縮関数を期待値の生成に使わない。
        let encoder = zstd::stream::write::Encoder::new(File::create(&compressed)?, 3)?; let mut builder = tar::Builder::new(encoder);
        if case != NativeUiPickerCase::ReadFailure {
            let mut header = tar::Header::new_gnu(); header.set_path("output_log_2026-10-08_12-00-00.txt")?; header.set_size(super::import_log().len() as u64); header.set_mode(0o644); header.set_cksum();
            builder.append(&header, super::import_log().as_bytes())?;
        }
        builder.into_inner()?.finish()?;
        let compressed_bytes = fs::read(&compressed)?;
        Ok(vec![(raw.display().to_string(), raw_bytes), (compressed.display().to_string(), compressed_bytes)])
    }

    fn native_ui_picker_stage(root: &Path, desktop: &NativeStartupDesktop, child: &NativeStartupProcess, expected: &str, deadline: std::time::Instant) -> StartupHandle {
        while fs::read_to_string(root.join("ui-picker.progress.txt")).ok().as_deref() != Some(expected) {
            assert!(child.running() && std::time::Instant::now() < deadline, "画面のpicker開始を確認できません: {expected}: {}", child.diagnostic()); std::thread::sleep(Duration::from_millis(20));
        }
        native_picker_dialog(desktop, child)
    }

    #[test]
    fn native_browser_ui_external_picker_cancels_selects_and_returns_to_archive() -> StartupTestResult {
        native_ui_picker_case("native_browser_ui_external_picker_cancels_selects_and_returns_to_archive", NativeUiPickerCase::Normal)
    }

    #[test]
    fn native_browser_ui_external_picker_read_failure_notifies_and_retries() -> StartupTestResult {
        native_ui_picker_case("native_browser_ui_external_picker_read_failure_notifies_and_retries", NativeUiPickerCase::ReadFailure)
    }

    #[test]
    fn native_browser_ui_external_picker_stream_lock_preserves_data_and_retries_same_file() -> StartupTestResult {
        native_ui_picker_case("native_browser_ui_external_picker_stream_lock_preserves_data_and_retries_same_file", NativeUiPickerCase::StreamLocked)
    }

    #[test]
    fn native_browser_ui_external_picker_partial_stream_preserves_rows_and_retries_repaired_file() -> StartupTestResult {
        native_ui_picker_case("native_browser_ui_external_picker_partial_stream_preserves_rows_and_retries_repaired_file", NativeUiPickerCase::StreamPartial)
    }

    /// 同じ所有境界で、正常選択・開始時拒否・メタ情報返却後故障と再選択を照合する。
    fn native_ui_picker_case(name: &str, case: NativeUiPickerCase) -> StartupTestResult {
        if let Some(root) = std::env::var_os("STELLA_STARTUP_TEST_ROOT") { return native_ui_picker_child(&PathBuf::from(root), case); }
        let external_read_failure = case == NativeUiPickerCase::ReadFailure; let external_stream_lock = case == NativeUiPickerCase::StreamLocked; let external_stream_partial = case == NativeUiPickerCase::StreamPartial;
        let _lock = NATIVE_STARTUP_TEST_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner); let input = native_startup_input_desktop();
        let directory = tempfile::Builder::new().prefix("stellarecord-native-startup-ui-picker-").tempdir()?;
        let archive_guard = NativeUiPickerArchive(directory.path(), case.prefix());
        let mut external = native_ui_picker_fixture(directory.path(), case)?; let expected = external.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>();
        let initial_external_file_hashes = external.iter().map(|(path, bytes)| (path.clone(), format!("{:x}", sha2::Sha256::digest(bytes)))).collect::<Vec<_>>();
        let mut stream_snapshots: Vec<NativeUiPickerDatabaseSnapshot> = Vec::new(); let mut lock_release_stage = None; let mut repair_stage = None;
        let registry = NativeStartupRegistry::new(directory.path())?; let registry_path = registry.path.clone(); let desktop = NativeStartupDesktop::new(directory.path()); let executable = std::env::current_exe()?;
        let child = NativeStartupProcess::start_with_args(&executable, directory.path(), &desktop, &registry,
            &format!("--exact commands::tests::startup::{name} --test-threads=1 --nocapture"));
        // 本体Browserの60秒を新しい15秒待機で先に打ち切らず、既存command childと同じ全体120秒で監視する。
        let deadline = std::time::Instant::now() + Duration::from_secs(120);
        let stages = if external_read_failure || external_stream_lock || external_stream_partial { vec!["picker-initial-cancel", "picker-select", "picker-retry-select", "picker-selected-cancel"] } else { vec!["picker-initial-cancel", "picker-select", "picker-selected-cancel"] };
        for stage in &stages {
            let dialog = native_ui_picker_stage(directory.path(), &desktop, &child, stage, deadline);
            if (external_stream_lock || external_stream_partial) && *stage != "picker-initial-cancel" {
                let database = directory.path().join("db/current.sqlite"); let wal = database.with_extension("sqlite-wal");
                let snapshot = NativeUiPickerDatabaseSnapshot { stage: (*stage).to_string(), database_bytes: fs::read(database)?, wal_bytes: if wal.exists() { Some(fs::read(wal)?) } else { None } };
                assert!(!snapshot.database_bytes.is_empty());
                if let Some(before) = stream_snapshots.first() {
                    let unchanged = before.database_bytes == snapshot.database_bytes && before.wal_bytes == snapshot.wal_bytes;
                    if !unchanged { fs::write(directory.path().join("ui-picker.database-mismatch.json"), startup_json((&stream_snapshots, &snapshot))?)?; }
                    assert!(unchanged, "stream故障・再試行後に保存済みDBまたはWALが変化しています");
                }
                stream_snapshots.push(snapshot);
                if external_stream_lock && *stage == "picker-retry-select" { assert_eq!(fs::read_to_string(directory.path().join("ui-picker.stream-lock-state.txt"))?, "released"); lock_release_stage = Some("before-picker-retry-select".to_string()); }
            }
            if external_stream_partial && *stage == "picker-retry-select" {
                assert_eq!(fs::read(&external[1].0)?, external[1].1);
                let contents = format!("{}\n2026.10.08 13:00:00 Log - 修復後の最終行\n", String::from_utf8(external[0].1.clone())?);
                let repaired = super::native_viewer_archive_fixture("output_log_選択 ★ 直接.txt", &contents, false)?;
                fs::write(&external[1].0, &repaired)?;
                fs::write(directory.path().join("ui-picker.partial-after.tar.zst"), &repaired)?;
                external[1].1 = repaired; repair_stage = Some("native-picker-open-before-same-path-reselection".to_string());
            }
            match *stage {
                "picker-select" => native_picker_select(dialog, if external_read_failure || external_stream_partial { &expected[1..] } else if external_stream_lock { &expected[..1] } else { &expected }),
                "picker-retry-select" => native_picker_select(dialog, if external_stream_partial { &expected[1..] } else { &expected[..1] }),
                _ => { assert_ne!(unsafe { PostMessageW(dialog, 0x10, 0, 0) }, 0); }
            }
        }
        let code = child.finish(); assert_eq!(code, 0, "画面から実pickerを通すchildが失敗しました: {}", child.diagnostic()); drop(child);
        assert_eq!(fs::read(directory.path().join("ui-picker.completed.txt"))?, b"registry removed and verification completed");
        let report = tauri::ipc::InvokeResponseBody::Json(fs::read_to_string(directory.path().join("ui-picker.report.json"))?).deserialize::<super::NativeProbeReport>()?;
        assert!(report.ok && report.error.is_empty()); assert_eq!(report.checks.len(), if external_read_failure || external_stream_lock || external_stream_partial { 11 } else { 10 }); assert_eq!(report.picker_results.len(), stages.len());
        assert!(report.picker_results[0].is_empty() && report.picker_results.last().is_some_and(Vec::is_empty));
        if external_read_failure { assert_eq!(report.picker_results[1], expected[1..]); assert_eq!(report.picker_results[2], expected[..1]); assert_eq!(report.viewer_failures.len(), 1); }
        else if external_stream_lock { assert_eq!(report.picker_results[1], expected[..1]); assert_eq!(report.picker_results[2], expected[..1]); assert!(report.viewer_failures.is_empty()); assert_eq!(stream_snapshots.len(), 3); assert_eq!(fs::read_to_string(directory.path().join("ui-picker.stream-lock-state.txt"))?, "released"); }
        else if external_stream_partial { assert_eq!(report.picker_results[1], expected[1..]); assert_eq!(report.picker_results[2], expected[1..]); assert!(report.viewer_failures.is_empty()); assert_eq!(stream_snapshots.len(), 3); assert!(repair_stage.is_some()); }
        else { let mut actual = report.picker_results[1].clone(); let mut expected = expected; actual.sort(); expected.sort(); assert_eq!(actual, expected); assert!(report.viewer_failures.is_empty()); }
        let mut hashes = Vec::new(); for (path, bytes) in external { assert_eq!(fs::read(&path)?, bytes); hashes.push((path, format!("{:x}", sha2::Sha256::digest(bytes)))); }
        let stream_warning = if external_stream_lock {
            let log = startup_log(directory.path())?; let warnings = log.lines().filter(|line| line.contains("ログビューアストリーム失敗:") && line.contains(&report.viewer_starts[0].file_path)).collect::<Vec<_>>();
            assert_eq!(warnings.len(), 1); warnings[0].to_string()
        } else if external_stream_partial {
            let log = startup_log(directory.path())?; let warnings = log.lines().filter(|line| line.contains("ログビューア行を読み取れませんでした:")).collect::<Vec<_>>();
            assert_eq!(warnings.len(), 1); warnings[0].to_string()
        } else { String::new() };
        let window_cleanup = desktop.wait_for_empty_windows(); drop(desktop); assert_eq!(native_startup_input_desktop(), input);
        drop(registry); assert!(winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(registry_path).is_err()); drop(archive_guard);
        let evidence = NativeUiPickerEvidence { report, stages: stages.iter().map(ToString::to_string).collect(), window_cleanup, external_file_hashes: hashes, child_exit_code: code,
            actual_frontend_main: true, actual_run: false, dialog_replaced: false, core_sdk_facade_used: true, input_desktop_unchanged: true, owned_registry_removed: true, owned_root_removed: startup_remove_fixture(directory)?,
            stream_warning, stream_snapshots, lock_release_stage, initial_external_file_hashes, repair_stage, physical_input_verified: false, release_entry_verified: false, verification_complete: false };
        if let Some(folder) = startup_evidence_directory()? { let target = folder.join(format!("stellarecord-native-{}-evidence.json", case.prefix())); assert!(!target.exists()); fs::write(target, startup_json(evidence)?)?; }
        Ok(())
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct NativePickerObservation { com_error: String, cancelled: Vec<String>, selected: Vec<String>, final_com_initialization: i32 }

    fn native_picker_paths(kind: &str) -> Result<Vec<String>, String> {
        match kind {
            "exe" => super::registry::pick_exe_file().map(|value| value.into_iter().collect()),
            "logs" => super::archive::pick_log_files(),
            _ => unreachable!(),
        }
    }

    fn native_picker_child(root: &Path, kind: &str) -> StartupTestResult {
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
        let root = root.canonicalize()?; assert_eq!(root.parent(), Some(std::env::temp_dir().canonicalize()?.as_path()));
        assert!(root.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with(&format!("stellarecord-native-startup-picker-{kind}-"))));
        assert_eq!(native_startup_desktop_name(unsafe { GetThreadDesktop(GetCurrentThreadId()) }), std::env::var("STELLA_STARTUP_TEST_DESKTOP")?);
        let path = std::env::var("STELLA_STARTUP_TEST_REGISTRY")?; assert_eq!(path, format!("Software\\StellaRecord.Tests\\startup-{}", root.file_name().unwrap().to_str().unwrap()));
        let registry = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey_with_flags(path, winreg::enums::KEY_READ | winreg::enums::KEY_WRITE)?;
        assert_eq!(PathBuf::from(registry.get_value::<String, _>("Owner")?).canonicalize()?, root);
        assert_eq!(unsafe { super::RegOverridePredefKey(winreg::enums::HKEY_CURRENT_USER, registry.raw_handle()) }, 0);
        // 別COM apartmentの実HRESULTを発生させ、故障除去後に同じthreadで取消し・再選択する。
        assert_eq!(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.0, 0);
        let com_error = native_picker_paths(kind).unwrap_err(); unsafe { CoUninitialize() };
        assert!(com_error.starts_with("COM 初期化に失敗しました: ") && com_error.contains("0x80010106"), "{com_error}");
        let cancelled = native_picker_paths(kind)?; assert!(cancelled.is_empty()); fs::write(root.join("picker.cancelled.txt"), b"cancelled")?;
        let selected = native_picker_paths(kind)?;
        // productionによるCOM解除を確認し、test自身が追加した初期化も解除する。
        let final_com_initialization = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.0; assert_eq!(final_com_initialization, 0); unsafe { CoUninitialize() };
        fs::write(root.join("picker.result.json"), startup_json(NativePickerObservation { com_error, cancelled, selected, final_com_initialization })?)?;
        assert_eq!(unsafe { super::RegOverridePredefKey(winreg::enums::HKEY_CURRENT_USER, 0) }, 0); Ok(())
    }

    fn native_picker_dialog(desktop: &NativeStartupDesktop, child: &NativeStartupProcess) -> StartupHandle {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            let dialogs: Vec<_> = desktop.windows().into_iter().filter(|window| {
                let mut pid = 0; unsafe { GetWindowThreadProcessId(*window, &raw mut pid) };
                pid == child.pid && startup_window_class(*window) == "#32770" && unsafe { IsWindowVisible(*window) } != 0 && !unsafe { GetDlgItem(*window, 2) }.is_null()
            }).collect();
            if dialogs.len() == 1 { return dialogs[0]; }
            assert!(child.running(), "所有するpickerの表示前にchildが終了しました: {}", child.diagnostic());
            assert!(std::time::Instant::now() < deadline, "所有するpickerを一意に確認できません: {:?}", desktop.window_snapshot()); std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn native_picker_select(dialog: StartupHandle, expected: &[String]) {
        unsafe extern "system" fn collect(window: StartupHandle, parameter: isize) -> i32 { unsafe { &mut *(parameter as *mut Vec<StartupHandle>) }.push(window); 1 }
        // Windows SDK dlgs.hのcmb13に含まれる実file名Editだけへ入力する。
        let field = unsafe { GetDlgItem(dialog, 0x047c) }; assert!(!field.is_null(), "所有するpickerのfile名controlがありません");
        let mut controls = Vec::new(); unsafe { EnumChildWindows(field, Some(collect), std::ptr::from_mut(&mut controls) as isize) };
        let edits: Vec<_> = controls.into_iter().filter(|window| startup_window_class(*window) == "Edit").collect(); assert_eq!(edits.len(), 1);
        let value = expected.iter().map(|path| format!("\"{path}\"")).collect::<Vec<_>>().join(" "); let mut received = 0;
        assert_ne!(unsafe { SendMessageTimeoutW(edits[0], 0x0c, 0, startup_wide(&value).as_ptr() as isize, 2, 1000, &raw mut received) }, 0);
        assert_eq!(startup_window_text(edits[0]).as_deref(), Some(value.as_str()));
        let button = unsafe { GetDlgItem(dialog, 1) }; assert!(!button.is_null()); assert_ne!(unsafe { PostMessageW(button, 0x00f5, 0, 0) }, 0);
    }

    fn native_picker_test(kind: &str, test_name: &str) -> StartupTestResult {
        if let Some(root) = std::env::var_os("STELLA_STARTUP_TEST_ROOT") { return native_picker_child(&PathBuf::from(root), kind); }
        let _lock = NATIVE_STARTUP_TEST_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner); let input = native_startup_input_desktop();
        let directory = tempfile::Builder::new().prefix(&format!("stellarecord-native-startup-picker-{kind}-")).tempdir()?;
        let desktop = NativeStartupDesktop::new(directory.path()); let registry = NativeStartupRegistry::new(directory.path())?; startup_fixture(directory.path(), &registry)?;
        let database = fs::read(directory.path().join("Data/db/stellarecord.db"))?;
        let names = if kind == "exe" { vec!["選択専用★ アプリ.exe"] } else { vec!["output_log_日本語 1.txt", "output_log_日本語 2.tar.zst"] };
        let mut expected = Vec::new();
        for name in names { let path = directory.path().join(name); fs::write(&path, "選択のみで保持する日本語の全byte列\n")?; expected.push(path.display().to_string()); }
        let executable = std::env::current_exe()?;
        let child = NativeStartupProcess::start_with_args(&executable, directory.path(), &desktop, &registry, &format!("--exact commands::tests::startup::{test_name} --nocapture"));
        let cancelled = native_picker_dialog(&desktop, &child); assert_ne!(unsafe { PostMessageW(cancelled, 0x10, 0, 0) }, 0);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !directory.path().join("picker.cancelled.txt").is_file() {
            assert!(child.running() && std::time::Instant::now() < deadline, "picker取消しが完了しません: {}", child.diagnostic()); std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(fs::read(directory.path().join("picker.cancelled.txt"))?, b"cancelled"); startup_preserved(directory.path(), &database)?;
        native_picker_select(native_picker_dialog(&desktop, &child), &expected); assert_eq!(child.finish(), 0); drop(child);
        let observation: NativePickerObservation = tauri::ipc::InvokeResponseBody::Json(fs::read_to_string(directory.path().join("picker.result.json"))?).deserialize()?;
        assert!(observation.cancelled.is_empty()); assert_eq!(observation.selected.len(), expected.len());
        let mut actual = observation.selected.clone(); actual.sort(); expected.sort(); assert_eq!(actual, expected); assert_eq!(observation.final_com_initialization, 0);
        assert!(observation.com_error.starts_with("COM 初期化に失敗しました: ") && observation.com_error.contains("0x80010106"));
        for path in &expected { assert_eq!(fs::read(path)?, "選択のみで保持する日本語の全byte列\n".as_bytes()); }
        startup_preserved(directory.path(), &database)?; let window_cleanup = desktop.wait_for_empty_windows(); drop(desktop); assert_eq!(native_startup_input_desktop(), input);
        let mut evidence = startup_evidence(if kind == "exe" { "picker-exe" } else { "picker-logs" }, None, None, vec![0]); evidence.actual_main_body = false; evidence.actual_run = false;
        evidence.window_cleanup = Some(window_cleanup); evidence.picker = Some(observation);
        startup_finish(evidence.kind, directory, &executable, &executable, registry, evidence)
    }

    #[test]
    fn native_file_picker_exe_recovers_com_conflict_cancels_and_selects_unicode() -> StartupTestResult {
        native_picker_test("exe", "native_file_picker_exe_recovers_com_conflict_cancels_and_selects_unicode")
    }

    #[test]
    fn native_file_picker_logs_recovers_com_conflict_cancels_and_selects_multiple() -> StartupTestResult {
        native_picker_test("logs", "native_file_picker_logs_recovers_com_conflict_cancels_and_selects_multiple")
    }
}
