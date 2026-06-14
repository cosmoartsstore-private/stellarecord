use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use chrono::Local;
use tauri::{AppHandle, Emitter};
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

/// コマンド層の汎用エラーメッセージを整形する。
///
/// # 引数
/// * `context` - 操作内容を表す説明文。
/// * `err` - 付加する元エラー値。
///
/// # 戻り値
/// UI 向けコマンドエラーに適した結合エラー文字列。
pub fn command_err<E: std::fmt::Display>(context: &str, err: E) -> String {
    format!("{context}: {err}")
}

/// オープン失敗時のエラーメッセージを整形する。
///
/// # 引数
/// * `path` - オープンに失敗したパス。
/// * `err` - 元エラー値。
///
/// # 戻り値
/// コンテキスト付きエラー文字列。
pub fn command_open_err<E: std::fmt::Display>(path: &std::path::Path, err: E) -> String {
    command_err(&format!("開けませんでした [{}]", path.display()), err)
}

/// 読み取り失敗時のエラーメッセージを整形する。
///
/// # 引数
/// * `path` - 読み取りに失敗したパス。
/// * `err` - 元エラー値。
///
/// # 戻り値
/// コンテキスト付きエラー文字列。
pub fn command_read_err<E: std::fmt::Display>(path: &std::path::Path, err: E) -> String {
    command_err(&format!("読み取れませんでした [{}]", path.display()), err)
}

/// 作成失敗時のエラーメッセージを整形する。
///
/// # 引数
/// * `path` - 作成に失敗したパス。
/// * `err` - 元エラー値。
///
/// # 戻り値
/// コンテキスト付きエラー文字列。
pub fn command_create_err<E: std::fmt::Display>(path: &std::path::Path, err: E) -> String {
    command_err(&format!("作成できませんでした [{}]", path.display()), err)
}

/// 削除失敗時のエラーメッセージを整形する。
///
/// # 引数
/// * `path` - 削除に失敗したパス。
/// * `err` - 元エラー値。
///
/// # 戻り値
/// コンテキスト付きエラー文字列。
pub fn command_remove_err<E: std::fmt::Display>(path: &std::path::Path, err: E) -> String {
    command_err(&format!("削除できませんでした [{}]", path.display()), err)
}

/// SQL に補間する識別子として安全な ASCII 英数字と `_` のみを許可する。
///
/// `SQLite` のテーブル名・カラム名は値バインドできないため、動的 SQL の入口で使う。
pub fn is_sql_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// レジストリからインストール済みの `CosmoArtsStore` アプリのディレクトリを解決する。
///
/// インストール先は Windows レジストリ
/// (`HKCU\Software\CosmoArtsStore\<component>`) に格納される。
/// エコシステム内の複数アプリが互いのインストール先を知らずとも
/// 相互発見できるようにするため。
///
/// # 引数
/// * `component_name` - `StellaRecord` や `Polaris` などのレジストリコンポーネントキー。
///
/// # 戻り値
/// レジストリエントリが存在し実在するパスを指す場合のインストールディレクトリ。
pub fn get_component_install_dir(component_name: &str) -> Option<PathBuf> {
    let key_path = format!("Software\\CosmoArtsStore\\{component_name}");
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let key = match root.open_subkey(&key_path) {
        Ok(key) => key,
        Err(err) => {
            // log_warn は append_log → get_stellarecord_data_dir → get_stellarecord_install_dir
            // を経由するため、ここで使うと無限再帰になる。stderr へ直接出力する。
            eprintln!("[WARN] レジストリキーを開けませんでした [{key_path}]: {err}");
            return None;
        }
    };

    let path: String = match key.get_value("InstallLocation") {
        Ok(path) => path,
        Err(err) => {
            eprintln!(
                "[WARN] レジストリ値を読み取れませんでした [{key_path}\\InstallLocation]: {err}"
            );
            return None;
        }
    };

    let install_dir = PathBuf::from(path);
    if !install_dir.exists() {
        eprintln!(
            "[WARN] インストール先が存在しません [{}]",
            install_dir.display()
        );
        return None;
    }

    Some(install_dir)
}

/// `StellaRecord` のインストールディレクトリを解決する。
///
/// # 戻り値
/// インストールディレクトリのパス。利用不可の場合は `None`。
pub fn get_stellarecord_install_dir() -> Option<PathBuf> {
    get_component_install_dir("StellaRecord")
}

/// `{InstallDir}/Data/{category}` 配下のカテゴリ別サブディレクトリを解決する。
///
/// 存在しない場合はディレクトリを作成する。
pub fn get_stellarecord_data_dir(category: &str) -> Option<PathBuf> {
    let dir = get_stellarecord_install_dir()?.join("Data").join(category);
    if !dir.exists() {
        if let Err(err) = fs::create_dir_all(&dir) {
            // log_warn は append_log → get_stellarecord_data_dir を経由するため、
            // ここで使うと無限再帰になる。stderr へ直接出力する。
            eprintln!(
                "[WARN] Data ディレクトリを作成できませんでした [{}]: {}",
                dir.display(),
                err
            );
            return None;
        }
    }
    Some(dir)
}

/// Polaris のインストールディレクトリを解決する。
///
/// # 戻り値
/// インストールディレクトリのパス。利用不可の場合は `None`。
pub fn get_polaris_install_dir() -> Option<PathBuf> {
    get_component_install_dir("Polaris")
}

/// `StellaRecord` の運用ログファイルに1行追記する。
///
/// # 引数
/// * `level` - ログ重要度ラベル。
/// * `msg` - ログメッセージ本文。
///
/// # 戻り値
/// ログ行を書き込めたかどうかを表す I/O 結果。
fn append_log(level: &str, msg: &str) -> io::Result<()> {
    let Some(log_dir) = get_stellarecord_data_dir("logs") else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "StellaRecord のインストール先が見つかりません",
        ));
    };
    append_log_to(&log_dir, level, msg)
}

/// 指定ログディレクトリの月次ファイルに1行追記する。
///
/// ログディレクトリを引数化することで、レジストリ解決を経由せず一時ディレクトリで
/// 月次ファイル名生成と追記書き込みを検証できる。
fn append_log_to(log_dir: &std::path::Path, level: &str, msg: &str) -> io::Result<()> {
    let month = Local::now().format("%Y-%m");
    let log_path = log_dir.join(format!("info-{month}.log"));

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let now = Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(file, "[{now}] [{level}] {msg}")
}

/// ベストエフォートでログ行を書き込み、ロガー障害時は stderr にフォールバックする。
///
/// # 引数
/// * `level` - ログ重要度ラベル。
/// * `msg` - ログメッセージ本文。
fn log_msg(level: &str, msg: &str) {
    if let Err(err) = append_log(level, msg) {
        // 意図的: ファイルロガー自体が壊れたときだけ、stderr を最後の退避先として使う。
        eprintln!("[{level}] {msg} (ログ退避にも失敗: {err})");
    }
}

/// WARN のログエントリを書き込む。
///
/// # 引数
/// * `msg` - 追記する警告テキスト。
pub fn log_warn(msg: &str) {
    log_msg("WARN", msg);
}

/// ERROR のログエントリを書き込む。
///
/// # 引数
/// * `msg` - 追記するエラーテキスト。
pub fn log_err(msg: &str) {
    log_msg("ERROR", msg);
}

/// Tauri イベントを送出し、配信失敗時は警告をログに記録する。
///
/// エラーは意図的に握りつぶす。配信失敗は通常フロントエンドが既に遷移済みであることを
/// 意味し、誰も受信していない更新を失うよりバックエンドがクラッシュする方が深刻なため。
///
/// # 引数
/// * `app` - 実行中の Tauri アプリケーションハンドル。
/// * `event_name` - フロントエンドに送るイベントチャネル名。
/// * `payload` - シリアライズ可能なイベントペイロード。
pub fn emit_event_warn<T: serde::Serialize + Clone>(app: &AppHandle, event_name: &str, payload: T) {
    if let Err(err) = app.emit(event_name, payload) {
        log_warn(&format!("イベント送信に失敗しました [{event_name}]: {err}"));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ── エラーフォーマッター ──

    #[test]
    fn command_err_formats_context_and_error() {
        let msg = command_err("操作に失敗しました", "disk full");
        assert_eq!(msg, "操作に失敗しました: disk full");
    }

    #[test]
    fn command_open_err_includes_path() {
        let path = std::path::Path::new("/some/file.db");
        let msg = command_open_err(path, "permission denied");
        assert!(msg.contains("/some/file.db"));
        assert!(msg.contains("permission denied"));
    }

    #[test]
    fn command_read_err_includes_path() {
        let path = std::path::Path::new("/data/archive.tar.zst");
        let msg = command_read_err(path, "corrupt archive");
        assert!(msg.contains("/data/archive.tar.zst"));
        assert!(msg.contains("corrupt archive"));
    }

    #[test]
    fn command_create_err_includes_path() {
        let path = std::path::Path::new("/new/dir");
        let msg = command_create_err(path, "no space");
        assert!(msg.contains("/new/dir"));
        assert!(msg.contains("no space"));
    }

    #[test]
    fn command_remove_err_includes_path() {
        let path = std::path::Path::new("/old/file.log");
        let msg = command_remove_err(path, "in use");
        assert!(msg.contains("/old/file.log"));
        assert!(msg.contains("in use"));
    }

    #[test]
    fn sql_identifier_validation() {
        assert!(is_sql_identifier("sessions"));
        assert!(is_sql_identifier("with_users_detail"));
        assert!(is_sql_identifier("table1"));
        assert!(!is_sql_identifier(""));
        assert!(!is_sql_identifier("users; DROP TABLE x"));
        assert!(!is_sql_identifier("users WHERE 1=1"));
        assert!(!is_sql_identifier("テーブル"));
    }

    // ── レジストリ: get_component_install_dir ──

    /// テスト終了時にレジストリキーを自動削除する RAII ガード。
    struct TestRegGuard(String);
    impl Drop for TestRegGuard {
        fn drop(&mut self) {
            let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.0);
        }
    }

    fn create_test_key(suffix: &str) -> (RegKey, TestRegGuard) {
        let path = format!("Software\\CosmoArtsStore\\_Test_{suffix}");
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = root.create_subkey(&path).unwrap();
        (key, TestRegGuard(path))
    }

    #[test]
    fn component_install_dir_returns_path_when_valid() {
        // 実在するパスでないと get_component_install_dir が None を返すため、
        // 環境非依存にするよう一時ディレクトリを InstallLocation に設定する。
        let (key, _guard) = create_test_key("install_valid");
        let temp = tempfile::tempdir().unwrap();
        let test_dir = temp.path().to_str().unwrap();
        key.set_value("InstallLocation", &test_dir).unwrap();

        let result = get_component_install_dir("_Test_install_valid");
        assert_eq!(result, Some(PathBuf::from(test_dir)));
    }

    #[test]
    fn component_install_dir_returns_none_for_missing_key() {
        let result = get_component_install_dir("_Test_nonexistent_key_12345");
        assert!(result.is_none());
    }

    #[test]
    fn component_install_dir_returns_none_for_nonexistent_path() {
        let (key, _guard) = create_test_key("install_nodir");
        key.set_value("InstallLocation", &r"Z:\does\not\exist\at\all")
            .unwrap();

        let result = get_component_install_dir("_Test_install_nodir");
        assert!(result.is_none());
    }

    #[test]
    fn component_install_dir_returns_none_when_value_missing() {
        let (_key, _guard) = create_test_key("install_noval");

        let result = get_component_install_dir("_Test_install_noval");
        assert!(result.is_none());
    }

    // ── append_log_to (一時ディレクトリで検証) ──

    #[test]
    fn append_log_to_writes_monthly_file() {
        let dir = tempfile::tempdir().unwrap();
        append_log_to(dir.path(), "WARN", "テストメッセージ").unwrap();

        let month = Local::now().format("%Y-%m");
        let log_path = dir.path().join(format!("info-{month}.log"));
        assert!(log_path.exists());

        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("[WARN]"));
        assert!(content.contains("テストメッセージ"));
    }

    #[test]
    fn append_log_to_appends_multiple_lines() {
        let dir = tempfile::tempdir().unwrap();
        append_log_to(dir.path(), "INFO", "1行目").unwrap();
        append_log_to(dir.path(), "ERROR", "2行目").unwrap();

        let month = Local::now().format("%Y-%m");
        let content =
            std::fs::read_to_string(dir.path().join(format!("info-{month}.log"))).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("1行目"));
        assert!(lines[1].contains("2行目"));
    }

    #[test]
    fn append_log_to_fails_on_invalid_dir() {
        let result = append_log_to(std::path::Path::new("Z:\\no\\such\\dir"), "WARN", "x");
        assert!(result.is_err());
    }
}
