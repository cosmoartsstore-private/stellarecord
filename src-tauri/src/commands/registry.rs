//! 外部アプリのランチャー登録・解除と、exe ファイル選択・表示名抽出に関するコマンド群。

use std::path::Path;

use rusqlite::params;

use crate::{platform, utils};

use super::get_db_path;

/// 登録済み外部アプリを起動する。
///
/// # Errors
/// 対象の実行ファイルを起動できない場合にエラーを返す。
#[tauri::command]
pub fn launch_external_app(app_path: &str) -> Result<(), String> {
    platform::launch_external_process(app_path)
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
        return Err("指定されたファイルが見つかりません。".to_string());
    }
    platform::read_exe_display_name(exe_path)
        .ok_or_else(|| "実行ファイルから表示名を取得できませんでした。".to_string())
}

/// 任意の exe をランチャーに登録する。
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn register_app(path: String, name: String, description: String) -> Result<(), String> {
    let exe_path = Path::new(&path);
    if !exe_path.is_file() {
        return Err("指定されたファイルが見つかりません。".to_string());
    }

    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("アプリ名を入力してください。".to_string());
    }

    let icon_png = platform::extract_exe_icon_png(exe_path);

    let db_path = get_db_path()?;
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| utils::command_open_err(&db_path, e))?;

    conn.execute(
        "INSERT INTO apps (name, description, path, icon) VALUES (?1, ?2, ?3, ?4)",
        params![name, description, path, icon_png],
    )
    .map_err(|e| {
        if let rusqlite::Error::SqliteFailure(err, _) = &e {
            if err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE {
                return "同じパスのアプリが既に登録されています。".to_string();
            }
        }
        utils::command_err("アプリの登録に失敗しました", e)
    })?;

    Ok(())
}

/// 登録済みアプリを解除する。
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn unregister_app(path: String) -> Result<(), String> {
    let db_path = get_db_path()?;
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| utils::command_open_err(&db_path, e))?;

    let affected = conn
        .execute("DELETE FROM apps WHERE path = ?1", params![path])
        .map_err(|e| utils::command_err("アプリの登録解除に失敗しました", e))?;

    if affected == 0 {
        return Err("該当するアプリが見つかりません。".to_string());
    }

    Ok(())
}
