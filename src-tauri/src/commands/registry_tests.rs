#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_apps_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE apps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
                path TEXT NOT NULL UNIQUE, icon BLOB
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn insert_app_record_success() {
        let conn = setup_apps_db();
        insert_app_record(&conn, "VRChat", "VR app", "/path/vrchat.exe", None).unwrap();

        let (name, path): (String, String) = conn
            .query_row("SELECT name, path FROM apps", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(name, "VRChat");
        assert_eq!(path, "/path/vrchat.exe");
    }

    #[test]
    fn insert_app_record_with_icon() {
        let conn = setup_apps_db();
        insert_app_record(&conn, "App", "", "/p", Some(&[0x89, 0x50])).unwrap();

        let icon: Option<Vec<u8>> = conn
            .query_row("SELECT icon FROM apps", [], |r| r.get(0))
            .unwrap();
        assert_eq!(icon, Some(vec![0x89, 0x50]));
    }

    #[test]
    fn insert_app_record_duplicate_path_returns_friendly_error() {
        let conn = setup_apps_db();
        insert_app_record(&conn, "App1", "", "/same/path.exe", None).unwrap();

        let err = insert_app_record(&conn, "App2", "", "/same/path.exe", None).unwrap_err();
        assert_eq!(err, "同じパスのアプリが既に登録されています。");
    }

    #[test]
    fn insert_app_record_allows_duplicate_name() {
        let conn = setup_apps_db();
        insert_app_record(&conn, "SameName", "", "/path/a.exe", None).unwrap();
        // 名前が同じでもパスが異なれば許可される
        insert_app_record(&conn, "SameName", "", "/path/b.exe", None).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM apps", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn delete_app_record_success() {
        let conn = setup_apps_db();
        insert_app_record(&conn, "App", "", "/path/app.exe", None).unwrap();

        delete_app_record(&conn, "/path/app.exe").unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM apps", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn delete_app_record_missing_returns_error() {
        let conn = setup_apps_db();
        let err = delete_app_record(&conn, "/nonexistent.exe").unwrap_err();
        assert_eq!(err, "該当するアプリが見つかりません。");
    }
}
