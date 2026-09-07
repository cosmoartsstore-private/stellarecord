//! フロントエンド向け管理設定の CRUD とレジストリカタログアクセス。

use crate::config::{self, RegistryCatalog};
use crate::platform;

use super::{ManagementSettings, ONE_MB_BYTES, STELLA_RECORD_RUN_VALUE};

const MAX_ARCHIVE_LIMIT_MB: u64 = 10_485_760;

/// 起動時ログ取り込みの許可状態と、画面に表示する保存先。
#[derive(serde::Serialize)]
pub struct StartupImportSettings {
    pub enabled: bool,
    pub preference_set: bool,
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
    let log_archive_path = stella
        .get_effective_archive_dir()
        .ok_or_else(|| "ログ保存先を取得できませんでした。".to_string())?;
    let database_path = stella
        .get_effective_db_path()
        .ok_or_else(|| "DB保存先を取得できませんでした。".to_string())?;

    Ok(StartupImportSettings {
        enabled: preference.enabled,
        preference_set: preference.preference_set,
        log_archive_path: log_archive_path.display().to_string(),
        database_path: database_path.display().to_string(),
    })
}

/// 起動時ログ取り込みについてユーザーが選択した許可状態を保存する。
///
/// # Errors
/// レジストリへ保存できない場合にエラーを返す。
#[tauri::command]
pub fn save_startup_import_preference(enabled: bool) -> Result<(), String> {
    config::save_startup_import_preference(enabled)
}

/// フロントエンド用の管理設定ダイアログ状態を返す。
///
/// ローカル `StellaRecord` 設定（スタートアップ設定）と Polaris 設定
/// （アーカイブ容量）の値を1つのペイロードにマージし、UI が1回の呼び出しで
/// 設定ダイアログの両セクションを表示できるようにする。
#[tauri::command]
pub fn get_management_settings() -> ManagementSettings {
    build_management_settings(
        &config::load_stellarecord_setting(),
        &config::load_polaris_setting(),
    )
}

/// `StellaRecord` と Polaris の設定を UI 向けペイロードへ統合する。
///
/// 設定値を引数で受け取ることで、レジストリ参照を経由せず統合・MB 丸めロジックを検証できる。
fn build_management_settings(
    stella: &config::StellaRecordSetting,
    polaris: &config::PolarisSetting,
) -> ManagementSettings {
    ManagementSettings {
        startup_enabled: stella.enable_startup,
        startup_preference_set: stella.startup_preference_set,
        archive_limit_mb: bytes_to_archive_limit_mb(polaris.capacity_threshold_bytes),
    }
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
        return Err("警告ラインは 1MB～10,485,760MB (10TB) の整数で指定してください".to_string());
    }
    mb.checked_mul(ONE_MB_BYTES)
        .ok_or_else(|| "警告ラインをバイト数へ変換できませんでした".to_string())
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
    let result = save_polaris(next_polaris)
        .and_then(|()| set_startup(next_stella.enable_startup))
        .and_then(|()| save_stella(next_stella));

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
        return Err(format!(
            "{err}。設定の復元にも失敗しました: {}",
            rollback_errors.join(" / ")
        ));
    }

    Ok(())
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
    let capacity_threshold_bytes = archive_limit_mb_to_bytes(archive_limit_mb)?;

    let original_stella = config::load_stellarecord_setting();
    let mut next_stella = original_stella.clone();
    next_stella.enable_startup = startup_enabled;
    next_stella.startup_preference_set = true;

    let original_polaris = config::load_polaris_setting();
    let mut next_polaris = original_polaris.clone();
    next_polaris.capacity_threshold_bytes = capacity_threshold_bytes;

    persist_management_settings(
        &original_stella,
        &next_stella,
        &original_polaris,
        &next_polaris,
        config::save_stellarecord_setting,
        |enabled| platform::set_startup_enabled(STELLA_RECORD_RUN_VALUE, enabled),
        config::save_polaris_setting,
    )
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
        assert_eq!(
            bytes_to_archive_limit_mb(300 * ONE_MB_BYTES + ONE_MB_BYTES / 2),
            301
        );
        assert_eq!(
            bytes_to_archive_limit_mb(300 * ONE_MB_BYTES + ONE_MB_BYTES / 2 - 1),
            300
        );
    }

    #[test]
    fn bytes_to_mb_minimum_one() {
        assert_eq!(bytes_to_archive_limit_mb(0), 1);
        assert_eq!(bytes_to_archive_limit_mb(1), 1);
    }

    #[test]
    fn bytes_to_mb_clamps_corrupt_oversized_value() {
        assert_eq!(bytes_to_archive_limit_mb(u64::MAX), MAX_ARCHIVE_LIMIT_MB);
    }

    #[test]
    fn mb_to_bytes_basic() {
        assert_eq!(archive_limit_mb_to_bytes(300), Ok(300 * ONE_MB_BYTES));
        assert_eq!(archive_limit_mb_to_bytes(1), Ok(ONE_MB_BYTES));
    }

    #[test]
    fn mb_to_bytes_rejects_out_of_range_values() {
        assert!(archive_limit_mb_to_bytes(0).is_err());
        assert!(archive_limit_mb_to_bytes(MAX_ARCHIVE_LIMIT_MB + 1).is_err());
        assert!(archive_limit_mb_to_bytes(u64::MAX).is_err());
    }

    #[test]
    fn mb_conversion_roundtrip() {
        // 整数 MB はラウンドトリップで保たれる
        for mb in [1u64, 300, 2048, MAX_ARCHIVE_LIMIT_MB] {
            assert_eq!(
                archive_limit_mb_to_bytes(mb).map(bytes_to_archive_limit_mb),
                Ok(mb)
            );
        }
    }

    #[test]
    fn persist_settings_restores_original_values_after_failure() {
        use std::cell::{Cell, RefCell};

        let calls = RefCell::new(Vec::new());
        let startup_calls = Cell::new(0);
        let original_stella = config::StellaRecordSetting {
            enable_startup: false,
            ..Default::default()
        };
        let next_stella = config::StellaRecordSetting {
            enable_startup: true,
            startup_preference_set: true,
            ..Default::default()
        };
        let original_polaris = config::PolarisSetting {
            capacity_threshold_bytes: 300 * ONE_MB_BYTES,
            ..Default::default()
        };
        let next_polaris = config::PolarisSetting {
            capacity_threshold_bytes: 600 * ONE_MB_BYTES,
            ..Default::default()
        };

        let result = persist_management_settings(
            &original_stella,
            &next_stella,
            &original_polaris,
            &next_polaris,
            |setting| {
                calls
                    .borrow_mut()
                    .push(format!("stella:{}", setting.enable_startup));
                Ok(())
            },
            |enabled| {
                calls.borrow_mut().push(format!("startup:{enabled}"));
                startup_calls.set(startup_calls.get() + 1);
                if startup_calls.get() == 1 {
                    Err("スタートアップ保存失敗".to_string())
                } else {
                    Ok(())
                }
            },
            |setting| {
                calls.borrow_mut().push(format!(
                    "polaris:{}",
                    setting.capacity_threshold_bytes / ONE_MB_BYTES
                ));
                Ok(())
            },
        );

        assert_eq!(result, Err("スタートアップ保存失敗".to_string()));
        assert_eq!(
            calls.into_inner(),
            [
                "polaris:600",
                "startup:true",
                "stella:false",
                "startup:false",
                "polaris:300"
            ]
        );
    }

    // ── build_management_settings (設定統合ロジック) ──

    #[test]
    fn build_management_merges_both_sources() {
        let stella = config::StellaRecordSetting {
            enable_startup: true,
            startup_preference_set: true,
            ..Default::default()
        };
        let polaris = config::PolarisSetting {
            capacity_threshold_bytes: 300 * ONE_MB_BYTES,
            ..Default::default()
        };

        let merged = build_management_settings(&stella, &polaris);
        assert!(merged.startup_enabled);
        assert!(merged.startup_preference_set);
        assert_eq!(merged.archive_limit_mb, 300);
    }

    #[test]
    fn build_management_defaults() {
        let merged = build_management_settings(
            &config::StellaRecordSetting::default(),
            &config::PolarisSetting::default(),
        );
        assert!(!merged.startup_enabled);
        assert!(!merged.startup_preference_set);
        // デフォルト容量 (300 MB) が MB に丸められる
        assert_eq!(merged.archive_limit_mb, 300);
    }
}
