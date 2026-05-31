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

    ManagementSettings {
        startup_enabled: stella_setting.enable_startup,
        startup_preference_set: stella_setting.startup_preference_set,
        archive_limit_mb: bytes_to_archive_limit_mb(polaris_setting.capacity_threshold_bytes),
    }
}

/// バイト数を表示用のメガバイト整数へ四捨五入する（最低 1 MB）。
///
/// 表示と保存で同じ丸め基準を共有するため独立関数にしている。
fn bytes_to_archive_limit_mb(bytes: u64) -> u64 {
    ((bytes + ONE_MB_BYTES / 2) / ONE_MB_BYTES).max(1)
}

/// メガバイト値をバイト数へ変換する（最低 1 MB ぶんを保証）。
///
/// 容量ゼロのアーカイブストアを防ぐため下限を 1 MB に正規化する。
fn archive_limit_mb_to_bytes(mb: u64) -> u64 {
    mb.max(1) * ONE_MB_BYTES
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
    let capacity_threshold_bytes = archive_limit_mb_to_bytes(archive_limit_mb);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_to_mb_rounds_to_nearest() {
        // 300 MB ちょうど
        assert_eq!(bytes_to_archive_limit_mb(300 * ONE_MB_BYTES), 300);
        // 端数は四捨五入（0.5 MB 以上で切り上げ）
        assert_eq!(bytes_to_archive_limit_mb(300 * ONE_MB_BYTES + ONE_MB_BYTES / 2), 301);
        assert_eq!(bytes_to_archive_limit_mb(300 * ONE_MB_BYTES + ONE_MB_BYTES / 2 - 1), 300);
    }

    #[test]
    fn bytes_to_mb_minimum_one() {
        assert_eq!(bytes_to_archive_limit_mb(0), 1);
        assert_eq!(bytes_to_archive_limit_mb(1), 1);
    }

    #[test]
    fn mb_to_bytes_basic() {
        assert_eq!(archive_limit_mb_to_bytes(300), 300 * ONE_MB_BYTES);
        assert_eq!(archive_limit_mb_to_bytes(1), ONE_MB_BYTES);
    }

    #[test]
    fn mb_to_bytes_minimum_one() {
        // 0 MB 指定でも最低 1 MB を保証
        assert_eq!(archive_limit_mb_to_bytes(0), ONE_MB_BYTES);
    }

    #[test]
    fn mb_conversion_roundtrip() {
        // 整数 MB はラウンドトリップで保たれる
        for mb in [1u64, 300, 2048, 10_485_760] {
            assert_eq!(bytes_to_archive_limit_mb(archive_limit_mb_to_bytes(mb)), mb);
        }
    }
}
