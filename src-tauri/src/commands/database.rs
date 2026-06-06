//! フロントエンドプレビューパネル用の読み取り専用データベーステーブルブラウザ。
//!
//! このモジュールの定数（`TABLE_COMMENTS`、テーブルごとのカラム配列）は生の
//! `SQLite` スキーマ名を日本語の表示ラベルにマッピングし、TypeScript に
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
    ColumnComment {
        name: "id",
        label: "ID",
        description: "解析セッションの内部ID",
    },
    ColumnComment {
        name: "log_name",
        label: "元ログ名",
        description: "解析元になった VRChat ログのファイル名",
    },
    ColumnComment {
        name: "account_id",
        label: "アカウントID",
        description: "ログに記録された自分の VRChat ユーザーID",
    },
    ColumnComment {
        name: "account_name",
        label: "アカウント名",
        description: "ログ時点での自分の表示名",
    },
    ColumnComment {
        name: "start_time",
        label: "開始時刻",
        description: "このログセッションの開始時刻",
    },
    ColumnComment {
        name: "end_time",
        label: "終了時刻",
        description: "このログセッションの終了時刻",
    },
];

const VISITS_COLUMNS: &[ColumnComment] = &[
    ColumnComment {
        name: "id",
        label: "ID",
        description: "ワールド訪問レコードの内部ID",
    },
    ColumnComment {
        name: "session_id",
        label: "セッション",
        description: "親の解析セッション",
    },
    ColumnComment {
        name: "world_name",
        label: "ワールド名",
        description: "訪問先ワールドの表示名",
    },
    ColumnComment {
        name: "instance_id",
        label: "インスタンスID",
        description: "訪問先インスタンスの識別子",
    },
    ColumnComment {
        name: "instance_type",
        label: "インスタンスタイプ",
        description: "公開区分",
    },
    ColumnComment {
        name: "region",
        label: "リージョン",
        description: "ワールドサーバーのリージョン情報",
    },
    ColumnComment {
        name: "join_time",
        label: "Join時刻",
        description: "ワールドへ入った時刻",
    },
    ColumnComment {
        name: "leave_time",
        label: "Leave時刻",
        description: "ワールドを離れた時刻",
    },
];

const FIND_USERS_COLUMNS: &[ColumnComment] = &[
    ColumnComment {
        name: "vrchat_id",
        label: "ユーザーID",
        description: "VRChat の usr_xxx 識別子",
    },
    ColumnComment {
        name: "account_name",
        label: "ユーザー名",
        description: "観測時点のプレイヤー表示名",
    },
];

const WITH_USERS_COLUMNS: &[ColumnComment] = &[
    ColumnComment {
        name: "id",
        label: "ID",
        description: "同席レコードの内部ID",
    },
    ColumnComment {
        name: "visit_id",
        label: "Join",
        description: "どのワールド訪問中の記録か",
    },
    ColumnComment {
        name: "vrchat_id",
        label: "ユーザーID",
        description: "プレイヤーの VRChat ID",
    },
    ColumnComment {
        name: "is_self",
        label: "is_self",
        description: "自分自身の入室記録かどうか",
    },
    ColumnComment {
        name: "join_time",
        label: "Join時刻",
        description: "そのプレイヤーを観測した開始時刻",
    },
    ColumnComment {
        name: "leave_time",
        label: "Leave時刻",
        description: "そのプレイヤーを観測しなくなった時刻",
    },
];

const WITH_USERS_DETAIL_COLUMNS: &[ColumnComment] = &[
    ColumnComment {
        name: "id",
        label: "ID",
        description: "同席レコードの内部ID",
    },
    ColumnComment {
        name: "visit_id",
        label: "Join",
        description: "どのワールド訪問中の記録か",
    },
    ColumnComment {
        name: "world_name",
        label: "ワールド名",
        description: "同席していたワールド",
    },
    ColumnComment {
        name: "vrchat_id",
        label: "ユーザーID",
        description: "プレイヤーの VRChat ID",
    },
    ColumnComment {
        name: "user_name",
        label: "ユーザー名",
        description: "プレイヤーの表示名",
    },
    ColumnComment {
        name: "is_self",
        label: "is_self",
        description: "自分自身の入室記録かどうか",
    },
    ColumnComment {
        name: "join_time",
        label: "Join時刻",
        description: "そのプレイヤーを観測した開始時刻",
    },
    ColumnComment {
        name: "leave_time",
        label: "Leave時刻",
        description: "そのプレイヤーを観測しなくなった時刻",
    },
];

const VISIT_SUMMARY_COLUMNS: &[ColumnComment] = &[
    ColumnComment {
        name: "visit_id",
        label: "ID",
        description: "ワールド訪問の内部ID",
    },
    ColumnComment {
        name: "world_name",
        label: "ワールド名",
        description: "訪問先ワールドの表示名",
    },
    ColumnComment {
        name: "instance_id",
        label: "インスタンスID",
        description: "インスタンスの識別子",
    },
    ColumnComment {
        name: "instance_type",
        label: "インスタンスタイプ",
        description: "公開区分",
    },
    ColumnComment {
        name: "region",
        label: "リージョン",
        description: "サーバーリージョン",
    },
    ColumnComment {
        name: "join_time",
        label: "Join時刻",
        description: "入室時刻",
    },
    ColumnComment {
        name: "leave_time",
        label: "Leave時刻",
        description: "退室時刻",
    },
    ColumnComment {
        name: "duration_sec",
        label: "滞在秒数",
        description: "滞在時間（秒）",
    },
    ColumnComment {
        name: "other_player_count",
        label: "他プレイヤー数",
        description: "同室していた他プレイヤーの数",
    },
];

const NOTIFICATIONS_COLUMNS: &[ColumnComment] = &[
    ColumnComment {
        name: "id",
        label: "ID",
        description: "通知履歴の内部ID",
    },
    ColumnComment {
        name: "notif_type",
        label: "通知種別",
        description: "通知タイプ",
    },
    ColumnComment {
        name: "message",
        label: "本文",
        description: "通知メッセージ本文",
    },
    ColumnComment {
        name: "sender_name",
        label: "送信者名",
        description: "通知送信者の表示名",
    },
    ColumnComment {
        name: "sender_user_id",
        label: "送信者ID",
        description: "通知送信者のユーザーID",
    },
    ColumnComment {
        name: "notif_id",
        label: "VRC通知ID",
        description: "VRChat 側の通知識別子",
    },
    ColumnComment {
        name: "session_id",
        label: "セッション",
        description: "通知を受けた解析セッション",
    },
    ColumnComment {
        name: "created_at",
        label: "作成時刻",
        description: "通知側での作成時刻",
    },
    ColumnComment {
        name: "received_at",
        label: "受信時刻",
        description: "STELLA RECORD が受け取った時刻",
    },
    ColumnComment {
        name: "target_world_name",
        label: "遷移先ワールド名",
        description: "通知から推定した遷移先ワールド名",
    },
    ColumnComment {
        name: "target_instance_id",
        label: "遷移先インスタンスID",
        description: "通知から推定したインスタンス識別子",
    },
    ColumnComment {
        name: "target_instance_type",
        label: "遷移先インスタンスタイプ",
        description: "通知から推定した公開区分",
    },
    ColumnComment {
        name: "target_owner",
        label: "遷移先オーナー",
        description: "通知から推定したインスタンス所有者",
    },
    ColumnComment {
        name: "target_region",
        label: "遷移先リージョン",
        description: "通知から推定したリージョン情報",
    },
];

const SCREENSHOTS_COLUMNS: &[ColumnComment] = &[
    ColumnComment {
        name: "id",
        label: "ID",
        description: "撮影イベントの内部ID",
    },
    ColumnComment {
        name: "visit_id",
        label: "Join",
        description: "撮影時に滞在していたワールド訪問",
    },
    ColumnComment {
        name: "file_path",
        label: "ファイルパス",
        description: "スクリーンショットの保存先フルパス",
    },
    ColumnComment {
        name: "resolution_width",
        label: "幅",
        description: "撮影解像度の幅 (px)",
    },
    ColumnComment {
        name: "resolution_height",
        label: "高さ",
        description: "撮影解像度の高さ (px)",
    },
    ColumnComment {
        name: "timestamp",
        label: "撮影時刻",
        description: "スクリーンショットを撮影した時刻",
    },
];

const SCREENSHOTS_DETAIL_COLUMNS: &[ColumnComment] = &[
    ColumnComment {
        name: "id",
        label: "ID",
        description: "撮影イベントの内部ID",
    },
    ColumnComment {
        name: "visit_id",
        label: "Join",
        description: "撮影時に滞在していたワールド訪問",
    },
    ColumnComment {
        name: "world_name",
        label: "ワールド名",
        description: "撮影時に滞在していたワールド",
    },
    ColumnComment {
        name: "file_path",
        label: "ファイルパス",
        description: "スクリーンショットの保存先フルパス",
    },
    ColumnComment {
        name: "resolution_width",
        label: "幅",
        description: "撮影解像度の幅 (px)",
    },
    ColumnComment {
        name: "resolution_height",
        label: "高さ",
        description: "撮影解像度の高さ (px)",
    },
    ColumnComment {
        name: "timestamp",
        label: "撮影時刻",
        description: "スクリーンショットを撮影した時刻",
    },
];

const OSC_COLUMNS: &[ColumnComment] = &[
    ColumnComment {
        name: "id",
        label: "ID",
        description: "OSCサービスイベントの内部ID",
    },
    ColumnComment {
        name: "session_id",
        label: "セッション",
        description: "イベントが発生した解析セッション",
    },
    ColumnComment {
        name: "event_type",
        label: "イベント種別",
        description: "found (外部ツール検出)",
    },
    ColumnComment {
        name: "service_name",
        label: "サービス名",
        description: "OSCサービスの識別名 (例: OyasumiVR)",
    },
    ColumnComment {
        name: "service_type",
        label: "サービス種別",
        description: "OSC / OSCQuery の区別",
    },
    ColumnComment {
        name: "ip_address",
        label: "IPアドレス",
        description: "検出時の接続先IPアドレス",
    },
    ColumnComment {
        name: "port",
        label: "ポート番号",
        description: "OSCサービスのポート番号",
    },
    ColumnComment {
        name: "timestamp",
        label: "検出時刻",
        description: "OSCサービスを検出または告知した時刻",
    },
];

const SUBSCRIPTION_COLUMNS: &[ColumnComment] = &[
    ColumnComment {
        name: "id",
        label: "ID",
        description: "サブスクリプション状態の内部ID",
    },
    ColumnComment {
        name: "session_id",
        label: "セッション",
        description: "確認が発生した解析セッション",
    },
    ColumnComment {
        name: "is_active",
        label: "有効フラグ",
        description: "VRChat+ が有効かどうか",
    },
    ColumnComment {
        name: "subscription_id",
        label: "VRC契約ID",
        description: "VRChat 側のサブスクリプション識別子 (NULL=無効)",
    },
    ColumnComment {
        name: "description",
        label: "説明",
        description: "サブスクリプション種別の説明テキスト",
    },
    ColumnComment {
        name: "checked_at",
        label: "確認時刻",
        description: "サブスクリプション状態を確認した時刻",
    },
];

const APPS_COLUMNS: &[ColumnComment] = &[
    ColumnComment {
        name: "name",
        label: "アプリ名",
        description: "連携アプリの表示名",
    },
    ColumnComment {
        name: "description",
        label: "説明",
        description: "連携アプリの説明文",
    },
    ColumnComment {
        name: "path",
        label: "パス",
        description: "実行ファイルのパス",
    },
    ColumnComment {
        name: "icon",
        label: "アイコン",
        description: "アイコン画像データ (BLOB)",
    },
];

const TABLE_COMMENTS: &[TableComment] = &[
    // ── テーブル ──
    TableComment {
        name: "sessions",
        label: "セッション",
        description: "ログファイルに記録されたログイン～ログアウトのセッション情報テーブル",
        storage: "Main DB",
        columns: SESSIONS_COLUMNS,
        default_sort: Some(("start_time", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "visits",
        label: "Join履歴",
        description: "Joinしたインスタンスに紐づくデータテーブル",
        storage: "Main DB",
        columns: VISITS_COLUMNS,
        default_sort: Some(("join_time", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "with_users",
        label: "遭遇ユーザー",
        description: "インスタンスごとに出会ったユーザー記録テーブル",
        storage: "Main DB",
        columns: WITH_USERS_COLUMNS,
        default_sort: Some(("join_time", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "find_users",
        label: "ユーザー一覧",
        description: "ログから検出したユーザーの一覧テーブル",
        storage: "Main DB",
        columns: FIND_USERS_COLUMNS,
        default_sort: None,
        is_view: false,
    },
    TableComment {
        name: "notifications",
        label: "通知",
        description: "インバイトやboop、グループ通知などの履歴テーブル",
        storage: "Main DB",
        columns: NOTIFICATIONS_COLUMNS,
        default_sort: Some(("received_at", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "screenshots",
        label: "写真",
        description: "撮影した写真に紐づく関連情報テーブル",
        storage: "Main DB",
        columns: SCREENSHOTS_COLUMNS,
        default_sort: Some(("timestamp", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "osc",
        label: "OSC",
        description: "OSCアプリケーションの関連情報テーブル",
        storage: "Main DB",
        columns: OSC_COLUMNS,
        default_sort: Some(("timestamp", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "subscription",
        label: "VRChat+加入状態",
        description: "セッションごとのVRChat+加入状態の記録テーブル",
        storage: "Main DB",
        columns: SUBSCRIPTION_COLUMNS,
        default_sort: Some(("checked_at", "DESC")),
        is_view: false,
    },
    TableComment {
        name: "apps",
        label: "連携アプリ",
        description: "ランチャーに登録されたアプリ情報テーブル",
        storage: "Main DB",
        columns: APPS_COLUMNS,
        default_sort: None,
        is_view: false,
    },
    // ── ビュー ──
    TableComment {
        name: "visit_summary",
        label: "Join履歴詳細",
        description: "Join履歴テーブル+ワールド名・滞在時間補完ビュー",
        storage: "Main DB",
        columns: VISIT_SUMMARY_COLUMNS,
        default_sort: Some(("join_time", "DESC")),
        is_view: true,
    },
    TableComment {
        name: "with_users_detail",
        label: "遭遇ユーザー詳細",
        description: "遭遇ユーザーテーブル+ワールド名・ユーザー名補完ビュー",
        storage: "Main DB",
        columns: WITH_USERS_DETAIL_COLUMNS,
        default_sort: Some(("join_time", "DESC")),
        is_view: true,
    },
    TableComment {
        name: "screenshots_detail",
        label: "写真詳細",
        description: "写真テーブル+ワールド名補完ビュー",
        storage: "Main DB",
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

/// `ORDER BY` に補間できるカラム名か確認する。
///
/// カラム名は値バインドできないため、文字種だけでなく登録済みメタデータ上に
/// 存在することも確認する。IPC から未知カラムを直接渡された場合は
/// SQL エラーに落とさず、明示的な入力エラーとして返す。
fn sanitize_sort_column(table_name: &str, column_name: &str) -> Result<&'static str, String> {
    if column_name.is_empty()
        || !column_name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err("ソートカラム名が不正です".to_string());
    }

    get_column_comment(table_name, column_name)
        .map(|column_comment| column_comment.name)
        .ok_or_else(|| format!("ソートカラムが見つかりません: {column_name}"))
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
        .map_err(|err| utils::command_err("テーブル存在確認に失敗しました", err))?;

    Ok(exists == 1)
}

/// 要求されたプレビューテーブルを所有するデータベースを開く。
fn open_preview_database(table_name: &str) -> Result<(rusqlite::Connection, String), String> {
    if get_table_comment(table_name).is_none() {
        return Err(format!("プレビュー未登録のテーブルです: {table_name}"));
    }

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
/// # Errors
/// データベースを列挙できない場合にエラーを返す。
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
/// # Errors
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
    fetch_table_data(
        &conn,
        table_name,
        &storage_label,
        page,
        sort_column,
        sort_dir,
    )
}

/// 開かれた接続から1テーブルのページネーション付きデータを取得する。
///
/// 接続を引数で受け取ることで、レジストリや実 DB ファイルを経由せず
/// インメモリ DB に対してカウント・ソート・ページング・セル整形ロジックを検証できる。
fn fetch_table_data(
    conn: &rusqlite::Connection,
    table_name: &str,
    storage_label: &str,
    page: Option<u32>,
    sort_column: Option<String>,
    sort_dir: Option<String>,
) -> Result<TableData, String> {
    let total_rows: u32 = conn
        .query_row(&format!("SELECT COUNT(*) FROM {table_name}"), [], |row| {
            row.get(0)
        })
        .map_err(|err| utils::command_err("行数カウントに失敗しました", err))?;

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
        _ => get_table_comment(table_name)
            .and_then(|tc| tc.default_sort)
            .map_or_else(String::new, |(col, dir)| format!("ORDER BY {col} {dir}")),
    };
    let sql =
        format!("SELECT * FROM {table_name} {order_clause} LIMIT {PAGE_SIZE} OFFSET {offset}");
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
        storage: table_comment.map_or_else(
            || storage_label.to_string(),
            |value| value.storage.to_string(),
        ),
        columns,
        rows: result_rows,
        total_rows,
    })
}

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
