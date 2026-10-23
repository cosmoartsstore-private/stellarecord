//! 外部アプリのランチャー登録・解除と、exe ファイル選択・表示名抽出に関するコマンド群。

use std::path::Path;

use rusqlite::params;

use crate::{platform, utils};

use super::open_initialized_main_db;

/// 登録済み外部アプリを起動する。
///
/// # Errors
/// 対象の実行ファイルを起動できない場合にエラーを返す。
#[tauri::command]
pub fn launch_external_app(app_path: &str) -> Result<(), String> {
    platform::launch_external_process(app_path)
}

/// Shell のパス解決を UI スレッドから分離し、アプリの実行ファイルを選択表示する。
///
/// # Errors
/// 実行ファイルの場所を開けない、または処理タスクが終了できない場合にエラーを返す。
#[tauri::command]
pub async fn show_app_in_folder(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || platform::show_file_in_folder(Path::new(&path))).await.map_err(|err| utils::command_err(message!("Commands.Registry.showAppInFolder.failed"), err))?
}

/// ネイティブファイルダイアログで exe ファイルを選択する。
#[tauri::command]
pub fn pick_exe_file() -> Result<Option<String>, String> {
    platform::pick_exe_file_dialog()
}

/// exe の `VersionInfo` から表示名を取得して返す。
///
/// `FileDescription` → `ProductName` → 拡張子なしファイル名の優先で解決する。
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn extract_exe_display_name(path: String) -> Result<String, String> {
    let exe_path = Path::new(&path);
    if !exe_path.is_file() {
        return Err(message!("Commands.Registry.extractExeDisplayName.fileMissing").to_string());
    }
    platform::read_exe_display_name(exe_path).ok_or_else(|| message!("Commands.Registry.extractExeDisplayName.fileDisplayReadFailed").to_string())
}

/// 任意の exe をランチャーに登録する。
///
/// exe のアイコンを自動抽出して DB に保存し、ランチャー一覧に表示する。
/// `path` の UNIQUE 制約で同一 exe の重複登録を防ぐ。
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn register_app(path: String, name: String, description: String) -> Result<(), String> {
    let exe_path = Path::new(&path);
    if !exe_path.is_file() {
        return Err(message!("Commands.Registry.registerApp.fileMissing").to_string());
    }

    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(message!("Commands.Registry.registerApp.text").to_string());
    }

    let icon_png = platform::extract_exe_icon_png(exe_path);

    let conn = open_initialized_main_db()?;

    insert_app_record(&conn, &name, &description, &path, icon_png.as_deref())
}

/// `apps` テーブルへアプリ1件を挿入する。
///
/// 接続を引数で受け取ることで、registryや実 DB を経由せずインメモリ DB に対して
/// 挿入と UNIQUE 制約違反時のエラーメッセージ整形を検証できる。
fn insert_app_record(conn: &rusqlite::Connection, name: &str, description: &str, path: &str, icon: Option<&[u8]>) -> Result<(), String> {
    conn.execute("INSERT INTO apps (name, description, path, icon) VALUES (?1, ?2, ?3, ?4)", params![name, description, path, icon]).map_err(|e| {
        if let rusqlite::Error::SqliteFailure(err, _) = &e {
            if err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE {
                return message!("Commands.Registry.insertAppRecord.pathRegistrationText").to_string();
            }
        }
        utils::command_err(message!("Commands.Registry.insertAppRecord.registrationFailed"), e)
    })?;

    Ok(())
}

/// 登録済みアプリを解除する。
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn unregister_app(path: String) -> Result<(), String> {
    let conn = open_initialized_main_db()?;

    delete_app_record(&conn, &path)
}

/// `apps` テーブルからパス一致のアプリ1件を削除する。
///
/// 接続を引数で受け取ることで、削除と「該当なし」時のエラー分岐をインメモリ DB で検証できる。
fn delete_app_record(conn: &rusqlite::Connection, path: &str) -> Result<(), String> {
    let affected = conn.execute("DELETE FROM apps WHERE path = ?1", params![path]).map_err(|e| utils::command_err(message!("Commands.Registry.deleteAppRecord.registrationFailed"), e))?;

    if affected == 0 {
        return Err(message!("Commands.Registry.deleteAppRecord.missing").to_string());
    }

    Ok(())
}

#[cfg(test)]
include!("registry_tests.rs");
