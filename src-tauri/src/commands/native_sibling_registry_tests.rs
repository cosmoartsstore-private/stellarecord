/// 姉妹アプリが実DBへ登録した行を、StellaRecordの通常catalog入口で再読み込みする。
#[test]
fn command_sibling_registration_uses_current_schema_and_catalog() -> NativeCommandTestResult {
    native_command_test("command_sibling_registration_uses_current_schema_and_catalog", |directory, _| {
        use sha2::Digest;
        let external = std::env::var_os("STELLA_SIBLING_REGISTRY_FIXTURE_ROOT").map(std::path::PathBuf::from);
        let phase = std::env::var("STELLA_SIBLING_REGISTRY_PHASE").unwrap_or_default();
        let root = external.as_deref().unwrap_or(directory.path());
        if external.is_some() {
            assert_eq!(fs::read_to_string(root.join("owner.marker"))?.trim(), "stella-sibling-registration-fixture-v1");
            let mut setting = config::load_stellarecord_setting();
            setting.db_path = root.join("db/current.sqlite").to_string_lossy().into_owned();
            config::save_stellarecord_setting(&setting)?;
        }
        let db = get_db_path()?;
        assert!(db.starts_with(root));
        if phase != "consume" {
            assert!(!db.exists());
            ensure_parent_dir(&db)?;
            let conn = Connection::open(&db)?;
            analyze::init_main_db(&conn)?;
            assert!(conn.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='visits'", [], |r| r.get::<_,i64>(0))? > 0);
            drop(conn);
            assert!(settings::read_registry_catalog().apps.is_empty());
            if phase == "prepare" {
                assert!(external.is_some());
                fs::write(root.join("stella-schema-prepared.json"), native_viewer_json(("StellaRecord::analyze::init_main_db", db.display().to_string(), format!("{:x}", sha2::Sha256::digest(fs::read(&db)?))))?)?;
                return Ok(());
            }
            assert!(phase.is_empty());
            let executable = root.join("owned-catalog.exe");
            fs::write(&executable, b"owned catalog fixture")?;
            registry::register_app(executable.to_string_lossy().into_owned(), "Stargazer".to_string(), "イベント抽選・キャストマッチング用デスクトップアプリ".to_string())?;
        }
        let expected_path = if external.is_some() {
            assert_eq!(phase, "consume");
            let path = fs::read_to_string(root.join("stargazer-expected-exe-path.txt"))?;
            let path = path.trim_end().to_string();
            assert!(std::path::Path::new(&path).is_file());
            path
        } else { root.join("owned-catalog.exe").to_string_lossy().into_owned() };
        let before = fs::read(&db)?;
        let catalog = settings::read_registry_catalog();
        assert_eq!(catalog.apps.len(), 1);
        assert_eq!(catalog.apps[0].name, "Stargazer");
        assert_eq!(catalog.apps[0].description, "イベント抽選・キャストマッチング用デスクトップアプリ");
        assert_eq!(catalog.apps[0].path, expected_path);
        let second = settings::read_registry_catalog();
        assert_eq!(native_viewer_json(&catalog)?, native_viewer_json(&second)?);
        assert_eq!(fs::read(&db)?, before);
        if external.is_some() {
            fs::write(root.join("stella-catalog-consumed.json"), native_viewer_json(("StellaRecord::settings::read_registry_catalog", catalog, format!("{:x}", sha2::Sha256::digest(before)), "database-bytes-preserved;registered-test-exe-not-launched"))?)?;
        }
        Ok(())
    })
}
