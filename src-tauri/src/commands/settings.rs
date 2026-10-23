//! Frontend向け管理設定の CRUD とregistryカタログアクセス。

use crate::config::{self, RegistryCatalog};
use crate::platform;

use super::{ManagementSettings, ONE_MB_BYTES, STELLA_RECORD_RUN_VALUE};

use crate::constants::MAX_ARCHIVE_LIMIT_MB;

/// 起動時ログ取り込みの許可状態と、画面に表示する読込元・保存先。
#[derive(serde::Serialize)]
pub struct StartupImportSettings {
    pub enabled: bool,
    pub preference_set: bool,
    pub source_log_path: String,
    pub log_archive_path: String,
    pub database_path: String,
}

/// 起動時ログ取り込みの状態と実効保存先を返す。
///
/// # Errors
/// ログまたはデータベースの保存先を解決できない場合にエラーを返す。
#[tauri::command]
pub fn get_startup_import_settings() -> Result<StartupImportSettings, String> {
    let stella = config::load_stellarecord_setting();
    let preference = config::load_startup_import_preference();
    let log_archive_path = stella.get_effective_archive_dir().ok_or_else(|| message!("Commands.Settings.getStartupImportSettings.logDestinationSaveReadFailed").to_string())?;
    let database_path = stella.get_effective_db_path().ok_or_else(|| message!("Commands.Settings.getStartupImportSettings.databaseDestinationSaveReadFailed").to_string())?;

    // Polaris が未導入でも、設定画面から別の読込元を選択できるようにする。
    let source_log_path = config::load_source_log_dir().map(|path| path.display().to_string()).unwrap_or_default();
    Ok(StartupImportSettings { enabled: preference.enabled, preference_set: preference.preference_set, source_log_path, log_archive_path: log_archive_path.display().to_string(), database_path: database_path.display().to_string() })
}

/// フォルダを選択して読込元を保存する。キャンセル時は現在の設定を保持する。
///
/// # Errors
/// フォルダ選択、読み取り確認、または設定保存に失敗した場合にエラーを返す。
#[tauri::command]
pub fn change_source_log_directory() -> Result<Option<String>, String> {
    // ダイアログの所有者を UI スレッドから取得するため、同期 command のまま実行する。
    let current = config::load_source_log_dir();
    platform::pick_log_source_directory_dialog(current.as_deref())?.map(save_source_log_directory).transpose()
}

/// 選択フォルダの読み取りを確認してから、単独のregistry値として保存する。
pub(super) fn save_source_log_directory(path: String) -> Result<String, String> {
    let directory = std::path::Path::new(&path);
    if !directory.is_absolute() || !directory.is_dir() {
        return Err(message!("Commands.Settings.saveSourceLogDirectory.invalid").to_string());
    }
    std::fs::read_dir(directory).map_err(|err| crate::utils::command_read_err(directory, err))?;
    config::save_source_log_path(&path)?;
    Ok(path)
}

/// 起動時ログ取り込みについてユーザーが選択した許可状態を保存する。
///
/// # Errors
/// registryへ保存できない場合にエラーを返す。
#[tauri::command]
pub fn save_startup_import_preference(enabled: bool) -> Result<(), String> {
    config::save_startup_import_preference(enabled)
}

/// Frontend用の管理設定ダイアログ状態を返す。
///
/// ローカル `StellaRecord` 設定（スタートアップ設定）と Polaris 設定
/// （アーカイブ容量）の値を1つのペイロードにマージし、UI が1回の呼び出しで
/// 設定ダイアログの両セクションを表示できるようにする。
#[tauri::command]
pub fn get_management_settings() -> ManagementSettings {
    build_management_settings(&config::load_stellarecord_setting(), &config::load_polaris_setting())
}

/// `StellaRecord` と Polaris の設定を UI 向けペイロードへ統合する。
///
/// 設定値を引数で受け取ることで、registry参照を経由せず統合・MB 丸めロジックを検証できる。
fn build_management_settings(stella: &config::StellaRecordSetting, polaris: &config::PolarisSetting) -> ManagementSettings {
    ManagementSettings { startup_enabled: stella.enable_startup, startup_preference_set: stella.startup_preference_set, archive_limit_mb: bytes_to_archive_limit_mb(polaris.capacity_threshold_bytes) }
}

/// バイト数を表示用のメガバイト整数へ四捨五入する（最低 1 MB）。
///
/// 表示と保存で同じ丸め基準を共有するため独立関数にしている。
fn bytes_to_archive_limit_mb(bytes: u64) -> u64 {
    (bytes.saturating_add(ONE_MB_BYTES / 2) / ONE_MB_BYTES).clamp(1, MAX_ARCHIVE_LIMIT_MB)
}

/// UI と同じ 1 MB～10 TB の範囲を検証してバイト数へ変換する。
fn archive_limit_mb_to_bytes(mb: u64) -> Result<u64, String> {
    if !(1..=MAX_ARCHIVE_LIMIT_MB).contains(&mb) {
        return Err(message!("Commands.Settings.archiveLimitMbToBytes.text").to_string());
    }
    mb.checked_mul(ONE_MB_BYTES).ok_or_else(|| message!("Commands.Settings.archiveLimitMbToBytes.convertFailed").to_string())
}

/// 3か所の設定を順に保存し、途中失敗時は開始前の値へ復元する。
fn persist_management_settings<SaveStella, SetStartup, SavePolaris>(
    original_stella: &config::StellaRecordSetting,
    next_stella: &config::StellaRecordSetting,
    original_polaris: &config::PolarisSetting,
    next_polaris: &config::PolarisSetting,
    mut save_stella: SaveStella,
    mut set_startup: SetStartup,
    mut save_polaris: SavePolaris,
) -> Result<(), String>
where
    SaveStella: FnMut(&config::StellaRecordSetting) -> Result<(), String>,
    SetStartup: FnMut(bool) -> Result<(), String>,
    SavePolaris: FnMut(&config::PolarisSetting) -> Result<(), String>,
{
    let result = save_polaris(next_polaris).and_then(|()| set_startup(next_stella.enable_startup)).and_then(|()| save_stella(next_stella));

    if let Err(err) = result {
        let mut rollback_errors = Vec::new();
        if let Err(rollback_err) = save_stella(original_stella) {
            rollback_errors.push(rollback_err);
        }
        if let Err(rollback_err) = set_startup(original_stella.enable_startup) {
            rollback_errors.push(rollback_err);
        }
        if let Err(rollback_err) = save_polaris(original_polaris) {
            rollback_errors.push(rollback_err);
        }

        if rollback_errors.is_empty() {
            return Err(err);
        }
        return Err(format!(message!("Commands.Settings.persistManagementSettings.settingsRestoreFailed"), rollback_errors.join(" / "), err = err));
    }

    Ok(())
}

/// スタートアップとアーカイブ容量の管理設定を永続化する。
///
/// # Errors
/// 設定ファイルまたはスタートアップ登録を更新できない場合にエラーを返す。
#[tauri::command]
pub fn save_management_settings(startup_enabled: bool, archive_limit_mb: u64) -> Result<(), String> {
    let capacity_threshold_bytes = archive_limit_mb_to_bytes(archive_limit_mb)?;

    let original_stella = config::load_stellarecord_setting();
    let mut next_stella = original_stella.clone();
    next_stella.enable_startup = startup_enabled;
    next_stella.startup_preference_set = true;

    let original_polaris = config::load_polaris_setting();
    let mut next_polaris = original_polaris.clone();
    next_polaris.capacity_threshold_bytes = capacity_threshold_bytes;

    persist_management_settings(&original_stella, &next_stella, &original_polaris, &next_polaris, config::save_stellarecord_setting, |enabled| platform::set_startup_enabled(STELLA_RECORD_RUN_VALUE, enabled), config::save_polaris_setting)
}

/// 外部アプリランチャーグリッドを構成するregistryカタログを返す。
///
/// ユーザーが登録したアプリ（名前、パス、アイコン）を一覧にし、
/// `StellaRecord` サイドバーからのクイック起動を提供する。
#[tauri::command]
pub fn read_registry_catalog() -> RegistryCatalog {
    config::load_registry_catalog()
}

#[cfg(test)]
include!("settings_tests.rs");
