//! Frontendから `invoke()` で呼び出される Tauri v2 コマンドハンドラ。
//!
//! 各サブモジュールがドメイン別にコマンドをグループ化する。
//! 共有型とパス解決ヘルパーはこの親モジュールに配置する。

pub(crate) mod archive;
pub(crate) mod database;
pub(crate) mod import;
pub(crate) mod registry;
pub(crate) mod settings;

use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;
use serde::Serialize;
use tauri::AppHandle;

use crate::analyze;
use crate::config;
use crate::models::AnalyzePayload;
use crate::utils;

use crate::constants::{APP_NAME as STELLA_RECORD_RUN_VALUE, BYTES_PER_MEBIBYTE as ONE_MB_BYTES};

/// UI の設定ダイアログに公開する管理設定。
///
/// `StellaRecord` と Polaris 両方の設定値を1つのペイロードにまとめ、
/// Frontendが1画面でアーカイブ容量とスタートアップ動作を編集できるようにする。
#[derive(Serialize)]
pub struct ManagementSettings {
    /// `StellaRecord` を OS のスタートアップに登録するかどうか。
    pub startup_enabled: bool,
    /// ユーザーがスタートアップの設定を既に選択済みかどうか。
    pub startup_preference_set: bool,
    /// 整数メガバイト単位で表示するアーカイブ容量制限。
    pub archive_limit_mb: u64,
}

/// Frontendが期待する共有の解析進捗イベントを送出する。
///
/// コマンド層でイベントフォーマットを集約し、全インポートエントリポイントが
/// 同一のペイロード形式で状態を報告する。
fn emit_analyze_progress(app: &AppHandle, status: String, progress: String, is_running: bool) {
    utils::emit_event_warn(app, "analyze-progress", AnalyzePayload { status, progress, is_running });
}

/// `StellaRecord` が管理する圧縮ログ用データdirectoryを解決する。
///
/// # 戻り値
/// `StellaRecord` 設定からのデータdirectoryパス。
///
/// # Errors
/// 設定が未定義でアーカイブ操作を続行できない場合にエラーを返す。
fn get_archive_store_dir() -> Result<PathBuf, String> {
    let setting = config::load_stellarecord_setting();
    setting.get_effective_archive_dir().ok_or_else(|| message!("Commands.getArchiveStoreDir.directoryMissing").to_string())
}

/// 選択された読込元を解決する。未設定時は Polaris のログ保存先を使用する。
fn get_source_log_dir() -> Result<PathBuf, String> {
    config::load_source_log_dir().ok_or_else(|| message!("Commands.getSourceLogDir.registryMissing").to_string())
}

/// `StellaRecord` の実効メインデータベースパスを解決する。
///
/// 明示的にregistryで指定されたパスの場合、親directoryが存在しない可能性が
/// あるため、必要に応じて作成する。
fn get_db_path() -> Result<PathBuf, String> {
    let setting = config::load_stellarecord_setting();
    let path = setting.get_effective_db_path().ok_or_else(|| message!("Commands.getDbPath.databasePathMissing").to_string())?;
    ensure_parent_dir(&path)?;
    Ok(path)
}

/// メイン DB を開き、現行schemaを初期化してから返す。
///
/// ランチャー登録など、取り込み前にも DB テーブルを必要とするコマンドで使用する。
fn open_initialized_main_db() -> Result<Connection, String> {
    let db_path = get_db_path()?;
    let conn = Connection::open(&db_path).map_err(|err| utils::command_open_err(&db_path, err))?;
    analyze::init_main_db(&conn).map_err(|err| utils::command_err(message!("Commands.openInitializedMainDb.databaseFailed"), err))?;
    Ok(conn)
}

/// パスの親directoryが存在しない場合に再帰的に作成する。
///
/// パスを引数で受け取ることで、registry解決を経由せず一時directoryで
/// 親directory生成ロジックを検証できる。
fn ensure_parent_dir(path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|err| format!(message!("Commands.ensureParentDir.databaseDirectoryCreateFailed"), parent.display(), err = err))?;
        }
    }
    Ok(())
}

/// Frontendで捕捉した詳細エラーをアプリログへ記録する。
#[tauri::command]
pub fn log_client_error(message: String) {
    let normalized = message.replace(['\r', '\n'], " ");
    let truncated: String = normalized.chars().take(4000).collect();
    utils::log_warn(&format!("frontend: {truncated}"));
}

#[cfg(test)]
include!("commands_tests.rs");
