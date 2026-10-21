#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ── エラーフォーマッター ──

    #[test]
    fn command_err_formats_context_and_error() {
        let msg = command_err("操作に失敗しました", "disk full");
        assert_eq!(msg, "操作に失敗しました: disk full");
    }

    #[test]
    fn command_open_err_includes_path() {
        let path = std::path::Path::new("/some/file.db");
        let msg = command_open_err(path, "permission denied");
        assert!(msg.contains("/some/file.db"));
        assert!(msg.contains("permission denied"));
    }

    #[test]
    fn command_read_err_includes_path() {
        let path = std::path::Path::new("/data/archive.tar.zst");
        let msg = command_read_err(path, "corrupt archive");
        assert!(msg.contains("/data/archive.tar.zst"));
        assert!(msg.contains("corrupt archive"));
    }

    #[test]
    fn command_create_err_includes_path() {
        let path = std::path::Path::new("/new/dir");
        let msg = command_create_err(path, "no space");
        assert!(msg.contains("/new/dir"));
        assert!(msg.contains("no space"));
    }

    #[test]
    fn command_remove_err_includes_path() {
        let path = std::path::Path::new("/old/file.log");
        let msg = command_remove_err(path, "in use");
        assert!(msg.contains("/old/file.log"));
        assert!(msg.contains("in use"));
    }

    #[test]
    fn sql_identifier_validation() {
        assert!(is_sql_identifier("sessions"));
        assert!(is_sql_identifier("with_users_detail"));
        assert!(is_sql_identifier("table1"));
        assert!(!is_sql_identifier(""));
        assert!(!is_sql_identifier("users; DROP TABLE x"));
        assert!(!is_sql_identifier("users WHERE 1=1"));
        assert!(!is_sql_identifier("テーブル"));
    }

    #[test]
    fn lossy_log_line_decoding_replaces_invalid_utf8_and_trims_newline() {
        assert_eq!(decode_log_line_lossy(b"ok\xffnext\r\n"), "ok\u{fffd}next");
        assert_eq!(decode_log_line_lossy(b"line\n"), "line");
        assert_eq!(decode_log_line_lossy(b"line"), "line");
    }

    // ── レジストリ: get_component_install_dir ──

    /// テスト終了時にレジストリキーを自動削除する RAII ガード。
    struct TestRegGuard(String);
    impl Drop for TestRegGuard {
        fn drop(&mut self) {
            let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.0);
        }
    }

    fn create_test_key(suffix: &str) -> (RegKey, TestRegGuard) {
        let path = format!("Software\\CosmoArtsStore\\_Test_{suffix}");
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = root.create_subkey(&path).unwrap();
        (key, TestRegGuard(path))
    }

    #[test]
    fn component_install_dir_returns_path_when_valid() {
        // 実在するパスでないと get_component_install_dir が None を返すため、
        // 環境非依存にするよう一時ディレクトリを InstallLocation に設定する。
        let (key, _guard) = create_test_key("install_valid");
        let temp = tempfile::tempdir().unwrap();
        let test_dir = temp.path().to_str().unwrap();
        key.set_value("InstallLocation", &test_dir).unwrap();

        let result = get_component_install_dir("_Test_install_valid");
        assert_eq!(result, Some(PathBuf::from(test_dir)));
    }

    #[test]
    fn component_install_dir_returns_none_for_missing_key() {
        let result = get_component_install_dir("_Test_nonexistent_key_12345");
        assert!(result.is_none());
    }

    #[test]
    fn component_install_dir_returns_none_for_nonexistent_path() {
        let (key, _guard) = create_test_key("install_nodir");
        key.set_value("InstallLocation", &r"Z:\does\not\exist\at\all")
            .unwrap();

        let result = get_component_install_dir("_Test_install_nodir");
        assert!(result.is_none());
    }

    #[test]
    fn component_install_dir_returns_none_when_value_missing() {
        let (_key, _guard) = create_test_key("install_noval");

        let result = get_component_install_dir("_Test_install_noval");
        assert!(result.is_none());
    }

    // ── append_log_to (一時ディレクトリで検証) ──

    #[test]
    fn append_log_to_writes_monthly_file() {
        let dir = tempfile::tempdir().unwrap();
        append_log_to(dir.path(), "WARN", "テストメッセージ").unwrap();

        let month = Local::now().format("%Y-%m");
        let log_path = dir.path().join(format!("info-{month}.log"));
        assert!(log_path.exists());

        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("[WARN]"));
        assert!(content.contains("テストメッセージ"));
    }

    #[test]
    fn append_log_to_appends_multiple_lines() {
        let dir = tempfile::tempdir().unwrap();
        append_log_to(dir.path(), "INFO", "1行目").unwrap();
        append_log_to(dir.path(), "ERROR", "2行目").unwrap();

        let month = Local::now().format("%Y-%m");
        let content =
            std::fs::read_to_string(dir.path().join(format!("info-{month}.log"))).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("1行目"));
        assert!(lines[1].contains("2行目"));
    }

    #[test]
    fn append_log_to_fails_on_invalid_dir() {
        let result = append_log_to(std::path::Path::new("Z:\\no\\such\\dir"), "WARN", "x");
        assert!(result.is_err());
    }
}
