use std::path::Path;

use rusqlite::params;

use crate::{platform, utils};

use super::get_db_path;

/// ネイティブファイルダイアログで exe ファイルを選択する。
#[tauri::command]
pub fn pick_exe_file() -> Result<Option<String>, String> {
    platform::pick_exe_file_dialog()
}

/// サードパーティアプリをランチャーに登録する。
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
        "INSERT INTO apps (name, description, path, category, icon) VALUES (?1, ?2, ?3, 'thirdparty', ?4)",
        params![name, description, path, icon_png],
    )
    .map_err(|e| {
        if let rusqlite::Error::SqliteFailure(err, _) = &e {
            if err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE {
                return "同名のアプリが既に登録されています。".to_string();
            }
        }
        utils::command_err("アプリの登録に失敗しました", e)
    })?;

    Ok(())
}

/// サードパーティアプリの登録を解除する。
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn unregister_app(name: String) -> Result<(), String> {
    let db_path = get_db_path()?;
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| utils::command_open_err(&db_path, e))?;

    let affected = conn
        .execute(
            "DELETE FROM apps WHERE name = ?1 AND category = 'thirdparty'",
            params![name],
        )
        .map_err(|e| utils::command_err("アプリの登録解除に失敗しました", e))?;

    if affected == 0 {
        return Err("該当するアプリが見つかりません。".to_string());
    }

    Ok(())
}
