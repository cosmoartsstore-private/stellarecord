#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_to_mb_rounds_to_nearest() {
        // 300 MB ちょうど
        assert_eq!(bytes_to_archive_limit_mb(300 * 1_048_576), 300);
        // 端数は四捨五入（0.5 MB 以上で切り上げ）
        assert_eq!(
            bytes_to_archive_limit_mb(300 * 1_048_576 + 1_048_576 / 2),
            301
        );
        assert_eq!(
            bytes_to_archive_limit_mb(300 * 1_048_576 + 1_048_576 / 2 - 1),
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
        assert_eq!(bytes_to_archive_limit_mb(u64::MAX), 10_485_760);
    }

    #[test]
    fn mb_to_bytes_basic() {
        assert_eq!(archive_limit_mb_to_bytes(300), Ok(300 * 1_048_576));
        assert_eq!(archive_limit_mb_to_bytes(1), Ok(1_048_576));
        assert_eq!(archive_limit_mb_to_bytes(10_485_760), Ok(10_995_116_277_760));
    }

    #[test]
    fn mb_to_bytes_rejects_out_of_range_values() {
        assert_eq!(archive_limit_mb_to_bytes(0), Err(message!("Commands.Settings.archiveLimitMbToBytes.text").to_string()));
        assert_eq!(archive_limit_mb_to_bytes(10_485_761), Err(message!("Commands.Settings.archiveLimitMbToBytes.text").to_string()));
        assert_eq!(archive_limit_mb_to_bytes(u64::MAX), Err(message!("Commands.Settings.archiveLimitMbToBytes.text").to_string()));
    }

    #[test]
    fn mb_conversion_roundtrip() {
        // 整数 MB はラウンドトリップで保たれる
        for mb in [1u64, 300, 2048, 10_485_760] {
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
            capacity_threshold_bytes: 300 * 1_048_576,
            ..Default::default()
        };
        let next_polaris = config::PolarisSetting {
            capacity_threshold_bytes: 600 * 1_048_576,
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
                    setting.capacity_threshold_bytes / 1_048_576
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

    /// 保存の各段階と復元の独立した失敗を、永続境界の状態変化と呼出順で照合する。
    fn verify_management_failure_and_retry(failure_stage: usize) {
        use std::cell::{Cell, RefCell};

        #[derive(Debug, PartialEq, Eq)]
        struct Saved {
            startup: bool,
            stella: bool,
            capacity: u64,
        }

        for rollback_mask in 0_u8..8 {
            let calls = RefCell::new(Vec::new());
            let counts = [Cell::new(0), Cell::new(0), Cell::new(0)];
            let fault_active = Cell::new(true);
            let state = RefCell::new(Saved { startup: false, stella: false, capacity: 300 });
            let old_stella = config::StellaRecordSetting { archive_path: "保持するarchive".to_string(), db_path: "保持するDB".to_string(), ..Default::default() };
            let next_stella = config::StellaRecordSetting { enable_startup: true, startup_preference_set: true, ..old_stella.clone() };
            let old_polaris = config::PolarisSetting { archive_path: "保持するPolaris保存先".to_string(), capacity_threshold_bytes: 300 };
            let next_polaris = config::PolarisSetting { capacity_threshold_bytes: 600, ..old_polaris.clone() };
            let apply = |stage: usize, value: bool| -> Result<(), String> {
                let names = ["polaris", "startup", "stella"];
                let rollback_bits = [4, 2, 1];
                let call = counts[stage].get();
                counts[stage].set(call + 1);
                calls.borrow_mut().push(format!("{}:{}", names[stage], if value { "next" } else { "old" }));
                if fault_active.get() && !value && rollback_mask & rollback_bits[stage] != 0 {
                    return Err(format!("restore-{}", names[stage]));
                }
                // 保存先が一部を書き換えてから失敗する場合も、復元の対象となる。
                match stage {
                    0 => state.borrow_mut().capacity = if value { 600 } else { 300 },
                    1 => state.borrow_mut().startup = value,
                    _ => state.borrow_mut().stella = value,
                }
                if fault_active.get() && call == 0 && stage == failure_stage { return Err(format!("save-{}", names[stage])); }
                Ok(())
            };
            let persist = || persist_management_settings(&old_stella, &next_stella, &old_polaris, &next_polaris,
                |setting| {
                    assert_eq!((&setting.archive_path, &setting.db_path), (&old_stella.archive_path, &old_stella.db_path));
                    apply(2, setting.enable_startup)
                },
                |enabled| apply(1, enabled),
                |setting| { assert_eq!(setting.archive_path, old_polaris.archive_path); apply(0, setting.capacity_threshold_bytes == 600) });
            let primary = format!("save-{}", ["polaris", "startup", "stella"][failure_stage]);
            let rollback_errors = ["restore-stella", "restore-startup", "restore-polaris"].into_iter().enumerate()
                .filter(|(index, _)| rollback_mask & (1 << index) != 0).map(|(_, value)| value).collect::<Vec<_>>();
            let expected_error = if rollback_errors.is_empty() { primary } else { format!("{primary}。設定の復元にも失敗しました: {}", rollback_errors.join(" / ")) };
            assert_eq!(persist(), Err(expected_error), "stage={failure_stage}; rollback={rollback_mask}");
            let mut expected_calls = ["polaris:next", "startup:next", "stella:next"][..=failure_stage].iter().map(ToString::to_string).collect::<Vec<_>>();
            expected_calls.extend(["stella:old", "startup:old", "polaris:old"].map(ToString::to_string));
            assert_eq!(*calls.borrow(), expected_calls);
            assert_eq!(*state.borrow(), Saved {
                startup: failure_stage >= 1 && rollback_mask & 2 != 0,
                stella: failure_stage == 2 && rollback_mask & 1 != 0,
                capacity: if rollback_mask & 4 != 0 { 600 } else { 300 },
            });
            fault_active.set(false);
            assert_eq!(persist(), Ok(()));
            assert_eq!(*state.borrow(), Saved { startup: true, stella: true, capacity: 600 });
            assert_eq!(&calls.borrow()[expected_calls.len()..], ["polaris:next", "startup:next", "stella:next"]);
        }
    }

    #[test]
    fn persist_settings_polaris_save_failure_reports_each_rollback_and_retries() {
        verify_management_failure_and_retry(0);
    }

    #[test]
    fn persist_settings_run_save_failure_reports_each_rollback_and_retries() {
        verify_management_failure_and_retry(1);
    }

    #[test]
    fn persist_settings_stella_save_failure_reports_each_rollback_and_retries() {
        verify_management_failure_and_retry(2);
    }

    #[test]
    fn build_management_merges_both_sources() {
        let stella = config::StellaRecordSetting {
            enable_startup: true,
            startup_preference_set: true,
            ..Default::default()
        };
        let polaris = config::PolarisSetting {
            capacity_threshold_bytes: 300 * 1_048_576,
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
