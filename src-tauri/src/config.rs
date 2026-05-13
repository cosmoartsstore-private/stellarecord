use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use crate::utils;

const STELLA_RECORD_KEY: &str = "Software\\CosmoArtsStore\\StellaRecord";
const POLARIS_KEY: &str = "Software\\CosmoArtsStore\\Polaris";

fn open_key(path: &str) -> Option<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey(path).ok()
}

fn create_key(path: &str) -> Result<RegKey, String> {
    RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(path)
        .map(|(key, _)| key)
        .map_err(|err| utils::command_err(&format!("レジストリキーを作成できませんでした [{path}]"), err))
}

fn read_str(key: &RegKey, name: &str) -> String {
    key.get_value::<String, _>(name).unwrap_or_default()
}

fn read_u64(key: &RegKey, name: &str, default: u64) -> u64 {
    key.get_value::<u64, _>(name).unwrap_or(default)
}

fn read_bool(key: &RegKey, name: &str, default: bool) -> bool {
    key.get_value::<u32, _>(name)
        .map(|v| v != 0)
        .unwrap_or(default)
}

/// `StellaRecord` から参照される Polaris 側のアーカイブ・スタートアップ設定。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PolarisSetting {
    pub archive_path: String,
    pub capacity_threshold_bytes: u64,
    pub enable_startup: bool,
    pub migration_status: String,
    pub migration_source_path: String,
}

const DEFAULT_CAPACITY: u64 = 314_572_800;

impl Default for PolarisSetting {
    fn default() -> Self {
        Self {
            archive_path: String::new(),
            capacity_threshold_bytes: DEFAULT_CAPACITY,
            enable_startup: true,
            migration_status: "done".to_string(),
            migration_source_path: String::new(),
        }
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

/// レジストリデータベースから読み込まれる起動可能アプリの記述子。
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

/// レジストリから Polaris 設定を読み込む。キーが存在しない場合はデフォルト値を返す。
pub fn load_polaris_setting() -> PolarisSetting {
    let Some(key) = open_key(POLARIS_KEY) else {
        return PolarisSetting::default();
    };

    PolarisSetting {
        archive_path: read_str(&key, "ArchivePath"),
        capacity_threshold_bytes: read_u64(&key, "CapacityThresholdBytes", DEFAULT_CAPACITY),
        enable_startup: read_bool(&key, "EnableStartup", true),
        migration_status: {
            let v = read_str(&key, "MigrationStatus");
            if v.is_empty() { "done".to_string() } else { v }
        },
        migration_source_path: read_str(&key, "MigrationSourcePath"),
    }
}

/// Polaris 設定をレジストリに保存する。
pub fn save_polaris_setting(setting: &PolarisSetting) -> Result<(), String> {
    let key = create_key(POLARIS_KEY)?;
    key.set_value("ArchivePath", &setting.archive_path)
        .map_err(|e| utils::command_err("ArchivePath の書き込みに失敗しました", e))?;
    key.set_value("CapacityThresholdBytes", &setting.capacity_threshold_bytes)
        .map_err(|e| utils::command_err("CapacityThresholdBytes の書き込みに失敗しました", e))?;
    key.set_value("EnableStartup", &u32::from(setting.enable_startup))
        .map_err(|e| utils::command_err("EnableStartup の書き込みに失敗しました", e))?;
    key.set_value("MigrationStatus", &setting.migration_status)
        .map_err(|e| utils::command_err("MigrationStatus の書き込みに失敗しました", e))?;
    key.set_value("MigrationSourcePath", &setting.migration_source_path)
        .map_err(|e| utils::command_err("MigrationSourcePath の書き込みに失敗しました", e))?;
    Ok(())
}

/// レジストリから StellaRecord 設定を読み込む。キーが存在しない場合はデフォルト値を返す。
pub fn load_stellarecord_setting() -> StellaRecordSetting {
    let Some(key) = open_key(STELLA_RECORD_KEY) else {
        return StellaRecordSetting::default();
    };

    StellaRecordSetting {
        archive_path: read_str(&key, "ArchivePath"),
        db_path: read_str(&key, "DbPath"),
        enable_startup: read_bool(&key, "EnableStartup", false),
        startup_preference_set: read_bool(&key, "StartupPreferenceSet", false),
    }
}

/// StellaRecord 設定をレジストリに保存する。
pub fn save_stellarecord_setting(setting: &StellaRecordSetting) -> Result<(), String> {
    let key = create_key(STELLA_RECORD_KEY)?;
    key.set_value("ArchivePath", &setting.archive_path)
        .map_err(|e| utils::command_err("ArchivePath の書き込みに失敗しました", e))?;
    key.set_value("DbPath", &setting.db_path)
        .map_err(|e| utils::command_err("DbPath の書き込みに失敗しました", e))?;
    key.set_value("EnableStartup", &u32::from(setting.enable_startup))
        .map_err(|e| utils::command_err("EnableStartup の書き込みに失敗しました", e))?;
    key.set_value("StartupPreferenceSet", &u32::from(setting.startup_preference_set))
        .map_err(|e| utils::command_err("StartupPreferenceSet の書き込みに失敗しました", e))?;
    Ok(())
}

/// メインデータベースの `apps` テーブルからランチャーカタログを読み込む。
pub fn load_registry_catalog() -> RegistryCatalog {
    let setting = load_stellarecord_setting();
    let Some(db_path) = setting.get_effective_db_path() else {
        utils::log_warn("レジストリ読み込み時にデータベースパスを取得できませんでした");
        return RegistryCatalog::default();
    };

    if !db_path.exists() {
        return RegistryCatalog::default();
    }

    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(err) => {
            utils::log_warn(&format!(
                "レジストリ読み込み時にデータベースを開けませんでした: {err}"
            ));
            return RegistryCatalog::default();
        }
    };

    let has_table: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='apps')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !has_table {
        return RegistryCatalog::default();
    }

    let mut stmt = match conn.prepare(
        "SELECT name, description, path, icon FROM apps ORDER BY name",
    ) {
        Ok(s) => s,
        Err(err) => {
            utils::log_warn(&format!("レジストリクエリの準備に失敗しました: {err}"));
            return RegistryCatalog::default();
        }
    };

    let rows = match stmt.query_map([], |row| {
        let name: String = row.get(0)?;
        let description: String = row.get(1)?;
        let path: String = row.get(2)?;
        let icon: Option<Vec<u8>> = row.get(3)?;
        let icon_data = icon.map(|bytes| BASE64.encode(&bytes));
        Ok(AppCard {
            name,
            description,
            path,
            icon_data,
        })
    }) {
        Ok(r) => r,
        Err(err) => {
            utils::log_warn(&format!("レジストリクエリの実行に失敗しました: {err}"));
            return RegistryCatalog::default();
        }
    };

    let mut catalog = RegistryCatalog::default();
    for card in rows.flatten() {
        catalog.apps.push(card);
    }

    catalog
}

impl PolarisSetting {
    /// Polaris が使用するアーカイブディレクトリを解決する。
    pub fn get_effective_archive_dir(&self) -> Option<PathBuf> {
        if !self.archive_path.is_empty() {
            return Some(PathBuf::from(&self.archive_path));
        }
        Some(utils::get_polaris_install_dir()?.join("Data").join("archive"))
    }
}

impl StellaRecordSetting {
    /// `StellaRecord` が管理する圧縮ログアーカイブのディレクトリを解決する。
    pub fn get_effective_archive_dir(&self) -> Option<PathBuf> {
        if !self.archive_path.is_empty() {
            return Some(PathBuf::from(&self.archive_path));
        }
        utils::get_stellarecord_data_dir("archive")
    }

    /// `StellaRecord` が開くメインデータベースのパスを解決する。
    pub fn get_effective_db_path(&self) -> Option<PathBuf> {
        if !self.db_path.is_empty() {
            return Some(PathBuf::from(&self.db_path));
        }
        Some(utils::get_stellarecord_data_dir("db")?.join("stellarecord.db"))
    }
}
