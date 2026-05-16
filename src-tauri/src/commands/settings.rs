//! フロントエンド向け管理設定の CRUD とレジストリカタログアクセス。

use crate::config::{self, RegistryCatalog};
use crate::platform;

use super::{ManagementSettings, ONE_MB_BYTES, STELLA_RECORD_RUN_VALUE};

/// フロントエンド用の管理設定ダイアログ状態を返す。
///
/// ローカル `StellaRecord` 設定（スタートアップ設定）と Polaris 設定
/// （アーカイブ容量）の値を1つのペイロードにマージし、UI が1回の呼び出しで
/// 設定ダイアログの両セクションを表示できるようにする。
#[tauri::command]
pub fn get_management_settings() -> ManagementSettings {
    let stella_setting = config::load_stellarecord_setting();
    let polaris_setting = config::load_polaris_setting();
    let archive_limit_mb = ((polaris_setting.capacity_threshold_bytes + ONE_MB_BYTES / 2) / ONE_MB_BYTES).max(1);

    ManagementSettings {
        startup_enabled: stella_setting.enable_startup,
        startup_preference_set: stella_setting.startup_preference_set,
        archive_limit_mb,
    }
}

/// スタートアップとアーカイブ容量の管理設定を永続化する。
///
/// # Errors
/// 設定ファイルまたはスタートアップ登録を更新できない場合にエラーを返す。
#[tauri::command]
pub fn save_management_settings(
    startup_enabled: bool,
    archive_limit_mb: u64,
) -> Result<(), String> {
    // 容量ゼロのアーカイブストアを防ぐため最低 1 MB にする。
    let normalized_limit_mb = archive_limit_mb.max(1);
    let capacity_threshold_bytes = normalized_limit_mb * ONE_MB_BYTES;

    let mut stella_setting = config::load_stellarecord_setting();
    stella_setting.enable_startup = startup_enabled;
    stella_setting.startup_preference_set = true;
    config::save_stellarecord_setting(&stella_setting)?;
    platform::set_startup_enabled(STELLA_RECORD_RUN_VALUE, startup_enabled)?;

    let mut polaris_setting = config::load_polaris_setting();
    polaris_setting.capacity_threshold_bytes = capacity_threshold_bytes;
    config::save_polaris_setting(&polaris_setting)?;

    Ok(())
}

/// 外部アプリランチャーグリッドを構成するレジストリカタログを返す。
///
/// ユーザーが登録したアプリ（名前、パス、アイコン）を一覧にし、
/// `StellaRecord` サイドバーからのクイック起動を提供する。
#[tauri::command]
pub fn read_registry_catalog() -> RegistryCatalog {
    config::load_registry_catalog()
}
