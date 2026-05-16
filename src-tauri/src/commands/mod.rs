//! フロントエンドから `invoke()` で呼び出される Tauri v2 コマンドハンドラ。
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

use serde::Serialize;
use tauri::AppHandle;

use crate::config;
use crate::models::AnalyzePayload;
use crate::utils;

/// アプリの自動起動を有効にする際に使用するスタートアップ登録名。
const STELLA_RECORD_RUN_VALUE: &str = "StellaRecord";

/// アーカイブ容量設定用の 1 メビバイト（バイト単位）。
const ONE_MB_BYTES: u64 = 1024 * 1024;

/// UI の設定ダイアログに公開する管理設定。
///
/// `StellaRecord` と Polaris 両方の設定値を1つのペイロードにまとめ、
/// フロントエンドが1画面でアーカイブ容量とスタートアップ動作を編集できるようにする。
#[derive(Serialize)]
pub struct ManagementSettings {
    /// `StellaRecord` を OS のスタートアップに登録するかどうか。
    pub startup_enabled: bool,
    /// ユーザーがスタートアップの設定を既に選択済みかどうか。
    pub startup_preference_set: bool,
    /// 整数メガバイト単位で表示するアーカイブ容量制限。
    pub archive_limit_mb: u64,
}

/// フロントエンドが期待する共有の解析進捗イベントを送出する。
///
/// コマンド層でイベントフォーマットを集約し、全インポートエントリポイントが
/// 同一のペイロード形式で状態を報告する。
fn emit_analyze_progress(app: &AppHandle, status: String, progress: String, is_running: bool) {
    utils::emit_event_warn(
        app,
        "analyze-progress",
        AnalyzePayload {
            status,
            progress,
            is_running,
        },
    );
}

/// `StellaRecord` が管理する圧縮ログ用データディレクトリを解決する。
///
/// # 戻り値
/// `StellaRecord` 設定からのデータディレクトリパス。
///
/// # Errors
/// 設定が未定義でアーカイブ操作を続行できない場合にエラーを返す。
fn get_data_dir() -> Result<PathBuf, String> {
    let setting = config::load_stellarecord_setting();
    setting
        .get_effective_archive_dir()
        .ok_or_else(|| "Data ディレクトリが見つかりません。".to_string())
}

/// コピー元として使用する Polaris 管理のログアーカイブディレクトリを解決する。
fn get_source_log_dir() -> Result<PathBuf, String> {
    let polaris_dir = utils::get_polaris_install_dir().ok_or_else(|| {
        "Polaris のインストール先が見つかりません。レジストリを確認してください。".to_string()
    })?;
    Ok(polaris_dir.join("Data").join("archive"))
}

/// `StellaRecord` の実効メインデータベースパスを解決する。
///
/// 明示的にレジストリで指定されたパスの場合、親ディレクトリが存在しない可能性が
/// あるため、必要に応じて作成する。
fn get_db_path() -> Result<PathBuf, String> {
    let setting = config::load_stellarecord_setting();
    let path = setting
        .get_effective_db_path()
        .ok_or_else(|| "データベースパスが見つかりません。".to_string())?;
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "データベースディレクトリを作成できませんでした [{}]: {err}",
                    parent.display()
                )
            })?;
        }
    }
    Ok(path)
}

/// 管理対象の `.tar.zst` アーカイブを格納するディレクトリを解決する。
fn get_archive_store_dir() -> Result<PathBuf, String> {
    get_data_dir()
}
