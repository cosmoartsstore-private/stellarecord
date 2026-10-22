//! アプリ全体の識別子、保存先、共通単位と容量設定の契約値。

pub const APP_NAME: &str = "StellaRecord";
pub const APP_DISPLAY_NAME: &str = "STELLA RECORD";
pub const POLARIS_APP_NAME: &str = "Polaris";
pub const REGISTRY_KEY_PREFIX: &str = "Software\\CosmoArtsStore";
pub const STELLA_RECORD_KEY: &str = "Software\\CosmoArtsStore\\StellaRecord";
pub const POLARIS_KEY: &str = "Software\\CosmoArtsStore\\Polaris";
pub const INSTALL_LOCATION_VALUE: &str = "InstallLocation";
pub const DATA_DIRECTORY: &str = "Data";
pub const ARCHIVE_DIRECTORY: &str = "archive";
pub const DATABASE_DIRECTORY: &str = "db";
pub const LOG_DIRECTORY: &str = "logs";
pub const DATABASE_FILE: &str = "stellarecord.db";
pub const BYTES_PER_MEBIBYTE: u64 = 1024 * 1024;
pub const DEFAULT_ARCHIVE_CAPACITY_BYTES: u64 = 300 * BYTES_PER_MEBIBYTE;
pub const MAX_ARCHIVE_LIMIT_MB: u64 = 10_485_760;
