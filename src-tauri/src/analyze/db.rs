//! メインデータベースの `SQLite` スキーマ定義と初期化。
//!
//! メインデータベースは WAL ジャーナルモードと外部キーによる参照整合性を使用する。
//! ここにある DDL を正とし、追加テーブルと追加列は既存データを保持したまま
//! 初期化時に適用する。

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

-- 同名ログの追記を検出するため、取り込み済み本文の暗号学的ハッシュと長さを保持する。
-- ファイル時刻や圧縮時の tar メタデータは内容同一性の判定に使用しない。
CREATE TABLE IF NOT EXISTS imported_logs (
    log_name        TEXT PRIMARY KEY REFERENCES sessions(log_name) ON DELETE CASCADE,
    content_sha256  BLOB NOT NULL CHECK(length(content_sha256) = 32),
    content_size    INTEGER NOT NULL CHECK(content_size >= 0),
    parser_version  INTEGER NOT NULL DEFAULT 1 CHECK(parser_version >= 1)
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
    friend_status   INTEGER NOT NULL DEFAULT 2 CHECK(friend_status IN (0,1,2)),
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
    timestamp         DATETIME NOT NULL,
    session_id        INTEGER REFERENCES sessions(id)
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
    wu.friend_status,
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

/// 指定テーブルにカラムが存在するか確認する。
fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for existing in columns {
        if existing? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// 旧 `screenshots` スキーマへセッション所有列を追加し、確定できる行だけ補完する。
///
/// `visit_id` がある行は親訪問からセッションを一意に特定できる。ワールド外撮影として
/// `visit_id` が `NULL` の旧行は所有元を推定せず、そのまま保持する。
fn migrate_screenshot_session_ownership(conn: &Connection) -> Result<()> {
    if !table_has_column(conn, "screenshots", "session_id")? {
        conn.execute(
            "ALTER TABLE screenshots
             ADD COLUMN session_id INTEGER REFERENCES sessions(id)",
            [],
        )?;
    }

    conn.execute(
        "UPDATE screenshots
         SET session_id = (
             SELECT visit.session_id
             FROM visits AS visit
             WHERE visit.id = screenshots.visit_id
         )
         WHERE session_id IS NULL
           AND visit_id IS NOT NULL",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_screenshots_session_id
         ON screenshots(session_id)",
        [],
    )?;
    Ok(())
}

/// 既存 DB に訪問時点のフレンド状態列を追加する。
fn migrate_friend_status(conn: &Connection) -> Result<()> {
    if !table_has_column(conn, "with_users", "friend_status")? {
        conn.execute(
            "ALTER TABLE with_users
             ADD COLUMN friend_status INTEGER NOT NULL DEFAULT 2
             CHECK(friend_status IN (0,1,2))",
            [],
        )?;
    }
    Ok(())
}

/// 既存 DB に解析バージョン列を追加する。
///
/// 旧取り込み行をバージョン1として保持し、現在のパーサーが必要なログだけを
/// ファイル単位 savepoint 内で再構築できるようにする。
fn migrate_import_parser_version(conn: &Connection) -> Result<()> {
    if !table_has_column(conn, "imported_logs", "parser_version")? {
        conn.execute(
            "ALTER TABLE imported_logs
             ADD COLUMN parser_version INTEGER NOT NULL DEFAULT 1
             CHECK(parser_version >= 1)",
            [],
        )?;
    }
    Ok(())
}

/// メイン `StellaRecord` スキーマと必要なビューを初期化する。
///
/// # Errors
/// `SQLite` プラグマ、スキーマ、加算的移行、またはビューの適用に失敗した場合に
/// エラーを返す。
pub fn init_main_db(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode = WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(MAIN_SCHEMA)?;
    migrate_screenshot_session_ownership(conn)?;
    migrate_friend_status(conn)?;
    migrate_import_parser_version(conn)?;
    if !table_has_column(conn, "with_users_detail", "friend_status")? {
        conn.execute_batch("DROP VIEW IF EXISTS with_users_detail;")?;
    }
    conn.execute_batch(MAIN_VIEWS)?;
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
