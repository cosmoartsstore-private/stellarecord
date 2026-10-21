// 一意性、証拠の消費、訪問境界をtrackerと実際のparser・SQLite保存の両方で確認する。
mod friendship_contract {
    use super::*;

    fn active_tracker() -> FriendshipTracker {
        let mut tracker = FriendshipTracker::default(); tracker.on_player_join("対象", "usr_target"); tracker.on_avatar_switch("対象", "専用アバター"); tracker
    }

    fn infer(tracker: &mut FriendshipTracker, status: Option<FriendStatus>, line: usize) -> Option<(String, i64)> {
        tracker.on_avatar_download(status, line); tracker.on_avatar_unpack("専用アバター by 作者", line + 1)
    }

    #[test]
    fn log_values_accept_only_exact_zero_or_one() {
        assert_eq!(FriendStatus::from_log_value("0"), Some(FriendStatus::NonFriend)); assert_eq!(FriendStatus::from_log_value("1"), Some(FriendStatus::Friend));
        for value in ["", "2", "-1", " 1", "1 ", "01", "true", "Friend:1"] { assert_eq!(FriendStatus::from_log_value(value), None, "{value}"); }
    }

    #[test]
    fn unique_user_and_repeat_evidence_preserve_each_known_status() {
        for (status, expected) in [(FriendStatus::NonFriend, 0), (FriendStatus::Friend, 1)] {
            let mut tracker = active_tracker(); tracker.on_player_join("対象", "usr_target");
            assert_eq!(infer(&mut tracker, Some(status), 10), Some(("usr_target".to_string(), expected)));
            assert_eq!(infer(&mut tracker, Some(status), 20), Some(("usr_target".to_string(), expected)));
        }
    }

    #[test]
    fn contradictory_evidence_stays_unknown_until_new_visit() {
        let mut tracker = active_tracker(); assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 1), Some(("usr_target".to_string(), 1)));
        for status in [FriendStatus::NonFriend, FriendStatus::Friend, FriendStatus::NonFriend] {
            assert_eq!(infer(&mut tracker, Some(status), 10), Some(("usr_target".to_string(), 2)));
        }
        tracker.reset_visit(); tracker.on_player_join("対象", "usr_target"); tracker.on_avatar_switch("対象", "専用アバター");
        assert_eq!(infer(&mut tracker, Some(FriendStatus::NonFriend), 20), Some(("usr_target".to_string(), 0)));
    }

    #[test]
    fn download_unpack_gap_accepts_boundary_and_rejects_stale_or_reversed_order() {
        for (download, unpack, expected) in [(10, 10, true), (10, 13, true), (10, 14, false), (10, 9, false), (usize::MAX - 3, usize::MAX, true)] {
            let mut tracker = active_tracker(); tracker.on_avatar_download(Some(FriendStatus::Friend), download);
            assert_eq!(tracker.on_avatar_unpack("専用アバター by 作者", unpack), expected.then(|| ("usr_target".to_string(), 1)));
            assert_eq!(tracker.on_avatar_unpack("専用アバター by 作者", download), None);
        }
    }

    #[test]
    fn missing_or_unknown_download_is_consumed_without_guessing() {
        let mut tracker = active_tracker(); assert_eq!(tracker.on_avatar_unpack("専用アバター by 作者", 1), None);
        assert_eq!(infer(&mut tracker, None, 2), None); assert_eq!(tracker.on_avatar_unpack("専用アバター by 作者", 3), None);
        assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 4), Some(("usr_target".to_string(), 1)));
    }

    #[test]
    fn multiple_downloads_are_ambiguous_even_if_one_has_no_status() {
        for statuses in [[Some(FriendStatus::Friend), Some(FriendStatus::Friend), Some(FriendStatus::Friend)], [None, Some(FriendStatus::Friend), None]] {
            let mut tracker = active_tracker(); for (index, status) in statuses.into_iter().enumerate() { tracker.on_avatar_download(status, index); }
            assert_eq!(tracker.on_avatar_unpack("専用アバター by 作者", 3), None);
            assert_eq!(infer(&mut tracker, Some(FriendStatus::NonFriend), 5), Some(("usr_target".to_string(), 0)));
        }
    }

    #[test]
    fn unpack_without_matching_avatar_consumes_pending_download() {
        let mut tracker = active_tracker(); tracker.on_avatar_download(Some(FriendStatus::Friend), 1);
        assert_eq!(tracker.on_avatar_unpack("異なるアバター by 作者", 2), None);
        assert_eq!(tracker.on_avatar_unpack("専用アバター by 作者", 3), None);
        assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 4), Some(("usr_target".to_string(), 1)));
    }

    #[test]
    fn avatar_requires_exact_name_and_by_delimiter() {
        for payload in ["専用アバター", "専用アバター by", "専用アバター別名 by 作者", "prefix 専用アバター by 作者"] {
            let mut tracker = active_tracker(); tracker.on_avatar_download(Some(FriendStatus::Friend), 1); assert_eq!(tracker.on_avatar_unpack(payload, 2), None);
        }
    }

    #[test]
    fn overlapping_avatar_prefixes_do_not_choose_hashmap_order() {
        let mut tracker = FriendshipTracker::default(); tracker.on_player_join("一人目", "usr_one"); tracker.on_player_join("二人目", "usr_two");
        tracker.on_avatar_switch("一人目", "Avatar"); tracker.on_avatar_switch("二人目", "Avatar by Creator");
        tracker.on_avatar_download(Some(FriendStatus::Friend), 1); assert_eq!(tracker.on_avatar_unpack("Avatar by Creator by Other", 2), None);
    }

    #[test]
    fn shared_avatar_becomes_unique_only_after_other_display_leaves() {
        let mut tracker = active_tracker(); tracker.on_player_join("別の対象", "usr_other"); tracker.on_avatar_switch("別の対象", "専用アバター");
        assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 1), None); tracker.on_player_left("別の対象", "usr_other");
        assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 3), Some(("usr_target".to_string(), 1)));
    }

    #[test]
    fn duplicate_display_requires_fresh_avatar_after_one_user_leaves() {
        let mut tracker = active_tracker(); tracker.on_player_join("対象", "usr_other");
        assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 1), None); tracker.on_avatar_switch("対象", "専用アバター");
        assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 3), None); tracker.on_player_left("対象", "usr_other");
        assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 5), None); tracker.on_avatar_switch("対象", "専用アバター");
        assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 7), Some(("usr_target".to_string(), 1)));
    }

    #[test]
    fn avatar_switch_clears_old_mapping_and_keeps_latest_name() {
        let mut tracker = active_tracker(); tracker.on_avatar_switch("対象", "新アバター");
        assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 1), None); tracker.on_avatar_download(Some(FriendStatus::NonFriend), 3);
        assert_eq!(tracker.on_avatar_unpack("新アバター by 作者", 4), Some(("usr_target".to_string(), 0)));
    }

    #[test]
    fn missing_active_user_and_departure_do_not_reuse_avatar() {
        let mut tracker = FriendshipTracker::default(); tracker.on_avatar_switch("対象", "専用アバター"); assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 1), None);
        tracker.on_player_join("対象", "usr_target"); assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 3), Some(("usr_target".to_string(), 1)));
        tracker.on_player_left("対象", "usr_target"); tracker.on_player_left("未登録", "usr_missing"); assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 5), None);
    }

    #[test]
    fn visit_reset_discards_members_avatar_and_unfinished_download() {
        let mut tracker = active_tracker(); tracker.on_avatar_download(Some(FriendStatus::Friend), 1); tracker.reset_visit();
        assert_eq!(tracker.on_avatar_unpack("専用アバター by 作者", 2), None); tracker.on_player_join("対象", "usr_target");
        assert_eq!(infer(&mut tracker, Some(FriendStatus::Friend), 3), None); tracker.on_avatar_switch("対象", "専用アバター");
        assert_eq!(infer(&mut tracker, Some(FriendStatus::NonFriend), 5), Some(("usr_target".to_string(), 0)));
    }

    fn parse_friendship_lines(lines: &[&str]) -> Connection {
        use std::fmt::Write as _;
        let conn = setup_test_db();
        let mut log = "2026.10.08 12:00:00 Log - User Authenticated: Owner (usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee)\n2026.10.08 12:01:00 Log - [Behaviour] Entering Room: Test\n2026.10.08 12:01:01 Log - [Behaviour] Joining wrld_abc123:123~public~region(jp)\n".to_string();
        for line in lines { writeln!(log, "2026.10.08 12:02:00 Log - {line}").unwrap(); } log.push_str("2026.10.08 12:10:00 Log - [Behaviour] OnLeftRoom\n");
        parse_and_import_reader(&conn, make_reader(&log), "friendship.txt", &AtomicBool::new(false), &mut |_, _| {}).unwrap(); conn
    }

    fn friendship_rows(conn: &Connection) -> Vec<(String, i64, i64)> {
        let mut statement = conn.prepare("SELECT fu.account_name, wu.friend_status, wu.is_self FROM with_users wu JOIN find_users fu ON fu.vrchat_id = wu.vrchat_id ORDER BY wu.visit_id, fu.account_name").unwrap();
        statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap()
    }

    #[test]
    fn parser_persists_known_values_but_never_assigns_friend_status_to_self() {
        let conn = parse_friendship_lines(&[
            "[Behaviour] OnPlayerJoined Owner (usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee)", "[Behaviour] Initialized PlayerAPI \"Owner\" is local",
            "[Behaviour] Switching Owner to avatar Local", "[AssetBundleDownloadManager] Download for avatar (Friend:1)", "[AssetBundleDownloadManager] [1] Unpacking Avatar (Local by Creator)",
            "[Behaviour] OnPlayerJoined A (usr_1111-2222-3333-4444-555555555555)", "[Behaviour] Switching A to avatar RemoteA",
            "[AssetBundleDownloadManager] Download for avatar (Friend:1)", "[AssetBundleDownloadManager] [2] Unpacking Avatar (RemoteA by Creator)",
            "[Behaviour] OnPlayerJoined B (usr_2222-3333-4444-5555-666666666666)", "[Behaviour] Switching B to avatar RemoteB",
            "[AssetBundleDownloadManager] Download for avatar (Friend:0)", "[AssetBundleDownloadManager] [3] Unpacking Avatar (RemoteB by Creator)",
        ]);
        assert_eq!(friendship_rows(&conn), [("A".to_string(), 1, 0), ("B".to_string(), 0, 0), ("Owner".to_string(), 2, 1)]);
    }

    #[test]
    fn parser_preserves_unknown_for_duplicate_display_and_new_dist_format() {
        let conn = parse_friendship_lines(&[
            "[Behaviour] OnPlayerJoined Same (usr_1111-2222-3333-4444-555555555555)", "[Behaviour] OnPlayerJoined Same (usr_2222-3333-4444-5555-666666666666)",
            "[Behaviour] Switching Same to avatar Shared", "[AssetBundleDownloadManager] Download for avatar (Friend:1)", "[AssetBundleDownloadManager] [1] Unpacking Avatar (Shared by Creator)",
            "[Behaviour] OnPlayerJoined Unique (usr_3333-4444-5555-6666-777777777777)", "[Behaviour] Switching Unique to avatar NewFormat",
            "[AssetBundleDownloadManager] Download for avatar (Dist:1)", "[AssetBundleDownloadManager] [2] Unpacking Avatar (NewFormat by Creator)",
        ]);
        let rows = friendship_rows(&conn); assert_eq!(rows.len(), 3); assert!(rows.iter().all(|(_, status, is_self)| *status == 2 && *is_self == 0));
    }

    #[test]
    fn parser_keeps_conflict_for_old_visit_and_resets_evidence_for_next_visit() {
        let conn = parse_friendship_lines(&[
            "[Behaviour] OnPlayerJoined A (usr_1111-2222-3333-4444-555555555555)", "[Behaviour] Switching A to avatar Remote",
            "[AssetBundleDownloadManager] Download for avatar (Friend:1)", "[AssetBundleDownloadManager] [1] Unpacking Avatar (Remote by Creator)",
            "[AssetBundleDownloadManager] Download for avatar (Friend:0)", "[AssetBundleDownloadManager] [2] Unpacking Avatar (Remote by Creator)",
            "[AssetBundleDownloadManager] Download for avatar (Friend:1)", "[AssetBundleDownloadManager] [3] Unpacking Avatar (Remote by Creator)",
            "[Behaviour] Entering Room: Next", "[Behaviour] Joining wrld_abc123:456~public~region(jp)",
            "[Behaviour] OnPlayerJoined A (usr_1111-2222-3333-4444-555555555555)", "[Behaviour] Switching A to avatar Remote",
            "[AssetBundleDownloadManager] Download for avatar (Friend:0)", "[AssetBundleDownloadManager] [4] Unpacking Avatar (Remote by Creator)",
        ]);
        assert_eq!(friendship_rows(&conn), [("A".to_string(), 2, 0), ("A".to_string(), 0, 0)]);
        assert_eq!(conn.query_row("SELECT count(*) FROM visits WHERE leave_time IS NULL", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
    }

    #[test]
    fn parser_does_not_carry_pending_download_across_room_boundary() {
        let conn = parse_friendship_lines(&[
            "[Behaviour] OnPlayerJoined A (usr_1111-2222-3333-4444-555555555555)", "[Behaviour] Switching A to avatar Remote",
            "[AssetBundleDownloadManager] Download for avatar (Friend:1)", "[Behaviour] OnLeftRoom",
            "[Behaviour] Entering Room: Next", "[Behaviour] Joining wrld_abc123:456~public~region(jp)",
            "[Behaviour] OnPlayerJoined A (usr_1111-2222-3333-4444-555555555555)", "[Behaviour] Switching A to avatar Remote",
            "[AssetBundleDownloadManager] [1] Unpacking Avatar (Remote by Creator)",
        ]);
        assert_eq!(friendship_rows(&conn), [("A".to_string(), 2, 0), ("A".to_string(), 2, 0)]);
    }

    #[test]
    fn parser_requires_fresh_avatar_after_player_departure_and_reentry() {
        let conn = parse_friendship_lines(&[
            "[Behaviour] OnPlayerJoined A (usr_1111-2222-3333-4444-555555555555)", "[Behaviour] Switching A to avatar Remote",
            "[Behaviour] OnPlayerLeft A (usr_1111-2222-3333-4444-555555555555)", "[Behaviour] OnPlayerJoined A (usr_1111-2222-3333-4444-555555555555)",
            "[AssetBundleDownloadManager] Download for avatar (Friend:1)", "[AssetBundleDownloadManager] [1] Unpacking Avatar (Remote by Creator)",
        ]);
        assert_eq!(friendship_rows(&conn), [("A".to_string(), 2, 0)]);
    }
}
