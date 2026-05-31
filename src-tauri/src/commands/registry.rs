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
///
/// exe のアイコンを自動抽出して DB に保存し、ランチャー一覧に表示する。
/// `path` の UNIQUE 制約で同一 exe の重複登録を防ぐ。
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

    insert_app_record(&conn, &name, &description, &path, icon_png.as_deref())
}

/// `apps` テーブルへアプリ1件を挿入する。
///
/// 接続を引数で受け取ることで、レジストリや実 DB を経由せずインメモリ DB に対して
/// 挿入と UNIQUE 制約違反時のエラーメッセージ整形を検証できる。
fn insert_app_record(
    conn: &rusqlite::Connection,
    name: &str,
    description: &str,
    path: &str,
    icon: Option<&[u8]>,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO apps (name, description, path, icon) VALUES (?1, ?2, ?3, ?4)",
        params![name, description, path, icon],
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

    delete_app_record(&conn, &path)
}

/// `apps` テーブルからパス一致のアプリ1件を削除する。
///
/// 接続を引数で受け取ることで、削除と「該当なし」時のエラー分岐をインメモリ DB で検証できる。
fn delete_app_record(conn: &rusqlite::Connection, path: &str) -> Result<(), String> {
    let affected = conn
        .execute("DELETE FROM apps WHERE path = ?1", params![path])
        .map_err(|e| utils::command_err("アプリの登録解除に失敗しました", e))?;

    if affected == 0 {
        return Err("該当するアプリが見つかりません。".to_string());
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_apps_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE apps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
                path TEXT NOT NULL UNIQUE, icon BLOB
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn insert_app_record_success() {
        let conn = setup_apps_db();
        insert_app_record(&conn, "VRChat", "VR app", "/path/vrchat.exe", None).unwrap();

        let (name, path): (String, String) = conn
            .query_row("SELECT name, path FROM apps", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(name, "VRChat");
        assert_eq!(path, "/path/vrchat.exe");
    }

    #[test]
    fn insert_app_record_with_icon() {
        let conn = setup_apps_db();
        insert_app_record(&conn, "App", "", "/p", Some(&[0x89, 0x50])).unwrap();

        let icon: Option<Vec<u8>> = conn
            .query_row("SELECT icon FROM apps", [], |r| r.get(0))
            .unwrap();
        assert_eq!(icon, Some(vec![0x89, 0x50]));
    }

    #[test]
    fn insert_app_record_duplicate_path_returns_friendly_error() {
        let conn = setup_apps_db();
        insert_app_record(&conn, "App1", "", "/same/path.exe", None).unwrap();

        let err = insert_app_record(&conn, "App2", "", "/same/path.exe", None).unwrap_err();
        assert_eq!(err, "同じパスのアプリが既に登録されています。");
    }

    #[test]
    fn insert_app_record_allows_duplicate_name() {
        let conn = setup_apps_db();
        insert_app_record(&conn, "SameName", "", "/path/a.exe", None).unwrap();
        // 名前が同じでもパスが異なれば許可される
        insert_app_record(&conn, "SameName", "", "/path/b.exe", None).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM apps", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn delete_app_record_success() {
        let conn = setup_apps_db();
        insert_app_record(&conn, "App", "", "/path/app.exe", None).unwrap();

        delete_app_record(&conn, "/path/app.exe").unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM apps", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn delete_app_record_missing_returns_error() {
        let conn = setup_apps_db();
        let err = delete_app_record(&conn, "/nonexistent.exe").unwrap_err();
        assert_eq!(err, "該当するアプリが見つかりません。");
    }
}
