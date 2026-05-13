import { useCallback, useEffect, useState } from 'react';
import type { ManagementSettings } from '../models/types';
import {
  loadManagementSettings as loadManagementSettingsCommand,
  saveManagementSettings as saveManagementSettingsCommand,
} from '../services/settingsService';

/** バックエンド設定読み込み前に使用する安全なデフォルト値 */
const defaultManagementSettings: ManagementSettings = {
  startup_enabled: false,
  startup_preference_set: false,
  archive_limit_mb: 300,
};

/** 設定の下書き管理・永続化を行うフック */
export function useSettingsState() {
  const [managementSettings, setManagementSettings] =
    useState<ManagementSettings>(defaultManagementSettings);
  const [archiveLimitDraft, setArchiveLimitDraft] = useState('300');
  const [isStartupEnabledDraft, setIsStartupEnabledDraft] = useState(false);

  /** バックエンドから設定を取得する */
  const loadSettings = useCallback(async () => {
    try {
      const settings = await loadManagementSettingsCommand();
      setManagementSettings(settings);
    } catch {
      // 次回の取得成功まで現在の状態を維持
    }
  }, []);

  /** 自動起動とアーカイブ上限を一括保存し、成功時にローカル状態を更新する */
  const saveManagementSettings = useCallback(
    async (startupEnabled: boolean, archiveLimitMb: number) => {
      await saveManagementSettingsCommand(startupEnabled, archiveLimitMb);
      setManagementSettings({
        startup_enabled: startupEnabled,
        startup_preference_set: true,
        archive_limit_mb: archiveLimitMb,
      });
    },
    [],
  );

  // 初回レンダリングをブロックしないよう次ティックで遅延取得
  useEffect(() => {
    const initialLoadTimer = window.setTimeout(() => {
      void loadSettings();
    }, 0);

    return () => {
      window.clearTimeout(initialLoadTimer);
    };
  }, [loadSettings]);

  // 永続化設定の変更時に下書きフィールドを同期（初回読み込みおよび保存後）
  useEffect(() => {
    setArchiveLimitDraft(String(managementSettings.archive_limit_mb));
    setIsStartupEnabledDraft(managementSettings.startup_enabled);
  }, [managementSettings]);

  /** 自動起動のON/OFFを切り替えて即座に保存する（失敗時はロールバック） */
  const toggleStartup = useCallback(async () => {
    const newValue = !isStartupEnabledDraft;
    setIsStartupEnabledDraft(newValue);
    const parsed = Number(archiveLimitDraft);
    const limitMb = Number.isFinite(parsed) && parsed > 0 ? parsed : 300;
    try {
      await saveManagementSettings(newValue, limitMb);
    } catch (error) {
      setIsStartupEnabledDraft(!newValue);
      throw error;
    }
  }, [archiveLimitDraft, isStartupEnabledDraft, saveManagementSettings]);

  /** 警告ラインの入力値をバリデーションして保存する */
  const saveArchiveLimit = useCallback(async () => {
    const parsed = Number(archiveLimitDraft);
    if (!Number.isFinite(parsed) || parsed <= 0 || !Number.isInteger(parsed) || parsed > 10485760) {
      throw new Error('警告ラインは 1MB～10,485,760MB (10TB) の整数で指定してください');
    }
    await saveManagementSettings(isStartupEnabledDraft, parsed);
  }, [archiveLimitDraft, isStartupEnabledDraft, saveManagementSettings]);

  return {
    archiveLimitDraft,
    isStartupEnabledDraft,
    setArchiveLimitDraft,
    toggleStartup,
    saveArchiveLimit,
  };
}
