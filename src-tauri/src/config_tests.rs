#[cfg(test)]
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

    fn create_test_key(suffix: &str) -> (RegKey, TestRegGuard) {
        let path = format!("Software\\CosmoArtsStore\\_Test_{suffix}");
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = root.create_subkey(&path).unwrap();
        (key, TestRegGuard(path))
    }

    // ── open_key / create_key ──

    #[test]
    fn open_key_returns_none_for_missing() {
        assert!(open_key("Software\\CosmoArtsStore\\_Test_missing_99999").is_none());
    }

    #[test]
    fn create_key_and_open_key_roundtrip() {
        let path = "Software\\CosmoArtsStore\\_Test_create_open";
        let _guard = TestRegGuard(path.to_string());

        let key = create_key(path).unwrap();
        key.set_value("Marker", &"ok").unwrap();

        let opened = open_key(path);
        assert!(opened.is_some());
        let val: String = opened.unwrap().get_value("Marker").unwrap();
        assert_eq!(val, "ok");
    }

    // ── read_str / read_u64 / read_bool ──

    #[test]
    fn read_str_returns_value() {
        let (key, _guard) = create_test_key("read_str");
        key.set_value("Name", &"hello").unwrap();
        assert_eq!(read_str(&key, "Name"), "hello");
    }

    #[test]
    fn read_str_returns_empty_when_missing() {
        let (key, _guard) = create_test_key("read_str_miss");
        assert_eq!(read_str(&key, "NoSuchValue"), "");
    }

    #[test]
    fn read_u64_returns_value() {
        let (key, _guard) = create_test_key("read_u64");
        key.set_value("Capacity", &314_572_800u64).unwrap();
        assert_eq!(read_u64(&key, "Capacity", 0), 314_572_800);
    }

    #[test]
    fn read_u64_returns_default_when_missing() {
        let (key, _guard) = create_test_key("read_u64_miss");
        assert_eq!(read_u64(&key, "NoSuchValue", 42), 42);
    }

    #[test]
    fn read_bool_true() {
        let (key, _guard) = create_test_key("read_bool_t");
        key.set_value("Flag", &1u32).unwrap();
        assert!(read_bool(&key, "Flag", false));
    }

    #[test]
    fn read_bool_false() {
        let (key, _guard) = create_test_key("read_bool_f");
        key.set_value("Flag", &0u32).unwrap();
        assert!(!read_bool(&key, "Flag", true));
    }

    #[test]
    fn read_bool_default_when_missing() {
        let (key, _guard) = create_test_key("read_bool_miss");
        assert!(read_bool(&key, "NoSuchValue", true));
        assert!(!read_bool(&key, "NoSuchValue", false));
    }

    // ── save/load roundtrip (テスト専用キーパスで実行) ──

    #[test]
    fn polaris_setting_save_load_roundtrip() {
        let test_path = "Software\\CosmoArtsStore\\_Test_Polaris_RT";
        let _guard = TestRegGuard(test_path.to_string());

        let setting = PolarisSetting {
            archive_path: r"C:\StellaRecordTest\archive".to_string(),
            capacity_threshold_bytes: 1_073_741_824,
        };

        save_polaris_setting_to(test_path, &setting).unwrap();
        let loaded = load_polaris_setting_from(test_path);

        assert_eq!(loaded.archive_path, setting.archive_path);
        assert_eq!(loaded.capacity_threshold_bytes, 1_073_741_824);
    }

    #[test]
    fn polaris_setting_load_missing_returns_default() {
        let loaded = load_polaris_setting_from("Software\\CosmoArtsStore\\_Test_Polaris_Missing");
        assert_eq!(loaded.archive_path, "");
        assert_eq!(loaded.capacity_threshold_bytes, 314_572_800);
    }

    #[test]
    fn stellarecord_setting_save_load_roundtrip() {
        let test_path = "Software\\CosmoArtsStore\\_Test_SR_RT";
        let _guard = TestRegGuard(test_path.to_string());

        let setting = StellaRecordSetting {
            archive_path: r"C:\StellaRecordTest\archive".to_string(),
            db_path: r"C:\StellaRecordTest\db\test.db".to_string(),
            enable_startup: true,
            startup_preference_set: true,
        };

        save_stellarecord_setting_to(test_path, &setting).unwrap();
        let loaded = load_stellarecord_setting_from(test_path);

        assert_eq!(loaded.archive_path, setting.archive_path);
        assert_eq!(loaded.db_path, setting.db_path);
        assert!(loaded.enable_startup);
        assert!(loaded.startup_preference_set);
    }

    #[test]
    fn stellarecord_setting_load_missing_returns_default() {
        let loaded = load_stellarecord_setting_from("Software\\CosmoArtsStore\\_Test_SR_Missing");
        assert_eq!(loaded.archive_path, "");
        assert_eq!(loaded.db_path, "");
        assert!(!loaded.enable_startup);
        assert!(!loaded.startup_preference_set);
    }

    // ── load_catalog_from_db (一時 DB で検証) ──

    #[test]
    fn catalog_from_db_returns_empty_when_db_missing() {
        let path = std::path::Path::new("Z:\\nonexistent\\catalog.db");
        let catalog = load_catalog_from_db(path);
        assert!(catalog.apps.is_empty());
    }

    #[test]
    fn catalog_from_db_returns_empty_when_no_apps_table() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("empty.db");
        Connection::open(&db_path).unwrap();
        let catalog = load_catalog_from_db(&db_path);
        assert!(catalog.apps.is_empty());
    }

    #[test]
    fn catalog_from_db_reads_apps_with_base64_icon() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("catalog.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE apps (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL, description TEXT DEFAULT '',
                path TEXT UNIQUE, icon BLOB
            );
            INSERT INTO apps (name, description, path, icon)
                VALUES ('Beta', 'second', '/b', NULL);
            INSERT INTO apps (name, description, path, icon)
                VALUES ('Alpha', 'first', '/a', x'89504E47');",
        )
        .unwrap();
        drop(conn);

        let catalog = load_catalog_from_db(&db_path);
        assert_eq!(catalog.apps.len(), 2);
        // ORDER BY name のため Alpha が先
        assert_eq!(catalog.apps[0].name, "Alpha");
        assert_eq!(catalog.apps[0].icon_data.as_deref(), Some("iVBORw==")); // base64(0x89504E47)
        assert_eq!(catalog.apps[1].name, "Beta");
        assert!(catalog.apps[1].icon_data.is_none());
    }

    // ── StellaRecordSetting パス解決 ──

    #[test]
    fn effective_archive_dir_uses_explicit_path() {
        let setting = StellaRecordSetting {
            archive_path: r"C:\StellaRecordTest\archive".to_string(),
            ..Default::default()
        };
        assert_eq!(
            setting.get_effective_archive_dir(),
            Some(PathBuf::from(r"C:\StellaRecordTest\archive"))
        );
    }

    #[test]
    fn effective_db_path_uses_explicit_path() {
        let setting = StellaRecordSetting {
            db_path: r"C:\StellaRecordTest\db\test.db".to_string(),
            ..Default::default()
        };
        assert_eq!(
            setting.get_effective_db_path(),
            Some(PathBuf::from(r"C:\StellaRecordTest\db\test.db"))
        );
    }
}
