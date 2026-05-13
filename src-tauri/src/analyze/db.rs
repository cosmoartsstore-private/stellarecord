//! メインデータベースの SQLite スキーマ定義と初期化。
//!
//! メインデータベースは WAL ジャーナルモードと外部キーによる参照整合性を使用する。

use rusqlite::{Connection, Result};

/// メイン StellaRecord データベースの DDL。
///
/// VRChat セッション構造を表現する: セッションがワールド訪問を所有し、
/// ワールド訪問がプレイヤー同席と動画再生を所有し、セッションが通知や
/// セッション単位のイベントストリームを所有する。`apps` テーブルは
/// VRChat とは無関係で、STELLA エコシステム内の外部アプリがランチャー UI に
/// 自己登録するエントリを格納する。
pub const MAIN_SCHEMA: &str = "
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

CREATE TABLE IF NOT EXISTS favorites (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  INTEGER NOT NULL REFERENCES sessions(id),
    target_type TEXT NOT NULL CHECK(target_type IN ('friend','avatar','world')),
    target_id   TEXT NOT NULL,
    action      TEXT NOT NULL CHECK(action IN ('added','removed')),
    timestamp   DATETIME NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_favorites_session_id ON favorites(session_id);
CREATE INDEX IF NOT EXISTS idx_favorites_timestamp  ON favorites(timestamp);

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
    name            TEXT NOT NULL UNIQUE,
    description     TEXT NOT NULL DEFAULT '',
    path            TEXT NOT NULL,
    icon            BLOB,
    registered_at   DATETIME DEFAULT (datetime('now', 'localtime'))
);
";

/// よく使われる集計を公開する事前定義ビュー。
///
/// `visit_summary` はワールド訪問ごとの滞在時間とプレイヤー数を算出し、
/// フロントエンドでの再計算を不要にする。
/// `player_stats` はプレイヤーごとの同室回数と初回/最終会遭時刻を集計する。
pub const MAIN_VIEWS: &str = "
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

CREATE VIEW IF NOT EXISTS player_stats AS
SELECT
    fu.vrchat_id,
    fu.account_name,
    COUNT(DISTINCT wu.visit_id)                          AS co_visit_count,
    MIN(wu.join_time)                                    AS first_met,
    MAX(COALESCE(wu.leave_time, wu.join_time))           AS last_met
FROM find_users fu
JOIN with_users wu ON wu.vrchat_id = fu.vrchat_id
WHERE wu.is_self = 0
GROUP BY fu.vrchat_id
ORDER BY co_visit_count DESC;

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

CREATE VIEW IF NOT EXISTS favorites_detail AS
SELECT
    f.id,
    f.session_id,
    f.target_type,
    f.target_id,
    CASE
        WHEN f.target_type = 'friend' THEN COALESCE(fu.account_name, f.target_id)
        ELSE f.target_id
    END                AS target_name,
    f.action,
    f.timestamp
FROM favorites f
LEFT JOIN find_users fu ON f.target_type = 'friend' AND fu.vrchat_id = f.target_id;

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
/// # エラー
/// SQLite プラグマ、スキーマ、またはビューの適用に失敗した場合にエラーを返す。
pub fn init_main_db(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode = WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(MAIN_SCHEMA)?;
    conn.execute_batch(MAIN_VIEWS)?;
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
