#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // ファイル処理に入ってからの中断を再現し、IPCが判断する取消し結果と旧履歴を確認する。
    fn verify_cancellation_while_reading(manual: bool) {
        let dir = tempfile::tempdir().unwrap();
        let archive_dir = dir.path().join("archive");
        let db_path = dir.path().join("stellarecord.db");
        fs::create_dir_all(&archive_dir).unwrap();
        let archive_path = archive_dir.join("output_log_cancel.txt.tar.zst");
        let content = b"2026.10.08 12:00:00 Log        -  [Behaviour] Entering Room: Canceled World\n";
        let mut encoder = zstd::stream::Encoder::new(Vec::new(), 3).unwrap();
        {
            let mut archive = tar::Builder::new(&mut encoder);
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive.append_data(&mut header, "output_log_cancel.txt", &content[..]).unwrap();
            archive.finish().unwrap();
        }
        fs::write(&archive_path, encoder.finish().unwrap()).unwrap();
        {
            let conn = Connection::open(&db_path).unwrap();
            db::init_main_db(&conn).unwrap();
            conn.execute("INSERT INTO sessions(log_name,account_name) VALUES('owned-existing.txt','Prior Owner')", []).unwrap();
        }
        let cancel = AtomicBool::new(false);
        let progress = |status: String, _: String| {
            if status == "取り込み中: output_log_cancel.txt" {
                cancel.store(true, Ordering::SeqCst);
            }
        };
        let result = if manual {
            run_enhanced_import_batch(&db_path, &[archive_path], &cancel, progress)
        } else {
            run_diff_import(&db_path, &archive_dir, &cancel, progress).map(|_| ())
        };
        assert!(cancel.load(Ordering::SeqCst));
        let conn = Connection::open(&db_path).unwrap();
        let sessions: Vec<(String, String)> = conn.prepare("SELECT log_name,account_name FROM sessions ORDER BY id").unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert_eq!(sessions, vec![("owned-existing.txt".to_string(), "Prior Owner".to_string())]);
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM imported_logs", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
        assert_eq!(result.unwrap_err(), ANALYZE_CANCELED_MESSAGE);
    }

    #[test]
    fn manual_cancellation_while_reading_preserves_saved_data_and_reports_canceled() {
        verify_cancellation_while_reading(true);
    }

    #[test]
    fn diff_cancellation_while_reading_preserves_saved_data_and_reports_canceled() {
        verify_cancellation_while_reading(false);
    }

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::init_main_db(&conn).unwrap();
        conn
    }

    fn make_reader(content: &str) -> BufReader<Cursor<Vec<u8>>> {
        BufReader::new(Cursor::new(content.as_bytes().to_vec()))
    }

    fn make_reader_bytes(content: Vec<u8>) -> BufReader<Cursor<Vec<u8>>> {
        BufReader::new(Cursor::new(content))
    }

    #[test]
    fn source_name_strips_tar_zst() {
        let path = Path::new("/data/output_log_2025-04-30.txt.tar.zst");
        assert_eq!(
            source_name_for_archive(path),
            Some("output_log_2025-04-30.txt".to_string())
        );
    }

    #[test]
    fn source_name_plain_txt() {
        let path = Path::new("/data/output_log_2025-04-30.txt");
        assert_eq!(
            source_name_for_archive(path),
            Some("output_log_2025-04-30.txt".to_string())
        );
    }

    #[test]
    fn ensure_not_canceled_ok() {
        let flag = AtomicBool::new(false);
        assert!(ensure_not_canceled(&flag).is_ok());
    }

    #[test]
    fn ensure_not_canceled_err() {
        let flag = AtomicBool::new(true);
        let err = ensure_not_canceled(&flag).unwrap_err();
        assert_eq!(err, "解析を中断しました。変更はロールバックされました。");
    }

    #[test]
    fn format_progress_fraction_zero() {
        assert_eq!(format_progress_fraction(0, 10), "0/10");
    }

    #[test]
    fn format_progress_fraction_done() {
        assert_eq!(format_progress_fraction(5, 5), "5/5");
    }

    #[test]
    fn import_session_with_auth_and_visit() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = "\
2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:01:00 Log        -  User Authenticated: TestUser (usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee)
2025.04.30 20:02:00 Log        -  [Behaviour] Entering Room: My Cool World
2025.04.30 20:02:01 Log        -  [Behaviour] Joining wrld_abc123:99999~private(usr_owner-1234)~region(jp)
2025.04.30 20:03:00 Log        -  [Behaviour] OnPlayerJoined FriendUser (usr_1111-2222-3333-4444-555555555555)
2025.04.30 20:04:00 Log        -  [Behaviour] Initialized PlayerAPI \"TestUser\" is local
2025.04.30 20:10:00 Log        -  [Behaviour] OnPlayerLeft FriendUser (usr_1111-2222-3333-4444-555555555555)
2025.04.30 20:15:00 Log        -  [Behaviour] OnLeftRoom
";
        let reader = make_reader(log);
        let mut progress_calls = Vec::new();
        parse_and_import_reader(
            &conn,
            reader,
            "test_log.txt",
            &cancel,
            &mut |status: String, progress: String| {
                progress_calls.push((status, progress));
            },
        )
        .unwrap();

        let counts: (i64, i64, i64, i64) = conn.query_row("SELECT (SELECT COUNT(*) FROM sessions), (SELECT COUNT(*) FROM visits), (SELECT COUNT(*) FROM find_users), (SELECT COUNT(*) FROM with_users)", [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).unwrap();
        assert_eq!(counts, (1, 1, 1, 1));
        let session: (String, String, String, String, String) = conn.query_row("SELECT log_name, account_id, account_name, start_time, end_time FROM sessions", [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))).unwrap();
        assert_eq!(session, ("test_log.txt".into(), "usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(), "TestUser".into(), "2025-04-30 20:00:00".into(), "2025-04-30 20:15:00".into()));
        let visit: (String, String, String, String, String, String, String) = conn.query_row("SELECT s.log_name, v.world_name, v.instance_id, v.instance_type, v.region, v.join_time, v.leave_time FROM visits v JOIN sessions s ON s.id = v.session_id", [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))).unwrap();
        assert_eq!(visit, ("test_log.txt".into(), "My Cool World".into(), "99999".into(), "private".into(), "jp".into(), "2025-04-30 20:02:01".into(), "2025-04-30 20:15:00".into()));
        // 先に退出した参加者の時刻を、後のワールド退室時刻で上書きしてはいけない。
        let participant: (String, String, String, i64, i64, String, String) = conn.query_row("SELECT s.log_name, u.vrchat_id, u.account_name, w.is_self, w.friend_status, w.join_time, w.leave_time FROM with_users w JOIN find_users u ON u.vrchat_id = w.vrchat_id JOIN visits v ON v.id = w.visit_id JOIN sessions s ON s.id = v.session_id", [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))).unwrap();
        assert_eq!(participant, ("test_log.txt".into(), "usr_1111-2222-3333-4444-555555555555".into(), "FriendUser".into(), 0, 2, "2025-04-30 20:03:00".into(), "2025-04-30 20:10:00".into()));
        assert_eq!(progress_calls, [("ログを解析しています…".into(), String::new()), ("解析結果を保存しています…".into(), String::new())]);
    }

    #[test]
    fn import_lossy_decodes_invalid_utf8_line() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let mut log = b"2025.04.30 20:01:00 Log        -  ".to_vec();
        log.push(0xff);
        log.extend_from_slice(
            b" User Authenticated: TestUser (usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee)\n",
        );
        let reader = make_reader_bytes(log);

        parse_and_import_reader(
            &conn,
            reader,
            "invalid_utf8_log.txt",
            &cancel,
            &mut |_status: String, _progress: String| {},
        )
        .unwrap();

        let account_name: Option<String> = conn
            .query_row(
                "SELECT account_name FROM sessions WHERE log_name = 'invalid_utf8_log.txt'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(account_name.as_deref(), Some("TestUser"));
    }

    #[test]
    fn import_screenshot_event() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = "\
2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:05:00 Log        -  [VRC Camera] Took screenshot to: C:\\Users\\test\\VRChat_2025-04-30_20-05-00.000_3840x2160.png
";
        let reader = make_reader(log);
        parse_and_import_reader(
            &conn,
            reader,
            "screenshot_test.txt",
            &cancel,
            &mut |_: String, _: String| {},
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM screenshots", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let (width, height, screenshot_session_id, parsed_session_id): (
            Option<i64>,
            Option<i64>,
            Option<i64>,
            i64,
        ) = conn
            .query_row(
                "SELECT
                    screenshot.resolution_width,
                    screenshot.resolution_height,
                    screenshot.session_id,
                    session.id
                 FROM screenshots AS screenshot
                 JOIN sessions AS session ON session.log_name = 'screenshot_test.txt'
                 LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(width, Some(3840));
        assert_eq!(height, Some(2160));
        assert_eq!(screenshot_session_id, Some(parsed_session_id));
    }

    #[test]
    fn import_osc_event() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = "\
2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:01:00 Log        -  Found new OSC Service: OyasumiVR at 127.0.0.1:61080
";
        let reader = make_reader(log);
        parse_and_import_reader(
            &conn,
            reader,
            "osc_test.txt",
            &cancel,
            &mut |_: String, _: String| {},
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM osc", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let service_name: String = conn
            .query_row("SELECT service_name FROM osc LIMIT 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(service_name, "OyasumiVR");
    }

    #[test]
    fn import_subscription_status() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = "\
2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:01:00 Log        -  Get VRChat Subscription Details! Subscription Id:sub_12345 active:True desc:VRChat Plus
";
        let reader = make_reader(log);
        parse_and_import_reader(
            &conn,
            reader,
            "sub_test.txt",
            &cancel,
            &mut |_: String, _: String| {},
        )
        .unwrap();

        let (is_active, desc): (bool, Option<String>) = conn
            .query_row(
                "SELECT is_active, description FROM subscription LIMIT 1",
                [],
                |row| Ok((row.get(0).unwrap(), row.get(1).ok())),
            )
            .unwrap();
        assert!(is_active);
        assert_eq!(desc.as_deref(), Some("VRChat Plus"));
    }

    #[test]
    fn import_cancellation_rolls_back() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(true);
        let log = "2025.04.30 20:00:00 Log        -  VRChat starting\n";
        let reader = make_reader(log);
        let result = parse_and_import_reader(
            &conn,
            reader,
            "cancel_test.txt",
            &cancel,
            &mut |_: String, _: String| {},
        );
        assert!(result.is_err());
    }

    #[test]
    fn import_single_line_notification() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = r#"2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:01:00 Log        -  Received Notification: <Notification from username:SomeUser, sender user id:usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee to usr_self of type: invite, id: not_1234-5678-abcd-ef01-234567890abc, created at: 04/30/2025 11:01:00 UTC, expires: 04/30/2025 12:01:00 UTC, message: "Come join me!">
"#;
        let reader = make_reader(log);
        parse_and_import_reader(
            &conn,
            reader,
            "notif_test.txt",
            &cancel,
            &mut |_: String, _: String| {},
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notifications", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let notif_type: String = conn
            .query_row("SELECT notif_type FROM notifications LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(notif_type, "invite");
    }

    #[test]
    fn import_empty_log_creates_session() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let reader = make_reader("");
        parse_and_import_reader(
            &conn,
            reader,
            "empty_test.txt",
            &cancel,
            &mut |_: String, _: String| {},
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn import_multiple_visits_closes_previous() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = "\
2025.04.30 20:00:00 Log        -  [Behaviour] Entering Room: World A
2025.04.30 20:00:01 Log        -  [Behaviour] Joining wrld_aaa:111~public~region(jp)
2025.04.30 20:10:00 Log        -  [Behaviour] Entering Room: World B
2025.04.30 20:10:01 Log        -  [Behaviour] Joining wrld_bbb:222~public~region(us)
2025.04.30 20:20:00 Log        -  [Behaviour] OnLeftRoom
";
        let reader = make_reader(log);
        parse_and_import_reader(
            &conn,
            reader,
            "multi_visit.txt",
            &cancel,
            &mut |_: String, _: String| {},
        )
        .unwrap();

        let counts: (i64, i64, i64, i64) = conn.query_row("SELECT (SELECT COUNT(*) FROM sessions), (SELECT COUNT(*) FROM visits), (SELECT COUNT(*) FROM find_users), (SELECT COUNT(*) FROM with_users)", [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).unwrap();
        assert_eq!(counts, (1, 2, 0, 0));
        let mut statement = conn.prepare("SELECT s.log_name, v.world_name, v.instance_id, v.instance_type, v.region, v.join_time, v.leave_time FROM visits v JOIN sessions s ON s.id = v.session_id ORDER BY v.id").unwrap();
        let visits: Vec<(String, String, String, String, String, String, String)> = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))).unwrap().collect::<Result<_>>().unwrap();
        // 次ワールドへの移動と明示的な退室を、別の終了時刻として保存する。
        assert_eq!(visits, [
            ("multi_visit.txt".into(), "World A".into(), "111".into(), "public".into(), "jp".into(), "2025-04-30 20:00:01".into(), "2025-04-30 20:10:00".into()),
            ("multi_visit.txt".into(), "World B".into(), "222".into(), "public".into(), "us".into(), "2025-04-30 20:10:01".into(), "2025-04-30 20:20:00".into()),
        ]);
    }

    #[test]
    fn diff_import_rebuilds_replaced_archive_and_skips_unchanged_content() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let archive_dir = dir.path().join("archive");
        let db_path = dir.path().join("stellarecord.db");
        fs::create_dir_all(&source_dir).unwrap();
        let source_path = source_dir.join("output_log_2025-04-30.txt");
        let first_log = "\
2025.04.30 20:00:00 Log        -  [Behaviour] Entering Room: World A
2025.04.30 20:00:01 Log        -  [Behaviour] Joining wrld_aaa:111~public~region(jp)
2025.04.30 20:05:00 Log        -  [Behaviour] OnLeftRoom
";
        fs::write(&source_path, first_log).unwrap();

        assert_eq!(
            crate::commands::archive::sync_source_logs_into_archive_store(
                &source_dir,
                &archive_dir
            )
            .unwrap(),
            1
        );
        run_diff_import(&db_path, &archive_dir, &AtomicBool::new(false), |_, _| {}).unwrap();

        let first_session_id: i64 = {
            let conn = Connection::open(&db_path).unwrap();
            conn.query_row("SELECT id FROM sessions", [], |row| row.get(0))
                .unwrap()
        };

        let mut unchanged_progress = Vec::new();
        run_diff_import(
            &db_path,
            &archive_dir,
            &AtomicBool::new(false),
            |status, _| unchanged_progress.push(status),
        )
        .unwrap();
        let unchanged_session_id: i64 = {
            let conn = Connection::open(&db_path).unwrap();
            conn.query_row("SELECT id FROM sessions", [], |row| row.get(0))
                .unwrap()
        };
        assert_eq!(unchanged_session_id, first_session_id);
        assert!(unchanged_progress
            .iter()
            .any(|status| status.contains("取り込み済み（変更なし）")));

        let extended_log = format!(
            "{first_log}\
2025.04.30 20:10:00 Log        -  [Behaviour] Entering Room: World B
2025.04.30 20:10:01 Log        -  [Behaviour] Joining wrld_bbb:222~public~region(us)
2025.04.30 20:15:00 Log        -  [Behaviour] OnLeftRoom
"
        );
        fs::write(&source_path, extended_log).unwrap();
        assert_eq!(
            crate::commands::archive::sync_source_logs_into_archive_store(
                &source_dir,
                &archive_dir
            )
            .unwrap(),
            1
        );
        run_diff_import(&db_path, &archive_dir, &AtomicBool::new(false), |_, _| {}).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let (session_count, visit_count, tracking_count): (i64, i64, i64) = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM sessions),
                    (SELECT COUNT(*) FROM visits),
                    (SELECT COUNT(*) FROM imported_logs)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let rebuilt_session_id: i64 = conn
            .query_row("SELECT id FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(session_count, 1);
        assert_eq!(visit_count, 2);
        assert_eq!(tracking_count, 1);
        assert_ne!(rebuilt_session_id, first_session_id);
    }

    #[test]
    fn diff_import_replaces_worldless_screenshot_with_new_session_owner() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let archive_dir = dir.path().join("archive");
        let db_path = dir.path().join("stellarecord.db");
        fs::create_dir_all(&source_dir).unwrap();
        let source_path = source_dir.join("output_log_2025-04-30.txt");
        let first_log = "\
2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:01:00 Log        -  [VRC Camera] Took screenshot to: C:\\Users\\test\\Pictures\\VRChat\\before_1920x1080.png
";
        fs::write(&source_path, first_log).unwrap();
        crate::commands::archive::sync_source_logs_into_archive_store(&source_dir, &archive_dir)
            .unwrap();
        run_diff_import(&db_path, &archive_dir, &AtomicBool::new(false), |_, _| {}).unwrap();

        let first_session_id: i64 = {
            let conn = Connection::open(&db_path).unwrap();
            let (session_id, screenshot_session_id, visit_id): (i64, Option<i64>, Option<i64>) =
                conn.query_row(
                    "SELECT session.id, screenshot.session_id, screenshot.visit_id
                     FROM sessions AS session
                     JOIN screenshots AS screenshot ON screenshot.session_id = session.id",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(screenshot_session_id, Some(session_id));
            assert_eq!(visit_id, None);
            session_id
        };

        let replacement_log = format!(
            "{first_log}\
2025.04.30 21:00:00 Log        -  Application status changed
"
        );
        fs::write(&source_path, replacement_log).unwrap();
        crate::commands::archive::sync_source_logs_into_archive_store(&source_dir, &archive_dir)
            .unwrap();
        run_diff_import(&db_path, &archive_dir, &AtomicBool::new(false), |_, _| {}).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let (session_count, screenshot_count): (i64, i64) = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM sessions),
                    (SELECT COUNT(*) FROM screenshots)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let (new_session_id, screenshot_session_id, visit_id, file_path): (
            i64,
            Option<i64>,
            Option<i64>,
            String,
        ) = conn
            .query_row(
                "SELECT session.id, screenshot.session_id, screenshot.visit_id, screenshot.file_path
                 FROM sessions AS session
                 JOIN screenshots AS screenshot ON screenshot.session_id = session.id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert_eq!(session_count, 1);
        assert_eq!(screenshot_count, 1);
        assert_ne!(new_session_id, first_session_id);
        assert_eq!(screenshot_session_id, Some(new_session_id));
        assert_eq!(visit_id, None);
        assert!(file_path.ends_with("before_1920x1080.png"));
    }

    #[test]
    fn current_schema_session_without_fingerprint_is_rebuilt_once() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let archive_dir = dir.path().join("archive");
        let db_path = dir.path().join("stellarecord.db");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(
            source_dir.join("output_log_2025-04-30.txt"),
            "\
2025.04.30 20:00:00 Log        -  [Behaviour] Entering Room: Current World
2025.04.30 20:00:01 Log        -  [Behaviour] Joining wrld_current:111~public~region(jp)
",
        )
        .unwrap();
        crate::commands::archive::sync_source_logs_into_archive_store(&source_dir, &archive_dir)
            .unwrap();

        {
            let conn = Connection::open(&db_path).unwrap();
            init_main_db(&conn).unwrap();
            conn.execute(
                "INSERT INTO sessions (log_name, start_time)
                 VALUES ('output_log_2025-04-30.txt', '2025-04-30')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO visits
                 (session_id, world_name, instance_id, instance_type, join_time)
                 VALUES (1, 'Stale World', 'old', 'public', '2025-04-30')",
                [],
            )
            .unwrap();
        }

        run_diff_import(&db_path, &archive_dir, &AtomicBool::new(false), |_, _| {}).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let world_name: String = conn
            .query_row("SELECT world_name FROM visits", [], |row| row.get(0))
            .unwrap();
        let tracking_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM imported_logs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(world_name, "Current World");
        assert_eq!(tracking_count, 1);
    }

    #[test]
    fn diff_import_restores_previous_session_when_rebuild_fails() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let archive_dir = dir.path().join("archive");
        let db_path = dir.path().join("stellarecord.db");
        fs::create_dir_all(&source_dir).unwrap();
        let source_path = source_dir.join("output_log_2025-04-30.txt");
        let first_log = "\
2025.04.30 20:00:00 Log        -  [VRC Camera] Took screenshot to: C:\\Users\\test\\Pictures\\VRChat\\preserved_1920x1080.png
2025.04.30 20:01:00 Log        -  [Behaviour] Entering Room: Preserved World
2025.04.30 20:01:01 Log        -  [Behaviour] Joining wrld_preserved:111~public~region(jp)
";
        fs::write(&source_path, first_log).unwrap();
        crate::commands::archive::sync_source_logs_into_archive_store(&source_dir, &archive_dir)
            .unwrap();
        run_diff_import(&db_path, &archive_dir, &AtomicBool::new(false), |_, _| {}).unwrap();

        let original_session_id: i64 = {
            let conn = Connection::open(&db_path).unwrap();
            let session_id = conn
                .query_row("SELECT id FROM sessions", [], |row| row.get(0))
                .unwrap();
            conn.execute_batch(
                "CREATE TRIGGER reject_reimported_visit
                 BEFORE INSERT ON visits
                 BEGIN
                     SELECT RAISE(ABORT, 'forced reimport failure');
                 END;",
            )
            .unwrap();
            session_id
        };

        fs::write(
            &source_path,
            format!(
                "{first_log}\
2025.04.30 20:10:00 Log        -  [Behaviour] Entering Room: Rejected World
2025.04.30 20:10:01 Log        -  [Behaviour] Joining wrld_rejected:222~public~region(us)
"
            ),
        )
        .unwrap();
        crate::commands::archive::sync_source_logs_into_archive_store(&source_dir, &archive_dir)
            .unwrap();

        let summary =
            run_diff_import(&db_path, &archive_dir, &AtomicBool::new(false), |_, _| {}).unwrap();
        assert_eq!(summary.total_count, 1);
        assert_eq!(summary.failed_filenames, vec!["output_log_2025-04-30.txt"]);

        let conn = Connection::open(&db_path).unwrap();
        let restored_session_id: i64 = conn
            .query_row("SELECT id FROM sessions", [], |row| row.get(0))
            .unwrap();
        let worlds: Vec<String> = conn
            .prepare("SELECT world_name FROM visits ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        let tracking_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM imported_logs", [], |row| row.get(0))
            .unwrap();
        let (screenshot_count, screenshot_session_id, screenshot_visit_id, screenshot_path): (
            i64,
            Option<i64>,
            Option<i64>,
            String,
        ) = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM screenshots),
                    session_id,
                    visit_id,
                    file_path
                 FROM screenshots",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(restored_session_id, original_session_id);
        assert_eq!(worlds, vec!["Preserved World"]);
        assert_eq!(tracking_count, 1);
        assert_eq!(screenshot_count, 1);
        assert_eq!(screenshot_session_id, Some(original_session_id));
        assert_eq!(screenshot_visit_id, None);
        assert!(screenshot_path.ends_with("preserved_1920x1080.png"));
    }

    #[test]
    fn diff_import_commits_successful_files_before_returning_failure_summary() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let archive_dir = dir.path().join("archive");
        let db_path = dir.path().join("stellarecord.db");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(
            source_dir.join("output_log_2025-04-30.txt"),
            "\
2025.04.30 20:00:00 Log        -  [VRC Camera] Took screenshot to: C:\\Users\\test\\Pictures\\VRChat\\saved_1920x1080.png
",
        )
        .unwrap();
        fs::write(
            source_dir.join("output_log_2025-05-01.txt"),
            "\
2025.05.01 20:00:00 Log        -  [Behaviour] Entering Room: Rejected World
2025.05.01 20:00:01 Log        -  [Behaviour] Joining wrld_rejected:222~public~region(us)
",
        )
        .unwrap();
        crate::commands::archive::sync_source_logs_into_archive_store(&source_dir, &archive_dir)
            .unwrap();

        {
            let conn = Connection::open(&db_path).unwrap();
            init_main_db(&conn).unwrap();
            conn.execute_batch(
                "CREATE TRIGGER reject_imported_visit
                 BEFORE INSERT ON visits
                 BEGIN
                     SELECT RAISE(ABORT, 'forced import failure');
                 END;",
            )
            .unwrap();
        }

        let summary =
            run_diff_import(&db_path, &archive_dir, &AtomicBool::new(false), |_, _| {}).unwrap();
        assert_eq!(summary.total_count, 2);
        assert_eq!(summary.failed_filenames, vec!["output_log_2025-05-01.txt"]);

        let conn = Connection::open(&db_path).unwrap();
        let log_names: Vec<String> = conn
            .prepare("SELECT log_name FROM sessions ORDER BY log_name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        let (tracking_count, screenshot_count, visit_count): (i64, i64, i64) = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM imported_logs),
                    (SELECT COUNT(*) FROM screenshots),
                    (SELECT COUNT(*) FROM visits)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(log_names, vec!["output_log_2025-04-30.txt"]);
        assert_eq!(tracking_count, 1);
        assert_eq!(screenshot_count, 1);
        assert_eq!(visit_count, 0);
    }

    #[test]
    fn collect_log_files_finds_matching_archives() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("output_log_2025-04-30.txt.tar.zst"),
            b"dummy",
        )
        .unwrap();
        fs::write(
            dir.path().join("output_log_2025-05-01.txt.tar.zst"),
            b"dummy",
        )
        .unwrap();
        fs::write(dir.path().join("random_file.tar.zst"), b"dummy").unwrap();
        fs::write(dir.path().join("output_log_2025-05-02.txt"), b"dummy").unwrap();

        let files = collect_log_files(dir.path());
        assert_eq!(files.len(), 2);
        assert!(files[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("04-30"));
        assert!(files[1]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("05-01"));
    }

    #[test]
    fn collect_log_files_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(collect_log_files(dir.path()).is_empty());
    }

    #[test]
    fn collect_log_files_nonexistent_dir() {
        let path = Path::new("/nonexistent/stellarecord/test/dir");
        assert!(collect_log_files(path).is_empty());
    }

    include!("friendship_tests.rs");

    // 通知本文中の偽イベントが、既に取り込んだセッション・訪問・参加者を変更しないことを確認する。
    fn assert_multiline_notification_context(
        conn: &Connection,
        log_name: &str,
        expected_end: &str,
        expected_participant_leave: &str,
        expected_notifications: i64,
    ) {
        let counts: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = conn.query_row(
            "SELECT (SELECT COUNT(*) FROM sessions), (SELECT COUNT(*) FROM visits),
                    (SELECT COUNT(*) FROM find_users), (SELECT COUNT(*) FROM with_users),
                    (SELECT COUNT(*) FROM notifications), (SELECT COUNT(*) FROM screenshots),
                    (SELECT COUNT(*) FROM osc), (SELECT COUNT(*) FROM subscription),
                    (SELECT COUNT(*) FROM apps), (SELECT COUNT(*) FROM imported_logs)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?,
                      row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?)),
        ).unwrap();
        assert_eq!(counts, (1, 1, 1, 1, expected_notifications, 0, 0, 0, 0, 0));

        let session: (i64, String, String, String, String, String) = conn.query_row(
            "SELECT id, log_name, account_id, account_name, start_time, end_time FROM sessions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        ).unwrap();
        assert_eq!(session, (
            1, log_name.into(), "usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(),
            "GenuineSelf".into(), "2025-04-30 20:00:00".into(), expected_end.into(),
        ));

        let visit: (i64, i64, String, String, String, String, String, String) = conn.query_row(
            "SELECT id, session_id, world_name, instance_id, instance_type, region,
                    join_time, leave_time FROM visits",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,
                      row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
        ).unwrap();
        assert_eq!(visit, (
            1, 1, "実際のワールド".into(), "111".into(), "public".into(), "jp".into(),
            "2025-04-30 20:02:01".into(), expected_end.into(),
        ));

        let participant: (i64, i64, String, String, i64, i64, String, String) = conn.query_row(
            "SELECT w.id, w.visit_id, w.vrchat_id, u.account_name, w.is_self, w.friend_status,
                    w.join_time, w.leave_time FROM with_users w
             JOIN find_users u ON u.vrchat_id = w.vrchat_id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,
                      row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
        ).unwrap();
        assert_eq!(participant, (
            1, 1, "usr_1111-2222-3333-4444-555555555555".into(), "GenuineFriend".into(),
            0, 2, "2025-04-30 20:03:00".into(), expected_participant_leave.into(),
        ));
    }

    #[test]
    fn import_multiline_notification_preserves_text_and_ignores_embedded_events() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = r#"2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:01:00 Log        -  User Authenticated: GenuineSelf (usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee)
2025.04.30 20:02:00 Log        -  [Behaviour] Entering Room: 実際のワールド
2025.04.30 20:02:01 Log        -  [Behaviour] Joining wrld_aaa:111~public~region(jp)
2025.04.30 20:03:00 Log        -  [Behaviour] OnPlayerJoined GenuineFriend (usr_1111-2222-3333-4444-555555555555)
2025.04.30 20:04:00 Log        -  Received Notification: <Notification from username:通知送信者, sender user id:usr_2222-3333-4444-5555-666666666666 to usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee of type: group, id: not_1234-5678-abcd-ef01-234567890abc, created at: 04/30/2025 11:04:00 UTC, expires: 04/30/2025 12:04:00 UTC, message: "一行目の通知本文
2045.12.31 23:01:00 Log        -  [Behaviour] OnPlayerLeft GenuineFriend (usr_1111-2222-3333-4444-555555555555)
2045.12.31 23:02:00 Log        -  [Behaviour] Entering Room: 偽のワールド
2045.12.31 23:02:01 Log        -  [Behaviour] Joining wrld_deadbeef:999~private(usr_ffffffff-ffff-ffff-ffff-ffffffffffff)~region(us)
2045.12.31 23:03:00 Log        -  [Behaviour] OnPlayerJoined Ghost (usr_3333-4444-5555-6666-777777777777)
終端の本文">
2025.04.30 20:05:00 Log        -  [Behaviour] OnPlayerLeft GenuineFriend (usr_1111-2222-3333-4444-555555555555)
2025.04.30 20:06:00 Log        -  [Behaviour] OnLeftRoom
"#;
        let mut progress = Vec::new();
        parse_and_import_reader(
            &conn, make_reader(log), "multiline_notification.txt", &cancel,
            &mut |status, fraction| progress.push((status, fraction)),
        ).unwrap();

        assert_multiline_notification_context(
            &conn, "multiline_notification.txt", "2025-04-30 20:06:00", "2025-04-30 20:05:00", 1,
        );
        let identity: (i64, i64, String, String, String, String) = conn.query_row(
            "SELECT id, session_id, notif_id, notif_type, sender_user_id, sender_name FROM notifications",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        ).unwrap();
        assert_eq!(identity, (
            1, 1, "not_1234-5678-abcd-ef01-234567890abc".into(), "group".into(),
            "usr_2222-3333-4444-5555-666666666666".into(), "通知送信者".into(),
        ));
        let payload: (String, String, String) = conn.query_row(
            "SELECT message, created_at, received_at FROM notifications", [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();
        // 期待本文は入力から切り出さず、改行と偽イベントを含む独立literalで定義する。
        let expected_message = "一行目の通知本文\n\
2045.12.31 23:01:00 Log        -  [Behaviour] OnPlayerLeft GenuineFriend (usr_1111-2222-3333-4444-555555555555)\n\
2045.12.31 23:02:00 Log        -  [Behaviour] Entering Room: 偽のワールド\n\
2045.12.31 23:02:01 Log        -  [Behaviour] Joining wrld_deadbeef:999~private(usr_ffffffff-ffff-ffff-ffff-ffffffffffff)~region(us)\n\
2045.12.31 23:03:00 Log        -  [Behaviour] OnPlayerJoined Ghost (usr_3333-4444-5555-6666-777777777777)\n\
終端の本文";
        assert_eq!(payload, (
            expected_message.into(), "2025-04-30 11:04:00".into(), "2025-04-30 20:04:00".into(),
        ));
        let target: [Option<String>; 5] = conn.query_row(
            "SELECT target_world_name, target_instance_id, target_instance_type, target_owner,
                    target_region FROM notifications",
            [],
            |row| Ok([row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?]),
        ).unwrap();
        assert_eq!(target, [None, None, None, None, None]);
        assert_eq!(progress, [("ログを解析しています…".into(), String::new()), ("解析結果を保存しています…".into(), String::new())]);
    }

    #[test]
    fn import_unclosed_multiline_notification_at_eof_preserves_existing_rows() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = r#"2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:01:00 Log        -  User Authenticated: GenuineSelf (usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee)
2025.04.30 20:02:00 Log        -  [Behaviour] Entering Room: 実際のワールド
2025.04.30 20:02:01 Log        -  [Behaviour] Joining wrld_aaa:111~public~region(jp)
2025.04.30 20:03:00 Log        -  [Behaviour] OnPlayerJoined GenuineFriend (usr_1111-2222-3333-4444-555555555555)
2025.04.30 20:04:00 Log        -  Received Notification: <Notification from username:通知送信者, sender user id:usr_2222-3333-4444-5555-666666666666 to usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee of type: group, id: not_1234-5678-abcd-ef01-234567890abc, created at: 04/30/2025 11:04:00 UTC, expires: 04/30/2025 12:04:00 UTC, message: "未閉鎖の通知本文
2045.12.31 23:01:00 Log        -  [Behaviour] OnPlayerLeft GenuineFriend (usr_1111-2222-3333-4444-555555555555)
2045.12.31 23:02:00 Log        -  [Behaviour] Entering Room: 偽のワールド
2045.12.31 23:02:01 Log        -  [Behaviour] Joining wrld_deadbeef:999~private(usr_ffffffff-ffff-ffff-ffff-ffffffffffff)~region(us)
2045.12.31 23:03:00 Log        -  [Behaviour] OnPlayerJoined Ghost (usr_3333-4444-5555-6666-777777777777)
終端も未閉鎖"#;
        assert!(!log.ends_with('\n'));
        let mut progress = Vec::new();
        parse_and_import_reader(
            &conn, make_reader(log), "unclosed_notification.txt", &cancel,
            &mut |status, fraction| progress.push((status, fraction)),
        ).unwrap();

        // 未閉鎖通知を保存せず、最後の実ログ時刻で既存訪問と参加者だけを閉じる。
        assert_multiline_notification_context(
            &conn, "unclosed_notification.txt", "2025-04-30 20:04:00", "2025-04-30 20:04:00", 0,
        );
        assert_eq!(progress, [("ログを解析しています…".into(), String::new()), ("解析結果を保存しています…".into(), String::new())]);
    }

    #[test]
    fn account_switches_keep_self_and_pending_events_scoped_to_each_visit() {
        let conn = setup_test_db();
        let log = r#"2026.10.22 12:00:00 Log - User Authenticated: Alpha (usr_aaaa)
2026.10.22 12:01:00 Log - [Behaviour] Entering Room: First
2026.10.22 12:01:01 Log - [Behaviour] Joining wrld_abc:1~region(jp)
2026.10.22 12:01:02 Log - [Behaviour] OnPlayerJoined Alpha (usr_aaaa)
2026.10.22 12:01:03 Log - [Behaviour] OnPlayerJoined Beta (usr_bbbb)
2026.10.22 12:01:04 Log - [Behaviour] Initialized PlayerAPI "Alpha" is local
2026.10.22 12:02:00 Log - User Authenticated: Beta (usr_bbbb)
2026.10.22 12:02:01 Log - [VRC Camera] Took screenshot to: C:/Photo/switch_1920x1080.png
2026.10.22 12:03:00 Log - [Behaviour] Entering Room: Second
2026.10.22 12:03:01 Log - [Behaviour] Joining wrld_abc:2~region(eu)
2026.10.22 12:03:02 Log - [Behaviour] OnPlayerJoined Alpha (usr_aaaa)
2026.10.22 12:03:03 Log - [Behaviour] OnPlayerJoined Beta (usr_bbbb)
2026.10.22 12:03:04 Log - [Behaviour] Initialized PlayerAPI "Beta" is local
2026.10.22 12:04:00 Log - [Behaviour] OnLeftRoom
2026.10.22 12:05:00 Log - User Authenticated: Alpha (usr_aaaa)
2026.10.22 12:06:00 Log - [Behaviour] Entering Room: Third
2026.10.22 12:06:01 Log - [Behaviour] Joining wrld_abc:3~region(us)
2026.10.22 12:06:02 Log - [Behaviour] OnPlayerJoined Beta (usr_bbbb)
2026.10.22 12:06:03 Log - [Behaviour] OnPlayerJoined Alpha (usr_aaaa)
2026.10.22 12:06:04 Log - [Behaviour] Initialized PlayerAPI "Alpha" is local
"#;
        parse_and_import_reader(&conn, make_reader(log), "switched.txt", &AtomicBool::new(false), &mut |_, _| {}).unwrap();
        let actual: Vec<(String, String, i64)> = conn.prepare(
            "SELECT v.world_name, w.vrchat_id, w.is_self FROM with_users w
             JOIN visits v ON v.id=w.visit_id ORDER BY v.id,w.vrchat_id",
        ).unwrap().query_map([], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap().collect::<Result<_>>().unwrap();
        assert_eq!(actual, [
            ("First".into(),"usr_aaaa".into(),1),("First".into(),"usr_bbbb".into(),0),
            ("Second".into(),"usr_aaaa".into(),0),("Second".into(),"usr_bbbb".into(),1),
            ("Third".into(),"usr_aaaa".into(),1),("Third".into(),"usr_bbbb".into(),0),
        ]);
        let owner: (Option<String>,Option<String>) = conn.query_row("SELECT account_id,account_name FROM sessions",[],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(owner,(None,None));
        assert_eq!(conn.query_row("SELECT leave_time FROM visits WHERE world_name='First'",[],|row|row.get::<_,String>(0)).unwrap(),"2026-10-22 12:02:00");
        assert!(conn.query_row("SELECT visit_id FROM screenshots",[],|row|row.get::<_,Option<i64>>(0)).unwrap().is_none());
        let regions: Vec<String> = conn.prepare("SELECT region FROM visits ORDER BY id").unwrap().query_map([],|row|row.get(0)).unwrap().collect::<Result<_>>().unwrap();
        assert_eq!(regions,["jp","eu","us"]);
    }

    #[test]
    fn local_initialization_before_join_and_ambiguous_names_never_choose_first_user() {
        for authenticated in [false,true] {
            let conn = setup_test_db();
            let auth = if authenticated { "2026.10.22 12:00:00 Log - User Authenticated: Same (usr_bbbb)\n" } else { "" };
            let log = format!(r#"{auth}2026.10.22 12:01:00 Log - [Behaviour] Entering Room: Same names
2026.10.22 12:01:01 Log - [Behaviour] Joining wrld_abc:1~region(jp)
2026.10.22 12:01:02 Log - [Behaviour] Initialized PlayerAPI "Same" is local
2026.10.22 12:01:03 Log - [Behaviour] OnPlayerJoined Same (usr_aaaa)
2026.10.22 12:01:04 Log - [Behaviour] OnPlayerJoined Same (usr_bbbb)
"#);
            parse_and_import_reader(&conn,make_reader(&log),"same.txt",&AtomicBool::new(false),&mut |_,_|{}).unwrap();
            let ids: Vec<String> = conn.prepare("SELECT vrchat_id FROM with_users WHERE is_self=1").unwrap().query_map([],|row|row.get(0)).unwrap().collect::<Result<_>>().unwrap();
            assert_eq!(ids, if authenticated { vec!["usr_bbbb"] } else { vec![] });
        }
    }

    #[test]
    fn explicit_local_player_replaces_stale_authentication_without_marking_old_account() {
        let conn = setup_test_db();
        let log = r#"2026.10.22 12:00:00 Log - User Authenticated: Previous (usr_aaaa)
2026.10.22 12:01:00 Log - [Behaviour] Entering Room: Observed local
2026.10.22 12:01:01 Log - [Behaviour] Joining wrld_abc:1~region(jp)
2026.10.22 12:01:02 Log - [Behaviour] OnPlayerJoined Previous (usr_aaaa)
2026.10.22 12:01:03 Log - [Behaviour] OnPlayerJoined Current (usr_bbbb)
2026.10.22 12:01:04 Log - [Behaviour] Initialized PlayerAPI "Current" is local
"#;
        parse_and_import_reader(&conn,make_reader(log),"local.txt",&AtomicBool::new(false),&mut |_,_|{}).unwrap();
        let ids: Vec<String> = conn.prepare("SELECT vrchat_id FROM with_users WHERE is_self=1").unwrap().query_map([],|row|row.get(0)).unwrap().collect::<Result<_>>().unwrap();
        assert_eq!(ids,["usr_bbbb"]);
        assert!(conn.query_row("SELECT account_id FROM sessions",[],|row|row.get::<_,Option<String>>(0)).unwrap().is_none());
    }

    #[test]
    fn changed_parser_reimports_saved_log_and_preserves_launcher_then_skips_same_content() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("db.sqlite");
        let file = dir.path().join("output_log_region.txt");
        fs::write(&file,"2026.10.22 12:00:00 Log - [Behaviour] Entering Room: Region\n2026.10.22 12:00:01 Log - [Behaviour] Joining wrld_abc:1~region(jp)\n").unwrap();
        run_enhanced_import_batch(&db,std::slice::from_ref(&file),&AtomicBool::new(false),|_,_|{}).unwrap();
        {
            let conn = Connection::open(&db).unwrap();
            conn.execute("INSERT INTO apps(name,path) VALUES('Keep','C:/keep.exe')",[]).unwrap();
            conn.execute("UPDATE visits SET region=NULL",[]).unwrap();
            conn.execute("UPDATE imported_logs SET parser_version=?1",[CURRENT_PARSER_VERSION-1]).unwrap();
        }
        let content = fs::read(&file).unwrap();
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, "output_log_region.txt", content.as_slice()).unwrap();
        fs::write(file.with_extension("txt.tar.zst"), zstd::stream::encode_all(builder.into_inner().unwrap().as_slice(), 3).unwrap()).unwrap();
        let first=run_diff_import(&db,dir.path(),&AtomicBool::new(false),|_,_|{}).unwrap();
        assert_eq!(first.total_count,1);
        assert!(first.failed_filenames.is_empty());
        let conn=Connection::open(&db).unwrap();
        assert_eq!(conn.query_row("SELECT region FROM visits",[],|row|row.get::<_,String>(0)).unwrap(),"jp");
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM apps",[],|row|row.get::<_,i64>(0)).unwrap(),1);
        assert_eq!(conn.query_row("SELECT parser_version FROM imported_logs",[],|row|row.get::<_,i64>(0)).unwrap(),CURRENT_PARSER_VERSION);
        let visit_id: i64 = conn.query_row("SELECT id FROM visits",[],|row|row.get(0)).unwrap();
        drop(conn);
        let second=run_diff_import(&db,dir.path(),&AtomicBool::new(false),|_,_|{}).unwrap();
        assert_eq!(second.total_count,1);
        assert!(second.failed_filenames.is_empty());
        let conn=Connection::open(&db).unwrap();
        assert_eq!(conn.query_row("SELECT id FROM visits",[],|row|row.get::<_,i64>(0)).unwrap(),visit_id);
    }

    #[test]
    fn invalid_tar_payload_preserves_saved_history_and_same_file_can_be_repaired() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("current.sqlite");
        let source = dir.path().join("output_log_integrity.txt");
        let archived = source.with_extension("txt.tar.zst");
        let original = "2026.10.22 12:00:00 Log - [Behaviour] Entering Room: Kept\n2026.10.22 12:00:01 Log - [Behaviour] Joining wrld_abc:1~region(jp)\n";
        fs::write(&source, original).unwrap();
        run_enhanced_import_batch(&database,std::slice::from_ref(&source),&AtomicBool::new(false),|_,_|{}).unwrap();
        let conn = Connection::open(&database).unwrap();
        conn.execute("INSERT INTO apps(name,path) VALUES('Kept app','C:/kept.exe')",[]).unwrap();
        let saved: (i64,Vec<u8>) = conn.query_row("SELECT s.id,i.content_sha256 FROM sessions s JOIN imported_logs i ON i.log_name=s.log_name",[],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        drop(conn);
        for directory_entry in [false,true] {
            let mut header = tar::Header::new_gnu();
            header.set_path("output_log_integrity.txt").unwrap();
            header.set_mode(0o644);
            header.set_size(if directory_entry {0} else {original.len() as u64});
            if directory_entry { header.set_entry_type(tar::EntryType::Directory); }
            header.set_cksum();
            let mut body = header.as_bytes().to_vec();
            if !directory_entry { body.extend_from_slice(original.lines().next().unwrap().as_bytes()); }
            fs::write(&archived,zstd::stream::encode_all(body.as_slice(),3).unwrap()).unwrap();
            assert!(run_enhanced_import_batch(&database,std::slice::from_ref(&archived),&AtomicBool::new(false),|_,_|{}).is_err());
            let summary=run_diff_import(&database,dir.path(),&AtomicBool::new(false),|_,_|{}).unwrap();
            assert_eq!(summary.failed_filenames,["output_log_integrity.txt"]);
            let conn = Connection::open(&database).unwrap();
            assert_eq!(conn.query_row("SELECT s.id,i.content_sha256 FROM sessions s JOIN imported_logs i ON i.log_name=s.log_name",[],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,Vec<u8>>(1)?))).unwrap(),saved);
            assert_eq!(conn.query_row("SELECT world_name FROM visits",[],|row|row.get::<_,String>(0)).unwrap(),"Kept");
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM apps",[],|row|row.get::<_,i64>(0)).unwrap(),1);
        }
        let repaired = format!("{original}2026.10.22 12:02:00 Log - [Behaviour] Entering Room: Repaired\n2026.10.22 12:02:01 Log - [Behaviour] Joining wrld_abc:2~region(us)\n");
        let mut builder=tar::Builder::new(Vec::new());
        let mut header=tar::Header::new_gnu();
        header.set_size(repaired.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header,"output_log_integrity.txt",repaired.as_bytes()).unwrap();
        fs::write(&archived,zstd::stream::encode_all(builder.into_inner().unwrap().as_slice(),3).unwrap()).unwrap();
        run_enhanced_import_batch(&database,&[archived],&AtomicBool::new(false),|_,_|{}).unwrap();
        let conn = Connection::open(&database).unwrap();
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM visits",[],|row|row.get::<_,i64>(0)).unwrap(),2);
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM apps",[],|row|row.get::<_,i64>(0)).unwrap(),1);
    }
}
