// registry解決から実際のDB・アーカイブ・OS操作までを、専用の子process内で検証する。
use std::os::windows::process::CommandExt;
use std::process::Command;
type NativeCommandTestResult = Result<(), Box<dyn std::error::Error>>;

// unit test executableにも既存のproduction resourceを取り込み、Common Controls v6を有効にする。
#[cfg(windows)]
#[link(name = "resource.lib", kind = "static", modifiers = "+verbatim,-bundle")]
extern "C" {}

#[link(name = "advapi32")]
extern "system" { fn RegOverridePredefKey(key: isize, replacement: isize) -> i32; }

/// 子processのHKCUだけを専用keyへ向け、実際のRun設定とインストール先に触れない。
struct NativeRegistryFixture { path: String, original: winreg::RegKey, isolated: winreg::RegKey }
impl NativeRegistryFixture {
    fn new() -> std::io::Result<Self> {
        let original = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(std::io::Error::other)?.as_nanos();
        let path = format!("Software\\StellaRecord.Tests\\fixture-{}-{nonce}", std::process::id());
        let (isolated, _) = original.create_subkey(&path)?;
        let status = unsafe { RegOverridePredefKey(winreg::enums::HKEY_CURRENT_USER, isolated.raw_handle()) };
        if status != 0 { original.delete_subkey_all(&path)?; return Err(std::io::Error::from_raw_os_error(status)); }
        Ok(Self { path, original, isolated })
    }
}
impl Drop for NativeRegistryFixture {
    fn drop(&mut self) {
        let _ = unsafe { RegOverridePredefKey(winreg::enums::HKEY_CURRENT_USER, 0) };
        let _ = self.original.delete_subkey_all(&self.path);
    }
}

fn native_command_test(name: &str, verify: impl FnOnce(&tempfile::TempDir, &NativeRegistryFixture) -> NativeCommandTestResult) -> NativeCommandTestResult {
    if std::env::var("STELLA_NATIVE_COMMAND_TEST_CHILD").ok().as_deref() != Some(name) {
        let stdout = tempfile::NamedTempFile::new()?; let stderr = tempfile::NamedTempFile::new()?;
        let mut child = Command::new(std::env::current_exe()?).args(["--exact", &format!("commands::tests::{name}"), "--nocapture"])
            .env("STELLA_NATIVE_COMMAND_TEST_CHILD", name).creation_flags(0x0800_0000)
            .stdout(stdout.as_file().try_clone()?).stderr(stderr.as_file().try_clone()?).spawn()?;
        // pipe容量による停止を避け、OS境界の停止時は自分で起動した子processだけを終了する。
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        let status = loop {
            if let Some(status) = child.try_wait()? { break status; }
            if std::time::Instant::now() >= deadline { child.kill()?; child.wait()?; return Err(format!("{name}: 専用子processが120秒以内に終了しませんでした").into()); }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let stdout = fs::read_to_string(stdout.path())?; let stderr = fs::read_to_string(stderr.path())?;
        assert!(status.success(), "{name}: {stdout} {stderr}");
        assert!(stdout.contains("test result: ok. 1 passed; 0 failed;"), "{name}: 専用子processで対象testの完了を確認できません: {stdout}"); return Ok(());
    }
    let directory = tempfile::tempdir()?; let registry = NativeRegistryFixture::new()?;
    initialize_native_command_directory(directory.path(), &registry)?;
    verify(&directory, &registry)
}

/// 通常のcommand childと専用desktopのUI childで、同じ現行設定と所有するDataだけを使用する。
fn initialize_native_command_directory(directory: &std::path::Path, registry: &NativeRegistryFixture) -> NativeCommandTestResult {
    let (stella, _) = registry.isolated.create_subkey("Software\\CosmoArtsStore\\StellaRecord")?;
    stella.set_value("InstallLocation", &directory.to_string_lossy().as_ref())?;
    assert_eq!(utils::get_stellarecord_install_dir().as_deref(), Some(directory));
    let polaris_root = directory.join("Polaris"); fs::create_dir(&polaris_root)?;
    let (polaris, _) = registry.isolated.create_subkey("Software\\CosmoArtsStore\\Polaris")?;
    polaris.set_value("InstallLocation", &polaris_root.to_string_lossy().as_ref())?;
    config::save_stellarecord_setting(&config::StellaRecordSetting { archive_path: directory.join("archive").to_string_lossy().into_owned(), db_path: directory.join("db/current.sqlite").to_string_lossy().into_owned(), ..Default::default() })?;
    config::save_polaris_setting(&config::PolarisSetting { archive_path: "保存済み設定".to_string(), capacity_threshold_bytes: 127 * 1_048_576 })?;
    Ok(())
}

#[test]
fn command_settings_persist_registry_and_run_registration() -> NativeCommandTestResult {
    native_command_test("command_settings_persist_registry_and_run_registration", |directory, registry| {
        let initial = settings::get_startup_import_settings()?;
        assert!(!initial.enabled); assert!(!initial.preference_set); assert_eq!(initial.database_path, directory.path().join("db/current.sqlite").display().to_string());
        settings::save_startup_import_preference(true)?; let enabled = settings::get_startup_import_settings()?; assert!(enabled.enabled && enabled.preference_set);
        settings::save_startup_import_preference(false)?; let disabled = settings::get_startup_import_settings()?; assert!(!disabled.enabled && disabled.preference_set);
        let initial = settings::get_management_settings(); assert_eq!(initial.archive_limit_mb, 127); assert!(!initial.startup_preference_set);
        assert!(settings::save_management_settings(true, 0).is_err()); assert!(settings::save_management_settings(true, 10_485_761).is_err());
        assert_eq!(settings::get_management_settings().archive_limit_mb, 127);
        settings::save_management_settings(true, 512)?;
        let run = registry.isolated.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")?;
        assert_eq!(run.get_value::<String, _>("StellaRecord")?, format!("\"{}\"", std::env::current_exe()?.display()));
        let actual = settings::get_management_settings(); assert!(actual.startup_enabled && actual.startup_preference_set); assert_eq!(actual.archive_limit_mb, 512);
        assert_eq!(config::load_polaris_setting().archive_path, "保存済み設定");
        settings::save_management_settings(false, 1)?; assert!(run.get_value::<String, _>("StellaRecord").is_err());
        assert!(!settings::get_management_settings().startup_enabled); assert_eq!(config::load_polaris_setting().capacity_threshold_bytes, 1_048_576);
        Ok(())
    })
}

#[test]
fn command_registry_catalog_round_trip_uses_current_database() -> NativeCommandTestResult {
    native_command_test("command_registry_catalog_round_trip_uses_current_database", |directory, _| {
        let path = directory.path().join("owned.exe"); fs::write(&path, "専用の非PE fixture".as_bytes())?; let path = path.to_string_lossy().into_owned();
        assert!(registry::register_app(directory.path().join("missing.exe").to_string_lossy().into_owned(), "未登録".to_string(), String::new()).is_err());
        assert!(registry::register_app(path.clone(), "  ".to_string(), String::new()).is_err());
        assert_eq!(registry::extract_exe_display_name(path.clone())?, "owned");
        assert!(registry::extract_exe_display_name(directory.path().join("missing.exe").to_string_lossy().into_owned()).is_err());
        registry::register_app(path.clone(), "  日本語アプリ  ".to_string(), "説明".to_string())?;
        assert!(registry::register_app(path.clone(), "変更不可".to_string(), String::new()).is_err());
        let catalog = settings::read_registry_catalog(); assert_eq!(catalog.apps.len(), 1); assert_eq!(catalog.apps[0].name, "日本語アプリ"); assert_eq!(catalog.apps[0].description, "説明"); assert_eq!(catalog.apps[0].path, path);
        assert!(registry::launch_external_app(&path).is_err());
        registry::unregister_app(path.clone())?; assert!(registry::unregister_app(path).is_err()); assert!(settings::read_registry_catalog().apps.is_empty());
        assert!(directory.path().join("owned.exe").exists()); Ok(())
    })
}

#[test]
fn command_database_preview_pages_and_sorts_real_current_tables() -> NativeCommandTestResult {
    native_command_test("command_database_preview_pages_and_sorts_real_current_tables", |_, _| {
        assert!(database::get_db_tables()?.is_empty()); let path = get_db_path()?; assert!(!path.exists());
        let mut conn = Connection::open(&path)?; analyze::init_main_db(&conn)?;
        let tx = conn.transaction()?;
        for index in 0..1001 { tx.execute("INSERT INTO apps (name, description, path) VALUES (?1, '説明', ?2)", rusqlite::params![format!("アプリ{index:04}"), format!("path-{index}")])?; }
        tx.commit()?; drop(conn);
        let tables = database::get_db_tables()?; assert!(tables.iter().any(|table| table.name == "apps"));
        let first = database::get_db_table_data("apps", None, Some("name".to_string()), Some("asc".to_string()))?;
        let column = first.columns.iter().position(|column| column.name == "name").ok_or("名前columnがありません")?;
        assert_eq!(first.total_rows, 1001); assert_eq!(first.rows.len(), 500); assert_eq!(first.rows[0][column], "アプリ0000"); assert_eq!(first.storage, "Main DB");
        let third = database::get_db_table_data("apps", Some(2), Some("name".to_string()), Some("asc".to_string()))?; assert_eq!(third.rows.len(), 1); assert_eq!(third.rows[0][column], "アプリ1000");
        let reversed = database::get_db_table_data("apps", Some(0), Some("name".to_string()), Some("desc".to_string()))?; assert_eq!(reversed.rows[0][column], "アプリ1000");
        assert!(database::get_db_table_data("apps", Some(u32::MAX), None, None)?.rows.is_empty());
        assert!(database::get_db_table_data("apps; DROP TABLE apps", None, None, None).is_err()); assert!(database::get_db_table_data("unknown", None, None, None).is_err());
        assert!(database::get_db_table_data("apps", None, Some("unknown".to_string()), None).is_err()); assert_eq!(database::get_db_table_data("apps", None, None, None)?.total_rows, 1001);
        Ok(())
    })
}

#[test]
fn command_database_open_failure_preserves_file_and_reports_error() -> NativeCommandTestResult {
    native_command_test("command_database_open_failure_preserves_file_and_reports_error", |_, _| {
        let path = get_db_path()?; fs::write(&path, b"not a database")?;
        assert!(database::get_db_tables().is_err()); assert!(database::get_db_table_data("apps", None, None, None).is_err()); assert!(open_initialized_main_db().is_err());
        assert_eq!(fs::read(&path)?, b"not a database"); Ok(())
    })
}

#[test]
fn command_archive_cleanup_deletes_only_exactly_archived_owned_logs() -> NativeCommandTestResult {
    native_command_test("command_archive_cleanup_deletes_only_exactly_archived_owned_logs", |directory, _| {
        let source = get_source_log_dir()?; let store = get_archive_store_dir()?; fs::create_dir_all(&source)?; fs::create_dir_all(&store)?;
        let first = "output_log_2026-10-08_12-00-00.txt"; let second = "output_log_2026-10-08_13-00-00.txt";
        fs::write(source.join(first), b"first log\n")?; fs::write(source.join(second), b"second log\n")?; fs::write(source.join("unrelated.txt"), b"keep")?;
        assert_eq!(archive::sync_source_logs_into_archive_store(&source, &store)?, 2);
        let listed = archive::list_archive_files()?; assert_eq!(listed.len(), 2); assert_eq!(listed[0].name, format!("{second}.tar.zst"));
        let candidates = archive::get_deletable_source_logs()?; assert_eq!(candidates.iter().map(|item| item.file_name.as_str()).collect::<Vec<_>>(), [first, second]);
        fs::write(source.join(first), b"changed after archive")?;
        assert!(archive::delete_source_logs(vec![first.to_string()]).is_err()); assert_eq!(fs::read(source.join(first))?, b"changed after archive");
        assert_eq!(archive::get_deletable_source_logs()?.len(), 1);
        assert!(archive::delete_source_logs(vec!["../outside.txt".to_string()]).is_err()); assert_eq!(fs::read(source.join("unrelated.txt"))?, b"keep");
        assert_eq!(archive::delete_source_logs(vec![second.to_string()])?, 1); assert_eq!(archive::delete_source_logs(vec![second.to_string()])?, 0); assert!(!source.join(second).exists());
        assert!(store.join(format!("{second}.tar.zst")).exists());
        let expected_size = listed.iter().map(|item| item.size_bytes).sum::<u64>(); assert_eq!(archive::get_storage_status()?, (expected_size, 127 * 1_048_576));
        assert!(directory.path().join("Polaris/Data/archive").is_dir()); Ok(())
    })
}

#[test]
fn command_archive_paths_report_unavailable_storage_without_deleting_source() -> NativeCommandTestResult {
    native_command_test("command_archive_paths_report_unavailable_storage_without_deleting_source", |directory, registry| {
        assert!(archive::list_archive_files()?.is_empty()); assert_eq!(archive::get_storage_status()?, (0, 127 * 1_048_576));
        let source = get_source_log_dir()?; fs::create_dir_all(&source)?; fs::write(source.join("output_log_owned.txt"), b"unarchived")?;
        assert!(archive::delete_source_logs(vec!["output_log_owned.txt".to_string()]).is_err()); assert_eq!(fs::read(source.join("output_log_owned.txt"))?, b"unarchived");
        let blocked = directory.path().join("blocked"); fs::write(&blocked, b"keep")?; let mut setting = config::load_stellarecord_setting(); setting.archive_path = blocked.to_string_lossy().into_owned(); config::save_stellarecord_setting(&setting)?;
        // 一覧はdirectoryを要求するが、容量計測はfileのmetadataも合計対象とする。
        assert!(archive::list_archive_files().is_err()); assert_eq!(archive::get_storage_status()?, (4, 127 * 1_048_576)); assert_eq!(fs::read(&blocked)?, b"keep");
        registry.isolated.delete_subkey_all("Software\\CosmoArtsStore\\Polaris")?; assert!(archive::get_deletable_source_logs().is_err());
        assert_eq!(fs::read(source.join("output_log_owned.txt"))?, b"unarchived"); Ok(())
    })
}

#[test]
#[allow(clippy::panic, reason = "専用子process内のpanic hookとfallback記録を確認するため")]
fn command_logging_panic_hook_and_native_launch_use_owned_paths() -> NativeCommandTestResult {
    native_command_test("command_logging_panic_hook_and_native_launch_use_owned_paths", |directory, _| {
        log_client_error(format!("改行\r\n{}捨てる", "あ".repeat(4010))); crate::platform::install_panic_hook();
        assert!(std::panic::catch_unwind(|| std::panic::panic_any("文字列panic")).is_err());
        assert!(std::panic::catch_unwind(|| std::panic::panic_any("所有文字列panic".to_string())).is_err());
        assert!(std::panic::catch_unwind(|| std::panic::panic_any(42_u32)).is_err());
        let log_dir = utils::get_stellarecord_data_dir("logs").ok_or("ログdirectoryがありません")?;
        let log_path = fs::read_dir(log_dir)?.next().ok_or("ログfileがありません")??.path(); let logged = fs::read_to_string(log_path)?;
        assert!(logged.contains("frontend: 改行  あ")); assert!(!logged.contains("捨てる")); assert!(logged.contains("所有文字列panic")); assert!(logged.contains("panic メッセージを取得できませんでした"));
        assert!(registry::launch_external_app(&directory.path().join("missing.exe").to_string_lossy()).is_err());
        let probe = directory.path().join("owned_launcher.rs"); let executable = directory.path().join("owned_launcher.exe");
        fs::write(&probe, include_str!("../../tests/fixtures/native_launcher.rs"))?;
        let compiled = Command::new("rustc").arg(&probe).args(["--edition", "2021", "-o"]).arg(&executable).creation_flags(0x0800_0000).output()?;
        assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr)); registry::launch_external_app(&executable.to_string_lossy())?;
        let marker = executable.with_extension("invoked"); let started = std::time::Instant::now();
        while !marker.exists() && started.elapsed() < std::time::Duration::from_secs(10) { std::thread::sleep(std::time::Duration::from_millis(10)); }
        assert_eq!(fs::read(marker)?, b"native launch reached");
        while fs::remove_file(&executable).is_err() && started.elapsed() < std::time::Duration::from_secs(10) { std::thread::sleep(std::time::Duration::from_millis(10)); }
        assert!(!executable.exists()); Ok(())
    })
}

#[derive(Debug, serde::Deserialize)]
struct NativeProgress { status: String, progress: String, is_running: bool }
struct NativeImportEvents { receiver: std::sync::mpsc::Receiver<(bool, String)> }

struct NativeImportApp { app: tauri::App }
impl std::ops::Deref for NativeImportApp { type Target = tauri::App; fn deref(&self) -> &Self::Target { &self.app } }
impl Drop for NativeImportApp {
    fn drop(&mut self) {
        use tauri::Manager;
        let status = self.app.state::<crate::AnalyzeCancelStatus>();
        if status.running.load(std::sync::atomic::Ordering::SeqCst) { status.cancel.store(true, std::sync::atomic::Ordering::SeqCst); }
        // assertionやevent待機が失敗しても、worker終了前にHKCUの復元とfixture削除を進めない。
        while status.running.load(std::sync::atomic::Ordering::SeqCst) { std::thread::sleep(std::time::Duration::from_millis(1)); }
    }
}

/// 実際のworkerをsavepoint成功後に停止し、競合とキャンセルを確定した順序で検証する。
struct NativeWorkerPause {
    entered: std::sync::mpsc::Receiver<()>, release: Option<std::sync::mpsc::Sender<()>>,
    running: std::sync::Arc<std::sync::atomic::AtomicBool>, cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

fn pause_native_import_after_file(app: &tauri::App, name: &str) -> NativeWorkerPause {
    use tauri::{Listener, Manager};
    let expected = format!("取り込み完了: {name}"); let (entered_sender, entered) = std::sync::mpsc::channel();
    let (release, release_receiver) = std::sync::mpsc::channel(); let release_receiver = std::sync::Mutex::new(release_receiver);
    let paused = std::sync::atomic::AtomicBool::new(false);
    app.listen("analyze-progress", move |event| {
        let Ok(progress) = tauri::ipc::InvokeResponseBody::Json(event.payload().to_string()).deserialize::<NativeProgress>() else { return; };
        if progress.status == expected && !paused.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let _ = entered_sender.send(()); if let Ok(receiver) = release_receiver.lock() { let _ = receiver.recv(); }
        }
    });
    let status = app.state::<crate::AnalyzeCancelStatus>(); NativeWorkerPause { entered, release: Some(release), running: status.running.clone(), cancel: status.cancel.clone() }
}

impl Drop for NativeWorkerPause {
    fn drop(&mut self) {
        if self.running.load(std::sync::atomic::Ordering::SeqCst) { self.cancel.store(true, std::sync::atomic::Ordering::SeqCst); }
        if let Some(release) = self.release.take() { let _ = release.send(()); }
        // workerが残る間はHKCUの復元とfixture削除を進めない。停止時は親processの期限で終了する。
        while self.running.load(std::sync::atomic::Ordering::SeqCst) { std::thread::sleep(std::time::Duration::from_millis(1)); }
    }
}

/// 実際のWry AppHandleとevent配信を構築する。productionのWindow作成とIPC dispatchは別途検証する。
fn native_import_app() -> Result<(NativeImportApp, NativeImportEvents), Box<dyn std::error::Error>> {
    use tauri::{Listener, Manager};
    let mut context = tauri::generate_context!(); context.config_mut().app.windows.clear();
    let status = crate::AnalyzeCancelStatus { cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)), running: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)) };
    let app = tauri::Builder::default().any_thread().manage(status).build(context)?;
    assert!(app.webview_windows().is_empty()); let (sender, receiver) = std::sync::mpsc::channel(); let progress_sender = sender.clone();
    app.listen("analyze-progress", move |event| { let _ = progress_sender.send((false, event.payload().to_string())); });
    app.listen("analyze-finished", move |event| { let _ = sender.send((true, event.payload().to_string())); });
    Ok((NativeImportApp { app }, NativeImportEvents { receiver }))
}

impl NativeImportEvents {
    fn finish(&self, app: &tauri::App) -> Result<Vec<NativeProgress>, Box<dyn std::error::Error>> {
        use tauri::Manager;
        let mut progress = Vec::new(); let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let (finished, payload) = self.receiver.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))?;
            if finished { assert_eq!(payload, "null"); break; }
            progress.push(tauri::ipc::InvokeResponseBody::Json(payload).deserialize::<NativeProgress>()?);
        }
        // 完了eventはguardのDropより先に送出されるため、次の開始を許すrunning解除まで確認する。
        let status = app.state::<crate::AnalyzeCancelStatus>(); while status.running.load(std::sync::atomic::Ordering::SeqCst) && std::time::Instant::now() < deadline { std::thread::sleep(std::time::Duration::from_millis(1)); }
        assert!(!status.running.load(std::sync::atomic::Ordering::SeqCst)); assert!(self.receiver.try_recv().is_err()); Ok(progress)
    }
}

fn import_log() -> &'static str {
    "2026.10.08 12:00:00 Log        -  User Authenticated: Owner (usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee)\n2026.10.08 12:01:00 Log        -  [Behaviour] Entering Room: Owned World\n2026.10.08 12:01:01 Log        -  [Behaviour] Joining wrld_abc123:123~public~region(jp)\n2026.10.08 12:05:00 Log        -  [Behaviour] OnLeftRoom\n"
}

fn create_native_import_archive(name: &str, contents: &str) -> Result<String, Box<dyn std::error::Error>> {
    let source = get_source_log_dir()?; fs::create_dir_all(&source)?; fs::write(source.join(name), contents)?;
    assert_eq!(archive::sync_source_logs_into_archive_store(&source, &get_archive_store_dir()?)?, 1);
    Ok(format!("{name}.tar.zst"))
}

#[test]
fn command_import_preflight_rejects_invalid_and_missing_paths_before_worker() -> NativeCommandTestResult {
    native_command_test("command_import_preflight_rejects_invalid_and_missing_paths_before_worker", |directory, _| {
        use tauri::Manager;
        let (app, events) = native_import_app()?; let status = app.state::<crate::AnalyzeCancelStatus>();
        status.cancel.store(true, std::sync::atomic::Ordering::SeqCst); let outside = directory.path().join("outside.txt"); fs::write(&outside, b"keep")?;
        for name in ["", "../output_log_owned.txt.tar.zst", "F:\\output_log_owned.txt.tar.zst", "output_log_owned.txt", "unknown.tar.zst", "output_log_missing.txt.tar.zst"] {
            assert!(import::launch_enhanced_import(app.handle().clone(), vec![name.to_string()], app.state()).is_err(), "{name}");
            assert!(!status.running.load(std::sync::atomic::Ordering::SeqCst)); assert!(status.cancel.load(std::sync::atomic::Ordering::SeqCst));
            assert!(events.receiver.try_recv().is_err()); assert!(!get_db_path()?.exists());
        }
        assert_eq!(fs::read(outside)?, b"keep"); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_import_empty_selection_emits_terminal_progress_and_releases_run() -> NativeCommandTestResult {
    native_command_test("command_import_empty_selection_emits_terminal_progress_and_releases_run", |_, _| {
        use tauri::Manager;
        let (app, events) = native_import_app()?;
        assert_eq!(import::launch_enhanced_import(app.handle().clone(), Vec::new(), app.state())?, "0件のアーカイブ同期を開始しました。");
        let progress = events.finish(&app)?; assert_eq!(progress.len(), 2); assert_eq!(progress[0].status, "処理対象ログなし");
        assert_eq!(progress[0].progress, "0/0"); assert!(progress[0].is_running);
        assert_eq!(progress[1].status, "0件のインポートが完了しました。"); assert_eq!(progress[1].progress, "100%"); assert!(!progress[1].is_running);
        let conn = Connection::open(get_db_path()?)?; assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 0);
        app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_import_enhanced_failure_rolls_back_prior_file_and_can_retry() -> NativeCommandTestResult {
    native_command_test("command_import_enhanced_failure_rolls_back_prior_file_and_can_retry", |_, _| {
        use tauri::Manager;
        let name = "output_log_2026-10-08_12-00-00.txt"; let valid = create_native_import_archive(name, import_log())?;
        let broken_name = "output_log_2026-10-08_13-00-00.txt"; let broken = format!("{broken_name}.tar.zst"); let store = get_archive_store_dir()?;
        fs::write(store.join(&broken), b"not a zstd archive")?; let original_archive = fs::read(store.join(&valid))?;
        let conn = open_initialized_main_db()?; conn.execute("INSERT INTO apps (name, description, path) VALUES ('維持する登録', '', 'owned')", [])?;
        let (app, events) = native_import_app()?;
        import::launch_enhanced_import(app.handle().clone(), vec![broken.clone(), valid.clone()], app.state())?;
        let progress = events.finish(&app)?; assert!(progress.iter().any(|item| item.status == format!("取り込み完了: {name}")));
        let last = progress.last().ok_or("失敗進捗がありません")?; assert!(last.status.starts_with("エラー: ")); assert_eq!(last.progress, "0%"); assert!(!last.is_running);
        assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 0);
        assert_eq!(conn.query_row("SELECT name FROM apps", [], |row| row.get::<_, String>(0))?, "維持する登録");
        assert_eq!(fs::read(store.join(&broken))?, b"not a zstd archive"); assert_eq!(fs::read(store.join(&valid))?, original_archive);
        // 破損fixtureだけを作り直し、同じcommandが失敗後に再開できることを確認する。
        fs::remove_file(store.join(&broken))?; create_native_import_archive(broken_name, &import_log().replace("12:", "13:"))?;
        import::launch_enhanced_import(app.handle().clone(), vec![broken, valid], app.state())?; let retried = events.finish(&app)?;
        assert_eq!(retried.last().ok_or("再試行の完了進捗がありません")?.status, "2件のインポートが完了しました。");
        assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 2);
        assert_eq!(conn.query_row("SELECT count(*) FROM visits", [], |row| row.get::<_, i64>(0))?, 2);
        assert_eq!(fs::read_to_string(get_source_log_dir()?.join(name))?, import_log()); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_import_startup_unset_and_disabled_skip_paths_but_enabled_validates() -> NativeCommandTestResult {
    native_command_test("command_import_startup_unset_and_disabled_skip_paths_but_enabled_validates", |_, registry| {
        use tauri::Manager;
        registry.isolated.delete_subkey_all("Software\\CosmoArtsStore\\Polaris")?; let (app, events) = native_import_app()?;
        import::launch_startup_archive_import(app.handle().clone(), app.state())?;
        settings::save_startup_import_preference(false)?; import::launch_startup_archive_import(app.handle().clone(), app.state())?;
        assert!(events.receiver.try_recv().is_err()); assert!(!app.state::<crate::AnalyzeCancelStatus>().running.load(std::sync::atomic::Ordering::SeqCst));
        assert!(!get_db_path()?.exists()); settings::save_startup_import_preference(true)?;
        assert!(import::launch_startup_archive_import(app.handle().clone(), app.state()).is_err()); assert!(events.receiver.try_recv().is_err());
        assert!(!app.state::<crate::AnalyzeCancelStatus>().running.load(std::sync::atomic::Ordering::SeqCst)); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_import_startup_without_logs_only_emits_finished() -> NativeCommandTestResult {
    native_command_test("command_import_startup_without_logs_only_emits_finished", |_, _| {
        use tauri::Manager;
        settings::save_startup_import_preference(true)?; let (app, events) = native_import_app()?;
        import::launch_startup_archive_import(app.handle().clone(), app.state())?; assert!(events.finish(&app)?.is_empty());
        assert!(!get_db_path()?.exists()); assert!(!get_archive_store_dir()?.exists()); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_import_startup_syncs_source_then_preserves_success_on_partial_failure() -> NativeCommandTestResult {
    native_command_test("command_import_startup_syncs_source_then_preserves_success_on_partial_failure", |_, _| {
        use tauri::Manager;
        let source = get_source_log_dir()?; fs::create_dir_all(&source)?; let name = "output_log_2026-10-08_12-00-00.txt"; fs::write(source.join(name), import_log())?;
        settings::save_startup_import_preference(true)?; let (app, events) = native_import_app()?;
        import::launch_startup_archive_import(app.handle().clone(), app.state())?; let progress = events.finish(&app)?;
        let last = progress.last().ok_or("完了進捗がありません")?; assert_eq!(last.status, "Data 内 zst アーカイブからの取り込みが完了しました。");
        assert_eq!(last.progress, "100%"); assert!(!last.is_running); let store = get_archive_store_dir()?; assert!(store.join(format!("{name}.tar.zst")).exists());
        let conn = Connection::open(get_db_path()?)?; assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 1);
        let broken_name = "output_log_2026-10-08_13-00-00.txt"; fs::write(store.join(format!("{broken_name}.tar.zst")), b"not a zstd archive")?;
        import::launch_startup_archive_import(app.handle().clone(), app.state())?; let progress = events.finish(&app)?;
        let last = progress.last().ok_or("部分失敗の進捗がありません")?; assert_eq!(last.status, format!("1件完了、1件失敗（成功分は保存済み）: {broken_name}"));
        assert_eq!(last.progress, "1/2"); assert!(!last.is_running);
        assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 1);
        assert_eq!(conn.query_row("SELECT count(*) FROM visits", [], |row| row.get::<_, i64>(0))?, 1);
        assert_eq!(fs::read_to_string(source.join(name))?, import_log()); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_import_startup_sync_failure_emits_finished_and_preserves_source() -> NativeCommandTestResult {
    native_command_test("command_import_startup_sync_failure_emits_finished_and_preserves_source", |directory, _| {
        use tauri::Manager;
        let source = get_source_log_dir()?; fs::create_dir_all(&source)?; let name = "output_log_2026-10-08_12-00-00.txt"; fs::write(source.join(name), import_log())?;
        let blocked = directory.path().join("blocked-store"); fs::write(&blocked, b"keep")?; let mut setting = config::load_stellarecord_setting();
        setting.archive_path = blocked.to_string_lossy().into_owned(); config::save_stellarecord_setting(&setting)?; settings::save_startup_import_preference(true)?;
        let (app, events) = native_import_app()?; import::launch_startup_archive_import(app.handle().clone(), app.state())?; let progress = events.finish(&app)?;
        let last = progress.last().ok_or("同期失敗の進捗がありません")?; assert!(last.status.starts_with("起動時 Data 同期に失敗しました: "));
        assert_eq!(last.progress, "0%"); assert!(!last.is_running); assert!(!get_db_path()?.exists()); assert_eq!(fs::read(blocked)?, b"keep");
        assert_eq!(fs::read_to_string(source.join(name))?, import_log()); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_import_startup_database_failure_preserves_file_and_allows_retry() -> NativeCommandTestResult {
    native_command_test("command_import_startup_database_failure_preserves_file_and_allows_retry", |_, _| {
        use tauri::Manager;
        create_native_import_archive("output_log_2026-10-08_12-00-00.txt", import_log())?; let path = get_db_path()?; fs::write(&path, b"not a database")?;
        settings::save_startup_import_preference(true)?; let (app, events) = native_import_app()?;
        import::launch_startup_archive_import(app.handle().clone(), app.state())?; let progress = events.finish(&app)?;
        let last = progress.last().ok_or("DB失敗の進捗がありません")?; assert!(last.status.starts_with("エラー: ")); assert_eq!(last.progress, "0%"); assert!(!last.is_running);
        assert_eq!(fs::read(&path)?, b"not a database"); fs::remove_file(&path)?;
        import::launch_startup_archive_import(app.handle().clone(), app.state())?; let progress = events.finish(&app)?;
        assert_eq!(progress.last().ok_or("再試行の完了進捗がありません")?.status, "Data 内 zst アーカイブからの取り込みが完了しました。");
        let conn = Connection::open(path)?; assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 1);
        app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_import_enhanced_busy_cancel_rolls_back_then_restarts() -> NativeCommandTestResult {
    native_command_test("command_import_enhanced_busy_cancel_rolls_back_then_restarts", |_, _| {
        use tauri::Manager;
        let name = "output_log_2026-10-08_12-00-00.txt"; let first = create_native_import_archive(name, import_log())?;
        let second = create_native_import_archive("output_log_2026-10-08_13-00-00.txt", &import_log().replace("12:", "13:"))?;
        let conn = open_initialized_main_db()?; conn.execute("INSERT INTO apps (name, description, path) VALUES ('維持する登録', '', 'owned')", [])?;
        let (app, events) = native_import_app()?; let mut pause = pause_native_import_after_file(&app, name);
        import::launch_enhanced_import(app.handle().clone(), vec![first.clone(), second], app.state())?;
        pause.entered.recv_timeout(std::time::Duration::from_secs(20))?;
        tauri::async_runtime::block_on(import::cancel_analyze(app.state()))?;
        let error = import::launch_enhanced_import(app.handle().clone(), vec![first.clone()], app.state()).err().ok_or("同時実行が拒否されませんでした")?;
        assert_eq!(error, "取り込みはすでに実行中です。完了後に再実行してください。"); assert!(app.state::<crate::AnalyzeCancelStatus>().cancel.load(std::sync::atomic::Ordering::SeqCst));
        pause.release.take().ok_or("worker解除channelがありません")?.send(())?; let progress = events.finish(&app)?; drop(pause);
        let last = progress.last().ok_or("キャンセル進捗がありません")?; assert_eq!(last.status, "キャンセルしました"); assert_eq!(last.progress, "0%"); assert!(!last.is_running);
        assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 0);
        assert_eq!(conn.query_row("SELECT name FROM apps", [], |row| row.get::<_, String>(0))?, "維持する登録");
        import::launch_enhanced_import(app.handle().clone(), vec![first], app.state())?; let progress = events.finish(&app)?;
        assert_eq!(progress.last().ok_or("再開の完了進捗がありません")?.status, "1件のインポートが完了しました。");
        assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 1);
        assert!(!app.state::<crate::AnalyzeCancelStatus>().cancel.load(std::sync::atomic::Ordering::SeqCst)); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_import_startup_busy_cancel_rolls_back_and_retries_all_sources() -> NativeCommandTestResult {
    native_command_test("command_import_startup_busy_cancel_rolls_back_and_retries_all_sources", |_, _| {
        use tauri::Manager;
        let source = get_source_log_dir()?; fs::create_dir_all(&source)?; let name = "output_log_2026-10-08_12-00-00.txt"; fs::write(source.join(name), import_log())?;
        let second = "output_log_2026-10-08_13-00-00.txt"; fs::write(source.join(second), import_log().replace("12:", "13:"))?;
        settings::save_startup_import_preference(true)?; let (app, events) = native_import_app()?; let mut pause = pause_native_import_after_file(&app, name);
        import::launch_startup_archive_import(app.handle().clone(), app.state())?; pause.entered.recv_timeout(std::time::Duration::from_secs(20))?;
        tauri::async_runtime::block_on(import::cancel_analyze(app.state()))?;
        assert_eq!(import::launch_startup_archive_import(app.handle().clone(), app.state()).err().ok_or("同時実行が拒否されませんでした")?, "取り込みはすでに実行中です。完了後に再実行してください。");
        assert!(app.state::<crate::AnalyzeCancelStatus>().cancel.load(std::sync::atomic::Ordering::SeqCst));
        pause.release.take().ok_or("worker解除channelがありません")?.send(())?; let progress = events.finish(&app)?; drop(pause);
        let last = progress.last().ok_or("キャンセル進捗がありません")?; assert_eq!(last.status, "キャンセルしました"); assert_eq!(last.progress, "0%"); assert!(!last.is_running);
        let conn = Connection::open(get_db_path()?)?; assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 0);
        assert_eq!(fs::read_to_string(source.join(name))?, import_log()); assert_eq!(fs::read_to_string(source.join(second))?, import_log().replace("12:", "13:"));
        import::launch_startup_archive_import(app.handle().clone(), app.state())?; let progress = events.finish(&app)?;
        assert_eq!(progress.last().ok_or("再開の完了進捗がありません")?.status, "Data 内 zst アーカイブからの取り込みが完了しました。");
        assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 2); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_import_enhanced_database_failure_preserves_bytes_and_can_retry() -> NativeCommandTestResult {
    native_command_test("command_import_enhanced_database_failure_preserves_bytes_and_can_retry", |_, _| {
        use tauri::Manager;
        let archive = create_native_import_archive("output_log_2026-10-08_12-00-00.txt", import_log())?; let path = get_db_path()?; fs::write(&path, b"not a database")?;
        let (app, events) = native_import_app()?; import::launch_enhanced_import(app.handle().clone(), vec![archive.clone()], app.state())?;
        let progress = events.finish(&app)?; let last = progress.last().ok_or("DB失敗の進捗がありません")?;
        assert!(last.status.starts_with("エラー: ")); assert_eq!(last.progress, "0%"); assert!(!last.is_running); assert_eq!(fs::read(&path)?, b"not a database");
        fs::remove_file(&path)?; import::launch_enhanced_import(app.handle().clone(), vec![archive], app.state())?; let progress = events.finish(&app)?;
        assert_eq!(progress.last().ok_or("再開の完了進捗がありません")?.status, "1件のインポートが完了しました。");
        let conn = Connection::open(path)?; assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 1); app.cleanup_before_exit(); Ok(())
    })
}

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
struct NativeBusySession { id: i64, log_name: String, account_id: Option<String>, account_name: Option<String>, start_time: String, end_time: String }
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
struct NativeBusyVisit { id: i64, session_id: i64, world_name: String, instance_id: String, instance_type: Option<String>, region: Option<String>, join_time: String, leave_time: Option<String> }
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
struct NativeBusyImportedLog { log_name: String, content_sha256: Vec<u8>, content_size: i64, parser_version: i64 }
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
struct NativeBusyDatabaseRows {
    sessions: Vec<NativeBusySession>, visits: Vec<NativeBusyVisit>, imported_logs: Vec<NativeBusyImportedLog>,
    apps: Vec<(i64, String, String, String)>, sequences: Vec<(String, i64)>, empty_tables: Vec<(String, i64)>, integrity: String, foreign_key_violations: usize,
}
#[derive(serde::Serialize)]
struct NativeBusyFile { kind: String, name: String, sha256: String, #[serde(skip)] bytes: Vec<u8> }
#[derive(serde::Serialize)]
struct NativeBusyPreview { table: String, total_rows: u32, columns: Vec<String>, rows: Vec<Vec<String>>, storage: String }
#[derive(serde::Serialize)]
struct NativeBusyStage {
    case: String, root: String, registry_key: String, stage: String, held_lock: Option<(String, i32)>, running: bool,
    progress: Vec<(String, String, bool)>, settings: String, database_sha256: String, wal_sha256: Option<String>,
    rows: NativeBusyDatabaseRows, files: Vec<NativeBusyFile>, preview_tables: Vec<String>, previews: Vec<NativeBusyPreview>,
    #[serde(skip)] database_bytes: Vec<u8>, #[serde(skip)] wal_bytes: Option<Vec<u8>>,
}

fn native_busy_sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}

/// 本体の解析結果を期待値に再利用せず、保存された全row・参照・採番を別connectionで読む。
fn native_busy_database_rows() -> Result<NativeBusyDatabaseRows, Box<dyn std::error::Error>> {
    let conn = Connection::open(get_db_path()?)?;
    let sessions = conn.prepare("SELECT id, log_name, account_id, account_name, start_time, end_time FROM sessions ORDER BY log_name")?
        .query_map([], |row| Ok(NativeBusySession { id: row.get(0)?, log_name: row.get(1)?, account_id: row.get(2)?, account_name: row.get(3)?, start_time: row.get(4)?, end_time: row.get(5)? }))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let visits = conn.prepare("SELECT id, session_id, world_name, instance_id, instance_type, region, join_time, leave_time FROM visits ORDER BY id")?
        .query_map([], |row| Ok(NativeBusyVisit { id: row.get(0)?, session_id: row.get(1)?, world_name: row.get(2)?, instance_id: row.get(3)?, instance_type: row.get(4)?, region: row.get(5)?, join_time: row.get(6)?, leave_time: row.get(7)? }))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let imported_logs = conn.prepare("SELECT log_name, content_sha256, content_size, parser_version FROM imported_logs ORDER BY log_name")?
        .query_map([], |row| Ok(NativeBusyImportedLog { log_name: row.get(0)?, content_sha256: row.get(1)?, content_size: row.get(2)?, parser_version: row.get(3)? }))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let apps = conn.prepare("SELECT id, name, description, path FROM apps ORDER BY id")?.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let sequences = conn.prepare("SELECT name, seq FROM sqlite_sequence ORDER BY name")?.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut empty_tables = Vec::new();
    for table in ["find_users", "with_users", "notifications", "screenshots", "osc", "subscription"] {
        empty_tables.push((table.to_string(), conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0))?));
    }
    let integrity = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    let foreign_key_violations = conn.prepare("PRAGMA foreign_key_check")?.query_map([], |_| Ok(()))?.collect::<rusqlite::Result<Vec<_>>>()?.len();
    Ok(NativeBusyDatabaseRows { sessions, visits, imported_logs, apps, sequences, empty_tables, integrity, foreign_key_violations })
}

/// writerが生存する段階でfileを先に保全し、queryやassertが失敗しても入力を残す。
fn native_busy_stage(case: &str, stage: &str, directory: &tempfile::TempDir, registry: &NativeRegistryFixture, app: &tauri::App, progress: &[NativeProgress], held_lock: Option<(&str, i32)>) -> Result<NativeBusyStage, Box<dyn std::error::Error>> {
    use tauri::Manager;
    let database = get_db_path()?; let database_bytes = fs::read(&database)?;
    let wal = database.with_extension("sqlite-wal"); let wal_bytes = if wal.is_file() { Some(fs::read(&wal)?) } else { None };
    let mut files = Vec::new();
    for (kind, path) in [("source", get_source_log_dir()?), ("archive", get_archive_store_dir()?)] {
        for entry in fs::read_dir(path)? {
            let entry = entry?; assert!(entry.file_type()?.is_file()); let bytes = fs::read(entry.path())?;
            files.push(NativeBusyFile { kind: kind.to_string(), name: entry.file_name().to_string_lossy().into_owned(), sha256: native_busy_sha256(&bytes), bytes });
        }
    }
    files.sort_by(|left, right| (&left.kind, &left.name).cmp(&(&right.kind, &right.name)));
    let preference = config::load_startup_import_preference();
    let settings = native_viewer_json((config::load_stellarecord_setting(), config::load_polaris_setting(), (preference.enabled, preference.preference_set)))?;
    let progress: Vec<_> = progress.iter().map(|item| (item.status.clone(), item.progress.clone(), item.is_running)).collect();
    let running = app.state::<crate::AnalyzeCancelStatus>().running.load(std::sync::atomic::Ordering::SeqCst);
    let output = if let Ok(base) = std::env::var("STELLA_STARTUP_TEST_EVIDENCE_DIR") {
        let output = std::path::Path::new(&base).join(format!("stella-sqlite-busy-{case}-{stage}")); fs::create_dir(&output)?;
        fs::write(output.join("database.sqlite"), &database_bytes)?;
        if let Some(bytes) = &wal_bytes { fs::write(output.join("database.sqlite-wal"), bytes)?; }
        for item in &files { fs::write(output.join(format!("{}-{}", item.kind, item.name)), &item.bytes)?; }
        fs::write(output.join("input.json"), native_viewer_json((case, stage, directory.path().display().to_string(), &registry.path, held_lock, &progress, &settings, running))?)?;
        Some(output)
    } else { None };
    let observed = (|| -> Result<NativeBusyStage, Box<dyn std::error::Error>> {
        let rows = native_busy_database_rows()?;
        let preview_tables = database::get_db_tables()?.into_iter().map(|table| table.name).collect();
        let mut previews = Vec::new();
        for table in ["apps", "sessions", "visits"] {
            let sort = if table == "apps" { "name" } else { "id" };
            let preview = database::get_db_table_data(table, Some(0), Some(sort.to_string()), Some("asc".to_string()))?;
            previews.push(NativeBusyPreview { table: table.to_string(), total_rows: preview.total_rows, columns: preview.columns.into_iter().map(|column| column.name).collect(), rows: preview.rows, storage: preview.storage });
        }
        Ok(NativeBusyStage { case: case.to_string(), root: directory.path().display().to_string(), registry_key: registry.path.clone(), stage: stage.to_string(), held_lock: held_lock.map(|(kind, code)| (kind.to_string(), code)), running, progress, settings,
            database_sha256: native_busy_sha256(&database_bytes), wal_sha256: wal_bytes.as_deref().map(native_busy_sha256), rows, files, preview_tables, previews, database_bytes, wal_bytes })
    })();
    if let Some(output) = output {
        match &observed {
            Ok(snapshot) => fs::write(output.join("observation.json"), native_viewer_json(snapshot)?)?,
            Err(error) => fs::write(output.join("failure.txt"), error.to_string())?,
        }
    }
    observed
}

fn native_busy_expected_session(id: i64, name: &str, start: &str, end: &str) -> NativeBusySession {
    NativeBusySession { id, log_name: name.to_string(), account_id: Some("usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()), account_name: Some("Owner".to_string()), start_time: start.to_string(), end_time: end.to_string() }
}
fn native_busy_expected_visit(id: i64, session_id: i64, world: &str, instance: &str, region: &str, join: &str, leave: &str) -> NativeBusyVisit {
    NativeBusyVisit { id, session_id, world_name: world.to_string(), instance_id: instance.to_string(), instance_type: Some("public".to_string()), region: Some(region.to_string()), join_time: join.to_string(), leave_time: Some(leave.to_string()) }
}
fn native_busy_expected_import(name: &str, contents: &str) -> Result<NativeBusyImportedLog, Box<dyn std::error::Error>> {
    use sha2::Digest;
    Ok(NativeBusyImportedLog { log_name: name.to_string(), content_sha256: sha2::Sha256::digest(contents.as_bytes()).to_vec(), content_size: i64::try_from(contents.len())?, parser_version: 2 })
}
fn native_busy_assert_integrity(rows: &NativeBusyDatabaseRows) {
    assert_eq!(rows.apps, [(1, "保持するランチャー登録".to_string(), "保持する説明".to_string(), "owned-preserved.exe".to_string())]);
    assert_eq!(rows.integrity, "ok"); assert_eq!(rows.foreign_key_violations, 0);
    assert_eq!(rows.empty_tables, ["find_users", "with_users", "notifications", "screenshots", "osc", "subscription"].map(|name| (name.to_string(), 0)));
}
fn native_busy_assert_previews(snapshot: &NativeBusyStage) -> NativeCommandTestResult {
    for table in ["apps", "sessions", "visits"] { assert!(snapshot.preview_tables.iter().any(|name| name == table)); }
    for preview in &snapshot.previews {
        assert_eq!(preview.storage, "Main DB");
        let expected_count = match preview.table.as_str() { "apps" => snapshot.rows.apps.len(), "sessions" => snapshot.rows.sessions.len(), "visits" => snapshot.rows.visits.len(), _ => return Err("未登録のpreview診断です".into()) };
        assert_eq!(usize::try_from(preview.total_rows)?, expected_count); assert_eq!(preview.rows.len(), expected_count);
        let id = preview.columns.iter().position(|name| name == "id").ok_or("previewにid列がありません")?;
        if preview.table == "visits" {
            let owner = preview.columns.iter().position(|name| name == "session_id").ok_or("previewにsession_id列がありません")?;
            let world = preview.columns.iter().position(|name| name == "world_name").ok_or("previewにworld_name列がありません")?;
            for (actual, expected) in preview.rows.iter().zip(&snapshot.rows.visits) {
                assert_eq!(actual[id], expected.id.to_string()); assert_eq!(actual[owner], expected.session_id.to_string()); assert_eq!(actual[world], expected.world_name);
                assert_ne!(actual[world], "未確定のwriter更新");
            }
        }
    }
    Ok(())
}

/// 既存schemaと解析rowをWALへ確定し、保持するconnectionを閉じずに実writer競合へ進む。
fn native_busy_seed_database(name: &str, contents: &str) -> Result<(Connection, String), Box<dyn std::error::Error>> {
    let archive_name = create_native_import_archive(name, contents)?; let database = get_db_path()?;
    let conn = Connection::open(&database)?; analyze::init_main_db(&conn)?;
    conn.execute_batch("PRAGMA wal_autocheckpoint = 0;")?; let main_before = fs::read(&database)?;
    conn.execute("INSERT INTO apps (name, description, path) VALUES ('保持するランチャー登録', '保持する説明', 'owned-preserved.exe')", [])?;
    analyze::run_enhanced_import_batch(&database, &[get_archive_store_dir()?.join(&archive_name)], &std::sync::atomic::AtomicBool::new(false), |_, _| {})?;
    assert_eq!(fs::read(&database)?, main_before); assert!(fs::read(database.with_extension("sqlite-wal"))?.len() > 32);
    fs::write(get_source_log_dir()?.join("unrelated.txt"), b"unrelated source bytes")?;
    fs::write(get_archive_store_dir()?.join("retained.bin"), b"unrelated archive bytes")?;
    Ok((conn, archive_name))
}

fn native_busy_writer_code() -> Result<i32, Box<dyn std::error::Error>> {
    let probe = Connection::open(get_db_path()?)?; probe.busy_timeout(std::time::Duration::ZERO)?;
    let error = probe.execute_batch("BEGIN IMMEDIATE;").err().ok_or("別writerのBEGIN IMMEDIATEが拒否されませんでした")?;
    if let rusqlite::Error::SqliteFailure(code, _) = error { assert_eq!(code.code, rusqlite::ErrorCode::DatabaseBusy); Ok(code.extended_code) }
    else { Err(format!("SQLite busy以外のfailureです: {error}").into()) }
}

fn native_busy_assert_archive(path: &std::path::Path, name: &str, contents: &str) -> NativeCommandTestResult {
    use std::io::Read;
    let tar_bytes = zstd::stream::decode_all(fs::File::open(path)?)?; let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes)); let mut entries = archive.entries()?;
    let mut entry = entries.next().ok_or("圧縮archiveに本文がありません")??;
    assert_eq!(entry.path()?.to_str(), Some(name)); assert_eq!(entry.header().size()?, u64::try_from(contents.len())?);
    let mut body = Vec::new(); entry.read_to_end(&mut body)?; assert_eq!(body, contents.as_bytes()); drop(entry); assert!(entries.next().is_none()); Ok(())
}

fn native_busy_assert_file_inventory(snapshot: &NativeBusyStage, first: &str, second: &str) {
    let mut sources = vec![first.to_string(), second.to_string(), "unrelated.txt".to_string()]; sources.sort();
    let mut archives = vec![format!("{first}.tar.zst"), format!("{second}.tar.zst"), "retained.bin".to_string()]; archives.sort();
    assert_eq!(snapshot.files.iter().filter(|item| item.kind == "source").map(|item| item.name.clone()).collect::<Vec<_>>(), sources);
    assert_eq!(snapshot.files.iter().filter(|item| item.kind == "archive").map(|item| item.name.clone()).collect::<Vec<_>>(), archives);
}

/// 同じlive WAL fixtureでmanual/startupそれぞれの初期化失敗と再実行を観測する。
fn native_busy_import_boundary(startup: bool, directory: &tempfile::TempDir, registry: &NativeRegistryFixture) -> NativeCommandTestResult {
    use tauri::Manager;
    let case = if startup { "startup" } else { "enhanced" }; let old_name = "output_log_2026-10-08_12-00-00.txt"; let pending_name = "output_log_2026-10-08_13-00-00.txt";
    let pending_contents = import_log().replace("12:", "13:").replace("Owned World", "後続の確定ワールド");
    let (keeper, old_archive) = native_busy_seed_database(old_name, import_log())?; let store = get_archive_store_dir()?; let source = get_source_log_dir()?;
    let pending_archive = format!("{pending_name}.tar.zst"); fs::write(source.join(pending_name), &pending_contents)?;
    if startup { settings::save_startup_import_preference(true)?; } else { assert_eq!(archive::sync_source_logs_into_archive_store(&source, &store)?, 1); }
    let (app, events) = native_import_app()?; let original = native_busy_stage(case, "01-before-lock", directory, registry, &app, &[], None)?;
    native_busy_assert_integrity(&original.rows); native_busy_assert_previews(&original)?;
    assert_eq!(original.rows.sessions, [native_busy_expected_session(1, old_name, "2026-10-08 12:00:00", "2026-10-08 12:05:00")]);
    assert_eq!(original.rows.visits, [native_busy_expected_visit(1, 1, "Owned World", "123", "jp", "2026-10-08 12:01:01", "2026-10-08 12:05:00")]);
    assert_eq!(original.rows.imported_logs, [native_busy_expected_import(old_name, import_log())?]);
    assert_eq!(original.rows.sequences, [("apps".to_string(), 1), ("sessions".to_string(), 1), ("visits".to_string(), 1)]);
    keeper.execute_batch("BEGIN IMMEDIATE; UPDATE visits SET world_name = '未確定のwriter更新' WHERE id = 1;")?;
    let busy_code = native_busy_writer_code()?; assert_eq!(busy_code, 5);
    let held = native_busy_stage(case, "02-held-before", directory, registry, &app, &[], Some(("sqlite-writer", busy_code)))?;
    assert_eq!(held.rows, original.rows); native_busy_assert_previews(&held)?;
    if startup { import::launch_startup_archive_import(app.handle().clone(), app.state())?; }
    else { import::launch_enhanced_import(app.handle().clone(), vec![pending_archive.clone()], app.state())?; }
    let failed_progress = events.finish(&app)?; assert!(!keeper.is_autocommit()); let still_busy = native_busy_writer_code()?;
    let failed = native_busy_stage(case, "03-failed-held", directory, registry, &app, &failed_progress, Some(("sqlite-writer", still_busy)))?;
    let terminal = failed_progress.last().ok_or("SQLite busyの終端進捗がありません")?;
    assert!(terminal.status.starts_with("エラー: メイン DB を初期化できませんでした: "), "{}", terminal.status); assert!(terminal.status.contains("database is locked"), "{}", terminal.status);
    assert_eq!(terminal.progress, "0%"); assert!(!terminal.is_running); assert!(!failed.running); assert_eq!(still_busy, 5);
    assert_eq!(keeper.query_row("SELECT world_name FROM visits WHERE id = 1", [], |row| row.get::<_, String>(0))?, "未確定のwriter更新");
    assert_eq!(failed.database_bytes, held.database_bytes); assert_eq!(failed.wal_bytes, held.wal_bytes); assert_eq!(failed.rows, original.rows); assert_eq!(failed.settings, original.settings); native_busy_assert_previews(&failed)?;
    assert_eq!(fs::read_to_string(source.join(old_name))?, import_log()); assert_eq!(fs::read_to_string(source.join(pending_name))?, pending_contents);
    assert_eq!(fs::read(source.join("unrelated.txt"))?, b"unrelated source bytes"); assert_eq!(fs::read(store.join("retained.bin"))?, b"unrelated archive bytes");
    native_busy_assert_archive(&store.join(&old_archive), old_name, import_log())?; native_busy_assert_archive(&store.join(&pending_archive), pending_name, &pending_contents)?;
    native_busy_assert_file_inventory(&failed, old_name, pending_name);
    if startup { assert!(!original.files.iter().any(|item| item.kind == "archive" && item.name == pending_archive)); }
    else { assert_eq!(failed.files.iter().map(|item| (&item.kind, &item.name, &item.sha256)).collect::<Vec<_>>(), original.files.iter().map(|item| (&item.kind, &item.name, &item.sha256)).collect::<Vec<_>>()); }
    keeper.execute_batch("ROLLBACK;")?; assert!(keeper.is_autocommit());
    let unlocked = native_busy_stage(case, "04-unlocked", directory, registry, &app, &[], None)?; assert_eq!(unlocked.rows, original.rows);
    if startup { import::launch_startup_archive_import(app.handle().clone(), app.state())?; }
    else { import::launch_enhanced_import(app.handle().clone(), vec![pending_archive], app.state())?; }
    let retried_progress = events.finish(&app)?; let retried = native_busy_stage(case, "05-retried", directory, registry, &app, &retried_progress, None)?;
    let terminal = retried_progress.last().ok_or("同じDB pathの再試行進捗がありません")?;
    assert_eq!(terminal.status, if startup { "Data 内 zst アーカイブからの取り込みが完了しました。" } else { "1件のインポートが完了しました。" }); assert_eq!(terminal.progress, "100%"); assert!(!terminal.is_running); assert!(!retried.running);
    assert_eq!(retried.rows.sessions, [native_busy_expected_session(1, old_name, "2026-10-08 12:00:00", "2026-10-08 12:05:00"), native_busy_expected_session(2, pending_name, "2026-10-08 13:00:00", "2026-10-08 13:05:00")]);
    assert_eq!(retried.rows.visits, [native_busy_expected_visit(1, 1, "Owned World", "123", "jp", "2026-10-08 12:01:01", "2026-10-08 12:05:00"), native_busy_expected_visit(2, 2, "後続の確定ワールド", "123", "jp", "2026-10-08 13:01:01", "2026-10-08 13:05:00")]);
    assert_eq!(retried.rows.imported_logs, [native_busy_expected_import(old_name, import_log())?, native_busy_expected_import(pending_name, &pending_contents)?]);
    assert_eq!(retried.rows.sequences, [("apps".to_string(), 1), ("sessions".to_string(), 2), ("visits".to_string(), 2)]);
    native_busy_assert_integrity(&retried.rows); native_busy_assert_previews(&retried)?; assert_eq!(retried.settings, original.settings);
    native_busy_assert_file_inventory(&retried, old_name, pending_name);
    assert_eq!(retried.files.iter().map(|item| (&item.kind, &item.name, &item.sha256)).collect::<Vec<_>>(), failed.files.iter().map(|item| (&item.kind, &item.name, &item.sha256)).collect::<Vec<_>>());
    app.cleanup_before_exit(); drop(keeper); Ok(())
}

#[test]
fn command_import_enhanced_sqlite_busy_preserves_wal_and_retries_same_path() -> NativeCommandTestResult {
    native_command_test("command_import_enhanced_sqlite_busy_preserves_wal_and_retries_same_path", |directory, registry| native_busy_import_boundary(false, directory, registry))
}
#[test]
fn command_import_startup_sqlite_busy_keeps_archive_and_retries_same_path() -> NativeCommandTestResult {
    native_command_test("command_import_startup_sqlite_busy_keeps_archive_and_retries_same_path", |directory, registry| native_busy_import_boundary(true, directory, registry))
}

#[test]
fn command_import_startup_archive_lock_preserves_partial_sync_and_retries_same_paths() -> NativeCommandTestResult {
    native_command_test("command_import_startup_archive_lock_preserves_partial_sync_and_retries_same_paths", |directory, registry| {
        use std::os::windows::fs::OpenOptionsExt;
        use tauri::Manager;
        use windows::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
        let case = "partial-archive"; let first_name = "output_log_2026-10-08_12-00-00.txt"; let second_name = "output_log_2026-10-08_13-00-00.txt";
        let second_old = import_log().replace("12:", "13:"); let (keeper, second_archive) = native_busy_seed_database(second_name, &second_old)?;
        let first_contents = import_log().replace("Owned World", "先行の確定ワールド"); let second_contents = format!("{second_old}2026.10.08 14:01:00 Log        -  [Behaviour] Entering Room: 追記の確定ワールド\n2026.10.08 14:01:01 Log        -  [Behaviour] Joining wrld_appended:456~public~region(us)\n2026.10.08 14:05:00 Log        -  [Behaviour] OnLeftRoom\n");
        let source = get_source_log_dir()?; let store = get_archive_store_dir()?; let second_path = store.join(&second_archive); let second_before = fs::read(&second_path)?;
        fs::write(source.join(first_name), &first_contents)?; fs::write(source.join(second_name), &second_contents)?; settings::save_startup_import_preference(true)?;
        let (app, events) = native_import_app()?; let original = native_busy_stage(case, "01-before-lock", directory, registry, &app, &[], None)?;
        native_busy_assert_integrity(&original.rows); assert_eq!(original.rows.sessions, [native_busy_expected_session(1, second_name, "2026-10-08 13:00:00", "2026-10-08 13:05:00")]);
        assert_eq!(original.rows.visits, [native_busy_expected_visit(1, 1, "Owned World", "123", "jp", "2026-10-08 13:01:01", "2026-10-08 13:05:00")]); assert_eq!(original.rows.imported_logs, [native_busy_expected_import(second_name, &second_old)?]);
        // plannerの読込を許可し、圧縮完了後の既存target renameだけを実共有違反にする。
        let locked = fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0).open(&second_path)?;
        import::launch_startup_archive_import(app.handle().clone(), app.state())?; let failed_progress = events.finish(&app)?;
        let lock_code = failed_progress.last().and_then(|item| item.status.rsplit_once("os error ")).and_then(|(_, tail)| tail.strip_suffix(')')).and_then(|value| value.parse::<i32>().ok()).unwrap_or_default();
        let failed = native_busy_stage(case, "02-failed-held", directory, registry, &app, &failed_progress, Some(("archive-no-delete", lock_code)))?;
        let terminal = failed_progress.last().ok_or("archive同期失敗の終端進捗がありません")?;
        assert!(terminal.status.starts_with("起動時 Data 同期に失敗しました: バックアップへ退避できませんでした ["), "{}", terminal.status); assert!(terminal.status.contains("os error 32"), "{}", terminal.status);
        assert_eq!(lock_code, 32); native_busy_assert_file_inventory(&failed, first_name, second_name);
        assert_eq!(terminal.progress, "0%"); assert!(!terminal.is_running); assert!(!failed.running);
        assert_eq!(failed.database_bytes, original.database_bytes); assert_eq!(failed.wal_bytes, original.wal_bytes); assert_eq!(failed.rows, original.rows); assert_eq!(failed.settings, original.settings);
        let first_archive = format!("{first_name}.tar.zst"); native_busy_assert_archive(&store.join(&first_archive), first_name, &first_contents)?;
        assert_eq!(fs::read(&second_path)?, second_before); native_busy_assert_archive(&second_path, second_name, &second_old)?;
        assert_eq!(fs::read_to_string(source.join(first_name))?, first_contents); assert_eq!(fs::read_to_string(source.join(second_name))?, second_contents);
        assert_eq!(fs::read(source.join("unrelated.txt"))?, b"unrelated source bytes"); assert_eq!(fs::read(store.join("retained.bin"))?, b"unrelated archive bytes");
        let mut names = fs::read_dir(&store)?.map(|entry| entry.map(|item| item.file_name().to_string_lossy().into_owned())).collect::<std::io::Result<Vec<_>>>()?; names.sort();
        let mut expected_names = vec![first_archive.clone(), second_archive.clone(), "retained.bin".to_string()]; expected_names.sort(); assert_eq!(names, expected_names);
        let first_before = fs::read(store.join(&first_archive))?; drop(locked);
        import::launch_startup_archive_import(app.handle().clone(), app.state())?; let retried_progress = events.finish(&app)?;
        let retried = native_busy_stage(case, "03-retried", directory, registry, &app, &retried_progress, None)?;
        let terminal = retried_progress.last().ok_or("同じarchive pathの再試行進捗がありません")?; assert_eq!(terminal.status, "Data 内 zst アーカイブからの取り込みが完了しました。"); assert_eq!(terminal.progress, "100%"); assert!(!terminal.is_running); assert!(!retried.running);
        native_busy_assert_archive(&store.join(&first_archive), first_name, &first_contents)?; native_busy_assert_archive(&second_path, second_name, &second_contents)?; assert_eq!(fs::read(store.join(&first_archive))?, first_before);
        assert_eq!(retried.rows.sessions, [native_busy_expected_session(2, first_name, "2026-10-08 12:00:00", "2026-10-08 12:05:00"), native_busy_expected_session(3, second_name, "2026-10-08 13:00:00", "2026-10-08 14:05:00")]);
        assert_eq!(retried.rows.visits, [native_busy_expected_visit(2, 2, "先行の確定ワールド", "123", "jp", "2026-10-08 12:01:01", "2026-10-08 12:05:00"), native_busy_expected_visit(3, 3, "Owned World", "123", "jp", "2026-10-08 13:01:01", "2026-10-08 13:05:00"), native_busy_expected_visit(4, 3, "追記の確定ワールド", "456", "us", "2026-10-08 14:01:01", "2026-10-08 14:05:00")]);
        assert_eq!(retried.rows.imported_logs, [native_busy_expected_import(first_name, &first_contents)?, native_busy_expected_import(second_name, &second_contents)?]); assert_eq!(retried.rows.sequences, [("apps".to_string(), 1), ("sessions".to_string(), 3), ("visits".to_string(), 4)]);
        native_busy_assert_integrity(&retried.rows); native_busy_assert_previews(&retried)?; assert_eq!(retried.settings, original.settings);
        native_busy_assert_file_inventory(&retried, first_name, second_name);
        assert_eq!(fs::read_to_string(source.join(first_name))?, first_contents); assert_eq!(fs::read_to_string(source.join(second_name))?, second_contents); assert_eq!(fs::read(source.join("unrelated.txt"))?, b"unrelated source bytes"); assert_eq!(fs::read(store.join("retained.bin"))?, b"unrelated archive bytes");
        let mut names = fs::read_dir(&store)?.map(|entry| entry.map(|item| item.file_name().to_string_lossy().into_owned())).collect::<std::io::Result<Vec<_>>>()?; names.sort(); assert_eq!(names, expected_names);
        app.cleanup_before_exit(); drop(keeper); Ok(())
    })
}

include!("native_viewer_tests.rs");

#[test]
fn command_import_enhanced_worker_commits_real_archive_and_emits_finished() -> NativeCommandTestResult {
    native_command_test("command_import_enhanced_worker_commits_real_archive_and_emits_finished", |_, _| {
        use tauri::Manager;
        let name = "output_log_2026-10-08_12-00-00.txt"; let filename = create_native_import_archive(name, import_log())?; let source = get_source_log_dir()?;
        let (app, events) = native_import_app()?; let status = app.state::<crate::AnalyzeCancelStatus>(); status.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(import::launch_enhanced_import(app.handle().clone(), vec![filename.clone()], status)?, "1件のアーカイブ同期を開始しました。");
        let progress = events.finish(&app)?; assert!(progress.iter().any(|item| item.is_running)); let last = progress.last().ok_or("完了進捗がありません")?;
        assert_eq!(last.status, "1件のインポートが完了しました。"); assert_eq!(last.progress, "100%"); assert!(!last.is_running);
        let conn = Connection::open(get_db_path()?)?; assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 1);
        assert_eq!(conn.query_row("SELECT count(*) FROM visits", [], |row| row.get::<_, i64>(0))?, 1); assert_eq!(fs::read_to_string(source.join(name))?, import_log());
        import::launch_enhanced_import(app.handle().clone(), vec![filename], app.state())?; events.finish(&app)?;
        assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 1); app.cleanup_before_exit(); Ok(())
    })
}


const NATIVE_STATEMENT_FIRST: &str = "output_log_2026-10-08_12-00-00.txt";
const NATIVE_STATEMENT_AFFECTED: &str = "output_log_2026-10-08_13-00-00.txt";
const NATIVE_STATEMENT_MARKER: &str = "stella_native_statement_abort_owned";
const NATIVE_STATEMENT_BOUNDARY_MARKER: &str = "stella_native_statement_boundary_not_reached_owned";
const NATIVE_STATEMENT_FIRST_BODY: &str = concat!(
    "2026.10.08 12:00:00 Log        -  User Authenticated: Owner (usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee)\n",
    "2026.10.08 12:01:00 Log        -  [Behaviour] Entering Room: First Committed World\n",
    "2026.10.08 12:01:01 Log        -  [Behaviour] Joining wrld_first:111~public~region(jp)\n",
    "2026.10.08 12:05:00 Log        -  [Behaviour] OnLeftRoom\n",
);
const NATIVE_STATEMENT_OLD_BODY: &str = concat!(
    "2026.10.08 13:00:00 Log        -  User Authenticated: Owner (usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee)\n",
    "2026.10.08 13:00:01 Log        -  [VRC Camera] Took screenshot to: C:\\owned_fixture\\preserved_1920x1080.png\n",
    "2026.10.08 13:01:00 Log        -  [Behaviour] Entering Room: Old Affected World\n",
    "2026.10.08 13:01:01 Log        -  [Behaviour] Joining wrld_old:123~public~region(jp)\n",
    "2026.10.08 13:05:00 Log        -  [Behaviour] OnLeftRoom\n",
);
const NATIVE_STATEMENT_APPEND: &str = concat!(
    "2026.10.08 14:01:00 Log        -  [Behaviour] Entering Room: Native Statement Rejected World\n",
    "2026.10.08 14:01:01 Log        -  [Behaviour] Joining wrld_rejected:222~public~region(us)\n",
    "2026.10.08 14:05:00 Log        -  [Behaviour] OnLeftRoom\n",
);

// 全columnをliteralのSELECTと列集合で確認し、production preview/parserを期待値へ再利用しない。
const NATIVE_STATEMENT_TABLES: [(&str, &str, &[&str]); 10] = [
    ("apps", "SELECT id, name, description, path, icon, registered_at FROM apps ORDER BY id", &["id", "name", "description", "path", "icon", "registered_at"]),
    ("sessions", "SELECT id, log_name, account_id, account_name, start_time, end_time FROM sessions ORDER BY id", &["id", "log_name", "account_id", "account_name", "start_time", "end_time"]),
    ("imported_logs", "SELECT log_name, content_sha256, content_size, parser_version FROM imported_logs ORDER BY log_name", &["log_name", "content_sha256", "content_size", "parser_version"]),
    ("visits", "SELECT id, session_id, world_name, instance_id, instance_type, region, join_time, leave_time FROM visits ORDER BY id", &["id", "session_id", "world_name", "instance_id", "instance_type", "region", "join_time", "leave_time"]),
    ("find_users", "SELECT vrchat_id, account_name FROM find_users ORDER BY vrchat_id", &["vrchat_id", "account_name"]),
    ("with_users", "SELECT id, visit_id, vrchat_id, is_self, friend_status, join_time, leave_time FROM with_users ORDER BY id", &["id", "visit_id", "vrchat_id", "is_self", "friend_status", "join_time", "leave_time"]),
    ("notifications", "SELECT id, session_id, notif_id, notif_type, sender_user_id, sender_name, message, created_at, received_at, target_world_name, target_instance_id, target_instance_type, target_owner, target_region FROM notifications ORDER BY id", &["id", "session_id", "notif_id", "notif_type", "sender_user_id", "sender_name", "message", "created_at", "received_at", "target_world_name", "target_instance_id", "target_instance_type", "target_owner", "target_region"]),
    ("screenshots", "SELECT id, visit_id, file_path, resolution_width, resolution_height, timestamp, session_id FROM screenshots ORDER BY id", &["id", "visit_id", "file_path", "resolution_width", "resolution_height", "timestamp", "session_id"]),
    ("osc", "SELECT id, session_id, event_type, service_name, service_type, ip_address, port, timestamp FROM osc ORDER BY id", &["id", "session_id", "event_type", "service_name", "service_type", "ip_address", "port", "timestamp"]),
    ("subscription", "SELECT id, session_id, is_active, subscription_id, description, checked_at FROM subscription ORDER BY id", &["id", "session_id", "is_active", "subscription_id", "description", "checked_at"]),
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
enum NativeStatementCell { Null, Integer(i64), RealBits(String), Text(String), Blob(Vec<u8>) }
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
struct NativeStatementTable { name: String, columns: Vec<String>, rows: Vec<Vec<NativeStatementCell>> }
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
struct NativeStatementDatabase { tables: Vec<NativeStatementTable>, sequences: Vec<(String, i64)>, integrity: Vec<String>, foreign_key_violations: Vec<(String, i64, String, i64)> }
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
struct NativeStatementFile { kind: String, name: String, sha256: String, #[serde(skip)] bytes: Vec<u8> }
// trigger名、対象table、保存SQLの順を保つ。
type NativeStatementTriggers = Vec<(String, String, String)>;
#[derive(serde::Serialize)]
struct NativeStatementStage {
    case: String, stage: String, root: String, registry_key: String, database_path: String, source_path: String, archive_path: String,
    running: bool, cancel: bool, progress: Vec<(String, String, bool)>, settings: String,
    rows: NativeStatementDatabase, triggers: NativeStatementTriggers, files: Vec<NativeStatementFile>, logs: Vec<NativeStatementFile>,
    database_sha256: String, wal_sha256: Option<String>,
}
#[derive(Clone, Copy)]
enum NativeStatementExpected { Original, Partial, Rebuilt }

fn native_statement_database() -> Result<(NativeStatementDatabase, NativeStatementTriggers), Box<dyn std::error::Error>> {
    let conn = Connection::open_with_flags(get_db_path()?, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut tables = Vec::new();
    for (name, sql, columns) in NATIVE_STATEMENT_TABLES {
        let actual_columns = conn.prepare(&format!("PRAGMA table_info({name})"))?.query_map([], |row| row.get::<_, String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let expected_columns = columns.iter().map(|value| (*value).to_string()).collect::<Vec<_>>();
        assert_eq!(actual_columns, expected_columns);
        let mut statement = conn.prepare(sql)?; assert_eq!(statement.column_names().as_slice(), columns);
        let rows = statement.query_map([], |row| {
            (0..columns.len()).map(|index| {
                Ok(match row.get::<_, rusqlite::types::Value>(index)? {
                    rusqlite::types::Value::Null => NativeStatementCell::Null,
                    rusqlite::types::Value::Integer(value) => NativeStatementCell::Integer(value),
                    rusqlite::types::Value::Real(value) => NativeStatementCell::RealBits(value.to_bits().to_string()),
                    rusqlite::types::Value::Text(value) => NativeStatementCell::Text(value),
                    rusqlite::types::Value::Blob(value) => NativeStatementCell::Blob(value),
                })
            }).collect::<rusqlite::Result<Vec<_>>>()
        })?.collect::<rusqlite::Result<Vec<_>>>()?;
        tables.push(NativeStatementTable { name: name.to_string(), columns: expected_columns, rows });
    }
    let sequences = conn.prepare("SELECT name, seq FROM sqlite_sequence ORDER BY name")?.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let integrity = conn.prepare("PRAGMA integrity_check")?.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let foreign_key_violations = conn.prepare("PRAGMA foreign_key_check")?.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let triggers = conn.prepare("SELECT name, tbl_name, sql FROM main.sqlite_schema WHERE type = 'trigger' ORDER BY name")?.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok((NativeStatementDatabase { tables, sequences, integrity, foreign_key_violations }, triggers))
}

/// IDと全値は既知fixtureから期待し、失敗中の再採番をold stateへ補正しない。
fn native_statement_expected(expected: NativeStatementExpected) -> Result<NativeStatementDatabase, Box<dyn std::error::Error>> {
    use sha2::Digest;
    use NativeStatementCell::{Blob as B, Integer as I, Null as N, Text as T};
    let rebuilt = matches!(expected, NativeStatementExpected::Rebuilt);
    let first_saved = !matches!(expected, NativeStatementExpected::Original);
    let affected_id = if rebuilt { 3 } else { 1 }; let screenshot_id = if rebuilt { 2 } else { 1 };
    let changed = format!("{NATIVE_STATEMENT_OLD_BODY}{NATIVE_STATEMENT_APPEND}");
    let mut tables = Vec::new();
    for (name, _, columns) in NATIVE_STATEMENT_TABLES {
        let rows = match name {
            "apps" => vec![vec![I(1), T("保持するランチャー登録".to_string()), T("保持する説明".to_string()), T("owned-preserved.exe".to_string()), B(vec![1, 2, 3]), T("2026-10-08 00:00:00".to_string())]],
            "sessions" => {
                let old = vec![I(affected_id), T(NATIVE_STATEMENT_AFFECTED.to_string()), T("usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()), T("Owner".to_string()), T("2026-10-08 13:00:00".to_string()), T(if rebuilt { "2026-10-08 14:05:00" } else { "2026-10-08 13:05:00" }.to_string())];
                let first = vec![I(2), T(NATIVE_STATEMENT_FIRST.to_string()), T("usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()), T("Owner".to_string()), T("2026-10-08 12:00:00".to_string()), T("2026-10-08 12:05:00".to_string())];
                if rebuilt { vec![first, old] } else if first_saved { vec![old, first] } else { vec![old] }
            },
            "imported_logs" => {
                let inputs = if first_saved { vec![(NATIVE_STATEMENT_FIRST, NATIVE_STATEMENT_FIRST_BODY), (NATIVE_STATEMENT_AFFECTED, if rebuilt { changed.as_str() } else { NATIVE_STATEMENT_OLD_BODY })] }
                    else { vec![(NATIVE_STATEMENT_AFFECTED, NATIVE_STATEMENT_OLD_BODY)] };
                inputs.into_iter().map(|(filename, body)| Ok(vec![T(filename.to_string()), B(sha2::Sha256::digest(body.as_bytes()).to_vec()), I(i64::try_from(body.len())?), I(2)])).collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?
            },
            "visits" => {
                let old = vec![I(if rebuilt { 3 } else { 1 }), I(affected_id), T("Old Affected World".to_string()), T("123".to_string()), T("public".to_string()), T("jp".to_string()), T("2026-10-08 13:01:01".to_string()), T("2026-10-08 13:05:00".to_string())];
                let first = vec![I(2), I(2), T("First Committed World".to_string()), T("111".to_string()), T("public".to_string()), T("jp".to_string()), T("2026-10-08 12:01:01".to_string()), T("2026-10-08 12:05:00".to_string())];
                if rebuilt { vec![first, old, vec![I(4), I(3), T("Native Statement Rejected World".to_string()), T("222".to_string()), T("public".to_string()), T("us".to_string()), T("2026-10-08 14:01:01".to_string()), T("2026-10-08 14:05:00".to_string())]] }
                else if first_saved { vec![old, first] } else { vec![old] }
            },
            "screenshots" => vec![vec![I(screenshot_id), N, T(r"C:\owned_fixture\preserved_1920x1080.png".to_string()), I(1920), I(1080), T("2026-10-08 13:00:01".to_string()), I(affected_id)]],
            "find_users" | "with_users" | "notifications" | "osc" | "subscription" => Vec::new(),
            _ => return Err("statement fixtureの未定義tableです".into()),
        };
        tables.push(NativeStatementTable { name: name.to_string(), columns: columns.iter().map(|value| (*value).to_string()).collect(), rows });
    }
    let sequences = [("apps", 1), ("screenshots", screenshot_id), ("sessions", if rebuilt { 3 } else if first_saved { 2 } else { 1 }), ("visits", if rebuilt { 4 } else if first_saved { 2 } else { 1 })].map(|(name, value)| (name.to_string(), value)).to_vec();
    Ok(NativeStatementDatabase { tables, sequences, integrity: vec!["ok".to_string()], foreign_key_violations: Vec::new() })
}

/// 固有の `ABORT` は旧削除と transaction 内の先行書込みが成立した場合だけ返す。
/// 境界が成立しない場合は別markerで失敗し、faultが早い段階へ移ったことを見逃さない。
fn native_statement_trigger_sql() -> String {
    format!(r"CREATE TRIGGER stella_owned_statement_abort
BEFORE INSERT ON visits
WHEN NEW.world_name = 'Native Statement Rejected World'
 AND EXISTS (SELECT 1 FROM sessions WHERE id = NEW.session_id AND log_name = 'output_log_2026-10-08_13-00-00.txt')
BEGIN
 SELECT CASE WHEN
   NEW.session_id = 3 AND NEW.instance_id = '222' AND NEW.instance_type = 'public' AND NEW.region = 'us' AND NEW.leave_time IS NULL
   AND NEW.join_time = '2026-10-08 14:01:01'
   AND NOT EXISTS (SELECT 1 FROM sessions WHERE id = 1)
   AND NOT EXISTS (SELECT 1 FROM visits WHERE id = 1)
   AND NOT EXISTS (SELECT 1 FROM screenshots WHERE id = 1)
   AND NOT EXISTS (SELECT 1 FROM imported_logs WHERE log_name = 'output_log_2026-10-08_13-00-00.txt')
   AND EXISTS (SELECT 1 FROM sessions WHERE id = 3 AND log_name = 'output_log_2026-10-08_13-00-00.txt' AND start_time = '' AND end_time IS NULL)
   AND EXISTS (SELECT 1 FROM screenshots WHERE id = 2 AND session_id = 3 AND visit_id IS NULL
       AND file_path = 'C:\owned_fixture\preserved_1920x1080.png' AND resolution_width = 1920 AND resolution_height = 1080 AND timestamp = '2026-10-08 13:00:01')
   AND EXISTS (SELECT 1 FROM visits WHERE id = 3 AND session_id = 3 AND world_name = 'Old Affected World'
       AND instance_id = '123' AND instance_type = 'public' AND region = 'jp' AND join_time = '2026-10-08 13:01:01' AND leave_time = '2026-10-08 13:05:00')
   AND EXISTS (SELECT 1 FROM sessions WHERE id = 2 AND log_name = 'output_log_2026-10-08_12-00-00.txt'
       AND account_id = 'usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee' AND account_name = 'Owner' AND start_time = '2026-10-08 12:00:00' AND end_time = '2026-10-08 12:05:00')
   AND EXISTS (SELECT 1 FROM visits WHERE id = 2 AND session_id = 2 AND world_name = 'First Committed World'
       AND instance_id = '111' AND instance_type = 'public' AND region = 'jp' AND join_time = '2026-10-08 12:01:01' AND leave_time = '2026-10-08 12:05:00')
   AND EXISTS (SELECT 1 FROM imported_logs WHERE log_name = 'output_log_2026-10-08_12-00-00.txt'
       AND content_sha256 = X'{}' AND content_size = {} AND parser_version = 2)
 THEN RAISE(ABORT, 'stella_native_statement_abort_owned')
 ELSE RAISE(ABORT, 'stella_native_statement_boundary_not_reached_owned') END;
END", native_busy_sha256(NATIVE_STATEMENT_FIRST_BODY.as_bytes()), NATIVE_STATEMENT_FIRST_BODY.len())
}

/// query前にowned fileを保全し、SQL／assert失敗時にも入力と原因を残す。DB/WAL byteは観測だけ。
fn native_statement_stage(case: &str, stage: &str, directory: &tempfile::TempDir, registry: &NativeRegistryFixture, app: &tauri::App, progress: &[NativeProgress]) -> Result<NativeStatementStage, Box<dyn std::error::Error>> {
    use tauri::Manager;
    let database = get_db_path()?; let source = get_source_log_dir()?; let archive = get_archive_store_dir()?;
    let log_dir = utils::get_stellarecord_data_dir("logs").ok_or("owned log directoryがありません")?;
    for path in [&database, &source, &archive, &log_dir] { assert!(path.starts_with(directory.path())); }
    assert_eq!(utils::get_stellarecord_install_dir().as_deref(), Some(directory.path()));
    let database_bytes = fs::read(&database)?; let wal = database.with_extension("sqlite-wal");
    let wal_bytes = if wal.is_file() { Some(fs::read(&wal)?) } else { None }; let mut files = Vec::new(); let mut logs = Vec::new();
    for (kind, path) in [("source", &source), ("archive", &archive), ("log", &log_dir)] {
        for entry in fs::read_dir(path)? {
            let entry = entry?; assert!(entry.file_type()?.is_file()); let bytes = fs::read(entry.path())?;
            let file = NativeStatementFile { kind: kind.to_string(), name: entry.file_name().to_string_lossy().into_owned(), sha256: native_busy_sha256(&bytes), bytes };
            if kind == "log" { assert!(file.name.ends_with(".log")); logs.push(file); } else { files.push(file); }
        }
    }
    files.sort_by(|left, right| (&left.kind, &left.name).cmp(&(&right.kind, &right.name))); logs.sort_by(|left, right| left.name.cmp(&right.name));
    let preference = config::load_startup_import_preference();
    let settings = native_viewer_json((config::load_stellarecord_setting(), config::load_polaris_setting(), (preference.enabled, preference.preference_set)))?;
    let running = app.state::<crate::AnalyzeCancelStatus>().running.load(std::sync::atomic::Ordering::SeqCst);
    let cancel = app.state::<crate::AnalyzeCancelStatus>().cancel.load(std::sync::atomic::Ordering::SeqCst);
    let progress = progress.iter().map(|item| (item.status.clone(), item.progress.clone(), item.is_running)).collect::<Vec<_>>();
    let output = if let Some(base) = std::env::var_os("STELLA_STARTUP_TEST_EVIDENCE_DIR") {
        let output = std::path::PathBuf::from(base).join(format!("stella-statement-{case}-{stage}")); fs::create_dir(&output)?;
        fs::write(output.join("database.sqlite"), &database_bytes)?; if let Some(bytes) = &wal_bytes { fs::write(output.join("database.sqlite-wal"), bytes)?; }
        for item in files.iter().chain(&logs) { fs::write(output.join(format!("{}-{}", item.kind, item.name)), &item.bytes)?; }
        fs::write(output.join("input.json"), native_viewer_json((case, stage, directory.path().display().to_string(), &registry.path, &progress, &settings, running, cancel))?)?; Some(output)
    } else { None };
    let observed = (|| -> Result<NativeStatementStage, Box<dyn std::error::Error>> {
        let (rows, triggers) = native_statement_database()?;
        Ok(NativeStatementStage { case: case.to_string(), stage: stage.to_string(), root: directory.path().display().to_string(), registry_key: registry.path.clone(),
            database_path: database.display().to_string(), source_path: source.display().to_string(), archive_path: archive.display().to_string(),
            running, cancel, progress, settings, rows, triggers, files, logs, database_sha256: native_busy_sha256(&database_bytes), wal_sha256: wal_bytes.as_deref().map(native_busy_sha256) })
    })();
    if let Some(output) = output {
        match &observed {
            Ok(value) => fs::write(output.join("observation.json"), native_viewer_json(value)?)?,
            Err(error) => fs::write(output.join("failure.txt"), error.to_string())?,
        }
    }
    observed
}

fn native_statement_assert_inventory(stage: &NativeStatementStage, first_archive: bool) {
    let mut expected = vec![("source", NATIVE_STATEMENT_FIRST.to_string()), ("source", NATIVE_STATEMENT_AFFECTED.to_string()), ("source", "unrelated.txt".to_string()),
        ("archive", format!("{NATIVE_STATEMENT_AFFECTED}.tar.zst")), ("archive", "retained.bin".to_string())];
    if first_archive { expected.push(("archive", format!("{NATIVE_STATEMENT_FIRST}.tar.zst"))); }
    expected.sort();
    assert_eq!(stage.files.iter().map(|file| (file.kind.as_str(), file.name.clone())).collect::<Vec<_>>(), expected);
}

fn native_statement_assert_progress(progress: &[NativeProgress], startup: bool) -> NativeCommandTestResult {
    let first = progress.iter().position(|item| item.status == format!("取り込み完了: {NATIVE_STATEMENT_FIRST}") && item.is_running).ok_or("先行savepoint成功eventがありません")?;
    let affected = progress.iter().position(|item| item.status == format!("処理中: {NATIVE_STATEMENT_AFFECTED}") && item.is_running).ok_or("対象fileの処理開始eventがありません")?;
    let parse = progress.iter().enumerate().filter(|(_, item)| item.status == "パース開始" && item.is_running).map(|(index, _)| index).collect::<Vec<_>>();
    assert_eq!(parse.len(), 2); assert!(parse[0] < first && first < affected && affected < parse[1]);
    assert!(!progress.iter().any(|item| item.status == format!("取り込み完了: {NATIVE_STATEMENT_AFFECTED}") || item.progress == "100%" || item.status.contains(NATIVE_STATEMENT_BOUNDARY_MARKER)));
    assert_eq!(progress.iter().filter(|item| !item.is_running).count(), 1);
    let terminal = progress.last().ok_or("statement故障の終端eventがありません")?; assert!(!terminal.is_running);
    if startup { assert_eq!(terminal.status, format!("1件完了、1件失敗（成功分は保存済み）: {NATIVE_STATEMENT_AFFECTED}")); assert_eq!(terminal.progress, "1/2"); }
    else { assert!(terminal.status.starts_with("エラー: 対象ログを取り込めませんでした: ")); assert!(terminal.status.contains(NATIVE_STATEMENT_MARKER)); assert_eq!(terminal.progress, "0%"); }
    Ok(())
}

/// 月名を固定せず実log集合の追加行を照合し、fixture境界不成立のmarkerをSQL原因へ読み替えない。
fn native_statement_new_errors(before: &[NativeStatementFile], after: &[NativeStatementFile]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    for file in before { assert!(after.iter().any(|value| value.name == file.name && value.bytes.starts_with(&file.bytes))); }
    let mut errors = Vec::new();
    for file in after {
        let offset = before.iter().find(|value| value.name == file.name).map_or(0, |value| value.bytes.len());
        let added = std::str::from_utf8(&file.bytes[offset..])?; assert!(!added.contains(NATIVE_STATEMENT_BOUNDARY_MARKER));
        errors.extend(added.lines().filter(|line| line.contains("[ERROR]")).map(str::to_string));
    }
    Ok(errors)
}

fn native_statement_boundary(startup: bool, directory: &tempfile::TempDir, registry: &NativeRegistryFixture) -> NativeCommandTestResult {
    use tauri::Manager;
    let case = if startup { "startup" } else { "enhanced" };
    if startup { settings::save_startup_import_preference(true)?; }
    let affected_archive = create_native_import_archive(NATIVE_STATEMENT_AFFECTED, NATIVE_STATEMENT_OLD_BODY)?;
    let (app, events) = native_import_app()?; let initial = app.state::<crate::AnalyzeCancelStatus>();
    assert!(!initial.running.load(std::sync::atomic::Ordering::SeqCst)); assert!(!initial.cancel.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(import::launch_enhanced_import(app.handle().clone(), vec![affected_archive.clone()], initial)?, "1件のアーカイブ同期を開始しました。");
    let seeded = events.finish(&app)?; let seed_terminal = seeded.last().ok_or("旧stateのseed取込が終了していません")?;
    assert_eq!(seed_terminal.status, "1件のインポートが完了しました。"); assert_eq!(seed_terminal.progress, "100%"); assert!(!seed_terminal.is_running);
    let conn = Connection::open(get_db_path()?)?; assert!(conn.is_autocommit());
    conn.execute("INSERT INTO apps (name, description, path, icon, registered_at) VALUES ('保持するランチャー登録', '保持する説明', 'owned-preserved.exe', X'010203', '2026-10-08 00:00:00')", [])?; drop(conn);
    let original = native_statement_stage(case, "00-seed", directory, registry, &app, &seeded)?;
    assert_eq!(original.rows, native_statement_expected(NativeStatementExpected::Original)?); assert!(original.triggers.is_empty()); assert!(!original.running); assert!(!original.cancel);
    let source = get_source_log_dir()?; let store = get_archive_store_dir()?; let changed = format!("{NATIVE_STATEMENT_OLD_BODY}{NATIVE_STATEMENT_APPEND}");
    let old_archive_bytes = fs::read(store.join(&affected_archive))?;
    native_busy_assert_archive(&store.join(&affected_archive), NATIVE_STATEMENT_AFFECTED, NATIVE_STATEMENT_OLD_BODY)?;
    fs::write(source.join(NATIVE_STATEMENT_FIRST), NATIVE_STATEMENT_FIRST_BODY)?; fs::write(source.join(NATIVE_STATEMENT_AFFECTED), &changed)?;
    fs::write(source.join("unrelated.txt"), b"unrelated source bytes")?; fs::write(store.join("retained.bin"), b"unrelated archive bytes")?;
    if !startup { assert_eq!(archive::sync_source_logs_into_archive_store(&source, &store)?, 2); }
    let trigger_sql = native_statement_trigger_sql(); let conn = Connection::open(get_db_path()?)?; assert!(conn.is_autocommit());
    conn.execute_batch(&trigger_sql)?; assert!(conn.is_autocommit()); drop(conn);
    let before = native_statement_stage(case, "01-before-fault", directory, registry, &app, &[])?;
    assert_eq!(before.rows, original.rows); assert_eq!(before.settings, original.settings); assert!(!before.running); assert!(!before.cancel);
    assert_eq!(before.triggers, [("stella_owned_statement_abort".to_string(), "visits".to_string(), trigger_sql.clone())]);
    native_statement_assert_inventory(&before, !startup);
    if startup { assert_eq!(fs::read(store.join(&affected_archive))?, old_archive_bytes); assert!(!store.join(format!("{NATIVE_STATEMENT_FIRST}.tar.zst")).exists()); }
    else { native_busy_assert_archive(&store.join(&affected_archive), NATIVE_STATEMENT_AFFECTED, &changed)?; native_busy_assert_archive(&store.join(format!("{NATIVE_STATEMENT_FIRST}.tar.zst")), NATIVE_STATEMENT_FIRST, NATIVE_STATEMENT_FIRST_BODY)?; }
    let selected = vec![affected_archive.clone(), format!("{NATIVE_STATEMENT_FIRST}.tar.zst")];
    if startup { import::launch_startup_archive_import(app.handle().clone(), app.state())?; }
    else { assert_eq!(import::launch_enhanced_import(app.handle().clone(), selected.clone(), app.state())?, "2件のアーカイブ同期を開始しました。"); }
    let failed_progress = events.finish(&app)?;
    let failed = native_statement_stage(case, "02-failed", directory, registry, &app, &failed_progress)?;
    native_statement_assert_progress(&failed_progress, startup)?;
    assert_eq!(failed.rows, native_statement_expected(if startup { NativeStatementExpected::Partial } else { NativeStatementExpected::Original })?);
    assert_eq!(failed.settings, before.settings); assert_eq!(failed.triggers, before.triggers); assert!(!failed.running); assert!(!failed.cancel); native_statement_assert_inventory(&failed, true);
    assert_eq!(failed.files.iter().filter(|file| file.kind == "source").collect::<Vec<_>>(), before.files.iter().filter(|file| file.kind == "source").collect::<Vec<_>>());
    assert_eq!(fs::read(source.join(NATIVE_STATEMENT_FIRST))?, NATIVE_STATEMENT_FIRST_BODY.as_bytes()); assert_eq!(fs::read(source.join(NATIVE_STATEMENT_AFFECTED))?, changed.as_bytes());
    assert_eq!(fs::read(source.join("unrelated.txt"))?, b"unrelated source bytes"); assert_eq!(fs::read(store.join("retained.bin"))?, b"unrelated archive bytes");
    native_busy_assert_archive(&store.join(&affected_archive), NATIVE_STATEMENT_AFFECTED, &changed)?; native_busy_assert_archive(&store.join(format!("{NATIVE_STATEMENT_FIRST}.tar.zst")), NATIVE_STATEMENT_FIRST, NATIVE_STATEMENT_FIRST_BODY)?;
    if startup { assert_ne!(fs::read(store.join(&affected_archive))?, old_archive_bytes); } else { assert_eq!(failed.files, before.files); }
    let added_errors = native_statement_new_errors(&before.logs, &failed.logs)?;
    if startup { assert_eq!(added_errors.len(), 1); assert!(added_errors[0].contains(NATIVE_STATEMENT_AFFECTED)); assert_eq!(added_errors[0].matches(NATIVE_STATEMENT_MARKER).count(), 1); }
    else { assert!(added_errors.is_empty()); }
    // fault解除は同じDBのtrigger除去だけ。source／archive／設定／appの修復・再作成は行わない。
    let conn = Connection::open(get_db_path()?)?; assert!(conn.is_autocommit()); conn.execute_batch("DROP TRIGGER stella_owned_statement_abort;")?; assert!(conn.is_autocommit()); drop(conn);
    let removed = native_statement_stage(case, "03-trigger-removed", directory, registry, &app, &[])?;
    assert!(removed.triggers.is_empty()); assert_eq!(removed.rows, failed.rows); assert_eq!(removed.files, failed.files); assert_eq!(removed.logs, failed.logs); assert_eq!(removed.settings, before.settings);
    if startup { import::launch_startup_archive_import(app.handle().clone(), app.state())?; }
    else { assert_eq!(import::launch_enhanced_import(app.handle().clone(), selected, app.state())?, "2件のアーカイブ同期を開始しました。"); }
    let retry_progress = events.finish(&app)?;
    let retried = native_statement_stage(case, "04-retried", directory, registry, &app, &retry_progress)?;
    let terminal = retry_progress.last().ok_or("statement再試行の終端eventがありません")?;
    assert_eq!(terminal.status, if startup { "Data 内 zst アーカイブからの取り込みが完了しました。" } else { "2件のインポートが完了しました。" }); assert_eq!(terminal.progress, "100%"); assert!(!terminal.is_running);
    assert_eq!(retry_progress.iter().filter(|item| !item.is_running).count(), 1);
    assert_eq!(retry_progress.iter().filter(|item| item.status == "パース開始" && item.is_running).count(), if startup { 1 } else { 2 });
    assert_eq!(retry_progress.iter().filter(|item| item.status == format!("取り込み完了: {NATIVE_STATEMENT_AFFECTED}") && item.is_running).count(), 1);
    assert_eq!(retry_progress.iter().filter(|item| item.status == format!("スキップ（内容変更なし）: {NATIVE_STATEMENT_FIRST}") && item.is_running).count(), usize::from(startup));
    assert_eq!(retried.rows, native_statement_expected(NativeStatementExpected::Rebuilt)?); assert!(retried.triggers.is_empty()); assert_eq!(retried.settings, before.settings);
    assert_eq!(retried.database_path, before.database_path); assert_eq!(retried.source_path, before.source_path); assert_eq!(retried.archive_path, before.archive_path);
    assert_eq!(retried.files, failed.files); assert_eq!(retried.logs, failed.logs); assert!(!retried.running); assert!(!retried.cancel); native_statement_assert_inventory(&retried, true);
    native_busy_assert_archive(&store.join(&affected_archive), NATIVE_STATEMENT_AFFECTED, &changed)?; native_busy_assert_archive(&store.join(format!("{NATIVE_STATEMENT_FIRST}.tar.zst")), NATIVE_STATEMENT_FIRST, NATIVE_STATEMENT_FIRST_BODY)?;
    app.cleanup_before_exit(); Ok(())
}

#[test]
fn command_import_enhanced_statement_abort_restores_rows_and_retries_same_paths() -> NativeCommandTestResult {
    native_command_test("command_import_enhanced_statement_abort_restores_rows_and_retries_same_paths", |directory, registry| native_statement_boundary(false, directory, registry))
}
#[test]
fn command_import_startup_statement_abort_preserves_success_and_retries_same_paths() -> NativeCommandTestResult {
    native_command_test("command_import_startup_statement_abort_preserves_success_and_retries_same_paths", |directory, registry| native_statement_boundary(true, directory, registry))
}
