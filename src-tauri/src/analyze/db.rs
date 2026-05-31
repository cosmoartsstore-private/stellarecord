//! メインデータベースの `SQLite` スキーマ定義と初期化。
//!
//! メインデータベースは WAL ジャーナルモードと外部キーによる参照整合性を使用する。

use rusqlite::{Connection, Result};

/// メイン `StellaRecord` データベースの DDL。
///
/// `VRChat` セッション構造を表現する: セッションがワールド訪問を所有し、
/// ワールド訪問がプレイヤー同席と動画再生を所有し、セッションが通知や
/// セッション単位のイベントストリームを所有する。`apps` テーブルは
/// `VRChat` とは無関係で、STELLA エコシステム内の外部アプリがランチャー UI に
/// 自己登録するエントリを格納する。
const MAIN_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    log_name        TEXT UNIQUE NOT NULL,
    account_id      TEXT,
    account_name    TEXT,
    start_time      DATETIME,
    end_time        DATETIME
);

CREATE TABLE IF NOT EXISTS visits (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      INTEGER NOT NULL REFERENCES sessions(id),
    world_name      TEXT NOT NULL,
    instance_id     TEXT NOT NULL,
    instance_type   TEXT CHECK(instance_type IN ('private','friends','hidden','public','group') OR instance_type IS NULL),
    region          TEXT,
    join_time       DATETIME NOT NULL,
    leave_time      DATETIME
);
CREATE INDEX IF NOT EXISTS idx_visits_join_time  ON visits(join_time);
CREATE INDEX IF NOT EXISTS idx_visits_session_id ON visits(session_id);

CREATE TABLE IF NOT EXISTS find_users (
    vrchat_id       TEXT PRIMARY KEY,
    account_name    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS with_users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    visit_id        INTEGER NOT NULL REFERENCES visits(id),
    vrchat_id       TEXT NOT NULL REFERENCES find_users(vrchat_id),
    is_self         BOOLEAN NOT NULL DEFAULT 0,
    join_time       DATETIME NOT NULL,
    leave_time      DATETIME,
    UNIQUE(visit_id, vrchat_id)
);
CREATE INDEX IF NOT EXISTS idx_with_users_visit_id   ON with_users(visit_id);
CREATE INDEX IF NOT EXISTS idx_with_users_vrchat_id  ON with_users(vrchat_id);

CREATE TABLE IF NOT EXISTS notifications (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id            INTEGER NOT NULL REFERENCES sessions(id),
    notif_id              TEXT UNIQUE,
    notif_type            TEXT NOT NULL CHECK(notif_type IN ('boop','friendRequest','requestInvite','invite','group')),
    sender_user_id        TEXT,
    sender_name           TEXT,
    message               TEXT,
    created_at            DATETIME,
    received_at           DATETIME NOT NULL,
    target_world_name     TEXT,
    target_instance_id    TEXT,
    target_instance_type  TEXT,
    target_owner          TEXT,
    target_region         TEXT
);
CREATE INDEX IF NOT EXISTS idx_notifications_type     ON notifications(notif_type);
CREATE INDEX IF NOT EXISTS idx_notifications_received ON notifications(received_at);

CREATE TABLE IF NOT EXISTS screenshots (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    visit_id          INTEGER REFERENCES visits(id),
    file_path         TEXT NOT NULL,
    resolution_width  INTEGER,
    resolution_height INTEGER,
    timestamp         DATETIME NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_screenshots_visit_id  ON screenshots(visit_id);
CREATE INDEX IF NOT EXISTS idx_screenshots_timestamp ON screenshots(timestamp);

CREATE TABLE IF NOT EXISTS osc (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id   INTEGER NOT NULL REFERENCES sessions(id),
    event_type   TEXT NOT NULL CHECK(event_type IN ('found')),
    service_name TEXT,
    service_type TEXT,
    ip_address   TEXT,
    port         INTEGER,
    timestamp    DATETIME NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_osc_session_id ON osc(session_id);
CREATE INDEX IF NOT EXISTS idx_osc_timestamp  ON osc(timestamp);


CREATE TABLE IF NOT EXISTS subscription (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      INTEGER NOT NULL UNIQUE REFERENCES sessions(id),
    is_active       BOOLEAN NOT NULL,
    subscription_id TEXT,
    description     TEXT,
    checked_at      DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS apps (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    path            TEXT NOT NULL UNIQUE,
    icon            BLOB,
    registered_at   DATETIME DEFAULT (datetime('now', 'localtime'))
);
";

/// よく使われる集計を公開する事前定義ビュー。
///
/// `visit_summary` はワールド訪問ごとの滞在時間とプレイヤー数を算出し、
/// フロントエンドでの再計算を不要にする。
/// `with_users_detail` / `screenshots_detail` は関連テーブルを結合した詳細ビュー。
const MAIN_VIEWS: &str = "
CREATE VIEW IF NOT EXISTS visit_summary AS
SELECT
    v.id               AS visit_id,
    v.world_name,
    v.instance_id,
    v.instance_type,
    v.region,
    v.join_time,
    v.leave_time,
    CAST((julianday(COALESCE(v.leave_time, datetime('now'))) - julianday(v.join_time)) * 86400 AS INTEGER)
                       AS duration_sec,
    (SELECT COUNT(*) FROM with_users wu
     WHERE wu.visit_id = v.id AND wu.is_self = 0)
                       AS other_player_count
FROM visits v
ORDER BY v.join_time DESC;


CREATE VIEW IF NOT EXISTS with_users_detail AS
SELECT
    wu.id,
    wu.visit_id,
    v.world_name,
    wu.vrchat_id,
    fu.account_name  AS user_name,
    wu.is_self,
    wu.join_time,
    wu.leave_time
FROM with_users wu
JOIN find_users fu ON fu.vrchat_id = wu.vrchat_id
JOIN visits v ON v.id = wu.visit_id;


CREATE VIEW IF NOT EXISTS screenshots_detail AS
SELECT
    s.id,
    s.visit_id,
    v.world_name,
    s.file_path,
    s.resolution_width,
    s.resolution_height,
    s.timestamp
FROM screenshots s
LEFT JOIN visits v ON v.id = s.visit_id;
";

/// メイン `StellaRecord` スキーマと必要なビューを初期化する。
///
/// # Errors
/// `SQLite` プラグマ、スキーマ、またはビューの適用に失敗した場合にエラーを返す。
pub fn init_main_db(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode = WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(MAIN_SCHEMA)?;
    conn.execute_batch(MAIN_VIEWS)?;
    migrate_apps_unique_to_path(conn)?;
    drop_legacy_apps_category(conn)?;
    Ok(())
}

/// 旧スキーマで残存している `apps.category` 列を削除するマイグレーション。
///
/// fastparty / thirdparty の区別を撤廃した際の後方互換のため、列が存在する場合のみ
/// `ALTER TABLE ... DROP COLUMN` を発行する。新規 DB では無操作。
fn drop_legacy_apps_category(conn: &Connection) -> Result<()> {
    let has_column: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('apps') WHERE name = 'category')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if has_column {
        conn.execute("ALTER TABLE apps DROP COLUMN category", [])?;
    }
    Ok(())
}

/// UNIQUE 制約を `name` から `path` へ移行する。
///
/// 旧スキーマでは `apps.name` が UNIQUE だったが、同パス（同一 exe）の重複登録を
/// 防ぐほうが実用的なため `path` に変更。既存 DB ではテーブル再作成で移行する。
fn migrate_apps_unique_to_path(conn: &Connection) -> Result<()> {
    let name_is_unique: bool = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM pragma_index_list('apps') il
                JOIN pragma_index_info(il.name) ii ON ii.name = 'name'
                WHERE il.\"unique\" = 1
            )",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if !name_is_unique {
        return Ok(());
    }
    conn.execute_batch(
        "BEGIN;
        CREATE TABLE apps_new (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            name            TEXT NOT NULL,
            description     TEXT NOT NULL DEFAULT '',
            path            TEXT NOT NULL UNIQUE,
            icon            BLOB,
            registered_at   DATETIME DEFAULT (datetime('now', 'localtime'))
        );
        INSERT OR IGNORE INTO apps_new (id, name, description, path, icon, registered_at)
            SELECT id, name, description, path, icon, registered_at FROM apps;
        DROP TABLE apps;
        ALTER TABLE apps_new RENAME TO apps;
        COMMIT;",
    )?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn init_main_db_creates_all_tables() {
        let conn = Connection::open_in_memory().unwrap();
        init_main_db(&conn).unwrap();

        let expected_tables = [
            "sessions", "visits", "find_users", "with_users",
            "notifications", "screenshots", "osc", "subscription", "apps",
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
        assert_eq!(table_count, 9);
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
    fn drop_legacy_apps_category_noop_on_new_db() {
        let conn = Connection::open_in_memory().unwrap();
        init_main_db(&conn).unwrap();
        drop_legacy_apps_category(&conn).unwrap();
    }

    #[test]
    fn migrate_apps_unique_noop_on_new_db() {
        let conn = Connection::open_in_memory().unwrap();
        init_main_db(&conn).unwrap();
        migrate_apps_unique_to_path(&conn).unwrap();
    }

    #[test]
    fn drop_legacy_apps_category_removes_column() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE apps (
                id INTEGER PRIMARY KEY, name TEXT, description TEXT DEFAULT '',
                path TEXT UNIQUE, icon BLOB, category TEXT,
                registered_at DATETIME DEFAULT (datetime('now','localtime'))
            );
            INSERT INTO apps (name, path, category) VALUES ('TestApp', '/test', 'thirdparty');",
        )
        .unwrap();

        drop_legacy_apps_category(&conn).unwrap();

        let has_category: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('apps') WHERE name = 'category')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!has_category, "category column should be removed");

        let name: String = conn
            .query_row("SELECT name FROM apps WHERE path = '/test'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(name, "TestApp", "existing data should survive migration");
    }

    #[test]
    fn migrate_apps_unique_moves_constraint_to_path() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE apps (
                id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL,
                description TEXT DEFAULT '', path TEXT NOT NULL, icon BLOB,
                registered_at DATETIME DEFAULT (datetime('now','localtime'))
            );
            INSERT INTO apps (name, path) VALUES ('App1', '/path/a');
            INSERT INTO apps (name, path) VALUES ('App2', '/path/b');",
        )
        .unwrap();

        migrate_apps_unique_to_path(&conn).unwrap();

        let dup_name = conn.execute(
            "INSERT INTO apps (name, path) VALUES ('App1', '/path/c')", [],
        );
        assert!(dup_name.is_ok(), "duplicate name should now be allowed");

        let dup_path = conn.execute(
            "INSERT INTO apps (name, path) VALUES ('App3', '/path/a')", [],
        );
        assert!(dup_path.is_err(), "duplicate path should be rejected after migration");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM apps", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3, "original rows + new name-dup row should exist");
    }
}
