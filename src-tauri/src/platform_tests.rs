#[cfg(all(test, windows))]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// テスト終了時にレジストリキーを自動削除する RAII ガード。
    struct TestRegGuard(String);
    impl Drop for TestRegGuard {
        fn drop(&mut self) {
            let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.0);
        }
    }

    fn create_test_run_key(suffix: &str) -> (RegKey, TestRegGuard) {
        let path = format!("Software\\CosmoArtsStore\\_Test_Run_{suffix}");
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = root.create_subkey(&path).unwrap();
        (key, TestRegGuard(path))
    }

    // ── apply_startup_value（テスト専用 Run キーで検証。実起動キーには触れない） ──

    #[test]
    fn apply_startup_value_registers_command() {
        let (key, _guard) = create_test_run_key("register");
        apply_startup_value(&key, "StellaRecordTest", Some("\"C:\\app\\test.exe\"")).unwrap();

        let stored: String = key.get_value("StellaRecordTest").unwrap();
        assert_eq!(stored, "\"C:\\app\\test.exe\"");
    }

    #[test]
    fn apply_startup_value_removes_existing() {
        let (key, _guard) = create_test_run_key("remove");
        key.set_value("StellaRecordTest", &"\"C:\\app\\test.exe\"")
            .unwrap();

        apply_startup_value(&key, "StellaRecordTest", None).unwrap();

        let result: std::io::Result<String> = key.get_value("StellaRecordTest");
        assert!(result.is_err(), "値は削除されているはず");
    }

    #[test]
    fn apply_startup_value_remove_missing_is_idempotent() {
        let (key, _guard) = create_test_run_key("remove_missing");
        // 値が存在しない状態での解除はエラーにならない（冪等）
        apply_startup_value(&key, "NeverRegistered", None).unwrap();
    }

    #[test]
    fn apply_startup_value_overwrites() {
        let (key, _guard) = create_test_run_key("overwrite");
        apply_startup_value(&key, "App", Some("\"C:\\v1.exe\"")).unwrap();
        apply_startup_value(&key, "App", Some("\"C:\\v2.exe\"")).unwrap();

        let stored: String = key.get_value("App").unwrap();
        assert_eq!(stored, "\"C:\\v2.exe\"");
    }

    // ── read_exe_display_name ──

    #[test]
    fn display_name_falls_back_to_file_stem() {
        // バージョン情報を持たない（存在しない）パスはファイル名（拡張子なし）を返す。
        let result = read_exe_display_name(Path::new("C:\\nonexistent\\MyCoolApp.exe"));
        assert_eq!(result, Some("MyCoolApp".to_string()));
    }

    #[test]
    fn display_name_reads_real_system_exe() {
        // 実在するシステム exe からは表示名（FileDescription 等）が取得できる。
        let notepad = Path::new("C:\\Windows\\System32\\notepad.exe");
        if notepad.exists() {
            let result = read_exe_display_name(notepad);
            assert!(result.is_some());
            assert!(!result.unwrap().is_empty());
        }
    }

    // ── launch_external_process ──

    #[test]
    fn launch_external_process_errors_on_missing_exe() {
        // 存在しない実行ファイルの起動はエラーを返す（プロセスは生成されない）。
        let result = launch_external_process("Z:\\nonexistent\\never_here.exe");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("起動に失敗しました"));
    }

    // ── extract_exe_icon_png ──

    #[test]
    fn extract_icon_produces_valid_png() {
        // 実在するシステム exe からアイコンを抽出し、PNG として正しくエンコードされることを確認。
        // COM/シェル API が利用できない環境では None になり得るため、取得できた場合のみ検証する。
        let notepad = Path::new("C:\\Windows\\System32\\notepad.exe");
        if notepad.exists() {
            if let Some(png) = extract_exe_icon_png(notepad) {
                // PNG シグネチャ: 89 50 4E 47 0D 0A 1A 0A
                assert!(png.len() > 8);
                assert_eq!(&png[..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
            }
        }
    }
}
