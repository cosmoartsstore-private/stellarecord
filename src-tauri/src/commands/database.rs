//! Frontendプレビューパネル用の読み取り専用データベーステーブルブラウザ。
//!
//! このモジュールの定数（`TABLE_COMMENTS`、テーブルごとのcolumn配列）は生の
//! `SQLite` schema名を日本語の表示ラベルにマッピングし、TypeScript に
//! 表示文字列をハードコードせずにフレンドリーなテーブルブラウザを描画可能にする。

use std::path::Path;

use crate::models::{DbColumnMeta, DbTableSummary, TableData};
use crate::utils;

use super::get_db_path;

/// プレビュー可能なテーブルcolumn1件の表示用メタデータ。
#[derive(Clone, Copy)]
struct ColumnComment {
    name: &'static str,
    label: &'static str,
    description: &'static str,
}

/// プレビュー可能なデータベーステーブル1件の表示用メタデータ。
#[derive(Clone, Copy)]
struct TableComment {
    name: &'static str,
    label: &'static str,
    description: &'static str,
    storage: &'static str,
    columns: &'static [ColumnComment],
    default_sort: Option<(&'static str, &'static str)>,
    is_view: bool,
}

const SESSIONS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: message!("Database.columns.sessions.id.label"), description: message!("Database.columns.sessions.id.description") },
    ColumnComment { name: "log_name", label: message!("Database.columns.sessions.logName.label"), description: message!("Database.columns.sessions.logName.description") },
    ColumnComment { name: "account_id", label: message!("Database.columns.sessions.accountId.label"), description: message!("Database.columns.sessions.accountId.description") },
    ColumnComment { name: "account_name", label: message!("Database.columns.sessions.accountName.label"), description: message!("Database.columns.sessions.accountName.description") },
    ColumnComment { name: "start_time", label: message!("Database.columns.sessions.startTime.label"), description: message!("Database.columns.sessions.startTime.description") },
    ColumnComment { name: "end_time", label: message!("Database.columns.sessions.endTime.label"), description: message!("Database.columns.sessions.endTime.description") },
];

const VISITS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: message!("Database.columns.visits.id.label"), description: message!("Database.columns.visits.id.description") },
    ColumnComment { name: "session_id", label: message!("Database.columns.visits.sessionId.label"), description: message!("Database.columns.visits.sessionId.description") },
    ColumnComment { name: "world_name", label: message!("Database.columns.visits.worldName.label"), description: message!("Database.columns.visits.worldName.description") },
    ColumnComment { name: "instance_id", label: message!("Database.columns.visits.instanceId.label"), description: message!("Database.columns.visits.instanceId.description") },
    ColumnComment { name: "instance_type", label: message!("Database.columns.visits.instanceType.label"), description: message!("Database.columns.visits.instanceType.description") },
    ColumnComment { name: "region", label: message!("Database.columns.visits.region.label"), description: message!("Database.columns.visits.region.description") },
    ColumnComment { name: "join_time", label: message!("Database.columns.visits.joinTime.label"), description: message!("Database.columns.visits.joinTime.description") },
    ColumnComment { name: "leave_time", label: message!("Database.columns.visits.leaveTime.label"), description: message!("Database.columns.visits.leaveTime.description") },
];

const FIND_USERS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "vrchat_id", label: message!("Database.columns.findUsers.vrchatId.label"), description: message!("Database.columns.findUsers.vrchatId.description") },
    ColumnComment { name: "account_name", label: message!("Database.columns.findUsers.accountName.label"), description: message!("Database.columns.findUsers.accountName.description") },
];

const WITH_USERS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: message!("Database.columns.withUsers.id.label"), description: message!("Database.columns.withUsers.id.description") },
    ColumnComment { name: "visit_id", label: message!("Database.columns.withUsers.visitId.label"), description: message!("Database.columns.withUsers.visitId.description") },
    ColumnComment { name: "vrchat_id", label: message!("Database.columns.withUsers.vrchatId.label"), description: message!("Database.columns.withUsers.vrchatId.description") },
    ColumnComment { name: "is_self", label: message!("Database.columns.withUsers.isSelf.label"), description: message!("Database.columns.withUsers.isSelf.description") },
    ColumnComment { name: "friend_status", label: message!("Database.columns.withUsers.friendStatus.label"), description: message!("Database.columns.withUsers.friendStatus.description") },
    ColumnComment { name: "join_time", label: message!("Database.columns.withUsers.joinTime.label"), description: message!("Database.columns.withUsers.joinTime.description") },
    ColumnComment { name: "leave_time", label: message!("Database.columns.withUsers.leaveTime.label"), description: message!("Database.columns.withUsers.leaveTime.description") },
];

const WITH_USERS_DETAIL_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: message!("Database.columns.withUsersDetail.id.label"), description: message!("Database.columns.withUsersDetail.id.description") },
    ColumnComment { name: "visit_id", label: message!("Database.columns.withUsersDetail.visitId.label"), description: message!("Database.columns.withUsersDetail.visitId.description") },
    ColumnComment { name: "world_name", label: message!("Database.columns.withUsersDetail.worldName.label"), description: message!("Database.columns.withUsersDetail.worldName.description") },
    ColumnComment { name: "vrchat_id", label: message!("Database.columns.withUsersDetail.vrchatId.label"), description: message!("Database.columns.withUsersDetail.vrchatId.description") },
    ColumnComment { name: "user_name", label: message!("Database.columns.withUsersDetail.userName.label"), description: message!("Database.columns.withUsersDetail.userName.description") },
    ColumnComment { name: "is_self", label: message!("Database.columns.withUsersDetail.isSelf.label"), description: message!("Database.columns.withUsersDetail.isSelf.description") },
    ColumnComment { name: "friend_status", label: message!("Database.columns.withUsersDetail.friendStatus.label"), description: message!("Database.columns.withUsersDetail.friendStatus.description") },
    ColumnComment { name: "join_time", label: message!("Database.columns.withUsersDetail.joinTime.label"), description: message!("Database.columns.withUsersDetail.joinTime.description") },
    ColumnComment { name: "leave_time", label: message!("Database.columns.withUsersDetail.leaveTime.label"), description: message!("Database.columns.withUsersDetail.leaveTime.description") },
];

const VISIT_SUMMARY_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "visit_id", label: message!("Database.columns.visitSummary.visitId.label"), description: message!("Database.columns.visitSummary.visitId.description") },
    ColumnComment { name: "world_name", label: message!("Database.columns.visitSummary.worldName.label"), description: message!("Database.columns.visitSummary.worldName.description") },
    ColumnComment { name: "instance_id", label: message!("Database.columns.visitSummary.instanceId.label"), description: message!("Database.columns.visitSummary.instanceId.description") },
    ColumnComment { name: "instance_type", label: message!("Database.columns.visitSummary.instanceType.label"), description: message!("Database.columns.visitSummary.instanceType.description") },
    ColumnComment { name: "region", label: message!("Database.columns.visitSummary.region.label"), description: message!("Database.columns.visitSummary.region.description") },
    ColumnComment { name: "join_time", label: message!("Database.columns.visitSummary.joinTime.label"), description: message!("Database.columns.visitSummary.joinTime.description") },
    ColumnComment { name: "leave_time", label: message!("Database.columns.visitSummary.leaveTime.label"), description: message!("Database.columns.visitSummary.leaveTime.description") },
    ColumnComment { name: "duration_sec", label: message!("Database.columns.visitSummary.durationSec.label"), description: message!("Database.columns.visitSummary.durationSec.description") },
    ColumnComment { name: "other_player_count", label: message!("Database.columns.visitSummary.otherPlayerCount.label"), description: message!("Database.columns.visitSummary.otherPlayerCount.description") },
];

const NOTIFICATIONS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: message!("Database.columns.notifications.id.label"), description: message!("Database.columns.notifications.id.description") },
    ColumnComment { name: "notif_type", label: message!("Database.columns.notifications.notifType.label"), description: message!("Database.columns.notifications.notifType.description") },
    ColumnComment { name: "message", label: message!("Database.columns.notifications.message.label"), description: message!("Database.columns.notifications.message.description") },
    ColumnComment { name: "sender_name", label: message!("Database.columns.notifications.senderName.label"), description: message!("Database.columns.notifications.senderName.description") },
    ColumnComment { name: "sender_user_id", label: message!("Database.columns.notifications.senderUserId.label"), description: message!("Database.columns.notifications.senderUserId.description") },
    ColumnComment { name: "notif_id", label: message!("Database.columns.notifications.notifId.label"), description: message!("Database.columns.notifications.notifId.description") },
    ColumnComment { name: "session_id", label: message!("Database.columns.notifications.sessionId.label"), description: message!("Database.columns.notifications.sessionId.description") },
    ColumnComment { name: "created_at", label: message!("Database.columns.notifications.createdAt.label"), description: message!("Database.columns.notifications.createdAt.description") },
    ColumnComment { name: "received_at", label: message!("Database.columns.notifications.receivedAt.label"), description: message!("Database.columns.notifications.receivedAt.description") },
    ColumnComment { name: "target_world_name", label: message!("Database.columns.notifications.targetWorldName.label"), description: message!("Database.columns.notifications.targetWorldName.description") },
    ColumnComment { name: "target_instance_id", label: message!("Database.columns.notifications.targetInstanceId.label"), description: message!("Database.columns.notifications.targetInstanceId.description") },
    ColumnComment { name: "target_instance_type", label: message!("Database.columns.notifications.targetInstanceType.label"), description: message!("Database.columns.notifications.targetInstanceType.description") },
    ColumnComment { name: "target_owner", label: message!("Database.columns.notifications.targetOwner.label"), description: message!("Database.columns.notifications.targetOwner.description") },
    ColumnComment { name: "target_region", label: message!("Database.columns.notifications.targetRegion.label"), description: message!("Database.columns.notifications.targetRegion.description") },
];

const SCREENSHOTS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: message!("Database.columns.screenshots.id.label"), description: message!("Database.columns.screenshots.id.description") },
    ColumnComment { name: "visit_id", label: message!("Database.columns.screenshots.visitId.label"), description: message!("Database.columns.screenshots.visitId.description") },
    ColumnComment { name: "session_id", label: message!("Database.columns.screenshots.sessionId.label"), description: message!("Database.columns.screenshots.sessionId.description") },
    ColumnComment { name: "file_path", label: message!("Database.columns.screenshots.filePath.label"), description: message!("Database.columns.screenshots.filePath.description") },
    ColumnComment { name: "resolution_width", label: message!("Database.columns.screenshots.resolutionWidth.label"), description: message!("Database.columns.screenshots.resolutionWidth.description") },
    ColumnComment { name: "resolution_height", label: message!("Database.columns.screenshots.resolutionHeight.label"), description: message!("Database.columns.screenshots.resolutionHeight.description") },
    ColumnComment { name: "timestamp", label: message!("Database.columns.screenshots.timestamp.label"), description: message!("Database.columns.screenshots.timestamp.description") },
];

const SCREENSHOTS_DETAIL_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: message!("Database.columns.screenshotsDetail.id.label"), description: message!("Database.columns.screenshotsDetail.id.description") },
    ColumnComment { name: "visit_id", label: message!("Database.columns.screenshotsDetail.visitId.label"), description: message!("Database.columns.screenshotsDetail.visitId.description") },
    ColumnComment { name: "world_name", label: message!("Database.columns.screenshotsDetail.worldName.label"), description: message!("Database.columns.screenshotsDetail.worldName.description") },
    ColumnComment { name: "file_path", label: message!("Database.columns.screenshotsDetail.filePath.label"), description: message!("Database.columns.screenshotsDetail.filePath.description") },
    ColumnComment { name: "resolution_width", label: message!("Database.columns.screenshotsDetail.resolutionWidth.label"), description: message!("Database.columns.screenshotsDetail.resolutionWidth.description") },
    ColumnComment { name: "resolution_height", label: message!("Database.columns.screenshotsDetail.resolutionHeight.label"), description: message!("Database.columns.screenshotsDetail.resolutionHeight.description") },
    ColumnComment { name: "timestamp", label: message!("Database.columns.screenshotsDetail.timestamp.label"), description: message!("Database.columns.screenshotsDetail.timestamp.description") },
];

const OSC_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: message!("Database.columns.osc.id.label"), description: message!("Database.columns.osc.id.description") },
    ColumnComment { name: "session_id", label: message!("Database.columns.osc.sessionId.label"), description: message!("Database.columns.osc.sessionId.description") },
    ColumnComment { name: "event_type", label: message!("Database.columns.osc.eventType.label"), description: message!("Database.columns.osc.eventType.description") },
    ColumnComment { name: "service_name", label: message!("Database.columns.osc.serviceName.label"), description: message!("Database.columns.osc.serviceName.description") },
    ColumnComment { name: "service_type", label: message!("Database.columns.osc.serviceType.label"), description: message!("Database.columns.osc.serviceType.description") },
    ColumnComment { name: "ip_address", label: message!("Database.columns.osc.ipAddress.label"), description: message!("Database.columns.osc.ipAddress.description") },
    ColumnComment { name: "port", label: message!("Database.columns.osc.port.label"), description: message!("Database.columns.osc.port.description") },
    ColumnComment { name: "timestamp", label: message!("Database.columns.osc.timestamp.label"), description: message!("Database.columns.osc.timestamp.description") },
];

const SUBSCRIPTION_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: message!("Database.columns.subscription.id.label"), description: message!("Database.columns.subscription.id.description") },
    ColumnComment { name: "session_id", label: message!("Database.columns.subscription.sessionId.label"), description: message!("Database.columns.subscription.sessionId.description") },
    ColumnComment { name: "is_active", label: message!("Database.columns.subscription.isActive.label"), description: message!("Database.columns.subscription.isActive.description") },
    ColumnComment { name: "subscription_id", label: message!("Database.columns.subscription.subscriptionId.label"), description: message!("Database.columns.subscription.subscriptionId.description") },
    ColumnComment { name: "description", label: message!("Database.columns.subscription.description.label"), description: message!("Database.columns.subscription.description.description") },
    ColumnComment { name: "checked_at", label: message!("Database.columns.subscription.checkedAt.label"), description: message!("Database.columns.subscription.checkedAt.description") },
];

const APPS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "name", label: message!("Database.columns.apps.name.label"), description: message!("Database.columns.apps.name.description") },
    ColumnComment { name: "description", label: message!("Database.columns.apps.description.label"), description: message!("Database.columns.apps.description.description") },
    ColumnComment { name: "path", label: message!("Database.columns.apps.path.label"), description: message!("Database.columns.apps.path.description") },
    ColumnComment { name: "icon", label: message!("Database.columns.apps.icon.label"), description: message!("Database.columns.apps.icon.description") },
];

const TABLE_COMMENTS: &[TableComment] = &[
    // ── テーブル ──
    TableComment {
        name: "sessions",
        label: message!("Database.tables.sessions.label"),
        description: message!("Database.tables.sessions.description"),
        storage: message!("Database.tables.sessions.storage"),
        columns: SESSIONS_COLUMNS,
        default_sort: Some(("start_time", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "visits",
        label: message!("Database.tables.visits.label"),
        description: message!("Database.tables.visits.description"),
        storage: message!("Database.tables.visits.storage"),
        columns: VISITS_COLUMNS,
        default_sort: Some(("join_time", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "with_users",
        label: message!("Database.tables.withUsers.label"),
        description: message!("Database.tables.withUsers.description"),
        storage: message!("Database.tables.withUsers.storage"),
        columns: WITH_USERS_COLUMNS,
        default_sort: Some(("join_time", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "find_users",
        label: message!("Database.tables.findUsers.label"),
        description: message!("Database.tables.findUsers.description"),
        storage: message!("Database.tables.findUsers.storage"),
        columns: FIND_USERS_COLUMNS,
        default_sort: None,
        is_view: false,
    },
    TableComment {
        name: "notifications",
        label: message!("Database.tables.notifications.label"),
        description: message!("Database.tables.notifications.description"),
        storage: message!("Database.tables.notifications.storage"),
        columns: NOTIFICATIONS_COLUMNS,
        default_sort: Some(("received_at", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "screenshots",
        label: message!("Database.tables.screenshots.label"),
        description: message!("Database.tables.screenshots.description"),
        storage: message!("Database.tables.screenshots.storage"),
        columns: SCREENSHOTS_COLUMNS,
        default_sort: Some(("timestamp", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "osc",
        label: message!("Database.tables.osc.label"),
        description: message!("Database.tables.osc.description"),
        storage: message!("Database.tables.osc.storage"),
        columns: OSC_COLUMNS,
        default_sort: Some(("timestamp", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "subscription",
        label: message!("Database.tables.subscription.label"),
        description: message!("Database.tables.subscription.description"),
        storage: message!("Database.tables.subscription.storage"),
        columns: SUBSCRIPTION_COLUMNS,
        default_sort: Some(("checked_at", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "apps",
        label: message!("Database.tables.apps.label"),
        description: message!("Database.tables.apps.description"),
        storage: message!("Database.tables.apps.storage"),
        columns: APPS_COLUMNS,
        default_sort: None,
        is_view: false,
    },
    // ── ビュー ──
    TableComment {
        name: "visit_summary",
        label: message!("Database.tables.visitSummary.label"),
        description: message!("Database.tables.visitSummary.description"),
        storage: message!("Database.tables.visitSummary.storage"),
        columns: VISIT_SUMMARY_COLUMNS,
        default_sort: Some(("join_time", "DESC")),
        is_view: true,
    },
    TableComment {
        name: "with_users_detail",
        label: message!("Database.tables.withUsersDetail.label"),
        description: message!("Database.tables.withUsersDetail.description"),
        storage: message!("Database.tables.withUsersDetail.storage"),
        columns: WITH_USERS_DETAIL_COLUMNS,
        default_sort: Some(("join_time", "DESC")),
        is_view: true,
    },
    TableComment {
        name: "screenshots_detail",
        label: message!("Database.tables.screenshotsDetail.label"),
        description: message!("Database.tables.screenshotsDetail.description"),
        storage: message!("Database.tables.screenshotsDetail.storage"),
        columns: SCREENSHOTS_DETAIL_COLUMNS,
        default_sort: Some(("timestamp", "DESC")),
        is_view: true,
    },
];

/// SQL に補間する前にテーブル名を検証する。
///
/// パラメータバインドではテーブル名を動的に指定できないため、
/// ASCII 英数字と `_` のみを許可してインジェクションを防ぐ。
///
/// # 戻り値
/// ASCII 英数字と `_` のみを含む場合は元のテーブル名。
///
/// # Errors
/// それ以外の文字が含まれる場合にエラーを返し、プレビュークエリの悪用を防ぐ。
fn sanitize_table_name(table_name: &str) -> Result<&str, String> {
    if utils::is_sql_identifier(table_name) {
        Ok(table_name)
    } else {
        Err(message!("Commands.Database.sanitizeTableName.tableInvalid").to_string())
    }
}

/// データベーステーブルの登録済み UI メタデータを検索する。
fn get_table_comment(table_name: &str) -> Option<&'static TableComment> {
    TABLE_COMMENTS.iter().find(|table_comment| table_comment.name == table_name)
}

/// テーブル内のcolumnの登録済み UI メタデータを検索する。
fn get_column_comment(table_name: &str, column_name: &str) -> Option<&'static ColumnComment> {
    get_table_comment(table_name).and_then(|table_comment| table_comment.columns.iter().find(|column_comment| column_comment.name == column_name))
}

/// `ORDER BY` に補間できるcolumn名か確認する。
///
/// column名は値バインドできないため、文字種だけでなく登録済みメタデータ上に
/// 存在することも確認する。IPC から未知columnを直接渡された場合は
/// SQL エラーに落とさず、明示的な入力エラーとして返す。
fn sanitize_sort_column(table_name: &str, column_name: &str) -> Result<&'static str, String> {
    if !utils::is_sql_identifier(column_name) {
        return Err(message!("Commands.Database.sanitizeSortColumn.columnInvalid").to_string());
    }

    get_column_comment(table_name, column_name).map(|column_comment| column_comment.name).ok_or_else(|| format!(message!("Commands.Database.sanitizeSortColumn.columnMissing"), column_name = column_name))
}

/// ビューアに表示するテーブル・ビューを `TABLE_COMMENTS` の登録順で返す。
///
/// `TABLE_COMMENTS` に登録されたもののみ表示し、未登録のテーブル/ビューは非表示。
fn list_visible_objects(path: &Path) -> Result<Vec<String>, String> {
    let conn = rusqlite::Connection::open(path).map_err(|err| utils::command_open_err(path, err))?;
    let mut stmt = conn
        .prepare(
            "SELECT name
             FROM sqlite_master
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'",
        )
        .map_err(|err| utils::command_err(message!("Commands.Database.listVisibleObjects.tablePrepareFailed"), err))?;

    let rows = stmt.query_map([], |row| row.get::<_, String>(0)).map_err(|err| utils::command_err(message!("Commands.Database.listVisibleObjects.tableFailed"), err))?;

    let mut db_objects: Vec<String> = Vec::new();
    for row in rows {
        match row {
            Ok(name) => db_objects.push(name),
            Err(err) => utils::log_warn(&format!(message!("Commands.Database.listVisibleObjects.tableRowDecodeFailed"), err = err)),
        }
    }

    Ok(TABLE_COMMENTS.iter().filter(|tc| db_objects.contains(&tc.name.to_string())).map(|tc| tc.name.to_string()).collect())
}

/// 開かれた `SQLite` 接続内に指定名のテーブルまたはビューが存在するか確認する。
fn object_exists(conn: &rusqlite::Connection, name: &str) -> Result<bool, String> {
    let exists = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM sqlite_master
                WHERE type IN ('table', 'view') AND name = ?1
            )",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|err| utils::command_err(message!("Commands.Database.objectExists.tableVerifyFailed"), err))?;

    Ok(exists == 1)
}

/// 要求されたプレビューテーブルを所有するデータベースを開く。
fn open_preview_database(table_name: &str) -> Result<(rusqlite::Connection, String), String> {
    if get_table_comment(table_name).is_none() {
        return Err(format!(message!("Commands.Database.openPreviewDatabase.registrationTableText"), table_name = table_name));
    }

    let db_path = get_db_path()?;
    if db_path.exists() {
        let conn = rusqlite::Connection::open(&db_path).map_err(|err| utils::command_open_err(&db_path, err))?;
        if object_exists(&conn, table_name)? {
            return Ok((conn, "Main DB".to_string()));
        }
    }

    Err(format!(message!("Commands.Database.openPreviewDatabase.tableMissing"), table_name = table_name))
}

/// 読み取り専用プレビュー可能な DB テーブルを一覧取得する。
///
/// # Errors
/// データベースを列挙できない場合にエラーを返す。
#[tauri::command]
pub fn get_db_tables() -> Result<Vec<DbTableSummary>, String> {
    let db_path = get_db_path()?;
    let visible = if db_path.exists() { list_visible_objects(&db_path)? } else { Vec::new() };

    Ok(visible
        .into_iter()
        .map(|name| {
            let comment = get_table_comment(&name);
            DbTableSummary {
                label: comment.map_or_else(|| name.clone(), |value| value.label.to_string()),
                description: comment.map_or_else(|| message!("Commands.Database.getDbTables.tableRegistrationText").to_string(), |value| value.description.to_string()),
                storage: comment.map_or_else(|| "DB".to_string(), |value| value.storage.to_string()),
                is_view: comment.is_some_and(|value| value.is_view),
                name,
            }
        })
        .collect())
}

/// DB プレビューで1ページあたりに返す最大行数。
/// Frontend側の `pageSize` と一致させる。
const PAGE_SIZE: u32 = 500;

/// 選択された DB テーブルのページネーション付きプレビュー行とcolumnメタデータを読み取る。
///
/// 並び順は `sort_column` / `sort_dir` 引数が優先され、未指定時は
/// `TABLE_COMMENTS` の `default_sort` にフォールバックする。1ページあたり
/// 最大 `PAGE_SIZE` 行を返す。
///
/// # Errors
/// テーブル名が不正、またはプレビュークエリに失敗した場合にエラーを返す。
#[tauri::command]
pub fn get_db_table_data(table_name: &str, page: Option<u32>, sort_column: Option<String>, sort_dir: Option<String>) -> Result<TableData, String> {
    let table_name = sanitize_table_name(table_name)?;
    let (conn, storage_label) = open_preview_database(table_name)?;
    fetch_table_data(&conn, table_name, &storage_label, page, sort_column, sort_dir)
}

/// 開かれた接続から1テーブルのページネーション付きデータを取得する。
///
/// 接続を引数で受け取ることで、registryや実 DB ファイルを経由せず
/// インメモリ DB に対してカウント・ソート・ページング・セル整形ロジックを検証できる。
fn fetch_table_data(conn: &rusqlite::Connection, table_name: &str, storage_label: &str, page: Option<u32>, sort_column: Option<String>, sort_dir: Option<String>) -> Result<TableData, String> {
    let total_rows: u32 = conn.query_row(&format!("SELECT COUNT(*) FROM {table_name}"), [], |row| row.get(0)).map_err(|err| utils::command_err(message!("Commands.Database.fetchTableData.failed"), err))?;

    let offset = page.unwrap_or(0).saturating_mul(PAGE_SIZE);
    let order_clause = match sort_column {
        Some(ref col) => {
            let col = sanitize_sort_column(table_name, col)?;
            let dir = match sort_dir.as_deref() {
                Some("asc") => "ASC",
                _ => "DESC",
            };
            format!("ORDER BY {col} {dir}")
        }
        _ => get_table_comment(table_name).and_then(|tc| tc.default_sort).map_or_else(String::new, |(col, dir)| format!("ORDER BY {col} {dir}")),
    };
    let sql = format!("SELECT * FROM {table_name} {order_clause} LIMIT {PAGE_SIZE} OFFSET {offset}");
    let mut stmt = conn.prepare(&sql).map_err(|err| utils::command_err(&format!(message!("Commands.Database.fetchTableData.prepareFailed"), sql = sql), err))?;

    let column_count = stmt.column_count();
    let columns = stmt
        .column_names()
        .iter()
        .map(|name| {
            let comment = get_column_comment(table_name, name);
            DbColumnMeta {
                name: (*name).to_string(),
                label: comment.map_or_else(|| (*name).to_string(), |value| value.label.to_string()),
                description: comment.map_or_else(|| message!("Commands.Database.fetchTableData.columnRegistrationText").to_string(), |value| value.description.to_string()),
            }
        })
        .collect::<Vec<_>>();

    let mut rows = stmt.query([]).map_err(|err| utils::command_err(message!("Commands.Database.fetchTableData.tableFailed"), err))?;
    let mut result_rows = Vec::new();

    while let Some(row) = rows.next().map_err(|err| utils::command_err(message!("Commands.Database.fetchTableData.tableReadFailed"), err))? {
        let mut values = Vec::with_capacity(column_count);
        for index in 0..column_count {
            let value: rusqlite::types::Value = row.get(index).map_err(|err| utils::command_err(message!("Commands.Database.fetchTableData.tableCellDecodeFailed"), err))?;
            values.push(match value {
                rusqlite::types::Value::Null => "NULL".to_string(),
                rusqlite::types::Value::Integer(number) => number.to_string(),
                rusqlite::types::Value::Real(number) => number.to_string(),
                rusqlite::types::Value::Text(text) => text,
                rusqlite::types::Value::Blob(_) => "<BLOB>".to_string(),
            });
        }
        result_rows.push(values);
    }

    let table_comment = get_table_comment(table_name);
    Ok(TableData {
        name: table_name.to_string(),
        label: table_comment.map_or_else(|| table_name.to_string(), |value| value.label.to_string()),
        description: table_comment.map_or_else(|| message!("Commands.Database.fetchTableData.tableRegistrationText").to_string(), |value| value.description.to_string()),
        storage: table_comment.map_or_else(|| storage_label.to_string(), |value| value.storage.to_string()),
        columns,
        rows: result_rows,
        total_rows,
    })
}

#[cfg(test)]
include!("database_tests.rs");
