/// Windowを作らず、実kernel mutexで多重起動の早期終了・所有process終了後の再起動を確認する。
#[test]
fn command_single_instance_guard_is_windowless_and_releases_on_process_exit() -> NativeCommandTestResult {
    native_command_test("command_single_instance_guard_is_windowless_and_releases_on_process_exit", |directory, _| {
        struct OwnedChild(std::process::Child);
        impl Drop for OwnedChild {
            fn drop(&mut self) {
                if matches!(self.0.try_wait(), Ok(None)) { let _ = self.0.kill(); let _ = self.0.wait(); }
            }
        }
        use std::io::Read;
        let mode = std::env::var("STELLA_GUARD_FIXTURE_MODE").unwrap_or_default();
        if !mode.is_empty() {
            let root = std::path::PathBuf::from(std::env::var("STELLA_GUARD_FIXTURE_ROOT")?);
            assert_eq!(fs::read_to_string(root.join("guard.owner"))?, "stella-owned-single-instance-v1");
            fs::write(root.join(format!("{mode}.before")), "before-guard")?;
            crate::platform::ensure_single_instance();
            fs::write(root.join(format!("{mode}.after")), "after-guard")?;
            if mode == "owner" {
                let mut release = Vec::new();
                std::io::stdin().read_to_end(&mut release)?;
            }
            return Ok(());
        }
        let root = directory.path();
        fs::write(root.join("guard.owner"), "stella-owned-single-instance-v1")?;
        let name = format!("Local\\StellaRecord.Tests.{}", directory.path().file_name().ok_or("専用root名がありません")?.to_string_lossy());
        let test_executable = std::env::current_exe()?;
        let launch = |mode: &str| {
            let mut command = Command::new(&test_executable);
            command.args(["--exact", "commands::tests::command_single_instance_guard_is_windowless_and_releases_on_process_exit", "--nocapture"])
                .env("STELLA_SINGLE_INSTANCE_FIXTURE_NAME", &name).env("STELLA_GUARD_FIXTURE_MODE", mode)
                .env("STELLA_GUARD_FIXTURE_ROOT", root).creation_flags(0x0800_0000);
            command
        };
        let stdout = tempfile::NamedTempFile::new()?;
        let stderr = tempfile::NamedTempFile::new()?;
        let mut owner = launch("owner").stdin(std::process::Stdio::piped())
            .stdout(stdout.as_file().try_clone()?).stderr(stderr.as_file().try_clone()?).spawn()?;
        // assertion失敗時も所有processだけを終了し、mutexと専用hiveを残さない。
        let release = owner.stdin.take().ok_or("専用ownerのstdinがありません")?;
        let mut owner = OwnedChild(owner);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !root.join("owner.after").exists() {
            assert!(owner.0.try_wait()?.is_none());
            assert!(std::time::Instant::now() < deadline, "専用ownerがmutex取得まで到達しません");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let duplicate = launch("duplicate").output()?;
        assert!(duplicate.status.success());
        assert!(root.join("duplicate.before").exists());
        assert!(!root.join("duplicate.after").exists());
        drop(release);
        assert!(owner.0.wait()?.success());
        let restarted = launch("restarted").output()?;
        assert!(restarted.status.success(), "{}", String::from_utf8_lossy(&restarted.stderr));
        assert!(root.join("restarted.after").exists());
        Ok(())
    })
}
