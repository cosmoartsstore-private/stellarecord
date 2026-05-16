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
            eprintln!("[WARN] レジストリ値を読み取れませんでした [{key_path}\\InstallLocation]: {err}");
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
    let month = Local::now().format("%Y-%m");
    let Some(log_path) = get_stellarecord_data_dir("logs")
        .map(|dir| dir.join(format!("info-{month}.log")))
    else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "StellaRecord のインストール先が見つかりません",
        ));
    };

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

/// 警告レベルのログエントリを書き込む。
///
/// # 引数
/// * `msg` - 追記する警告テキスト。
pub fn log_warn(msg: &str) {
    log_msg("WARN", msg);
}

/// エラーレベルのログエントリを書き込む。
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

