use serde::Serialize;

/// バックグラウンド解析の実行中に送出される進捗ペイロード。
#[derive(Clone, Serialize)]
pub struct AnalyzePayload {
    pub status: String,
    pub progress: String,
    pub is_running: bool,
}

/// DB 画面でプレビュー可能なテーブル1件の概要メタデータ。
#[derive(Debug, Serialize)]
pub struct DbTableSummary {
    pub name: String,
    pub label: String,
    pub description: String,
    pub storage: String,
    pub is_view: bool,
}

/// プレビュー対象テーブルの1カラムに対する UI 向けメタデータ。
#[derive(Debug, Serialize)]
pub struct DbColumnMeta {
    pub name: String,
    pub label: String,
    pub description: String,
}

/// DB 画面に返すテーブルプレビューの完全ペイロード。
#[derive(Debug, Serialize)]
pub struct TableData {
    pub name: String,
    pub label: String,
    pub description: String,
    pub storage: String,
    pub columns: Vec<DbColumnMeta>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: u32,
}

/// ログビューアのストリーム開始時に同期的に返すメタデータ。
#[derive(Debug, Clone, Serialize)]
pub struct LogViewerMeta {
    pub session_id: String,
    pub archive_name: String,
    pub source_name: String,
}

/// ストリーミング中に Tauri イベントとして送出される処理済みログ行の1バッチ。
///
/// Level:    0=plain 1=info 2=warning 3=error 4=debug
/// Category: 0=plain 1=world 2=notification 3=player_join 4=player_ready 5=player_left 6=debug-system
#[derive(Debug, Clone, Serialize)]
pub struct LogViewerChunk {
    pub session_id: String,
    pub timestamps: Vec<String>,
    pub levels: Vec<u8>,
    pub categories: Vec<u8>,
    pub raw_lines: Vec<String>,
    pub highlights: Vec<Option<String>>,
}

/// アーカイブ選択モーダルに返すファイル記述子。
#[derive(Debug, Clone, Serialize)]
pub struct ArchiveFileItem {
    pub name: String,
    pub size_bytes: u64,
}

/// .tar.zst アーカイブ済みで安全に削除可能なソースログファイル。
#[derive(Debug, Clone, Serialize)]
pub struct DeletableLogInfo {
    pub file_name: String,
    pub size_bytes: u64,
}
