// 実際のfile読込とTauriのRust listenerへのevent payloadを確認する。BrowserのIPC配信は別途検証する。
#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct NativeViewerChunk {
    session_id: String, timestamps: Vec<String>, levels: Vec<u8>, categories: Vec<u8>, raw_lines: Vec<String>, highlights: Vec<Option<NativeViewerHighlight>>,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct NativeViewerHighlight { start: usize, end: usize }

fn native_viewer_highlight_text(chunk: &NativeViewerChunk, index: usize) -> Option<String> {
    let range = chunk.highlights.get(index)?.as_ref()?;
    let line = chunk.raw_lines.get(index)?;
    let header = analyze::RE_LOG_HEADER.captures(line);
    let body = header.as_ref().and_then(|capture| capture.get(3)).map_or(line.as_str(), |value| value.as_str());
    Some(String::from_utf16_lossy(&body.encode_utf16().skip(range.start).take(range.end - range.start).collect::<Vec<_>>()))
}

fn native_viewer_json(value: impl serde::Serialize) -> Result<String, Box<dyn std::error::Error>> {
    use tauri::ipc::IpcResponse;
    match value.body()? {
        tauri::ipc::InvokeResponseBody::Json(json) => Ok(json),
        tauri::ipc::InvokeResponseBody::Raw(_) => Err("viewer診断がJSONではありません".into()),
    }
}

/// 所有fixtureの解除後も、受信payloadと実fileのbyte列を独立して再照合できるよう保存する。
fn save_native_viewer_boundary_evidence(
    name: &str,
    evidence: impl serde::Serialize,
    files: &[(&str, &[u8])],
) -> NativeCommandTestResult {
    if let Ok(root) = std::env::var("STELLA_STARTUP_TEST_EVIDENCE_DIR") {
        let output = std::path::Path::new(&root).join(name);
        fs::create_dir(&output)?;
        for (file_name, contents) in files { fs::write(output.join(file_name), contents)?; }
        fs::write(output.join("evidence.json"), native_viewer_json(evidence)?)?;
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct NativeViewerReadRetryEvidence {
    root: String,
    registry_key: String,
    archive_path: String,
    metadata: Vec<(String, String, String)>,
    failed: Vec<NativeViewerChunk>,
    recovered: Vec<NativeViewerChunk>,
    settings_before: String,
    settings_after: String,
    read_failure_warning: bool,
    database_bytes_preserved: bool,
    unrelated_source_bytes_preserved: bool,
}

/// 先行frameは完全に解凍でき、次のframeの読込だけが失敗する独立した入力を作る。
fn native_viewer_archive_fixture(name: &str, contents: &str, damaged: bool) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Read;
    let mut header = tar::Header::new_gnu();
    header.set_path(name)?;
    header.set_mode(0o644);
    header.set_size(u64::try_from(contents.len())? + if damaged { 128 } else { 0 });
    header.set_cksum();
    let tar_bytes = if damaged {
        let mut bytes = header.as_bytes().to_vec();
        bytes.extend_from_slice(contents.as_bytes());
        bytes
    } else {
        let mut builder = tar::Builder::new(Vec::new());
        builder.append_data(&mut header, name, contents.as_bytes())?;
        builder.into_inner()?
    };
    let mut encoded = zstd::stream::encode_all(std::io::Cursor::new(&tar_bytes), 3)?;
    if damaged { encoded.extend_from_slice(b"invalid subsequent zstd frame"); }
    let mut decoded = Vec::new();
    let result = zstd::stream::Decoder::new(std::io::Cursor::new(&encoded))?.read_to_end(&mut decoded);
    assert_eq!(decoded, tar_bytes);
    assert_eq!(result.is_err(), damaged);
    Ok(encoded)
}

#[test]
fn command_viewer_partial_tar_read_failure_preserves_chunks_and_retries_same_file() -> NativeCommandTestResult {
    native_command_test("command_viewer_partial_tar_read_failure_preserves_chunks_and_retries_same_file", |directory, registry| {
        let name = "output_log_2026-10-10_12-00-00.txt";
        let file_name = format!("{name}.tar.zst");
        let store = get_archive_store_dir()?;
        fs::create_dir_all(&store)?;
        let archive_path = store.join(&file_name);
        let rows = (0..501).map(|index| format!("2026.10.10 12:00:00 Log - 途中読込{index} 日本語")).collect::<Vec<_>>();
        let contents = format!("{}\n", rows.join("\n"));
        let damaged = native_viewer_archive_fixture(name, &contents, true)?;
        fs::write(&archive_path, &damaged)?;
        let database = get_db_path()?;
        let conn = Connection::open(&database)?;
        analyze::init_main_db(&conn)?;
        drop(conn);
        let database_before = fs::read(&database)?;
        let source = get_source_log_dir()?;
        fs::create_dir_all(&source)?;
        let unrelated = source.join("output_log_unrelated.txt");
        fs::write(&unrelated, b"unrelated source must remain intact")?;
        let settings_before = native_viewer_json((config::load_stellarecord_setting(), config::load_polaris_setting()))?;
        let (app, _) = native_import_app()?;
        let events = NativeViewerEvents::listen(&app);
        let mut metadata = Vec::new();
        let mut failed = Vec::new();
        for session in ["managed-damaged", "external-damaged"] {
            let meta = if session == "managed-damaged" {
                archive::read_archive_log_viewer(file_name.clone(), session.to_string(), app.handle().clone())?
            } else {
                archive::read_external_log_viewer(archive_path.display().to_string(), session.to_string(), app.handle().clone())?
            };
            assert_eq!(meta.session_id, session);
            assert_eq!(meta.source_name, name);
            let chunks = events.finish(&[session])?;
            assert_eq!(chunks.iter().map(|chunk| chunk.raw_lines.len()).collect::<Vec<_>>(), [500, 1]);
            assert_eq!(chunks.iter().flat_map(|chunk| chunk.raw_lines.iter()).collect::<Vec<_>>(), rows.iter().collect::<Vec<_>>());
            for chunk in &chunks {
                assert_eq!(chunk.timestamps, vec!["2026-10-10 12:00:00"; chunk.raw_lines.len()]);
                assert_eq!(chunk.levels, vec![1; chunk.raw_lines.len()]);
                assert_eq!(chunk.categories, vec![0; chunk.raw_lines.len()]);
                assert_eq!(chunk.highlights, vec![None; chunk.raw_lines.len()]);
            }
            metadata.push((meta.session_id, meta.archive_name, meta.source_name));
            failed.extend(chunks);
            assert_eq!(fs::read(&archive_path)?, damaged);
        }
        let log_dir = utils::get_stellarecord_data_dir("logs").ok_or("ログdirectoryがありません")?;
        let logged = fs::read_to_string(fs::read_dir(log_dir)?.next().ok_or("ログfileがありません")??.path())?;
        assert_eq!(logged.matches("ログビューア行を読み取れませんでした:").count(), 2);

        // 同じpathだけを修復し、同じAppで両方の入口から全文を読み直す。
        let recovered_rows = rows.iter().cloned().chain(["2026.10.10 12:00:00 Log - 修復後の最終行".to_string()]).collect::<Vec<_>>();
        let recovered_contents = format!("{}\n", recovered_rows.join("\n"));
        let repaired = native_viewer_archive_fixture(name, &recovered_contents, false)?;
        fs::write(&archive_path, &repaired)?;
        let mut recovered = Vec::new();
        for session in ["managed-repaired", "external-repaired"] {
            let meta = if session == "managed-repaired" {
                archive::read_archive_log_viewer(file_name.clone(), session.to_string(), app.handle().clone())?
            } else {
                archive::read_external_log_viewer(archive_path.display().to_string(), session.to_string(), app.handle().clone())?
            };
            assert_eq!(meta.session_id, session);
            assert_eq!(meta.source_name, name);
            let chunks = events.finish(&[session])?;
            assert_eq!(chunks.iter().map(|chunk| chunk.raw_lines.len()).collect::<Vec<_>>(), [500, 2]);
            assert_eq!(chunks.iter().flat_map(|chunk| chunk.raw_lines.iter()).collect::<Vec<_>>(), recovered_rows.iter().collect::<Vec<_>>());
            metadata.push((meta.session_id, meta.archive_name, meta.source_name));
            recovered.extend(chunks);
            assert_eq!(fs::read(&archive_path)?, repaired);
        }
        let database_after = fs::read(&database)?;
        assert_eq!(database_after, database_before);
        assert!(!database.with_extension("sqlite-wal").exists());
        let unrelated_after = fs::read(&unrelated)?;
        assert_eq!(unrelated_after, b"unrelated source must remain intact");
        let settings_after = native_viewer_json((config::load_stellarecord_setting(), config::load_polaris_setting()))?;
        assert_eq!(settings_after, settings_before);
        let evidence = NativeViewerReadRetryEvidence { root: directory.path().display().to_string(), registry_key: registry.path.clone(), archive_path: archive_path.display().to_string(), metadata, failed, recovered, settings_before, settings_after, read_failure_warning: true, database_bytes_preserved: true, unrelated_source_bytes_preserved: true };
        save_native_viewer_boundary_evidence("stella-viewer-partial-read-retry", evidence, &[("damaged.tar.zst", &damaged), ("repaired.tar.zst", &repaired), ("database-before.sqlite", &database_before), ("database-after.sqlite", &database_after), ("unrelated-source.txt", &unrelated_after)])?;
        app.cleanup_before_exit();
        Ok(())
    })
}

#[derive(serde::Serialize)]
struct NativeViewerWalEvidence {
    root: String,
    registry_key: String,
    database_path: String,
    archive_path: String,
    chunks: Vec<NativeViewerChunk>,
    committed_world: String,
    uncommitted_world: String,
    committed_visit_count: i64,
    database_bytes_preserved: bool,
    wal_bytes_preserved: bool,
    settings_before: String,
    settings_after: String,
}

#[test]
fn command_viewer_active_wal_reads_committed_hints_without_checkpoint_or_dirty_read() -> NativeCommandTestResult {
    native_command_test("command_viewer_active_wal_reads_committed_hints_without_checkpoint_or_dirty_read", |directory, registry| {
        let name = "output_log_2026-10-08_12-00-00.txt";
        let committed_world = "WAL 内の確定ワールド";
        let uncommitted_world = "未確定の書込み";
        let contents = import_log().replace("Owned World", committed_world);
        let file_name = create_native_import_archive(name, &contents)?;
        let archive_path = get_archive_store_dir()?.join(&file_name);
        let archive_before = fs::read(&archive_path)?;
        let database = get_db_path()?;
        let conn = Connection::open(&database)?;
        analyze::init_main_db(&conn)?;
        conn.execute_batch("PRAGMA wal_autocheckpoint = 0; PRAGMA wal_checkpoint(TRUNCATE);")?;
        let database_checkpointed = fs::read(&database)?;
        conn.execute("INSERT INTO sessions (log_name, start_time) VALUES (?1, '2026-10-08 12:00:00')", [name])?;
        conn.execute("INSERT INTO visits (session_id, world_name, instance_id, instance_type, join_time, leave_time) VALUES (1, ?1, '123', 'public', '2026-10-08 12:01:00', '2026-10-08 12:05:00')", [committed_world])?;
        assert_eq!(fs::read(&database)?, database_checkpointed);
        let wal = database.with_extension("sqlite-wal");
        let wal_before = fs::read(&wal)?;
        assert!(wal_before.len() > 32);
        // 専用writerが未確定の更新を保持する間にも、別connectionから確定済みWALだけを読む。
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        conn.execute("UPDATE visits SET world_name = ?1 WHERE id = 1", [uncommitted_world])?;
        assert_eq!(conn.query_row("SELECT world_name FROM visits WHERE id = 1", [], |row| row.get::<_, String>(0))?, uncommitted_world);
        let settings_before = native_viewer_json((config::load_stellarecord_setting(), config::load_polaris_setting()))?;
        let (app, _) = native_import_app()?;
        let events = NativeViewerEvents::listen(&app);
        archive::read_archive_log_viewer(file_name, "active-wal".to_string(), app.handle().clone())?;
        let chunks = events.finish(&["active-wal"])?;
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].raw_lines, contents.lines().collect::<Vec<_>>());
        assert_eq!(chunks[0].categories, [0, 1, 0, 1]);
        assert_eq!(native_viewer_highlight_text(&chunks[0], 1).as_deref(), Some(committed_world));
        assert!((0..chunks[0].raw_lines.len()).all(|index| native_viewer_highlight_text(&chunks[0], index).as_deref() != Some(uncommitted_world)));
        let database_after = fs::read(&database)?;
        let wal_after = fs::read(&wal)?;
        assert_eq!(database_after, database_checkpointed);
        assert_eq!(wal_after, wal_before);
        assert_eq!(fs::read(&archive_path)?, archive_before);
        assert_eq!(fs::read_to_string(get_source_log_dir()?.join(name))?, contents);
        let settings_after = native_viewer_json((config::load_stellarecord_setting(), config::load_polaris_setting()))?;
        assert_eq!(settings_after, settings_before);
        conn.execute_batch("ROLLBACK;")?;
        let (stored_world, count): (String, i64) = conn.query_row("SELECT world_name, (SELECT count(*) FROM visits) FROM visits WHERE id = 1", [], |row| Ok((row.get(0)?, row.get(1)?)))?;
        assert_eq!(stored_world, committed_world);
        assert_eq!(count, 1);
        let evidence = NativeViewerWalEvidence { root: directory.path().display().to_string(), registry_key: registry.path.clone(), database_path: database.display().to_string(), archive_path: archive_path.display().to_string(), chunks, committed_world: stored_world, uncommitted_world: uncommitted_world.to_string(), committed_visit_count: count, database_bytes_preserved: true, wal_bytes_preserved: true, settings_before, settings_after };
        save_native_viewer_boundary_evidence("stella-viewer-active-wal", evidence, &[("database-before.sqlite", &database_checkpointed), ("database-after.sqlite", &database_after), ("database-before.sqlite-wal", &wal_before), ("database-after.sqlite-wal", &wal_after), ("archive.tar.zst", &archive_before), ("source.txt", contents.as_bytes())])?;
        app.cleanup_before_exit();
        drop(conn);
        Ok(())
    })
}
struct NativeViewerEvents { receiver: std::sync::mpsc::Receiver<(bool, String)> }

impl NativeViewerEvents {
    fn listen(app: &tauri::App) -> Self {
        use tauri::Listener;
        let (sender, receiver) = std::sync::mpsc::channel(); let chunk_sender = sender.clone();
        app.listen("log_viewer_chunk", move |event| { let _ = chunk_sender.send((false, event.payload().to_string())); });
        app.listen("log_viewer_done", move |event| { let _ = sender.send((true, event.payload().to_string())); }); Self { receiver }
    }

    fn finish(&self, sessions: &[&str]) -> Result<Vec<NativeViewerChunk>, Box<dyn std::error::Error>> {
        let mut chunks = Vec::new(); let mut finished = std::collections::HashSet::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while finished.len() < sessions.len() {
            let (done, payload) = self.receiver.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))?;
            if done {
                let session = tauri::ipc::InvokeResponseBody::Json(payload).deserialize::<String>()?;
                assert!(sessions.contains(&session.as_str())); assert!(finished.insert(session));
            } else {
                let chunk = tauri::ipc::InvokeResponseBody::Json(payload).deserialize::<NativeViewerChunk>()?;
                assert!(sessions.contains(&chunk.session_id.as_str())); assert!(!finished.contains(&chunk.session_id)); let count = chunk.raw_lines.len();
                assert_eq!(chunk.timestamps.len(), count); assert_eq!(chunk.levels.len(), count); assert_eq!(chunk.categories.len(), count); assert_eq!(chunk.highlights.len(), count);
                chunks.push(chunk);
            }
        }
        assert!(self.receiver.try_recv().is_err()); Ok(chunks)
    }
}

#[test]
fn command_viewer_archive_uses_real_saved_categories_and_highlights() -> NativeCommandTestResult {
    native_command_test("command_viewer_archive_uses_real_saved_categories_and_highlights", |_, _| {
        let name = "output_log_2026-10-08_12-00-00.txt"; let filename = create_native_import_archive(name, import_log())?;
        analyze::run_enhanced_import_batch(&get_db_path()?, &[get_archive_store_dir()?.join(&filename)], &std::sync::atomic::AtomicBool::new(false), |_, _| {})?;
        let (app, _) = native_import_app()?; let events = NativeViewerEvents::listen(&app);
        let meta = archive::read_archive_log_viewer(filename.clone(), "saved".to_string(), app.handle().clone())?;
        assert_eq!(meta.session_id, "saved"); assert_eq!(meta.archive_name, filename); assert_eq!(meta.source_name, name);
        let chunks = events.finish(&["saved"])?; assert_eq!(chunks.len(), 1); assert_eq!(chunks[0].raw_lines, import_log().lines().collect::<Vec<_>>());
        assert_eq!(chunks[0].categories, [0, 1, 0, 1]); assert_eq!(chunks[0].timestamps[2], "2026-10-08 12:01:01");
        assert_eq!(native_viewer_highlight_text(&chunks[0], 1).as_deref(), Some("Owned World")); assert_eq!(chunks[0].highlights[0], None); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_viewer_plain_streams_multiple_chunks_and_decodes_invalid_utf8() -> NativeCommandTestResult {
    native_command_test("command_viewer_plain_streams_multiple_chunks_and_decodes_invalid_utf8", |directory, _| {
        let path = directory.path().join("output_log_external.txt"); let mut bytes = Vec::new();
        for index in 0..1000 { bytes.extend_from_slice(format!("2026.10.08 12:00:00 Log - 行{index}\r\n").as_bytes()); }
        bytes.extend_from_slice(b"last \xff\r\n"); fs::write(&path, &bytes)?; let (app, _) = native_import_app()?; let events = NativeViewerEvents::listen(&app);
        let meta = archive::read_external_log_viewer(path.to_string_lossy().into_owned(), "plain".to_string(), app.handle().clone())?;
        assert_eq!(meta.archive_name, path.to_string_lossy()); assert_eq!(meta.source_name, "output_log_external.txt");
        let chunks = events.finish(&["plain"])?; assert_eq!(chunks.iter().map(|chunk| chunk.raw_lines.len()).collect::<Vec<_>>(), [500, 500, 1]);
        assert_eq!(chunks[0].raw_lines[0], "2026.10.08 12:00:00 Log - 行0"); assert_eq!(chunks[1].raw_lines[499], "2026.10.08 12:00:00 Log - 行999");
        assert_eq!(chunks[2].raw_lines, ["last �"]); assert_eq!(chunks[2].timestamps, [""]); assert_eq!(chunks[2].levels, [4]);
        assert!(chunks.iter().flat_map(|chunk| &chunk.highlights).all(Option::is_none)); assert_eq!(fs::read(path)?, bytes); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_viewer_headerless_rows_do_not_inherit_event_categories() -> NativeCommandTestResult {
    native_command_test("command_viewer_headerless_rows_do_not_inherit_event_categories", |directory, _| {
        let path = directory.path().join("output_log_ranges.txt");
        let contents = "2026.10.08 12:00:00 Log - [UserInfoLogger] Environment Info:\n    2026.10.08 12:00:01 Log - [Behaviour] OnPlayerJoined A (usr_1111-2222-3333-4444-555555555555)\n2026.10.08 12:00:02 Log - plain\n2026.10.08 12:00:03 Log - Received Notification: <Notification message=\"first\n    continued\n    final\">\n2026.10.08 12:00:04 Log - [Behaviour] OnPlayerJoined B (usr_2222-3333-4444-5555-666666666666)\n";
        fs::write(&path, contents)?; let (app, _) = native_import_app()?; let events = NativeViewerEvents::listen(&app);
        archive::read_external_log_viewer(path.to_string_lossy().into_owned(), "ranges".to_string(), app.handle().clone())?;
        let chunks = events.finish(&["ranges"])?; assert_eq!(chunks.len(), 1); assert_eq!(chunks[0].categories, [0, 0, 0, 2, 0, 0, 3]);
        assert_eq!(chunks[0].levels, [1, 4, 1, 1, 4, 4, 1]); assert!(chunks[0].highlights.iter().all(Option::is_none)); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_viewer_long_headerless_runs_keep_debug_classification() -> NativeCommandTestResult {
    native_command_test("command_viewer_long_headerless_runs_keep_debug_classification", |directory, _| {
        let path = directory.path().join("output_log_limits.txt"); let (app, _) = native_import_app()?; let events = NativeViewerEvents::listen(&app);
        for (header, body, limit, category, session) in [
            ("Received Notification: <Notification from username:a", "    plain", 200, 2, "notification-limit"),
            ("[UserInfoLogger] User Settings Info:", "    plain", 2000, 0, "debug-limit"),
        ] {
            let mut contents = format!("2026.10.08 12:00:00 Log - {header}\n"); for _ in 0..limit { contents.push_str(body); contents.push('\n'); } fs::write(&path, contents)?;
            archive::read_external_log_viewer(path.to_string_lossy().into_owned(), session.to_string(), app.handle().clone())?; let chunks = events.finish(&[session])?;
            let categories = chunks.iter().flat_map(|chunk| chunk.categories.iter().copied()).collect::<Vec<_>>(); assert_eq!(categories.len(), limit + 1);
            assert_eq!(categories[0], category); assert!(categories[1..].iter().all(|value| *value == 0));
        }
        app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_viewer_empty_file_and_external_compressed_archive_emit_exact_sessions() -> NativeCommandTestResult {
    native_command_test("command_viewer_empty_file_and_external_compressed_archive_emit_exact_sessions", |directory, _| {
        let empty = directory.path().join("output_log_empty.txt"); fs::write(&empty, b"")?; let (app, _) = native_import_app()?; let events = NativeViewerEvents::listen(&app);
        archive::read_external_log_viewer(empty.to_string_lossy().into_owned(), "empty".to_string(), app.handle().clone())?; assert!(events.finish(&["empty"])?.is_empty());
        let filename = create_native_import_archive("output_log_2026-10-08_12-00-00.txt", import_log())?; let path = get_archive_store_dir()?.join(filename);
        let meta = archive::read_external_log_viewer(path.to_string_lossy().into_owned(), "external-zst".to_string(), app.handle().clone())?;
        assert_eq!(meta.source_name, "output_log_2026-10-08_12-00-00.txt"); let chunks = events.finish(&["external-zst"])?;
        assert_eq!(chunks.len(), 1); assert_eq!(chunks[0].raw_lines.len(), 4); assert!(chunks[0].highlights.iter().all(Option::is_none)); assert!(!get_db_path()?.exists());
        app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_viewer_invalid_missing_and_corrupt_files_fail_before_streaming() -> NativeCommandTestResult {
    native_command_test("command_viewer_invalid_missing_and_corrupt_files_fail_before_streaming", |directory, _| {
        let store = get_archive_store_dir()?; fs::create_dir_all(&store)?; let broken = "output_log_broken.txt.tar.zst"; fs::write(store.join(broken), b"not an archive")?;
        let (app, _) = native_import_app()?; let events = NativeViewerEvents::listen(&app);
        for name in ["../output_log_external.txt.tar.zst", "output_log_missing.txt.tar.zst", broken] {
            assert!(archive::read_archive_log_viewer(name.to_string(), "invalid".to_string(), app.handle().clone()).is_err()); assert!(events.receiver.try_recv().is_err());
        }
        let unsupported = directory.path().join("keep.bin"); fs::write(&unsupported, b"keep")?; let log_directory = directory.path().join("output_log_directory.txt"); fs::create_dir(&log_directory)?;
        for path in [unsupported.clone(), directory.path().join("output_log_missing.txt"), log_directory, store.join(broken)] {
            assert!(archive::read_external_log_viewer(path.to_string_lossy().into_owned(), "invalid".to_string(), app.handle().clone()).is_err()); assert!(events.receiver.try_recv().is_err());
        }
        assert_eq!(fs::read(unsupported)?, b"keep"); assert_eq!(fs::read(store.join(broken))?, b"not an archive"); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_viewer_locked_file_reports_done_and_preserves_owned_file() -> NativeCommandTestResult {
    native_command_test("command_viewer_locked_file_reports_done_and_preserves_owned_file", |directory, _| {
        use std::os::windows::fs::OpenOptionsExt;
        let path = directory.path().join("output_log_locked.txt"); fs::write(&path, b"keep")?; let lock = fs::OpenOptions::new().read(true).share_mode(0).open(&path)?;
        let (app, _) = native_import_app()?; let events = NativeViewerEvents::listen(&app);
        archive::read_external_log_viewer(path.to_string_lossy().into_owned(), "locked".to_string(), app.handle().clone())?; assert!(events.finish(&["locked"])?.is_empty());
        drop(lock); assert_eq!(fs::read(path)?, b"keep"); let log_dir = utils::get_stellarecord_data_dir("logs").ok_or("ログdirectoryがありません")?;
        let logged = fs::read_to_string(fs::read_dir(log_dir)?.next().ok_or("ログfileがありません")??.path())?;
        assert!(logged.contains("ログビューアストリーム失敗:")); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_viewer_corrupt_database_falls_back_without_modifying_database() -> NativeCommandTestResult {
    native_command_test("command_viewer_corrupt_database_falls_back_without_modifying_database", |_, _| {
        let filename = create_native_import_archive("output_log_2026-10-08_12-00-00.txt", import_log())?; let database = get_db_path()?; fs::write(&database, b"not a database")?;
        let (app, _) = native_import_app()?; let events = NativeViewerEvents::listen(&app);
        archive::read_archive_log_viewer(filename, "fallback".to_string(), app.handle().clone())?; let chunks = events.finish(&["fallback"])?;
        assert_eq!(chunks.len(), 1); assert_eq!(chunks[0].categories, [0, 1, 0, 1]); assert!(chunks[0].highlights.iter().all(Option::is_none));
        assert_eq!(fs::read(database)?, b"not a database"); app.cleanup_before_exit(); Ok(())
    })
}

#[test]
fn command_viewer_concurrent_streams_keep_chunk_and_done_session_ids_separate() -> NativeCommandTestResult {
    native_command_test("command_viewer_concurrent_streams_keep_chunk_and_done_session_ids_separate", |directory, _| {
        let path = directory.path().join("output_log_concurrent.txt"); let contents = "2026.10.08 12:00:00 Log - line\n".repeat(1001); fs::write(&path, &contents)?;
        let (app, _) = native_import_app()?; let events = NativeViewerEvents::listen(&app);
        for session in ["old", "new"] { archive::read_external_log_viewer(path.to_string_lossy().into_owned(), session.to_string(), app.handle().clone())?; }
        let chunks = events.finish(&["old", "new"])?;
        for session in ["old", "new"] {
            let selected = chunks.iter().filter(|chunk| chunk.session_id == session).collect::<Vec<_>>(); assert_eq!(selected.len(), 3);
            assert_eq!(selected.iter().map(|chunk| chunk.raw_lines.len()).collect::<Vec<_>>(), [500, 500, 1]); assert_eq!(selected.iter().map(|chunk| chunk.raw_lines.len()).sum::<usize>(), 1001);
        }
        assert_eq!(fs::read_to_string(path)?, contents); app.cleanup_before_exit(); Ok(())
    })
}
