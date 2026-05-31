use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use crate::utils;

const STELLA_RECORD_KEY: &str = "Software\\CosmoArtsStore\\StellaRecord";
const POLARIS_KEY: &str = "Software\\CosmoArtsStore\\Polaris";

/// HKCU 配下のキーを開く。存在しない場合は `None` を返す。
fn open_key(path: &str) -> Option<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey(path).ok()
}

/// HKCU 配下にキーを作成し、失敗時はコマンドエラー文字列に整形して返す。
fn create_key(path: &str) -> Result<RegKey, String> {
    RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(path)
        .map(|(key, _)| key)
        .map_err(|err| {
            utils::command_err(
                &format!("レジストリキーを作成できませんでした [{path}]"),
                err,
            )
        })
}

/// レジストリから文字列値を読み取る。失敗時は空文字列を返す。
fn read_str(key: &RegKey, name: &str) -> String {
    key.get_value::<String, _>(name).unwrap_or_default()
}

/// レジストリから u64 値を読み取る。失敗時は `default` を返す。
fn read_u64(key: &RegKey, name: &str, default: u64) -> u64 {
    key.get_value::<u64, _>(name).unwrap_or(default)
}

/// レジストリの u32 値（0/1）を bool として読み取る。失敗時は `default` を返す。
fn read_bool(key: &RegKey, name: &str, default: bool) -> bool {
    key.get_value::<u32, _>(name)
        .map(|v| v != 0)
        .unwrap_or(default)
}

/// `StellaRecord` から参照される Polaris 側のアーカイブ設定。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PolarisSetting {
    pub archive_path: String,
    pub capacity_threshold_bytes: u64,
}

const DEFAULT_CAPACITY: u64 = 314_572_800;

impl Default for PolarisSetting {
    fn default() -> Self {
        Self {
            archive_path: String::new(),
            capacity_threshold_bytes: DEFAULT_CAPACITY,
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
    load_polaris_setting_from(POLARIS_KEY)
}

/// 指定されたレジストリキーパスから Polaris 設定を読み込む。
///
/// 本番では `POLARIS_KEY` を渡す。キーパスを引数化することで、本番キーと干渉せず
/// テスト専用キーで読み込みロジックを検証できる。
fn load_polaris_setting_from(key_path: &str) -> PolarisSetting {
    let Some(key) = open_key(key_path) else {
        return PolarisSetting::default();
    };

    PolarisSetting {
        archive_path: read_str(&key, "ArchivePath"),
        capacity_threshold_bytes: read_u64(&key, "CapacityThresholdBytes", DEFAULT_CAPACITY),
    }
}

/// Polaris 設定をレジストリに保存する。
///
/// # Errors
/// レジストリキーへの書き込みに失敗した場合にエラーを返す。
pub fn save_polaris_setting(setting: &PolarisSetting) -> Result<(), String> {
    save_polaris_setting_to(POLARIS_KEY, setting)
}

/// 指定されたレジストリキーパスに Polaris 設定を保存する。
fn save_polaris_setting_to(key_path: &str, setting: &PolarisSetting) -> Result<(), String> {
    let key = create_key(key_path)?;
    key.set_value("ArchivePath", &setting.archive_path)
        .map_err(|e| utils::command_err("ArchivePath の書き込みに失敗しました", e))?;
    key.set_value("CapacityThresholdBytes", &setting.capacity_threshold_bytes)
        .map_err(|e| utils::command_err("CapacityThresholdBytes の書き込みに失敗しました", e))?;
    Ok(())
}

/// レジストリから `StellaRecord` 設定を読み込む。キーが存在しない場合はデフォルト値を返す。
pub fn load_stellarecord_setting() -> StellaRecordSetting {
    load_stellarecord_setting_from(STELLA_RECORD_KEY)
}

/// 指定されたレジストリキーパスから `StellaRecord` 設定を読み込む。
fn load_stellarecord_setting_from(key_path: &str) -> StellaRecordSetting {
    let Some(key) = open_key(key_path) else {
        return StellaRecordSetting::default();
    };

    StellaRecordSetting {
        archive_path: read_str(&key, "ArchivePath"),
        db_path: read_str(&key, "DbPath"),
        enable_startup: read_bool(&key, "EnableStartup", false),
        startup_preference_set: read_bool(&key, "StartupPreferenceSet", false),
    }
}

/// `StellaRecord` 設定をレジストリに保存する。
///
/// # Errors
/// レジストリキーへの書き込みに失敗した場合にエラーを返す。
pub fn save_stellarecord_setting(setting: &StellaRecordSetting) -> Result<(), String> {
    save_stellarecord_setting_to(STELLA_RECORD_KEY, setting)
}

/// 指定されたレジストリキーパスに `StellaRecord` 設定を保存する。
fn save_stellarecord_setting_to(
    key_path: &str,
    setting: &StellaRecordSetting,
) -> Result<(), String> {
    let key = create_key(key_path)?;
    key.set_value("ArchivePath", &setting.archive_path)
        .map_err(|e| utils::command_err("ArchivePath の書き込みに失敗しました", e))?;
    key.set_value("DbPath", &setting.db_path)
        .map_err(|e| utils::command_err("DbPath の書き込みに失敗しました", e))?;
    key.set_value("EnableStartup", &u32::from(setting.enable_startup))
        .map_err(|e| utils::command_err("EnableStartup の書き込みに失敗しました", e))?;
    key.set_value(
        "StartupPreferenceSet",
        &u32::from(setting.startup_preference_set),
    )
    .map_err(|e| utils::command_err("StartupPreferenceSet の書き込みに失敗しました", e))?;
    Ok(())
}

/// メインデータベースの `apps` テーブルからランチャーカタログを読み込む。
///
/// DB が存在しない場合やテーブル未作成の場合は空のカタログを返す。
/// アイコン BLOB は Base64 にエンコードし、フロントエンドが `<img src>` で
/// 直接使用できる形にする。
pub fn load_registry_catalog() -> RegistryCatalog {
    let setting = load_stellarecord_setting();
    let Some(db_path) = setting.get_effective_db_path() else {
        utils::log_warn("レジストリ読み込み時にデータベースパスを取得できませんでした");
        return RegistryCatalog::default();
    };
    load_catalog_from_db(&db_path)
}

/// 指定された DB パスから `apps` カタログを読み込む。
///
/// DB パスを引数化することで、レジストリ解決を経由せず一時 DB でカタログ読み込み
/// （存在確認・BLOB の Base64 化・行マッピング）を検証できる。
fn load_catalog_from_db(db_path: &std::path::Path) -> RegistryCatalog {
    if !db_path.exists() {
        return RegistryCatalog::default();
    }

    let conn = match Connection::open(db_path) {
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

    let mut stmt =
        match conn.prepare("SELECT name, description, path, icon FROM apps ORDER BY name") {
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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

    // ── open_key / create_key ──

    #[test]
    fn open_key_returns_none_for_missing() {
        assert!(open_key("Software\\CosmoArtsStore\\_Test_missing_99999").is_none());
    }

    #[test]
    fn create_key_and_open_key_roundtrip() {
        let path = "Software\\CosmoArtsStore\\_Test_create_open";
        let _guard = TestRegGuard(path.to_string());

        let key = create_key(path).unwrap();
        key.set_value("Marker", &"ok").unwrap();

        let opened = open_key(path);
        assert!(opened.is_some());
        let val: String = opened.unwrap().get_value("Marker").unwrap();
        assert_eq!(val, "ok");
    }

    // ── read_str / read_u64 / read_bool ──

    #[test]
    fn read_str_returns_value() {
        let (key, _guard) = create_test_key("read_str");
        key.set_value("Name", &"hello").unwrap();
        assert_eq!(read_str(&key, "Name"), "hello");
    }

    #[test]
    fn read_str_returns_empty_when_missing() {
        let (key, _guard) = create_test_key("read_str_miss");
        assert_eq!(read_str(&key, "NoSuchValue"), "");
    }

    #[test]
    fn read_u64_returns_value() {
        let (key, _guard) = create_test_key("read_u64");
        key.set_value("Capacity", &314_572_800u64).unwrap();
        assert_eq!(read_u64(&key, "Capacity", 0), 314_572_800);
    }

    #[test]
    fn read_u64_returns_default_when_missing() {
        let (key, _guard) = create_test_key("read_u64_miss");
        assert_eq!(read_u64(&key, "NoSuchValue", 42), 42);
    }

    #[test]
    fn read_bool_true() {
        let (key, _guard) = create_test_key("read_bool_t");
        key.set_value("Flag", &1u32).unwrap();
        assert!(read_bool(&key, "Flag", false));
    }

    #[test]
    fn read_bool_false() {
        let (key, _guard) = create_test_key("read_bool_f");
        key.set_value("Flag", &0u32).unwrap();
        assert!(!read_bool(&key, "Flag", true));
    }

    #[test]
    fn read_bool_default_when_missing() {
        let (key, _guard) = create_test_key("read_bool_miss");
        assert!(read_bool(&key, "NoSuchValue", true));
        assert!(!read_bool(&key, "NoSuchValue", false));
    }

    // ── save/load roundtrip (テスト専用キーパスで実行) ──

    #[test]
    fn polaris_setting_save_load_roundtrip() {
        let test_path = "Software\\CosmoArtsStore\\_Test_Polaris_RT";
        let _guard = TestRegGuard(test_path.to_string());

        let setting = PolarisSetting {
            archive_path: r"F:\planetes-atelier\software\AppTest\archive".to_string(),
            capacity_threshold_bytes: 1_073_741_824,
        };

        save_polaris_setting_to(test_path, &setting).unwrap();
        let loaded = load_polaris_setting_from(test_path);

        assert_eq!(loaded.archive_path, setting.archive_path);
        assert_eq!(loaded.capacity_threshold_bytes, 1_073_741_824);
    }

    #[test]
    fn polaris_setting_load_missing_returns_default() {
        let loaded = load_polaris_setting_from("Software\\CosmoArtsStore\\_Test_Polaris_Missing");
        assert_eq!(loaded.archive_path, "");
        assert_eq!(loaded.capacity_threshold_bytes, DEFAULT_CAPACITY);
    }

    #[test]
    fn stellarecord_setting_save_load_roundtrip() {
        let test_path = "Software\\CosmoArtsStore\\_Test_SR_RT";
        let _guard = TestRegGuard(test_path.to_string());

        let setting = StellaRecordSetting {
            archive_path: r"F:\planetes-atelier\software\AppTest\archive".to_string(),
            db_path: r"F:\planetes-atelier\software\AppTest\db\test.db".to_string(),
            enable_startup: true,
            startup_preference_set: true,
        };

        save_stellarecord_setting_to(test_path, &setting).unwrap();
        let loaded = load_stellarecord_setting_from(test_path);

        assert_eq!(loaded.archive_path, setting.archive_path);
        assert_eq!(loaded.db_path, setting.db_path);
        assert!(loaded.enable_startup);
        assert!(loaded.startup_preference_set);
    }

    #[test]
    fn stellarecord_setting_load_missing_returns_default() {
        let loaded = load_stellarecord_setting_from("Software\\CosmoArtsStore\\_Test_SR_Missing");
        assert_eq!(loaded.archive_path, "");
        assert_eq!(loaded.db_path, "");
        assert!(!loaded.enable_startup);
        assert!(!loaded.startup_preference_set);
    }

    // ── load_catalog_from_db (一時 DB で検証) ──

    #[test]
    fn catalog_from_db_returns_empty_when_db_missing() {
        let path = std::path::Path::new("Z:\\nonexistent\\catalog.db");
        let catalog = load_catalog_from_db(path);
        assert!(catalog.apps.is_empty());
    }

    #[test]
    fn catalog_from_db_returns_empty_when_no_apps_table() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("empty.db");
        Connection::open(&db_path).unwrap();
        let catalog = load_catalog_from_db(&db_path);
        assert!(catalog.apps.is_empty());
    }

    #[test]
    fn catalog_from_db_reads_apps_with_base64_icon() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("catalog.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE apps (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL, description TEXT DEFAULT '',
                path TEXT UNIQUE, icon BLOB
            );
            INSERT INTO apps (name, description, path, icon)
                VALUES ('Beta', 'second', '/b', NULL);
            INSERT INTO apps (name, description, path, icon)
                VALUES ('Alpha', 'first', '/a', x'89504E47');",
        )
        .unwrap();
        drop(conn);

        let catalog = load_catalog_from_db(&db_path);
        assert_eq!(catalog.apps.len(), 2);
        // ORDER BY name のため Alpha が先
        assert_eq!(catalog.apps[0].name, "Alpha");
        assert_eq!(catalog.apps[0].icon_data.as_deref(), Some("iVBORw==")); // base64(0x89504E47)
        assert_eq!(catalog.apps[1].name, "Beta");
        assert!(catalog.apps[1].icon_data.is_none());
    }

    // ── StellaRecordSetting パス解決 ──

    #[test]
    fn effective_archive_dir_uses_explicit_path() {
        let setting = StellaRecordSetting {
            archive_path: r"F:\planetes-atelier\software\AppTest\archive".to_string(),
            ..Default::default()
        };
        assert_eq!(
            setting.get_effective_archive_dir(),
            Some(PathBuf::from(
                r"F:\planetes-atelier\software\AppTest\archive"
            ))
        );
    }

    #[test]
    fn effective_db_path_uses_explicit_path() {
        let setting = StellaRecordSetting {
            db_path: r"F:\planetes-atelier\software\AppTest\db\test.db".to_string(),
            ..Default::default()
        };
        assert_eq!(
            setting.get_effective_db_path(),
            Some(PathBuf::from(
                r"F:\planetes-atelier\software\AppTest\db\test.db"
            ))
        );
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
