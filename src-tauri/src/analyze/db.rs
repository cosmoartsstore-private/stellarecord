//! メインデータベースの `SQLite` schema定義と初期化。
//!
//! メインデータベースは WAL ジャーナルモードと外部キーによる参照整合性を使用する。
//! ここにある DDL を初版の構成とし、旧構成の DB は自動変換せず拒否する。

use rusqlite::{Connection, Result};

/// メイン `StellaRecord` データベースの DDL。
///
/// `VRChat` セッション構造を表現する: セッションがワールド訪問を所有し、
/// ワールド訪問がプレイヤー同席を所有し、セッションが通知や
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
CREATE INDEX IF NOT EXISTS idx_screenshots_session_id ON screenshots(session_id);

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
/// Frontendでの再計算を不要にする。
/// `with_users_detail` / `screenshots_detail` は関連テーブルを結合した詳細ビュー。
// 滞在秒数の派生viewは現行の計算式で再生成し、保存済みの訪問行は保持する。
const MAIN_VIEWS: &str = "
DROP VIEW IF EXISTS visit_summary;
CREATE VIEW visit_summary AS
SELECT
    v.id               AS visit_id,
    v.world_name,
    v.instance_id,
    v.instance_type,
    v.region,
    v.join_time,
    v.leave_time,
    unixepoch(COALESCE(v.leave_time, datetime('now', 'localtime'))) - unixepoch(v.join_time)
                       AS duration_sec,
    (SELECT COUNT(*) FROM with_users wu
     WHERE wu.visit_id = v.id AND wu.is_self = 0)
                       AS other_player_count
FROM visits v
ORDER BY v.join_time DESC;


DROP VIEW IF EXISTS with_users_detail;
CREATE VIEW with_users_detail AS
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


DROP VIEW IF EXISTS screenshots_detail;
CREATE VIEW screenshots_detail AS
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

/// 内部管理テーブルを除いた、保存済みのテーブル名を読む。
fn table_names(conn: &Connection) -> Result<Vec<String>> {
    let mut statement = conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;
    let rows = statement.query_map([], |row| row.get(0))?;
    rows.collect()
}

/// 列名・型・必須指定・主キーを、物理的な列順に依存せず照合する。
fn table_layout(conn: &Connection, table: &str) -> Result<Vec<(String, String, bool, i64)>> {
    let mut statement = conn.prepare("SELECT name, type, \"notnull\", pk FROM pragma_table_info(?1) ORDER BY name")?;
    let rows = statement.query_map([table], |row| Ok((row.get(0)?, row.get::<_, String>(1)?.to_ascii_uppercase(), row.get(2)?, row.get(3)?)))?;
    rows.collect()
}

/// 初版DDLを比較元に使い、旧構成への列追加やデータ補完を行わない。
/// 空DBと姉妹アプリが先に作る現行appsだけのDBは、新規初期化として受け入れる。
fn validate_current_schema(conn: &Connection) -> Result<()> {
    let existing = table_names(conn)?;
    if existing.is_empty() {
        return Ok(());
    }

    let reference = Connection::open_in_memory()?;
    reference.execute_batch(MAIN_SCHEMA)?;
    let catalog_only = existing.len() == 1 && existing[0] == "apps";
    for table in table_names(&reference)? {
        if catalog_only && table != "apps" {
            continue;
        }
        if table_layout(conn, &table)? != table_layout(&reference, &table)? {
            return Err(rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_SCHEMA), Some(format!(message!("Analyze.Db.validateCurrentSchema.incompatible"), table = table))));
        }
    }
    Ok(())
}

/// メイン `StellaRecord` schemaと必要なビューを初期化する。
///
/// # Errors
/// DBが初版の構成と一致しない、またはプラグマ・schema・ビューの適用に失敗した場合に
/// エラーを返す。
pub fn init_main_db(conn: &Connection) -> Result<()> {
    validate_current_schema(conn)?;
    conn.execute_batch("PRAGMA journal_mode = WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    // schemaと派生viewを同じ確定範囲へ置き、初期化失敗で途中の構成を残さない。
    conn.execute_batch("SAVEPOINT initialize_main_db;")?;
    if let Err(error) = conn.execute_batch(MAIN_SCHEMA).and_then(|()| conn.execute_batch(MAIN_VIEWS)) {
        if let Err(rollback_error) = conn.execute_batch("ROLLBACK TO initialize_main_db; RELEASE initialize_main_db;") {
            return Err(rusqlite::Error::InvalidParameterName(format!(message!("Analyze.Db.initMainDb.restoreFailed"), error = error, rollback_error = rollback_error)));
        }
        return Err(error);
    }
    conn.execute_batch("RELEASE initialize_main_db;")?;
    Ok(())
}

#[cfg(test)]
include!("db_tests.rs");
