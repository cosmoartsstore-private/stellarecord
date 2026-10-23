/// `VRChat` 生ログ1件を単一エントリの `.tar.zst` アーカイブに圧縮する。
///
/// tar でラップすることで元のファイル名が保持され、圧縮後も後続の
/// インポートやログビューア処理がソース名を使い続けられる。
/// zstd 圧縮レベル 3 は圧縮速度と圧縮率のバランスに優れ、テキストログに対して
/// 十分な圧縮率を達成しつつリアルタイム同期を妨げない。
#[cfg(test)]
fn compress_single_file(src: &Path, dst: &Path) -> Result<(), String> {
    let output = fs::File::create(dst).map_err(|err| utils::command_create_err(dst, err))?;
    compress_single_file_to_output(src, output)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ── classify_text_category (純粋: DB ヒント不在時の行テキストからのカテゴリ推定) ──

    #[test]
    fn classify_text_category_detects_world() {
        assert_eq!(
            classify_text_category(
                "2025.05.15 20:00:00 Log        -  [Behaviour] Entering Room: Crystal Garden"
            ),
            Some("world")
        );
        assert_eq!(
            classify_text_category("2025.05.15 20:00:00 Log        -  [Behaviour] Joining wrld_abc12345-0000-4000-8000-000000000000:1~public~region(jp)"),
            None
        );
        assert_eq!(
            classify_text_category("2025.05.15 20:00:00 Log        -  [Behaviour] OnLeftRoom"),
            Some("world")
        );
    }

    #[test]
    fn classify_text_category_detects_players() {
        assert_eq!(
            classify_text_category("2025.05.15 20:00:00 Log        -  [Behaviour] OnPlayerJoined Comet (usr_11111111-2222-4333-8444-555555555555)"),
            Some("player_join")
        );
        assert_eq!(
            classify_text_category(
                "2025.05.15 20:00:00 Log        -  [Behaviour] OnPlayerJoinComplete Comet"
            ),
            None
        );
        assert_eq!(
            classify_text_category("2025.05.15 20:00:00 Log        -  [Behaviour] OnPlayerLeft Comet (usr_11111111-2222-4333-8444-555555555555)"),
            Some("player_left")
        );
    }

    #[test]
    fn classify_text_category_detects_notification() {
        assert_eq!(
            classify_text_category(
                "2025.05.15 20:00:00 Log        -  Received Notification: <Notification from username:Comet ...>"
            ),
            Some("notification")
        );
    }

    #[test]
    fn classify_text_category_returns_none_for_plain_lines() {
        assert_eq!(
            classify_text_category(
                "2025.05.15 20:00:00 Log        -  [Behaviour] Successfully joined room"
            ),
            Some("world")
        );
        assert_eq!(
            classify_text_category(
                "2025.05.15 20:00:00 Debug      -  Odin Serializer initialization"
            ),
            None
        );
    }

    // ── classify_log_level ──

    #[test]
    fn classify_plain_line() {
        assert_eq!(
            classify_log_level("2025.04.30 20:00:00 Log        -  some message"),
            "info"
        );
    }

    #[test]
    fn classify_error_line() {
        assert_eq!(
            classify_log_level("2025.04.30 20:00:00 Error      -  something broke"),
            "error"
        );
        assert_eq!(classify_log_level("some Error in the line"), "debug");
    }

    #[test]
    fn classify_warning_line() {
        assert_eq!(
            classify_log_level("2025.04.30 20:00:00 Warning    -  degraded"),
            "warning"
        );
        assert_eq!(classify_log_level("some Warning about X"), "debug");
    }

    #[test]
    fn classify_debug_line() {
        assert_eq!(
            classify_log_level("2025.04.30 20:00:00 Debug      -  internal"),
            "debug"
        );
        assert_eq!(
            classify_log_level("[UserInfoLogger] Environment Info: stuff"),
            "debug"
        );
        assert_eq!(
            classify_log_level("[UserInfoLogger] User Settings Info: stuff"),
            "debug"
        );
        assert_eq!(classify_log_level("Microphones installed (3)"), "debug");
    }

    // ── collect_source_logs ──

    #[test]
    fn collect_source_logs_filters_correctly() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("output_log_2025-04-30.txt"), b"log1").unwrap();
        fs::write(dir.path().join("output_log_2025-05-01.txt"), b"log2").unwrap();
        fs::write(dir.path().join("other_file.txt"), b"nope").unwrap();
        fs::write(dir.path().join("output_log_2025-05-02.log"), b"wrong ext").unwrap();

        let logs = collect_source_logs(dir.path()).unwrap();
        assert_eq!(logs.len(), 2);
        assert!(logs[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("04-30"));
        assert!(logs[1]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("05-01"));
    }

    // ── file name validation (IPC 境界: パストラバーサル防止) ──

    #[test]
    fn managed_archive_name_rejects_path_traversal() {
        assert!(matches_managed_archive_name(
            "output_log_2025-04-30.txt.tar.zst"
        ));
        assert!(!matches_managed_archive_name(
            "../output_log_2025-04-30.txt.tar.zst"
        ));
        assert!(!matches_managed_archive_name(
            "output_log_..\\..\\secret.txt.tar.zst"
        ));
        assert!(!matches_managed_archive_name(
            "C:\\temp\\output_log_a.txt.tar.zst"
        ));
        assert!(!matches_managed_archive_name(
            "output_log_2025-04-30.tar.zst"
        ));
    }

    #[test]
    fn source_log_name_rejects_path_traversal() {
        assert!(matches_source_log_file_name("output_log_2025-04-30.txt"));
        assert!(!matches_source_log_file_name(
            "../output_log_2025-04-30.txt"
        ));
        assert!(!matches_source_log_file_name(
            "output_log_..\\..\\secret.txt"
        ));
        assert!(!matches_source_log_file_name("C:\\temp\\output_log_a.txt"));
        assert!(!matches_source_log_file_name("output_log_2025-04-30.log"));
    }

    #[test]
    fn resolve_managed_archive_path_keeps_path_inside_archive_dir() {
        let dir = tempfile::tempdir().unwrap();
        let resolved =
            resolve_managed_archive_path(dir.path(), "output_log_2025-04-30.txt.tar.zst").unwrap();
        assert_eq!(
            resolved,
            dir.path().join("output_log_2025-04-30.txt.tar.zst")
        );
        assert!(
            resolve_managed_archive_path(dir.path(), "output_log_..\\..\\secret.txt.tar.zst")
                .is_err()
        );
    }

    #[test]
    fn collect_source_logs_returns_sorted() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("output_log_2025-05-03.txt"), b"c").unwrap();
        fs::write(dir.path().join("output_log_2025-05-01.txt"), b"a").unwrap();
        fs::write(dir.path().join("output_log_2025-05-02.txt"), b"b").unwrap();

        let logs = collect_source_logs(dir.path()).unwrap();
        let names: Vec<_> = logs
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert_eq!(
            names,
            vec![
                "output_log_2025-05-01.txt",
                "output_log_2025-05-02.txt",
                "output_log_2025-05-03.txt",
            ]
        );
    }

    // ── compress_single_file ──

    #[test]
    fn compress_and_verify_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("output_log_2025-04-30.txt");
        let dst = dir.path().join("output_log_2025-04-30.txt.tar.zst");

        let original_content = "2025.04.30 20:00:00 Log - VRChat starting\n2025.04.30 20:01:00 Log - User Authenticated\n";
        fs::write(&src, original_content).unwrap();

        compress_single_file(&src, &dst).unwrap();
        assert!(dst.exists());
        assert!(dst.metadata().unwrap().len() > 0);

        let archive_file = fs::File::open(&dst).unwrap();
        let decoder = zstd::stream::Decoder::new(archive_file).unwrap();
        let mut archive = tar::Archive::new(decoder);
        let mut entries = archive.entries().unwrap();
        let entry = entries.next().unwrap().unwrap();

        assert_eq!(
            entry.header().size().unwrap(),
            original_content.len() as u64
        );
        let entry_path = entry.path().unwrap();
        assert_eq!(entry_path.to_str().unwrap(), "output_log_2025-04-30.txt");
    }

    #[test]
    fn safe_create_failure_leaves_no_final_or_temp_archive() {
        let dir = tempfile::tempdir().unwrap();
        let missing_source = dir.path().join("output_log_missing.txt");
        let target = dir.path().join("output_log_missing.txt.tar.zst");

        let result = write_archive_safely(&missing_source, &target, ArchiveSyncMode::Create);

        assert!(result.is_err());
        assert!(!target.exists(), "失敗時に最終名を残してはならない");
        let remaining_names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            remaining_names.is_empty(),
            "失敗した一時アーカイブも除去される"
        );
    }

    #[test]
    fn safe_replace_failure_preserves_existing_archive() {
        let dir = tempfile::tempdir().unwrap();
        let missing_source = dir.path().join("output_log_missing.txt");
        let target = dir.path().join("output_log_missing.txt.tar.zst");
        fs::write(&target, b"existing archive").unwrap();

        let result = write_archive_safely(&missing_source, &target, ArchiveSyncMode::Replace);

        assert!(result.is_err());
        assert_eq!(fs::read(&target).unwrap(), b"existing archive");
    }

    fn assert_single_archived_log(path: &Path, source_name: &str, expected: &[u8]) {
        let decoder = zstd::stream::Decoder::new(fs::File::open(path).unwrap()).unwrap();
        let mut archive = tar::Archive::new(decoder); let mut entries = archive.entries().unwrap();
        let mut entry = entries.next().unwrap().unwrap();
        assert_eq!(entry.path().unwrap().to_str(), Some(source_name)); assert_eq!(entry.header().size().unwrap(), expected.len() as u64);
        let mut actual = Vec::new(); entry.read_to_end(&mut actual).unwrap(); assert_eq!(actual, expected); drop(entry); assert!(entries.next().is_none());
    }

    #[cfg(windows)]
    #[test]
    fn safe_replace_native_file_locks_preserve_archive_and_same_paths_retry() {
        use std::os::windows::fs::OpenOptionsExt;
        // 元ログと置換先の排他ロックを解除した後、同じパスで再実行できることを確認する。
        for locked_kind in ["source", "target"] {
            let dir = tempfile::tempdir().unwrap(); let source_name = "output_log_2026-10-08.txt";
            let source = dir.path().join(source_name); let target = dir.path().join(format!("{source_name}.tar.zst")); let backup = target.with_extension("bak");
            fs::write(&source, "保存済みの日本語ログ\n").unwrap(); compress_single_file(&source, &target).unwrap();
            let previous = fs::read(&target).unwrap(); let expected = "保存済みの日本語ログ\n追記済みのログ\n".as_bytes(); fs::write(&source, expected).unwrap();
            let backup_bytes = b"retained previous backup";
            let locked_path = match locked_kind { "source" => &source, "target" => &target, "backup" => { fs::write(&backup, backup_bytes).unwrap(); &backup }, _ => unreachable!() };
            let mut locked = fs::OpenOptions::new().read(true).share_mode(0).open(locked_path).unwrap();
            let error = write_archive_safely(&source, &target, ArchiveSyncMode::Replace).unwrap_err();
            let prefix = match locked_kind {
                "source" => format!("開けませんでした [{}]: ", source.display()),
                "target" => format!("開けませんでした [{}]: ", target.display()),
                "backup" => format!("削除できませんでした [{}]: ", backup.display()),
                _ => unreachable!(),
            };
            assert!(error.starts_with(&prefix), "{locked_kind}: {error}");
            let mut held_bytes = Vec::new(); locked.read_to_end(&mut held_bytes).unwrap();
            assert_eq!(held_bytes.as_slice(), match locked_kind { "source" => expected, "target" => previous.as_slice(), "backup" => backup_bytes, _ => unreachable!() });
            let mut names: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect(); names.sort();
            let mut expected_names = vec![source_name.to_owned(), format!("{source_name}.tar.zst")];
            if locked_kind == "backup" { expected_names.push(format!("{source_name}.tar.bak")); }
            expected_names.sort(); assert_eq!(names, expected_names, "失敗した一時archiveを残しています: {locked_kind}");
            drop(locked); assert_eq!(fs::read(&source).unwrap(), expected); assert_eq!(fs::read(&target).unwrap(), previous);
            if locked_kind == "backup" { assert_eq!(fs::read(&backup).unwrap(), backup_bytes); }
            write_archive_safely(&source, &target, ArchiveSyncMode::Replace).unwrap();
            assert_single_archived_log(&target, source_name, expected); assert_eq!(fs::read(&source).unwrap(), expected);
            assert!(!backup.exists()); assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
        }
    }

    #[test]
    fn failed_final_rename_restores_original_bytes_and_same_paths_retry() {
        let dir = tempfile::tempdir().unwrap(); let target = dir.path().join("archive.zst"); let temporary = dir.path().join("new.tmp"); let backup = target.with_extension("bak");
        fs::write(&target, "完全な保存済みarchive").unwrap();
        let error = replace_file_atomically(&temporary, &target).unwrap_err();
        assert!(error.starts_with(&format!("置き換えに失敗しました [{}]: ", target.display())));
        assert_eq!(fs::read(&target).unwrap(), "完全な保存済みarchive".as_bytes()); assert!(!backup.exists()); assert!(!temporary.exists());
        let expected = "同じpathで再試行する完全なarchive".as_bytes(); fs::write(&temporary, expected).unwrap(); replace_file_atomically(&temporary, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), expected); assert!(!temporary.exists()); assert!(!backup.exists()); assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn safe_create_final_directory_collision_keeps_source_and_existing_contents_then_retries() {
        let dir = tempfile::tempdir().unwrap(); let source_name = "output_log_collision.txt";
        let source = dir.path().join(source_name); let target = dir.path().join(format!("{source_name}.tar.zst")); let retained = target.join("retained.txt");
        let expected = "衝突後も保持する元log\n".as_bytes(); fs::write(&source, expected).unwrap(); fs::create_dir(&target).unwrap(); fs::write(&retained, b"retained").unwrap();
        let error = write_archive_safely(&source, &target, ArchiveSyncMode::Create).unwrap_err();
        assert!(error.starts_with(&format!("新規アーカイブを最終名へ移動できませんでした [{}]: ", target.display())));
        assert_eq!(fs::read(&source).unwrap(), expected); assert_eq!(fs::read(&retained).unwrap(), b"retained");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2); assert_eq!(fs::read_dir(&target).unwrap().count(), 1);
        // 所有fixtureの衝突directoryだけを空にして除去し、同じ保存先へ再試行する。
        fs::remove_file(&retained).unwrap(); fs::remove_dir(&target).unwrap(); write_archive_safely(&source, &target, ArchiveSyncMode::Create).unwrap();
        assert_single_archived_log(&target, source_name, expected); assert_eq!(fs::read(&source).unwrap(), expected); assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    // ── build_archive_sync_plan ──

    #[test]
    fn sync_plan_create_when_no_archive_exists() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("output_log_2025-04-30.txt");
        let archive_dir = dir.path().join("archive");
        fs::create_dir(&archive_dir).unwrap();
        fs::write(&src, "log content").unwrap();

        let plan = build_archive_sync_plan(&src, &archive_dir).unwrap();
        assert!(plan.is_some());
        assert!(matches!(plan.unwrap().mode, ArchiveSyncMode::Create));
    }

    #[test]
    fn sync_plan_skip_when_source_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("output_log_2025-04-30.txt");
        let archive_dir = dir.path().join("archive");
        fs::create_dir(&archive_dir).unwrap();

        let content = "same content in both";
        fs::write(&src, content).unwrap();
        compress_single_file(&src, &archive_dir.join("output_log_2025-04-30.txt.tar.zst")).unwrap();

        let plan = build_archive_sync_plan(&src, &archive_dir).unwrap();
        assert!(plan.is_none(), "identical source and archive should skip");
    }

    #[test]
    fn source_log_exactly_archived_accepts_identical_archive() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("output_log_2025-04-30.txt");
        let archive = dir.path().join("output_log_2025-04-30.txt.tar.zst");

        fs::write(&src, "same content in both").unwrap();
        compress_single_file(&src, &archive).unwrap();

        assert!(open_archived_source_for_cleanup(&src, &archive).unwrap().is_some());
    }

    #[test]
    fn source_log_exactly_archived_rejects_stale_archive() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("output_log_2025-04-30.txt");
        let archive = dir.path().join("output_log_2025-04-30.txt.tar.zst");

        fs::write(&src, "first line\n").unwrap();
        compress_single_file(&src, &archive).unwrap();
        fs::write(&src, "first line\nsecond line\n").unwrap();

        assert!(open_archived_source_for_cleanup(&src, &archive).unwrap().is_none());
    }

    #[test]
    fn sync_plan_replace_when_source_extended() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("output_log_2025-04-30.txt");
        let archive_dir = dir.path().join("archive");
        fs::create_dir(&archive_dir).unwrap();

        let short_content = "first line\n";
        fs::write(&src, short_content).unwrap();
        compress_single_file(&src, &archive_dir.join("output_log_2025-04-30.txt.tar.zst")).unwrap();

        let extended_content = "first line\nsecond line\n";
        fs::write(&src, extended_content).unwrap();

        let plan = build_archive_sync_plan(&src, &archive_dir).unwrap();
        assert!(plan.is_some());
        assert!(matches!(plan.unwrap().mode, ArchiveSyncMode::Replace));
    }

    #[test]
    fn sync_plan_skip_when_source_shorter() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("output_log_2025-04-30.txt");
        let archive_dir = dir.path().join("archive");
        fs::create_dir(&archive_dir).unwrap();

        let long_content = "long content here\n";
        fs::write(&src, long_content).unwrap();
        compress_single_file(&src, &archive_dir.join("output_log_2025-04-30.txt.tar.zst")).unwrap();

        fs::write(&src, "short").unwrap();

        let plan = build_archive_sync_plan(&src, &archive_dir).unwrap();
        assert!(
            plan.is_none(),
            "shorter source should be skipped for safety"
        );
    }

    // ── encode_log_level_u8 / encode_log_category_u8 (純粋: フロントエンド定数と同期) ──

    #[test]
    fn encode_level_mapping() {
        assert_eq!(encode_log_level_u8("info"), 1);
        assert_eq!(encode_log_level_u8("warning"), 2);
        assert_eq!(encode_log_level_u8("error"), 3);
        assert_eq!(encode_log_level_u8("debug"), 4);
        assert_eq!(encode_log_level_u8("plain"), 0);
        assert_eq!(encode_log_level_u8("unknown"), 0);
    }

    #[test]
    fn encode_category_mapping() {
        assert_eq!(encode_log_category_u8("world"), 1);
        assert_eq!(encode_log_category_u8("notification"), 2);
        assert_eq!(encode_log_category_u8("player_join"), 3);
        assert_eq!(encode_log_category_u8("player_ready"), 0);
        assert_eq!(encode_log_category_u8("player_left"), 4);
        assert_eq!(encode_log_category_u8("debug-system"), 0);
        assert_eq!(encode_log_category_u8("video"), 5);
        assert_eq!(encode_log_category_u8("photo"), 6);
        assert_eq!(encode_log_category_u8("plain"), 0);
    }

    #[test]
    fn event_categories_require_header_and_canonical_body() {
        for body in [
            "[Behaviour] OnPlayerJoinComplete StarGazer",
            "[Behaviour] Initialized PlayerAPI \"StarGazer\" is remote",
            "Fetched wallet balance from internal ledger. Balance=0",
            "[Behaviour] uSpeak [1][0]: ProcessEncodeAsync task cancelled (read).",
            "[Behaviour] Joining wrld_abc:1~region(jp)",
        ] {
            assert_eq!(classify_text_category(&format!("2026.10.22 12:00:00 Debug - {body}")), None);
            assert_eq!(classify_text_category(body), None);
        }
        assert_eq!(classify_text_category("2026.10.22 12:00:00 Log - [Behaviour] Successfully joined room"), Some("world"));
    }

    fn highlighted_text(body: &str, range: &LogViewerHighlight) -> String {
        String::from_utf16_lossy(&body.encode_utf16().skip(range.start).take(range.end - range.start).collect::<Vec<_>>())
    }

    #[test]
    fn db_highlight_matches_exact_field_and_prefers_notification_message() {
        let markers = vec![DbKeywordMarker { text: "Same Name".into() }, DbKeywordMarker { text: "Cozy Lounge".into() }];
        let body = "Received Notification: <Notification from username:Same Name, sender user id:usr_a to usr_b of type: invite, id: not_1234, created at: now, message: \"Same Name\">";
        let range = resolve_db_highlight(body, "notification", &markers).unwrap();
        assert_eq!(highlighted_text(body, &range), "Same Name");
        assert_eq!(range.start, body.rfind("Same Name").unwrap());
        assert!(resolve_db_highlight("[Behaviour] Entering Room: Cozy Lounge Annex", "world", &markers).is_none());
        let body = "[Behaviour] Entering Room: Cozy Lounge";
        assert_eq!(highlighted_text(body, &resolve_db_highlight(body, "world", &markers).unwrap()), "Cozy Lounge");
    }

    #[test]
    fn db_highlight_uses_utf16_offsets_and_never_matches_id_or_internal_name() {
        let markers = vec![DbKeywordMarker { text: "😀名前".into() }, DbKeywordMarker { text: "wrld_abcdef".into() }];
        let body = "[Behaviour] OnPlayerJoined 😀名前 (usr_1111-2222)";
        assert_eq!(highlighted_text(body, &resolve_db_highlight(body, "player_join", &markers).unwrap()), "😀名前");
        assert!(resolve_db_highlight("[Behaviour] Joining wrld_abcdef:1", "world", &markers).is_none());
        assert!(resolve_db_highlight("Fetched wallet balance 😀名前", "plain", &markers).is_none());
    }

    #[test]
    fn video_and_photo_highlights_do_not_need_database_markers() {
        let body = "[USharpVideo] Started video load for URL: https://example.com/watch?v=123 requested by Other";
        assert_eq!(classify_text_category(&format!("2026.10.22 12:00:00 Debug - {body}")), Some("video"));
        assert_eq!(highlighted_text(body, &resolve_video_highlight(body, false).unwrap()), "https://example.com/watch?v=123");
        let title = "[YTTL] title: 日本語の動画";
        assert_eq!(highlighted_text(title, &resolve_video_highlight(title, false).unwrap()), "日本語の動画");
        assert_eq!(classify_text_category("2026.10.22 12:00:00 Debug - [USharpVideo] Failed to load https://example.com/watch"), None);
        assert_eq!(classify_text_category("2026.10.22 12:00:00 Debug - [VRC Camera] Took screenshot to: C:/Photo/VRChat_2026_3840x2160.png"), Some("photo"));
    }

    #[test]
    fn truncated_archive_body_never_authorizes_source_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("output_log_truncated.txt");
        let archived = dir.path().join("output_log_truncated.txt.tar.zst");
        fs::write(&source, b"abcdefghijk").unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_path("output_log_truncated.txt").unwrap();
        header.set_size(11);
        header.set_mode(0o644);
        header.set_cksum();
        let mut incomplete = header.as_bytes().to_vec();
        incomplete.extend_from_slice(b"abc");
        fs::write(&archived, zstd::stream::encode_all(incomplete.as_slice(), 3).unwrap()).unwrap();
        assert!(open_archived_source_for_cleanup(&source, &archived).unwrap().is_none());
        assert_eq!(fs::read(&source).unwrap(), b"abcdefghijk");
    }

    #[test]
    fn replacement_rechecks_actual_staged_content_after_plan() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("output_log_changed.txt");
        let archived = dir.path().join("output_log_changed.txt.tar.zst");
        fs::write(&source, b"original log").unwrap();
        write_archive_safely(&source, &archived, ArchiveSyncMode::Create).unwrap();
        let original = fs::read(&archived).unwrap();
        fs::write(&source, b"original log extended").unwrap();
        let plan = build_archive_sync_plan(&source, dir.path()).unwrap().unwrap();
        fs::write(&source, b"different").unwrap();
        assert!(write_archive_safely(&source, &archived, plan.mode).is_err());
        assert_eq!(fs::read(&archived).unwrap(), original);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
        fs::write(&source, b"original log extended").unwrap();
        write_archive_safely(&source, &archived, plan.mode).unwrap();
        assert_single_archived_log(&archived, "output_log_changed.txt", b"original log extended");
    }

    #[test]
    fn cleanup_holds_verified_file_against_writes_and_rename_until_handle_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("output_log_held.txt");
        let archived = dir.path().join("output_log_held.txt.tar.zst");
        fs::write(&source, b"retained in archive").unwrap();
        write_archive_safely(&source, &archived, ArchiveSyncMode::Create).unwrap();
        let verified = open_archived_source_for_cleanup(&source, &archived).unwrap().unwrap();
        assert!(fs::OpenOptions::new().write(true).open(&source).is_err());
        assert!(fs::rename(&source, dir.path().join("renamed.txt")).is_err());
        delete_open_source_log(verified, &source).unwrap();
        assert!(!source.exists());
        assert_single_archived_log(&archived, "output_log_held.txt", b"retained in archive");
    }

    // ── collect_directory_size (一時ディレクトリ: 再帰サイズ集計) ──

    #[test]
    fn directory_size_sums_recursively() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"12345").unwrap(); // 5 bytes
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("b.txt"), b"678").unwrap(); // 3 bytes

        let total = collect_directory_size(dir.path()).unwrap();
        assert_eq!(total, 8);
    }

    #[test]
    fn directory_size_single_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("only.bin");
        fs::write(&file, vec![0u8; 100]).unwrap();
        assert_eq!(collect_directory_size(&file).unwrap(), 100);
    }

    // ── replace_file_atomically (一時ディレクトリ: 原子的置き換え) ──

    #[test]
    fn replace_file_swaps_content_and_cleans_backup() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("archive.tar.zst");
        let temp = dir.path().join("archive.tmp");
        fs::write(&target, b"old content").unwrap();
        fs::write(&temp, b"new content").unwrap();

        replace_file_atomically(&temp, &target).unwrap();

        assert_eq!(fs::read(&target).unwrap(), b"new content");
        assert!(!temp.exists(), "一時ファイルは消費されている");
        assert!(
            !target.with_extension("bak").exists(),
            "バックアップは削除されている"
        );
    }

    // ── read_archive_source_name (実圧縮ファイル: tar ヘッダー読み取り) ──

    #[test]
    fn read_source_name_from_archive() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("output_log_2025-04-30.txt");
        let archive = dir.path().join("output_log_2025-04-30.txt.tar.zst");
        fs::write(&src, "log content").unwrap();
        compress_single_file(&src, &archive).unwrap();

        let name = read_archive_source_name(&archive).unwrap();
        assert_eq!(name, "output_log_2025-04-30.txt");
    }

    // ── collect_pending_archive_sync_plans / sync_source_logs_into_archive_store ──

    #[test]
    fn pending_plans_for_new_logs() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let archive = dir.path().join("archive");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&archive).unwrap();
        fs::write(source.join("output_log_2025-04-30.txt"), "a").unwrap();
        fs::write(source.join("output_log_2025-05-01.txt"), "b").unwrap();

        let plans = collect_pending_archive_sync_plans(&source, &archive).unwrap();
        assert_eq!(plans.len(), 2, "未アーカイブのログ2件が計画される");
    }

    #[test]
    fn sync_creates_archives_and_counts() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let archive = dir.path().join("archive");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("output_log_2025-04-30.txt"), "content A").unwrap();

        let count = sync_source_logs_into_archive_store(&source, &archive).unwrap();
        assert_eq!(count, 1);
        assert!(archive.join("output_log_2025-04-30.txt.tar.zst").exists());

        // 2回目は既にアーカイブ済みなのでスキップ（0件）
        let count2 = sync_source_logs_into_archive_store(&source, &archive).unwrap();
        assert_eq!(count2, 0);
    }

    // ── collect_db_log_categories / collect_db_keyword_markers (インメモリ DB) ──

    fn seed_viewer_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        analyze::init_main_db(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO sessions (id, log_name, start_time) VALUES (1, 'log.txt', '2025-04-30');
             INSERT INTO visits (id, session_id, world_name, instance_id, instance_type, join_time, leave_time)
                VALUES (1, 1, 'Cozy Lounge', 'inst1', 'public', '2025.04.30 20:15:30', '2025.04.30 20:20:10');
             INSERT INTO find_users (vrchat_id, account_name) VALUES ('usr_a', 'StarGazer');
             INSERT INTO with_users (visit_id, vrchat_id, is_self, join_time, leave_time)
                VALUES (1, 'usr_a', 0, '2025.04.30 20:15:35', '2025.04.30 20:18:45');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn db_keyword_markers_are_sorted_and_unique() {
        let conn = seed_viewer_db();
        let markers = collect_db_keyword_markers(&conn, "log.txt").unwrap();

        // ワールド名とユーザー名が収集される
        assert!(markers.iter().any(|m| m.text == "Cozy Lounge"));
        assert!(markers.iter().any(|m| m.text == "StarGazer"));
        // 項目ごとの完全一致照合に使うため、文字列順で重複を除く。
        for pair in markers.windows(2) {
            assert!(pair[0].text < pair[1].text);
        }
    }

    #[test]
    fn db_keyword_markers_empty_for_unknown_session() {
        let conn = seed_viewer_db();
        let markers = collect_db_keyword_markers(&conn, "nonexistent.txt").unwrap();
        assert!(markers.is_empty());
    }
}
