use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use crate::constants::{ARCHIVE_DIRECTORY, DATABASE_DIRECTORY, DATABASE_FILE, DEFAULT_ARCHIVE_CAPACITY_BYTES as DEFAULT_CAPACITY, POLARIS_KEY, STELLA_RECORD_KEY};
use crate::utils;

/// 起動時ログ取り込みについて、ユーザーが選択した許可状態。
#[derive(Debug, Clone, Copy, Default)]
pub struct StartupImportPreference {
    pub enabled: bool,
    pub preference_set: bool,
}

/// HKCU 配下のキーを開く。存在しない場合は `None` を返す。
fn open_key(path: &str) -> Option<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey(path).ok()
}

/// HKCU 配下にキーを作成し、失敗時はコマンドエラー文字列に整形して返す。
fn create_key(path: &str) -> Result<RegKey, String> {
    RegKey::predef(HKEY_CURRENT_USER).create_subkey(path).map(|(key, _)| key).map_err(|err| utils::command_err(&format!(message!("Config.createKey.registryCreateFailed"), path = path), err))
}

/// registryから文字列値を読み取る。失敗時は空文字列を返す。
fn read_str(key: &RegKey, name: &str) -> String {
    key.get_value::<String, _>(name).unwrap_or_default()
}

/// registryから u64 値を読み取る。失敗時は `default` を返す。
fn read_u64(key: &RegKey, name: &str, default: u64) -> u64 {
    key.get_value::<u64, _>(name).unwrap_or(default)
}

/// registryの u32 値（0/1）を bool として読み取る。失敗時は `default` を返す。
fn read_bool(key: &RegKey, name: &str, default: bool) -> bool {
    key.get_value::<u32, _>(name).map(|v| v != 0).unwrap_or(default)
}

/// `StellaRecord` から参照される Polaris 側のアーカイブ設定。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PolarisSetting {
    pub archive_path: String,
    pub capacity_threshold_bytes: u64,
}

impl Default for PolarisSetting {
    fn default() -> Self {
        Self { archive_path: String::new(), capacity_threshold_bytes: DEFAULT_CAPACITY }
    }
}

/// `StellaRecord` 側のアーカイブ・スタートアップ設定。
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct StellaRecordSetting {
    pub archive_path: String,
    pub db_path: String,
    pub enable_startup: bool,
    pub startup_preference_set: bool,
}

/// registryデータベースから読み込まれる起動可能アプリの記述子。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppCard {
    pub name: String,
    pub description: String,
    pub path: String,
    pub icon_data: Option<String>,
}

/// ランチャーに表示するアプリ一覧。区別なしの平坦リスト。
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RegistryCatalog {
    #[serde(default)]
    pub apps: Vec<AppCard>,
}

/// registryから Polaris 設定を読み込む。キーが存在しない場合はデフォルト値を返す。
pub fn load_polaris_setting() -> PolarisSetting {
    load_polaris_setting_from(POLARIS_KEY)
}

/// 指定されたregistryキーパスから Polaris 設定を読み込む。
///
/// 本番では `POLARIS_KEY` を渡す。キーパスを引数化することで、本番キーと干渉せず
/// テスト専用キーで読み込みロジックを検証できる。
fn load_polaris_setting_from(key_path: &str) -> PolarisSetting {
    let Some(key) = open_key(key_path) else {
        return PolarisSetting::default();
    };

    PolarisSetting { archive_path: read_str(&key, "ArchivePath"), capacity_threshold_bytes: read_u64(&key, "CapacityThresholdBytes", DEFAULT_CAPACITY) }
}

/// Polaris 設定をregistryに保存する。
///
/// # Errors
/// registryキーへの書き込みに失敗した場合にエラーを返す。
pub fn save_polaris_setting(setting: &PolarisSetting) -> Result<(), String> {
    save_polaris_setting_to(POLARIS_KEY, setting)
}

/// 指定されたregistryキーパスに Polaris 設定を保存する。
fn save_polaris_setting_to(key_path: &str, setting: &PolarisSetting) -> Result<(), String> {
    let key = create_key(key_path)?;
    key.set_value("ArchivePath", &setting.archive_path).map_err(|e| utils::command_err(message!("Config.savePolarisSettingTo.failed"), e))?;
    key.set_value("CapacityThresholdBytes", &setting.capacity_threshold_bytes).map_err(|e| utils::command_err(message!("Config.savePolarisSettingTo.capacityThresholdWriteFailed"), e))?;
    Ok(())
}

/// registryから `StellaRecord` 設定を読み込む。キーが存在しない場合はデフォルト値を返す。
pub fn load_stellarecord_setting() -> StellaRecordSetting {
    load_stellarecord_setting_from(STELLA_RECORD_KEY)
}

/// 指定されたregistryキーパスから `StellaRecord` 設定を読み込む。
fn load_stellarecord_setting_from(key_path: &str) -> StellaRecordSetting {
    let Some(key) = open_key(key_path) else {
        return StellaRecordSetting::default();
    };

    StellaRecordSetting { archive_path: read_str(&key, "ArchivePath"), db_path: read_str(&key, "DbPath"), enable_startup: read_bool(&key, "EnableStartup", false), startup_preference_set: read_bool(&key, "StartupPreferenceSet", false) }
}

/// `StellaRecord` 設定をregistryに保存する。
///
/// # Errors
/// registryキーへの書き込みに失敗した場合にエラーを返す。
pub fn save_stellarecord_setting(setting: &StellaRecordSetting) -> Result<(), String> {
    save_stellarecord_setting_to(STELLA_RECORD_KEY, setting)
}

/// 起動時ログ取り込みの許可状態をregistryから読み込む。
pub fn load_startup_import_preference() -> StartupImportPreference {
    let Some(key) = open_key(STELLA_RECORD_KEY) else {
        return StartupImportPreference::default();
    };

    StartupImportPreference { enabled: read_bool(&key, "StartupImportEnabled", false), preference_set: read_bool(&key, "StartupImportPreferenceSet", false) }
}

/// 起動時ログ取り込みの選択結果をregistryへ保存する。
///
/// 許可状態を先に書き、回答済み状態を最後に書くことで、途中失敗時に未回答のまま
/// 起動時取り込みが有効になることを防ぐ。
///
/// # Errors
/// registryキーの作成、許可状態、または回答済み状態の書き込みに失敗した場合に
/// エラーを返す。
pub fn save_startup_import_preference(enabled: bool) -> Result<(), String> {
    let key = create_key(STELLA_RECORD_KEY)?;
    key.set_value("StartupImportEnabled", &u32::from(enabled)).map_err(|e| utils::command_err(message!("Config.saveStartupImportPreference.logSettingsFailed"), e))?;
    key.set_value("StartupImportPreferenceSet", &1u32).map_err(|e| utils::command_err(message!("Config.saveStartupImportPreference.logSettingsCommitFailed"), e))?;
    Ok(())
}

/// 指定されたregistryキーパスに `StellaRecord` 設定を保存する。
fn save_stellarecord_setting_to(key_path: &str, setting: &StellaRecordSetting) -> Result<(), String> {
    let key = create_key(key_path)?;
    key.set_value("ArchivePath", &setting.archive_path).map_err(|e| utils::command_err(message!("Config.saveStellarecordSettingTo.failed"), e))?;
    key.set_value("DbPath", &setting.db_path).map_err(|e| utils::command_err(message!("Config.saveStellarecordSettingTo.databasePathWriteFailed"), e))?;
    key.set_value("EnableStartup", &u32::from(setting.enable_startup)).map_err(|e| utils::command_err(message!("Config.saveStellarecordSettingTo.startupEnabledWriteFailed"), e))?;
    key.set_value("StartupPreferenceSet", &u32::from(setting.startup_preference_set)).map_err(|e| utils::command_err(message!("Config.saveStellarecordSettingTo.startupPreferenceWriteFailed"), e))?;
    Ok(())
}

/// メインデータベースの `apps` テーブルからランチャーカタログを読み込む。
///
/// DB が存在しない場合やテーブル未作成の場合は空のカタログを返す。
/// アイコン BLOB は Base64 にエンコードし、Frontendが `<img src>` で
/// 直接使用できる形にする。
pub fn load_registry_catalog() -> RegistryCatalog {
    let setting = load_stellarecord_setting();
    let Some(db_path) = setting.get_effective_db_path() else {
        utils::log_warn(message!("Config.loadRegistryCatalog.registryDatabasePathReadFailed"));
        return RegistryCatalog::default();
    };
    load_catalog_from_db(&db_path)
}

/// 指定された DB パスから `apps` カタログを読み込む。
///
/// DB パスを引数化することで、registry解決を経由せず一時 DB でカタログ読み込み
/// （存在確認・BLOB の Base64 化・行マッピング）を検証できる。
fn load_catalog_from_db(db_path: &std::path::Path) -> RegistryCatalog {
    if !db_path.exists() {
        return RegistryCatalog::default();
    }

    let conn = match Connection::open(db_path) {
        Ok(c) => c,
        Err(err) => {
            utils::log_warn(&format!(message!("Config.loadCatalogFromDb.registryDatabaseOpenFailed"), err = err));
            return RegistryCatalog::default();
        }
    };

    let has_table: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='apps')", [], |row| row.get(0)).unwrap_or(false);

    if !has_table {
        return RegistryCatalog::default();
    }

    let mut stmt = match conn.prepare("SELECT name, description, path, icon FROM apps ORDER BY name") {
        Ok(s) => s,
        Err(err) => {
            utils::log_warn(&format!(message!("Config.loadCatalogFromDb.registryPrepareFailed"), err = err));
            return RegistryCatalog::default();
        }
    };

    let rows = match stmt.query_map([], |row| {
        let name: String = row.get(0)?;
        let description: String = row.get(1)?;
        let path: String = row.get(2)?;
        let icon: Option<Vec<u8>> = row.get(3)?;
        let icon_data = icon.map(|bytes| BASE64.encode(&bytes));
        Ok(AppCard { name, description, path, icon_data })
    }) {
        Ok(r) => r,
        Err(err) => {
            utils::log_warn(&format!(message!("Config.loadCatalogFromDb.registryFailed"), err = err));
            return RegistryCatalog::default();
        }
    };

    let mut catalog = RegistryCatalog::default();
    for card in rows.flatten() {
        catalog.apps.push(card);
    }

    catalog
}

impl StellaRecordSetting {
    /// `StellaRecord` が管理する圧縮ログアーカイブのdirectoryを解決する。
    pub fn get_effective_archive_dir(&self) -> Option<PathBuf> {
        if !self.archive_path.is_empty() {
            return Some(PathBuf::from(&self.archive_path));
        }
        utils::get_stellarecord_data_dir(ARCHIVE_DIRECTORY)
    }

    /// `StellaRecord` が開くメインデータベースのパスを解決する。
    pub fn get_effective_db_path(&self) -> Option<PathBuf> {
        if !self.db_path.is_empty() {
            return Some(PathBuf::from(&self.db_path));
        }
        Some(utils::get_stellarecord_data_dir(DATABASE_DIRECTORY)?.join(DATABASE_FILE))
    }
}

#[cfg(test)]
include!("config_tests.rs");
