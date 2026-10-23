/// Polarisの実監視出力を登録経由で受け取り、圧縮・取込・再読み込み・viewerまで接続する。
#[test]
fn command_cross_app_consumes_polaris_output_and_reopens_saved_results() -> NativeCommandTestResult {
    if let Some(root) = std::env::var_os("STELLA_CROSS_APP_COLD_ROOT") {
        return verify_cross_app_cold_process(std::path::Path::new(&root));
    }
    native_command_test("command_cross_app_consumes_polaris_output_and_reopens_saved_results", |directory, registry| {
        use sha2::Digest;
        let name = "output_log_2026-10-08_09-59-50.txt";
        let expected = include_bytes!("../../tests/fixtures/cross_app_producer.log");
        let external = std::env::var_os("POLARIS_HEADLESS_CONSUMER_FIXTURE_ROOT").map(std::path::PathBuf::from);
        let producer = if let Some(root) = &external {
            assert_eq!(fs::read_to_string(root.join("owner.marker"))?.trim(), "polaris-headless-consumer-fixture-v1");
            root.clone()
        } else {
            let root = directory.path().join("standalone-producer");
            fs::create_dir_all(root.join("Data/archive"))?;
            fs::write(root.join("Data/archive").join(name), expected)?;
            root
        };
        let path = producer.join("Data/archive").join(name);
        let before = fs::read(&path)?;
        assert_eq!(before, expected);
        let hash = format!("{:x}", sha2::Sha256::digest(&before));
        assert_eq!(hash, "0cbec1da285e131dd7c45d75debd6ca49cb875a1b68283a27d0fef21c3b8afec");
        registry.isolated.open_subkey_with_flags("Software\\CosmoArtsStore\\Polaris", winreg::enums::KEY_SET_VALUE)?
            .set_value("InstallLocation", &producer.to_string_lossy().as_ref())?;
        let source = get_source_log_dir()?;
        assert_eq!(source, producer.join("Data/archive"));
        let store = get_archive_store_dir()?;
        assert_eq!(archive::sync_source_logs_into_archive_store(&source, &store)?, 1);
        assert_eq!(archive::sync_source_logs_into_archive_store(&source, &store)?, 0);
        let archived = store.join(format!("{name}.tar.zst"));
        let archive_before = fs::read(&archived)?;
        let db = get_db_path()?;
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let summary = analyze::run_diff_import(&db, &store, &cancel, |_, _| {})?;
        assert_eq!(summary.total_count, 1);
        assert!(summary.failed_filenames.is_empty());
        // 接続を作り直して、取込中のobjectではなく永続化された結果を検証する。
        let conn = Connection::open(&db)?;
        let session = conn.query_row("SELECT log_name,account_id,account_name,start_time,end_time FROM sessions", [], |row| {
            Ok((row.get::<_, String>(0)?,row.get::<_, String>(1)?,row.get::<_, String>(2)?,row.get::<_, String>(3)?,row.get::<_, String>(4)?))
        })?;
        assert_eq!(session, (name.to_string(), "usr_aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".to_string(), "Fixture Owner".to_string(), "2026-10-08 09:59:50".to_string(), "2026-10-08 11:30:00".to_string()));
        let visits = conn.prepare("SELECT world_name,instance_id,join_time,leave_time,duration_sec FROM visit_summary ORDER BY join_time")?
            .query_map([], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        assert_eq!(visits, [
            ("連携・朝のワールド".to_string(),"10001".to_string(),"2026-10-08 10:00:01".to_string(),"2026-10-08 10:30:00".to_string(),1799),
            ("連携 \"夜\" ワールド".to_string(),"20002".to_string(),"2026-10-08 11:00:01".to_string(),"2026-10-08 11:30:00".to_string(),1799),
        ]);
        assert_eq!(conn.query_row("SELECT region FROM visits WHERE instance_id='10001'", [], |r| r.get::<_,String>(0))?, "jp");
        // access segment省略形式の対応意図は保留し、連携の他条件を止めず実観測値を記録する。
        let region_without_access = conn.query_row("SELECT region FROM visits WHERE instance_id='20002'", [], |r| r.get::<_,Option<String>>(0))?;
        let guest = conn.query_row("SELECT vrchat_id,user_name,join_time,leave_time FROM with_users_detail WHERE is_self=0", [], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?;
        assert_eq!(guest, ("usr_bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb".to_string(),"連携ゲスト".to_string(),"2026-10-08 10:05:00".to_string(),"2026-10-08 10:10:00".to_string()));
        let photo = conn.query_row("SELECT world_name,file_path,resolution_width,resolution_height,timestamp FROM screenshots_detail", [], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,String>(4)?)))?;
        assert_eq!(photo, ("連携・朝のワールド".to_string(),r"C:\Fixture\VRChat_2026-10-08_10-15-00.000_1920x1080.png".to_string(),1920,1080,"2026-10-08 10:15:00".to_string()));
        drop(conn);
        let mut repeated_progress = Vec::new();
        let second = analyze::run_diff_import(&db, &store, &cancel, |status, _| repeated_progress.push(status))?;
        assert_eq!(second.total_count, 1);
        assert!(second.failed_filenames.is_empty());
        assert_eq!(repeated_progress.iter().filter(|status| status.starts_with("取り込み済み（変更なし）:")).count(), 1);
        assert!(!repeated_progress.iter().any(|status| status.starts_with("取り込み完了:")));
        let reread = database::get_db_table_data("visit_summary", None, Some("join_time".to_string()), Some("asc".to_string()))?;
        assert_eq!(reread.total_rows, 2);
        fs::write(directory.path().join("cold-reader.owner"), "stella-cross-app-cold-owner-v1")?;
        let cold = Command::new(std::env::current_exe()?)
            .args(["--exact", "commands::tests::command_cross_app_consumes_polaris_output_and_reopens_saved_results", "--nocapture"])
            .env("STELLA_CROSS_APP_COLD_ROOT", directory.path())
            .env("STELLA_CROSS_APP_COLD_HIVE", &registry.path)
            .creation_flags(0x0800_0000).output()?;
        let cold_stdout = String::from_utf8(cold.stdout)?;
        assert!(cold.status.success(), "cold reader: {cold_stdout} {}", String::from_utf8_lossy(&cold.stderr));
        let cold_result = fs::read_to_string(directory.path().join("cold-reader.result"))?;
        assert_eq!(cold_result, "cross-app-cold-reader: session=1 visits=2 guest=1 screenshot=1");
        let (app, _) = native_import_app()?;
        let events = NativeViewerEvents::listen(&app);
        let metadata = archive::read_archive_log_viewer(format!("{name}.tar.zst"), "cross-app".to_string(), app.handle().clone())?;
        assert_eq!(metadata.source_name, name);
        let chunks = events.finish(&["cross-app"])?;
        let lines = chunks.iter().flat_map(|chunk| chunk.raw_lines.iter()).cloned().collect::<Vec<_>>();
        assert_eq!(lines, std::str::from_utf8(expected)?.lines().collect::<Vec<_>>());
        app.cleanup_before_exit();
        assert_eq!(fs::read(&path)?, before);
        assert_eq!(fs::read(&archived)?, archive_before);
        if let Some(root) = std::env::var_os("POLARIS_HEADLESS_CONSUMER_RECEIPT_ROOT") {
            #[derive(serde::Serialize)]
            struct Observation { consumer: &'static str, external_producer: bool, source_path: std::path::PathBuf,
                source_sha256: String, checks: [&'static str; 6], results_json: String }
            let observation = Observation { consumer: "StellaRecord", external_producer: external.is_some(), source_path: path, source_sha256: hash,
                checks: ["source-bytes-preserved", "actual-archive-sync", "actual-import", "fresh-sqlite-connection", "repeat-import-unchanged", "windowless-viewer"],
                results_json: native_viewer_json((session, visits, guest, photo, second.total_count, lines, region_without_access, cold_result))? };
            fs::write(std::path::PathBuf::from(root).join("stellarecord-cross-app-observation.json"), native_viewer_json(observation)?)?;
        }
        Ok(())
    })
}

/// 親processが所有するhiveを参照だけに使い、新processの通常commandから永続結果を読む。
fn verify_cross_app_cold_process(root: &std::path::Path) -> NativeCommandTestResult {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) { let _ = unsafe { RegOverridePredefKey(winreg::enums::HKEY_CURRENT_USER, 0) }; }
    }
    assert_eq!(fs::read_to_string(root.join("cold-reader.owner"))?, "stella-cross-app-cold-owner-v1");
    let hive = std::env::var("STELLA_CROSS_APP_COLD_HIVE")?;
    assert!(hive.starts_with("Software\\StellaRecord.Tests\\fixture-"));
    let original = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let isolated = original.open_subkey(&hive)?;
    assert_eq!(unsafe { RegOverridePredefKey(winreg::enums::HKEY_CURRENT_USER, isolated.raw_handle()) }, 0);
    let _restore = Restore;
    assert_eq!(get_db_path()?, root.join("db/current.sqlite"));
    assert_eq!(get_archive_store_dir()?, root.join("archive"));
    let conn = Connection::open_with_flags(get_db_path()?, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let counts = conn.query_row("SELECT (SELECT count(*) FROM sessions),(SELECT count(*) FROM visits),(SELECT count(*) FROM with_users WHERE is_self=0),(SELECT count(*) FROM screenshots)", [], |r| Ok((r.get::<_,i64>(0)?, r.get::<_,i64>(1)?, r.get::<_,i64>(2)?, r.get::<_,i64>(3)?)))?;
    assert_eq!(counts, (1,2,1,1));
    let worlds = conn.prepare("SELECT world_name FROM visits ORDER BY join_time")?.query_map([], |r| r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    assert_eq!(worlds, ["連携・朝のワールド", "連携 \"夜\" ワールド"]);
    assert_eq!(database::get_db_table_data("visit_summary", None, None, None)?.total_rows, 2);
    assert!(!settings::get_startup_import_settings()?.enabled);
    assert_eq!(settings::get_management_settings().archive_limit_mb, 127);
    fs::write(root.join("cold-reader.result"), "cross-app-cold-reader: session=1 visits=2 guest=1 screenshot=1")?;
    Ok(())
}
