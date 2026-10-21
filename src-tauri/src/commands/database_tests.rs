#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    // ── sanitize_table_name ──

    #[test]
    fn sanitize_accepts_valid_names() {
        assert_eq!(sanitize_table_name("sessions").unwrap(), "sessions");
        assert_eq!(
            sanitize_table_name("with_users_detail").unwrap(),
            "with_users_detail"
        );
        assert_eq!(sanitize_table_name("table1").unwrap(), "table1");
    }

    #[test]
    fn sanitize_rejects_injection_attempts() {
        assert!(sanitize_table_name("").is_err());
        assert!(sanitize_table_name("users; DROP TABLE x").is_err());
        assert!(sanitize_table_name("users--").is_err());
        assert!(sanitize_table_name("users WHERE 1=1").is_err());
        assert!(sanitize_table_name("テーブル").is_err());
    }

    // ── get_table_comment / get_column_comment ──

    #[test]
    fn table_comment_lookup() {
        assert!(get_table_comment("sessions").is_some());
        assert!(get_table_comment("nonexistent_table").is_none());
    }

    #[test]
    fn column_comment_lookup() {
        assert!(get_column_comment("sessions", "log_name").is_some());
        assert!(get_column_comment("screenshots", "session_id").is_some());
        assert!(get_column_comment("sessions", "no_such_column").is_none());
        assert!(get_column_comment("no_such_table", "log_name").is_none());
    }

    #[test]
    fn sanitize_sort_column_accepts_registered_column() {
        assert_eq!(
            sanitize_sort_column("sessions", "log_name").unwrap(),
            "log_name"
        );
    }

    #[test]
    fn sanitize_sort_column_rejects_unknown_or_injected_column() {
        assert!(sanitize_sort_column("sessions", "no_such_column").is_err());
        assert!(sanitize_sort_column("sessions", "log_name DESC").is_err());
        assert!(sanitize_sort_column("sessions", "名前").is_err());
    }

    // ── object_exists ──

    #[test]
    fn object_exists_for_table_and_view() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE foo (id INTEGER);
             CREATE VIEW bar AS SELECT * FROM foo;",
        )
        .unwrap();
        assert!(object_exists(&conn, "foo").unwrap());
        assert!(object_exists(&conn, "bar").unwrap());
        assert!(!object_exists(&conn, "baz").unwrap());
    }

    // ── list_visible_objects (一時 DB ファイル) ──

    #[test]
    fn list_visible_objects_filters_to_registered() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (id INTEGER);
             CREATE TABLE visits (id INTEGER);
             CREATE TABLE unregistered_table (id INTEGER);",
        )
        .unwrap();
        drop(conn);

        let visible = list_visible_objects(&db_path).unwrap();
        // TABLE_COMMENTS 登録順で、登録済みのもののみ
        assert!(visible.contains(&"sessions".to_string()));
        assert!(visible.contains(&"visits".to_string()));
        assert!(!visible.contains(&"unregistered_table".to_string()));
    }

    // ── fetch_table_data (インメモリ DB) ──

    fn seed_sessions(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE sessions (
                id INTEGER PRIMARY KEY, log_name TEXT, account_id TEXT,
                account_name TEXT, start_time DATETIME, end_time DATETIME
            );
            INSERT INTO sessions (log_name, account_name, start_time) VALUES ('a.txt', 'User1', '2025-05-01');
            INSERT INTO sessions (log_name, account_name, start_time) VALUES ('b.txt', 'User2', '2025-05-02');
            INSERT INTO sessions (log_name, account_name, start_time) VALUES ('c.txt', 'User3', '2025-05-03');",
        )
        .unwrap();
    }

    #[test]
    fn fetch_returns_rows_and_metadata() {
        let conn = Connection::open_in_memory().unwrap();
        seed_sessions(&conn);

        let data = fetch_table_data(&conn, "sessions", "Main DB", None, None, None).unwrap();
        assert_eq!(data.total_rows, 3);
        assert_eq!(data.rows.len(), 3);
        assert_eq!(data.name, "sessions");
        // 登録済みラベルが付与される
        assert!(!data.label.is_empty());
        assert!(data.columns.iter().any(|c| c.name == "log_name"));
    }

    #[test]
    fn fetch_explicit_sort_ascending() {
        let conn = Connection::open_in_memory().unwrap();
        seed_sessions(&conn);

        let data = fetch_table_data(
            &conn,
            "sessions",
            "Main DB",
            None,
            Some("account_name".to_string()),
            Some("asc".to_string()),
        )
        .unwrap();
        let name_idx = data
            .columns
            .iter()
            .position(|c| c.name == "account_name")
            .unwrap();
        assert_eq!(data.rows[0][name_idx], "User1");
        assert_eq!(data.rows[2][name_idx], "User3");
    }

    #[test]
    fn fetch_explicit_sort_descending() {
        let conn = Connection::open_in_memory().unwrap();
        seed_sessions(&conn);

        let data = fetch_table_data(
            &conn,
            "sessions",
            "Main DB",
            None,
            Some("account_name".to_string()),
            Some("desc".to_string()),
        )
        .unwrap();
        let name_idx = data
            .columns
            .iter()
            .position(|c| c.name == "account_name")
            .unwrap();
        assert_eq!(data.rows[0][name_idx], "User3");
    }

    #[test]
    fn fetch_rejects_unknown_sort_column() {
        let conn = Connection::open_in_memory().unwrap();
        seed_sessions(&conn);

        let err = fetch_table_data(
            &conn,
            "sessions",
            "Main DB",
            None,
            Some("missing_column".to_string()),
            Some("asc".to_string()),
        )
        .unwrap_err();
        assert!(err.contains("ソートカラムが見つかりません"));
    }

    #[test]
    fn fetch_formats_null_and_blob() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE apps (id INTEGER PRIMARY KEY, name TEXT, description TEXT, path TEXT, icon BLOB);
             INSERT INTO apps (name, description, path, icon) VALUES ('App', NULL, '/p', x'0102');",
        )
        .unwrap();

        let data = fetch_table_data(&conn, "apps", "Main DB", None, None, None).unwrap();
        let desc_idx = data
            .columns
            .iter()
            .position(|c| c.name == "description")
            .unwrap();
        let icon_idx = data.columns.iter().position(|c| c.name == "icon").unwrap();
        assert_eq!(data.rows[0][desc_idx], "NULL");
        assert_eq!(data.rows[0][icon_idx], "<BLOB>");
    }

    #[test]
    fn fetch_pagination_second_page() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE apps (id INTEGER PRIMARY KEY, name TEXT, description TEXT, path TEXT, icon BLOB)", []).unwrap();
        for i in 0..600 {
            conn.execute(
                "INSERT INTO apps (name, path) VALUES (?1, ?2)",
                rusqlite::params![format!("App{i}"), format!("/p{i}")],
            )
            .unwrap();
        }

        let page0 = fetch_table_data(&conn, "apps", "Main DB", Some(0), None, None).unwrap();
        assert_eq!(page0.total_rows, 600);
        assert_eq!(page0.rows.len(), 500);

        let page1 = fetch_table_data(&conn, "apps", "Main DB", Some(1), None, None).unwrap();
        assert_eq!(page1.total_rows, 600);
        assert_eq!(page1.rows.len(), 100);
    }

    #[test]
    fn fetch_uses_storage_label_for_unregistered_table() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE custom (id INTEGER PRIMARY KEY)", [])
            .unwrap();

        let data = fetch_table_data(&conn, "custom", "Custom Store", None, None, None).unwrap();
        assert_eq!(data.storage, "Custom Store");
        assert_eq!(data.label, "custom");
    }
}
