use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use crate::constants::{APP_NAME, DATA_DIRECTORY, INSTALL_LOCATION_VALUE, LOG_DIRECTORY, POLARIS_APP_NAME, REGISTRY_KEY_PREFIX};
use chrono::Local;
use tauri::{AppHandle, Emitter};
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

/// Win32へ渡す文字列を、OSの文字表現を保持してNUL終端のUTF-16へ変換する。
#[cfg(windows)]
pub fn to_wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value.as_ref().encode_wide().chain([0]).collect()
}

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
    command_err(&format!(message!("Utils.commandOpenErr.openFailed"), path.display()), err)
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
    command_err(&format!(message!("Utils.commandReadErr.readFailed"), path.display()), err)
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
    command_err(&format!(message!("Utils.commandCreateErr.createFailed"), path.display()), err)
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
    command_err(&format!(message!("Utils.commandRemoveErr.deleteFailed"), path.display()), err)
}

/// SQL に補間する識別子として安全な ASCII 英数字と `_` のみを許可する。
///
/// `SQLite` のテーブル名・column名は値バインドできないため、動的 SQL の入口で使う。
pub fn is_sql_identifier(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// ログ1行の無効 UTF-8 バイトだけを置換し、行末の改行を取り除く。
pub fn decode_log_line_lossy(bytes: &[u8]) -> String {
    let line = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    String::from_utf8_lossy(line).into_owned()
}

/// registryからインストール済みの `CosmoArtsStore` アプリのdirectoryを解決する。
///
/// インストール先は Windows registry
/// (`HKCU\Software\CosmoArtsStore\<component>`) に格納される。
/// エコシステム内の複数アプリが互いのインストール先を知らずとも
/// 相互発見できるようにするため。
///
/// # 引数
/// * `component_name` - `StellaRecord` や `Polaris` などのregistryコンポーネントキー。
///
/// # 戻り値
/// registryエントリが存在し実在するパスを指す場合のインストールdirectory。
pub fn get_component_install_dir(component_name: &str) -> Option<PathBuf> {
    let key_path = format!("{REGISTRY_KEY_PREFIX}\\{component_name}");
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let key = match root.open_subkey(&key_path) {
        Ok(key) => key,
        Err(err) => {
            // log_warn は append_log → get_stellarecord_data_dir → get_stellarecord_install_dir
            // を経由するため、ここで使うと無限再帰になる。stderr へ直接出力する。
            eprintln!(message!("Utils.getComponentInstallDir.registryOpenFailed"), err = err, key_path = key_path);
            return None;
        }
    };

    let path: String = match key.get_value(INSTALL_LOCATION_VALUE) {
        Ok(path) => path,
        Err(err) => {
            eprintln!(message!("Utils.getComponentInstallDir.registryReadFailed"), err = err, key_path = key_path);
            return None;
        }
    };

    let install_dir = PathBuf::from(path);
    if !install_dir.exists() {
        eprintln!(message!("Utils.getComponentInstallDir.missing"), install_dir.display());
        return None;
    }

    Some(install_dir)
}

/// `StellaRecord` のインストールdirectoryを解決する。
///
/// # 戻り値
/// インストールdirectoryのパス。利用不可の場合は `None`。
pub fn get_stellarecord_install_dir() -> Option<PathBuf> {
    get_component_install_dir(APP_NAME)
}

/// `{InstallDir}/Data/{category}` 配下のカテゴリ別サブdirectoryを解決する。
///
/// 存在しない場合はdirectoryを作成する。
pub fn get_stellarecord_data_dir(category: &str) -> Option<PathBuf> {
    let dir = get_stellarecord_install_dir()?.join(DATA_DIRECTORY).join(category);
    if !dir.exists() {
        if let Err(err) = fs::create_dir_all(&dir) {
            // log_warn は append_log → get_stellarecord_data_dir を経由するため、
            // ここで使うと無限再帰になる。stderr へ直接出力する。
            eprintln!(message!("Utils.getStellarecordDataDir.directoryCreateFailed"), dir.display(), err);
            return None;
        }
    }
    Some(dir)
}

/// Polaris のインストールdirectoryを解決する。
///
/// # 戻り値
/// インストールdirectoryのパス。利用不可の場合は `None`。
pub fn get_polaris_install_dir() -> Option<PathBuf> {
    get_component_install_dir(POLARIS_APP_NAME)
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
    let Some(log_dir) = get_stellarecord_data_dir(LOG_DIRECTORY) else {
        return Err(io::Error::new(io::ErrorKind::NotFound, message!("Utils.appendLog.missing")));
    };
    append_log_to(&log_dir, level, msg)
}

/// 指定ログdirectoryの月次ファイルに1行追記する。
///
/// ログdirectoryを引数化することで、registry解決を経由せず一時directoryで
/// 月次ファイル名生成と追記書き込みを検証できる。
fn append_log_to(log_dir: &std::path::Path, level: &str, msg: &str) -> io::Result<()> {
    let month = Local::now().format("%Y-%m");
    let log_path = log_dir.join(format!("info-{month}.log"));

    let mut file = OpenOptions::new().create(true).append(true).open(&log_path)?;
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
        eprintln!(message!("Utils.logMsg.logFailed"), err = err, level = level, msg = msg);
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
/// エラーは意図的に握りつぶす。配信失敗は通常Frontendが既に遷移済みであることを
/// 意味し、誰も受信していない更新を失うよりBackendがクラッシュする方が深刻なため。
///
/// # 引数
/// * `app` - 実行中の Tauri アプリケーションハンドル。
/// * `event_name` - Frontendに送るイベントチャネル名。
/// * `payload` - シリアライズ可能なイベントペイロード。
pub fn emit_event_warn<T: serde::Serialize + Clone>(app: &AppHandle, event_name: &str, payload: T) {
    if let Err(err) = app.emit(event_name, payload) {
        log_warn(&format!(message!("Utils.emitEventWarn.eventFailed"), err = err, event_name = event_name));
    }
}

#[cfg(test)]
include!("utils_tests.rs");
