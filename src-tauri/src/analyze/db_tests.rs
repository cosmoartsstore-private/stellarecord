#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn visit_summary_elapsed_seconds_match_saved_and_ongoing_local_times() {
        let conn = Connection::open_in_memory().unwrap();
        init_main_db(&conn).unwrap();
        conn.execute("INSERT INTO sessions(id,log_name) VALUES(1,'elapsed-seconds.txt')", []).unwrap();
        for (id, start, end, seconds) in [
            (1, "2026-10-08 12:01:01", "2026-10-08 12:05:00", 239),
            (2, "2026-10-08 12:00:00", "2026-10-08 12:01:00", 60),
            (3, "2026-10-08 12:00:00", "2026-10-08 12:00:00", 0),
        ] {
            conn.execute("INSERT INTO visits(id,session_id,world_name,instance_id,join_time,leave_time) VALUES(?1,1,'Elapsed World','123',?2,?3)", rusqlite::params![id, start, end]).unwrap();
            let actual: i64 = conn.query_row("SELECT duration_sec FROM visit_summary WHERE visit_id=?1", [id], |row| row.get(0)).unwrap();
            assert_eq!(actual, seconds);
        }
        let join_time = (chrono::Local::now() - chrono::Duration::seconds(61)).format("%Y-%m-%d %H:%M:%S").to_string();
        conn.execute("INSERT INTO visits(id,session_id,world_name,instance_id,join_time) VALUES(4,1,'Ongoing World','456',?1)", [&join_time]).unwrap();
        let ongoing: i64 = conn.query_row("SELECT duration_sec FROM visit_summary WHERE visit_id=4", [], |row| row.get(0)).unwrap();
        assert!((61..=64).contains(&ongoing), "進行中の滞在秒数がlocal timeと不一致: {ongoing}");
        init_main_db(&conn).unwrap();
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM visits", [], |row| row.get::<_, i64>(0)).unwrap(), 4);
        assert_eq!(conn.query_row("SELECT duration_sec FROM visit_summary WHERE visit_id=1", [], |row| row.get::<_, i64>(0)).unwrap(), 239);
    }

    #[test]
    fn init_main_db_creates_all_tables() {
        let conn = Connection::open_in_memory().unwrap();
        init_main_db(&conn).unwrap();

        let expected_tables = [
            "sessions",
            "imported_logs",
            "visits",
            "find_users",
            "with_users",
            "notifications",
            "screenshots",
            "osc",
            "subscription",
            "apps",
        ];
        for table in expected_tables {
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "table '{table}' should exist");
        }
    }

    #[test]
    fn init_main_db_creates_views() {
        let conn = Connection::open_in_memory().unwrap();
        init_main_db(&conn).unwrap();

        let expected_views = ["visit_summary", "with_users_detail", "screenshots_detail"];
        for view in expected_views {
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='view' AND name=?1)",
                    [view],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "view '{view}' should exist");
        }
    }

    #[test]
    fn init_main_db_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init_main_db(&conn).unwrap();
        init_main_db(&conn).unwrap();

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 10);
    }

    #[test]
    fn init_main_db_enables_wal() {
        let conn = Connection::open_in_memory().unwrap();
        init_main_db(&conn).unwrap();

        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert!(mode == "wal" || mode == "memory");
    }

    #[test]
    fn init_main_db_enables_foreign_keys() {
        let conn = Connection::open_in_memory().unwrap();
        init_main_db(&conn).unwrap();

        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn apps_table_has_unique_path() {
        let conn = Connection::open_in_memory().unwrap();
        init_main_db(&conn).unwrap();

        conn.execute(
            "INSERT INTO apps (name, path) VALUES ('App1', '/path/to/app')",
            [],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO apps (name, path) VALUES ('App2', '/path/to/app')",
            [],
        );
        assert!(result.is_err(), "duplicate path should be rejected");
    }

    #[test]
    fn instance_type_check_constraint() {
        let conn = Connection::open_in_memory().unwrap();
        init_main_db(&conn).unwrap();

        conn.execute(
            "INSERT INTO sessions (log_name, start_time) VALUES ('test', '2025-01-01')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO visits (session_id, world_name, instance_id, instance_type, join_time) VALUES (1, 'World', '123', 'private', '2025-01-01')",
            [],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO visits (session_id, world_name, instance_id, instance_type, join_time) VALUES (1, 'World', '456', 'invalid_type', '2025-01-01')",
            [],
        );
        assert!(result.is_err(), "invalid instance_type should be rejected");
    }

    #[test]
    fn init_main_db_adds_import_tracking_to_existing_database() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                log_name TEXT UNIQUE NOT NULL,
                account_id TEXT,
                account_name TEXT,
                start_time DATETIME,
                end_time DATETIME
             );
             INSERT INTO sessions (log_name, start_time) VALUES ('legacy.txt', '2025-01-01');",
        )
        .unwrap();

        init_main_db(&conn).unwrap();

        let session_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        let tracking_exists: bool = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type = 'table' AND name = 'imported_logs'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(session_count, 1);
        assert!(tracking_exists);
    }

    #[test]
    fn init_main_db_migrates_screenshot_session_ownership_idempotently() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE sessions (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 log_name TEXT UNIQUE NOT NULL,
                 account_id TEXT,
                 account_name TEXT,
                 start_time DATETIME,
                 end_time DATETIME
             );
             CREATE TABLE visits (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 session_id INTEGER NOT NULL REFERENCES sessions(id),
                 world_name TEXT NOT NULL,
                 instance_id TEXT NOT NULL,
                 instance_type TEXT,
                 region TEXT,
                 join_time DATETIME NOT NULL,
                 leave_time DATETIME
             );
             CREATE TABLE screenshots (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 visit_id INTEGER REFERENCES visits(id),
                 file_path TEXT NOT NULL,
                 resolution_width INTEGER,
                 resolution_height INTEGER,
                 timestamp DATETIME NOT NULL
             );
             INSERT INTO sessions (log_name, start_time)
             VALUES ('legacy.txt', '2025-01-01');
             INSERT INTO visits
                 (session_id, world_name, instance_id, instance_type, join_time)
             VALUES (1, 'Legacy World', 'legacy', 'public', '2025-01-01');
             INSERT INTO screenshots
                 (visit_id, file_path, resolution_width, resolution_height, timestamp)
             VALUES
                 (1, 'C:\\with-visit.png', 1920, 1080, '2025-01-01 00:01:00'),
                 (NULL, 'C:\\without-visit.png', 1920, 1080, '2025-01-01 00:02:00');",
        )
        .unwrap();

        init_main_db(&conn).unwrap();
        init_main_db(&conn).unwrap();

        let session_id_column_count: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM pragma_table_info('screenshots')
                 WHERE name = 'session_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let with_visit_owner: Option<i64> = conn
            .query_row(
                "SELECT session_id FROM screenshots WHERE visit_id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let without_visit_owner: Option<i64> = conn
            .query_row(
                "SELECT session_id FROM screenshots WHERE visit_id IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let screenshot_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM screenshots", [], |row| row.get(0))
            .unwrap();
        let session_index_exists: bool = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type = 'index'
                      AND name = 'idx_screenshots_session_id'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(session_id_column_count, 1);
        assert_eq!(with_visit_owner, Some(1));
        assert_eq!(without_visit_owner, None);
        assert_eq!(screenshot_count, 2);
        assert!(session_index_exists);
    }

    #[test]
    fn imported_log_tracking_is_removed_with_session() {
        let conn = Connection::open_in_memory().unwrap();
        init_main_db(&conn).unwrap();
        conn.execute(
            "INSERT INTO sessions (log_name, start_time) VALUES ('tracked.txt', '2025-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO imported_logs (log_name, content_sha256, content_size)
             VALUES ('tracked.txt', ?1, 3)",
            [&[0_u8; 32][..]],
        )
        .unwrap();

        conn.execute("DELETE FROM sessions WHERE log_name = 'tracked.txt'", [])
            .unwrap();

        let tracking_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM imported_logs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tracking_count, 0);
    }
}
