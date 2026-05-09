//! 外部アプリケーション起動と Polaris コンパニオンプロセス管理。

use crate::platform;
use crate::utils;

/// 登録済み外部アプリを起動する。
///
/// # エラー
/// 対象の実行ファイルを起動できない場合にエラーを返す。
#[tauri::command]
pub fn launch_external_app(app_path: &str) -> Result<(), String> {
    platform::launch_external_process(app_path)
}

/// Polaris コンパニオンプロセスが現在実行中かどうかを返す。
#[tauri::command]
pub fn get_polaris_status() -> bool {
    platform::get_polaris_status()
}

/// UI 向けに Polaris の最新ログ行を読み取る。
///
/// # エラー
/// Polaris のインストールディレクトリまたはログファイルを読み取れない場合にエラーを返す。
#[tauri::command]
pub fn get_polaris_logs() -> Result<Vec<String>, String> {
    let log_path = utils::get_polaris_install_dir()
        .map(|path| path.join("info.log"))
        .ok_or_else(|| "Polaris のインストール先を取得できませんでした".to_string())?;

    if !log_path.exists() {
        return Ok(vec!["ログファイルが見つかりません。".to_string()]);
    }

    utils::read_recent_lines(&log_path, 100)
}

/// Polaris がインストール済みかつ未起動の場合に起動する。
///
/// # エラー
/// Polaris 実行ファイルパスを解決できない、または起動できない場合にエラーを返す。
#[tauri::command]
pub async fn start_polaris() -> Result<String, String> {
    if platform::get_polaris_status() {
        return Ok("Polaris は既に起動しています。".to_string());
    }

    let polaris_exe = platform::get_polaris_exe_path()
        .ok_or_else(|| "Polaris の実行ファイルパスを特定できませんでした".to_string())?;
    if !polaris_exe.exists() {
        return Err("Polaris.exe が見つかりません。".to_string());
    }

    let executable = polaris_exe.to_string_lossy().to_string();
    platform::launch_external_process(&executable)?;
    Ok("Polaris を起動しました。".to_string())
}
