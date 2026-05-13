//! フロントエンドプレビューパネル用の読み取り専用データベーステーブルブラウザ。
//!
//! このモジュールの定数（`TABLE_COMMENTS`、テーブルごとのカラム配列）は生の
//! SQLite スキーマ名を日本語の表示ラベルにマッピングし、TypeScript に
//! 表示文字列をハードコードせずにフレンドリーなテーブルブラウザを描画可能にする。

use std::path::Path;

use crate::models::{DbColumnMeta, DbTableSummary, TableData};
use crate::utils;

use super::get_db_path;

/// プレビュー可能なテーブルカラム1件の表示用メタデータ。
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
    ColumnComment { name: "id", label: "ID", description: "解析セッションの内部ID" },
    ColumnComment { name: "log_name", label: "元ログ名", description: "解析元になった VRChat ログのファイル名" },
    ColumnComment { name: "account_id", label: "アカウントID", description: "ログに記録された自分の VRChat ユーザーID" },
    ColumnComment { name: "account_name", label: "アカウント名", description: "ログ時点での自分の表示名" },
    ColumnComment { name: "start_time", label: "開始時刻", description: "このログセッションの開始時刻" },
    ColumnComment { name: "end_time", label: "終了時刻", description: "このログセッションの終了時刻" },
];

const VISITS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: "ID", description: "ワールド訪問レコードの内部ID" },
    ColumnComment { name: "session_id", label: "セッション", description: "親の解析セッション" },
    ColumnComment { name: "world_name", label: "ワールド名", description: "訪問先ワールドの表示名" },
    ColumnComment { name: "instance_id", label: "インスタンスID", description: "訪問先インスタンスの識別子" },
    ColumnComment { name: "instance_type", label: "インスタンスタイプ", description: "公開区分" },
    ColumnComment { name: "region", label: "リージョン", description: "ワールドサーバーのリージョン情報" },
    ColumnComment { name: "join_time", label: "Join時刻", description: "ワールドへ入った時刻" },
    ColumnComment { name: "leave_time", label: "Leave時刻", description: "ワールドを離れた時刻" },
];

const FIND_USERS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "vrchat_id", label: "ユーザーID", description: "VRChat の usr_xxx 識別子" },
    ColumnComment { name: "account_name", label: "ユーザー名", description: "観測時点のプレイヤー表示名" },
];

const WITH_USERS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: "ID", description: "同席レコードの内部ID" },
    ColumnComment { name: "visit_id", label: "Join", description: "どのワールド訪問中の記録か" },
    ColumnComment { name: "vrchat_id", label: "ユーザーID", description: "プレイヤーの VRChat ID" },
    ColumnComment { name: "is_self", label: "is_self", description: "自分自身の入室記録かどうか" },
    ColumnComment { name: "join_time", label: "Join時刻", description: "そのプレイヤーを観測した開始時刻" },
    ColumnComment { name: "leave_time", label: "Leave時刻", description: "そのプレイヤーを観測しなくなった時刻" },
];

const WITH_USERS_DETAIL_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: "ID", description: "同席レコードの内部ID" },
    ColumnComment { name: "visit_id", label: "Join", description: "どのワールド訪問中の記録か" },
    ColumnComment { name: "world_name", label: "ワールド名", description: "同席していたワールド" },
    ColumnComment { name: "vrchat_id", label: "ユーザーID", description: "プレイヤーの VRChat ID" },
    ColumnComment { name: "user_name", label: "ユーザー名", description: "プレイヤーの表示名" },
    ColumnComment { name: "is_self", label: "is_self", description: "自分自身の入室記録かどうか" },
    ColumnComment { name: "join_time", label: "Join時刻", description: "そのプレイヤーを観測した開始時刻" },
    ColumnComment { name: "leave_time", label: "Leave時刻", description: "そのプレイヤーを観測しなくなった時刻" },
];

const FAVORITES_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: "ID", description: "お気に入り変更イベントの内部ID" },
    ColumnComment { name: "session_id", label: "セッション", description: "操作が発生した解析セッション" },
    ColumnComment { name: "target_type", label: "対象種別", description: "friend / avatar / world" },
    ColumnComment { name: "target_id", label: "対象ID", description: "対象の識別子 (usr_ / wrld_) またはアバター名" },
    ColumnComment { name: "action", label: "操作", description: "added (追加) / removed (削除)" },
    ColumnComment { name: "timestamp", label: "操作時刻", description: "お気に入りを変更した時刻" },
];

const FAVORITES_DETAIL_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: "ID", description: "お気に入り変更イベントの内部ID" },
    ColumnComment { name: "session_id", label: "セッション", description: "操作が発生した解析セッション" },
    ColumnComment { name: "target_type", label: "対象種別", description: "friend / avatar / world" },
    ColumnComment { name: "target_id", label: "対象ID", description: "対象の識別子 (usr_ / wrld_) またはアバター名" },
    ColumnComment { name: "target_name", label: "対象名", description: "フレンドは表示名に解決、それ以外はIDそのまま" },
    ColumnComment { name: "action", label: "操作", description: "added (追加) / removed (削除)" },
    ColumnComment { name: "timestamp", label: "操作時刻", description: "お気に入りを変更した時刻" },
];

const VISIT_SUMMARY_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "visit_id", label: "ID", description: "ワールド訪問の内部ID" },
    ColumnComment { name: "world_name", label: "ワールド名", description: "訪問先ワールドの表示名" },
    ColumnComment { name: "instance_id", label: "インスタンスID", description: "インスタンスの識別子" },
    ColumnComment { name: "instance_type", label: "インスタンスタイプ", description: "公開区分" },
    ColumnComment { name: "region", label: "リージョン", description: "サーバーリージョン" },
    ColumnComment { name: "join_time", label: "Join時刻", description: "入室時刻" },
    ColumnComment { name: "leave_time", label: "Leave時刻", description: "退室時刻" },
    ColumnComment { name: "duration_sec", label: "滞在秒数", description: "滞在時間（秒）" },
    ColumnComment { name: "other_player_count", label: "他プレイヤー数", description: "同室していた他プレイヤーの数" },
];

const PLAYER_STATS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "vrchat_id", label: "ユーザーID", description: "VRChat の usr_xxx 識別子" },
    ColumnComment { name: "account_name", label: "ユーザー名", description: "プレイヤーの表示名" },
    ColumnComment { name: "co_visit_count", label: "同室回数", description: "一緒にいたワールド訪問の回数" },
    ColumnComment { name: "first_met", label: "初回", description: "初めて同室した時刻" },
    ColumnComment { name: "last_met", label: "最終", description: "最後に同室した時刻" },
];

const NOTIFICATIONS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: "ID", description: "通知履歴の内部ID" },
    ColumnComment { name: "notif_type", label: "通知種別", description: "通知タイプ" },
    ColumnComment { name: "message", label: "本文", description: "通知メッセージ本文" },
    ColumnComment { name: "sender_name", label: "送信者名", description: "通知送信者の表示名" },
    ColumnComment { name: "sender_user_id", label: "送信者ID", description: "通知送信者のユーザーID" },
    ColumnComment { name: "notif_id", label: "VRC通知ID", description: "VRChat 側の通知識別子" },
    ColumnComment { name: "session_id", label: "セッション", description: "通知を受けた解析セッション" },
    ColumnComment { name: "created_at", label: "作成時刻", description: "通知側での作成時刻" },
    ColumnComment { name: "received_at", label: "受信時刻", description: "STELLA RECORD が受け取った時刻" },
    ColumnComment { name: "target_world_name", label: "遷移先ワールド名", description: "通知から推定した遷移先ワールド名" },
    ColumnComment { name: "target_instance_id", label: "遷移先インスタンスID", description: "通知から推定したインスタンス識別子" },
    ColumnComment { name: "target_instance_type", label: "遷移先インスタンスタイプ", description: "通知から推定した公開区分" },
    ColumnComment { name: "target_owner", label: "遷移先オーナー", description: "通知から推定したインスタンス所有者" },
    ColumnComment { name: "target_region", label: "遷移先リージョン", description: "通知から推定したリージョン情報" },
];

const SCREENSHOTS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: "ID", description: "撮影イベントの内部ID" },
    ColumnComment { name: "visit_id", label: "Join", description: "撮影時に滞在していたワールド訪問" },
    ColumnComment { name: "file_path", label: "ファイルパス", description: "スクリーンショットの保存先フルパス" },
    ColumnComment { name: "resolution_width", label: "幅", description: "撮影解像度の幅 (px)" },
    ColumnComment { name: "resolution_height", label: "高さ", description: "撮影解像度の高さ (px)" },
    ColumnComment { name: "timestamp", label: "撮影時刻", description: "スクリーンショットを撮影した時刻" },
];

const SCREENSHOTS_DETAIL_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: "ID", description: "撮影イベントの内部ID" },
    ColumnComment { name: "visit_id", label: "Join", description: "撮影時に滞在していたワールド訪問" },
    ColumnComment { name: "world_name", label: "ワールド名", description: "撮影時に滞在していたワールド" },
    ColumnComment { name: "file_path", label: "ファイルパス", description: "スクリーンショットの保存先フルパス" },
    ColumnComment { name: "resolution_width", label: "幅", description: "撮影解像度の幅 (px)" },
    ColumnComment { name: "resolution_height", label: "高さ", description: "撮影解像度の高さ (px)" },
    ColumnComment { name: "timestamp", label: "撮影時刻", description: "スクリーンショットを撮影した時刻" },
];

const OSC_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: "ID", description: "OSCサービスイベントの内部ID" },
    ColumnComment { name: "session_id", label: "セッション", description: "イベントが発生した解析セッション" },
    ColumnComment { name: "event_type", label: "イベント種別", description: "found (外部ツール検出)" },
    ColumnComment { name: "service_name", label: "サービス名", description: "OSCサービスの識別名 (例: OyasumiVR)" },
    ColumnComment { name: "service_type", label: "サービス種別", description: "OSC / OSCQuery の区別" },
    ColumnComment { name: "ip_address", label: "IPアドレス", description: "検出時の接続先IPアドレス" },
    ColumnComment { name: "port", label: "ポート番号", description: "OSCサービスのポート番号" },
    ColumnComment { name: "timestamp", label: "検出時刻", description: "OSCサービスを検出または告知した時刻" },
];


const SUBSCRIPTION_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "id", label: "ID", description: "サブスクリプション状態の内部ID" },
    ColumnComment { name: "session_id", label: "セッション", description: "確認が発生した解析セッション" },
    ColumnComment { name: "is_active", label: "有効フラグ", description: "VRChat+ が有効かどうか" },
    ColumnComment { name: "subscription_id", label: "VRC契約ID", description: "VRChat 側のサブスクリプション識別子 (NULL=無効)" },
    ColumnComment { name: "description", label: "説明", description: "サブスクリプション種別の説明テキスト" },
    ColumnComment { name: "checked_at", label: "確認時刻", description: "サブスクリプション状態を確認した時刻" },
];

const APPS_COLUMNS: &[ColumnComment] = &[
    ColumnComment { name: "name", label: "アプリ名", description: "連携アプリの表示名" },
    ColumnComment { name: "description", label: "説明", description: "連携アプリの説明文" },
    ColumnComment { name: "path", label: "パス", description: "実行ファイルのパス" },
    ColumnComment { name: "icon", label: "アイコン", description: "アイコン画像データ (BLOB)" },
];

const TABLE_COMMENTS: &[TableComment] = &[
    // ── テーブル ──
    TableComment { name: "sessions", label: "セッション", description: "ログ単位の解析セッション", storage: "Main DB", columns: SESSIONS_COLUMNS, default_sort: Some(("start_time", "DESC")), is_view: false },
    TableComment { name: "visits", label: "Join履歴", description: "ワールド入退室の記録", storage: "Main DB", columns: VISITS_COLUMNS, default_sort: Some(("join_time", "DESC")), is_view: false },
    TableComment { name: "with_users", label: "同室ユーザー", description: "プレイヤー同席の記録", storage: "Main DB", columns: WITH_USERS_COLUMNS, default_sort: Some(("join_time", "DESC")), is_view: false },
    TableComment { name: "find_users", label: "ユーザー一覧", description: "観測プレイヤーの基本情報", storage: "Main DB", columns: FIND_USERS_COLUMNS, default_sort: None, is_view: false },
    TableComment { name: "notifications", label: "通知", description: "招待・フレンド申請など", storage: "Main DB", columns: NOTIFICATIONS_COLUMNS, default_sort: Some(("received_at", "DESC")), is_view: false },
    TableComment { name: "screenshots", label: "スクリーンショット", description: "VRC Camera の撮影記録", storage: "Main DB", columns: SCREENSHOTS_COLUMNS, default_sort: Some(("timestamp", "DESC")), is_view: false },
    TableComment { name: "osc", label: "OSC", description: "OSCサービスの検出履歴", storage: "Main DB", columns: OSC_COLUMNS, default_sort: Some(("timestamp", "DESC")), is_view: false },
    TableComment { name: "favorites", label: "お気に入り", description: "追加/削除イベント", storage: "Main DB", columns: FAVORITES_COLUMNS, default_sort: Some(("timestamp", "DESC")), is_view: false },
    TableComment { name: "subscription", label: "サブスクリプション", description: "VRChat+ 加入状態", storage: "Main DB", columns: SUBSCRIPTION_COLUMNS, default_sort: Some(("checked_at", "DESC")), is_view: false },
    TableComment { name: "apps", label: "連携アプリ", description: "ランチャー登録アプリ", storage: "Main DB", columns: APPS_COLUMNS, default_sort: None, is_view: false },
    // ── ビュー ──
    TableComment { name: "visit_summary", label: "ワールド訪問", description: "滞在時間・同室人数付き", storage: "Main DB", columns: VISIT_SUMMARY_COLUMNS, default_sort: Some(("join_time", "DESC")), is_view: true },
    TableComment { name: "player_stats", label: "プレイヤー統計", description: "同室回数・初回/最終", storage: "Main DB", columns: PLAYER_STATS_COLUMNS, default_sort: Some(("co_visit_count", "DESC")), is_view: true },
    TableComment { name: "with_users_detail", label: "同室詳細", description: "ワールド名・ユーザー名付き", storage: "Main DB", columns: WITH_USERS_DETAIL_COLUMNS, default_sort: Some(("join_time", "DESC")), is_view: true },
    TableComment { name: "favorites_detail", label: "お気に入り詳細", description: "フレンド名を解決済み", storage: "Main DB", columns: FAVORITES_DETAIL_COLUMNS, default_sort: Some(("timestamp", "DESC")), is_view: true },
    TableComment { name: "screenshots_detail", label: "スクショ詳細", description: "ワールド名付き", storage: "Main DB", columns: SCREENSHOTS_DETAIL_COLUMNS, default_sort: Some(("timestamp", "DESC")), is_view: true },
];

/// SQL に補間する前にテーブル名を検証する。
///
/// # 戻り値
/// ASCII 英数字と `_` のみを含む場合は元のテーブル名。
///
/// # エラー
/// それ以外の文字が含まれる場合にエラーを返し、プレビュークエリの悪用を防ぐ。
fn sanitize_table_name(table_name: &str) -> Result<&str, String> {
    if !table_name.is_empty()
        && table_name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        Ok(table_name)
    } else {
        Err("テーブル名が不正です".to_string())
    }
}

/// データベーステーブルの登録済み UI メタデータを検索する。
fn get_table_comment(table_name: &str) -> Option<&'static TableComment> {
    TABLE_COMMENTS
        .iter()
        .find(|table_comment| table_comment.name == table_name)
}

/// テーブル内のカラムの登録済み UI メタデータを検索する。
fn get_column_comment(table_name: &str, column_name: &str) -> Option<&'static ColumnComment> {
    get_table_comment(table_name).and_then(|table_comment| {
        table_comment
            .columns
            .iter()
            .find(|column_comment| column_comment.name == column_name)
    })
}

/// ビューアに表示するテーブル・ビューを `TABLE_COMMENTS` の登録順で返す。
///
/// `TABLE_COMMENTS` に登録されたもののみ表示し、未登録のテーブル/ビューは非表示。
fn list_visible_objects(path: &Path) -> Result<Vec<String>, String> {
    let conn =
        rusqlite::Connection::open(path).map_err(|err| utils::command_open_err(path, err))?;
    let mut stmt = conn
        .prepare(
            "SELECT name
             FROM sqlite_master
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'",
        )
        .map_err(|err| utils::command_err("テーブル一覧クエリを準備できませんでした", err))?;

    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| utils::command_err("テーブル一覧クエリを実行できませんでした", err))?;

    let mut db_objects: Vec<String> = Vec::new();
    for row in rows {
        match row {
            Ok(name) => db_objects.push(name),
            Err(err) => utils::log_warn(&format!("テーブル行をデコードできませんでした: {err}")),
        }
    }

    Ok(TABLE_COMMENTS
        .iter()
        .filter(|tc| db_objects.contains(&tc.name.to_string()))
        .map(|tc| tc.name.to_string())
        .collect())
}

/// 開かれた SQLite 接続内に指定名のテーブルまたはビューが存在するか確認する。
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
        .map_err(|err| utils::command_err("テーブル存在確認に失敗しました", err))?;

    Ok(exists == 1)
}

/// 要求されたプレビューテーブルを所有するデータベースを開く。
fn open_preview_database(table_name: &str) -> Result<(rusqlite::Connection, String), String> {
    let db_path = get_db_path()?;
    if db_path.exists() {
        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|err| utils::command_open_err(&db_path, err))?;
        if object_exists(&conn, table_name)? {
            return Ok((conn, "Main DB".to_string()));
        }
    }

    Err(format!("指定テーブルが見つかりませんでした: {table_name}"))
}

/// 読み取り専用プレビュー可能な DB テーブルを一覧取得する。
///
/// # エラー
/// メインまたは拡張データベースを列挙できない場合にエラーを返す。
#[tauri::command]
pub fn get_db_tables() -> Result<Vec<DbTableSummary>, String> {
    let db_path = get_db_path()?;
    let visible = if db_path.exists() {
        list_visible_objects(&db_path)?
    } else {
        Vec::new()
    };

    Ok(visible
        .into_iter()
        .map(|name| {
            let comment = get_table_comment(&name);
            DbTableSummary {
                label: comment.map_or_else(|| name.clone(), |value| value.label.to_string()),
                description: comment.map_or_else(
                    || "テーブル説明は未登録です。".to_string(),
                    |value| value.description.to_string(),
                ),
                storage: comment
                    .map_or_else(|| "DB".to_string(), |value| value.storage.to_string()),
                is_view: comment.is_some_and(|value| value.is_view),
                name,
            }
        })
        .collect())
}

/// プレビューページあたりの最大行数。フロントエンドの仮想スクロールウィンドウに合わせる。
const PAGE_SIZE: u32 = 500;

/// 選択された DB テーブルのページネーション付きプレビュー行とカラムメタデータを読み取る。
///
/// 並び順は `sort_column` / `sort_dir` 引数が優先され、未指定時は
/// `TABLE_COMMENTS` の `default_sort` にフォールバックする。1ページあたり
/// 最大 `PAGE_SIZE` 行を返す。
///
/// # エラー
/// テーブル名が不正、またはプレビュークエリに失敗した場合にエラーを返す。
#[tauri::command]
pub fn get_db_table_data(
    table_name: &str,
    page: Option<u32>,
    sort_column: Option<String>,
    sort_dir: Option<String>,
) -> Result<TableData, String> {
    let table_name = sanitize_table_name(table_name)?;
    let (conn, storage_label) = open_preview_database(table_name)?;

    let total_rows: u32 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM {table_name}"),
            [],
            |row| row.get(0),
        )
        .map_err(|err| utils::command_err("行数カウントに失敗しました", err))?;

    let offset = page.unwrap_or(0) * PAGE_SIZE;
    let order_clause = match sort_column {
        Some(ref col) if !col.is_empty() && col.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => {
            let dir = match sort_dir.as_deref() {
                Some("asc") => "ASC",
                _ => "DESC",
            };
            format!("ORDER BY {col} {dir}")
        }
        _ => get_table_comment(table_name)
            .and_then(|tc| tc.default_sort)
            .map_or_else(String::new, |(col, dir)| format!("ORDER BY {col} {dir}")),
    };
    let sql = format!("SELECT * FROM {table_name} {order_clause} LIMIT {PAGE_SIZE} OFFSET {offset}");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|err| utils::command_err(&format!("クエリを準備できませんでした [{sql}]"), err))?;

    let column_count = stmt.column_count();
    let columns = stmt
        .column_names()
        .iter()
        .map(|name| {
            let comment = get_column_comment(table_name, name);
            DbColumnMeta {
                name: (*name).to_string(),
                label: comment.map_or_else(|| (*name).to_string(), |value| value.label.to_string()),
                description: comment.map_or_else(
                    || "カラム説明は未登録です。".to_string(),
                    |value| value.description.to_string(),
                ),
            }
        })
        .collect::<Vec<_>>();

    let mut rows = stmt
        .query([])
        .map_err(|err| utils::command_err("テーブルデータクエリを実行できませんでした", err))?;
    let mut result_rows = Vec::new();

    while let Some(row) = rows
        .next()
        .map_err(|err| utils::command_err("テーブル行を取得できませんでした", err))?
    {
        let mut values = Vec::with_capacity(column_count);
        for index in 0..column_count {
            let value: rusqlite::types::Value = row
                .get(index)
                .map_err(|err| utils::command_err("テーブルセルをデコードできませんでした", err))?;
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
        label: table_comment
            .map_or_else(|| table_name.to_string(), |value| value.label.to_string()),
        description: table_comment.map_or_else(
            || "テーブル説明は未登録です。".to_string(),
            |value| value.description.to_string(),
        ),
        storage: table_comment.map_or(storage_label, |value| value.storage.to_string()),
        columns,
        rows: result_rows,
        total_rows,
    })
}

