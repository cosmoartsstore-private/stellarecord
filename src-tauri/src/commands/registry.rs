use std::path::Path;

use rusqlite::params;

use crate::{analyze, platform, utils};

use super::get_db_path;

/// 自アプリ（StellaRecord 本体）をランチャーへ登録するときの表示名。
const SELF_APP_NAME: &str = "StellaRecord";
/// 自アプリ（StellaRecord 本体）の説明文。
const SELF_APP_DESCRIPTION: &str = "VRChat ログ管理・閲覧";

/// 登録済み外部アプリを起動する。
///
/// # エラー
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

/// exe の VersionInfo から表示名を取得して返す。
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

/// StellaRecord 本体を `apps` テーブルへ登録（または更新）する。
///
/// アプリ起動時に1度だけ呼び出すことを想定。失敗時はクリティカルではないため
/// 呼び出し側で警告ログにとどめ、アプリ起動を継続できるようにする。
///
/// # エラー
/// 現在の実行ファイルパス取得や DB 操作に失敗した場合にエラーを返す。
pub fn ensure_self_app_registered() -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|err| utils::command_err("自身の実行ファイルパスを取得できませんでした", err))?;
    let icon_png = platform::extract_exe_icon_png(&current_exe);
    let path_str = current_exe.to_string_lossy().into_owned();

    let db_path = get_db_path()?;
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| utils::command_open_err(&db_path, e))?;

    // 初回起動でテーブル未作成のケースに備えてスキーマを保証する。
    analyze::init_main_db(&conn)
        .map_err(|e| utils::command_err("メイン DB を初期化できませんでした", e))?;

    conn.execute(
        "INSERT INTO apps (name, description, path, icon)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(path) DO UPDATE SET
             name        = excluded.name,
             description = excluded.description,
             icon        = excluded.icon",
        params![SELF_APP_NAME, SELF_APP_DESCRIPTION, path_str, icon_png],
    )
    .map_err(|e| utils::command_err("自アプリのランチャー登録に失敗しました", e))?;

    Ok(())
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

/// 登録済みアプリを解除する。自アプリも含めて任意の行を削除可能。
///
/// 自アプリを削除した場合、次回起動時の `ensure_self_app_registered` で再登録される。
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
