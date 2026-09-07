import { useCallback, useEffect, useRef, useState } from 'react';
import type { ManagementSettings } from '../models/types';
import {
  loadManagementSettings as loadManagementSettingsCommand,
  saveManagementSettings as saveManagementSettingsCommand,
} from '../services/settingsService';
import { UserFacingError } from '../../../shared/lib/errors';

/** バックエンド設定読み込み前に使用する安全なデフォルト値 */
const defaultManagementSettings: ManagementSettings = {
  startup_enabled: false,
  startup_preference_set: false,
  archive_limit_mb: 300,
};

const maxArchiveLimitMb = 10_485_760;

/** 保存可能な容量上限だけを数値へ変換する */
function parseArchiveLimit(draft: string): number | null {
  const parsed = Number(draft);
  return Number.isFinite(parsed) &&
    parsed > 0 &&
    Number.isInteger(parsed) &&
    parsed <= maxArchiveLimitMb
    ? parsed
    : null;
}

/** 設定の下書き管理・永続化を行うフック */
export function useSettingsState() {
  const [managementSettings, setManagementSettings] =
    useState<ManagementSettings>(defaultManagementSettings);
  const [archiveLimitDraft, setArchiveLimitDraft] = useState('300');
  const [isStartupEnabledDraft, setIsStartupEnabledDraft] = useState(false);
  const persistedSettingsRef = useRef<ManagementSettings | null>(null);
  const settingsLoadPromiseRef = useRef<Promise<ManagementSettings> | null>(null);
  const isDraftSynchronizedRef = useRef(false);

  /** 初回取得を呼び出し元間で共有し、永続設定を一度だけ読み込む */
  const requestSettings = useCallback(() => {
    settingsLoadPromiseRef.current ??= loadManagementSettingsCommand()
      .then((settings) => {
        persistedSettingsRef.current = settings;
        isDraftSynchronizedRef.current = false;
        setManagementSettings(settings);
        return settings;
      })
      .catch((error: unknown) => {
        settingsLoadPromiseRef.current = null;
        throw error;
      });
    return settingsLoadPromiseRef.current;
  }, []);

  /** 自動起動とアーカイブ上限を一括保存し、成功時にローカル状態を更新する */
  const saveManagementSettings = useCallback(
    async (startupEnabled: boolean, archiveLimitMb: number) => {
      await saveManagementSettingsCommand(startupEnabled, archiveLimitMb);
      const settings: ManagementSettings = {
        startup_enabled: startupEnabled,
        startup_preference_set: true,
        archive_limit_mb: archiveLimitMb,
      };
      persistedSettingsRef.current = settings;
      isDraftSynchronizedRef.current = false;
      setManagementSettings(settings);
    },
    [],
  );

  // 初回レンダリングをブロックしないよう次ティックで遅延取得
  useEffect(() => {
    const initialLoadTimer = window.setTimeout(() => {
      void requestSettings().catch(() => {
        // 次回の操作または再マウント時に再取得する
      });
    }, 0);

    return () => {
      window.clearTimeout(initialLoadTimer);
    };
  }, [requestSettings]);

  // 永続化設定の変更時に下書きフィールドを同期（初回読み込みおよび保存後）
  useEffect(() => {
    setArchiveLimitDraft(String(managementSettings.archive_limit_mb));
    setIsStartupEnabledDraft(managementSettings.startup_enabled);
    isDraftSynchronizedRef.current = persistedSettingsRef.current === managementSettings;
  }, [managementSettings]);

  /** 自動起動のON/OFFを切り替えて即座に保存する（失敗時はロールバック） */
  const toggleStartup = useCallback(async () => {
    const shouldEnable = !isStartupEnabledDraft;
    const canUseArchiveLimitDraft = isDraftSynchronizedRef.current;
    setIsStartupEnabledDraft(shouldEnable);
    try {
      const persistedSettings = persistedSettingsRef.current ?? (await requestSettings());
      const limitMb =
        (canUseArchiveLimitDraft ? parseArchiveLimit(archiveLimitDraft) : null) ??
        persistedSettings.archive_limit_mb;
      await saveManagementSettings(shouldEnable, limitMb);
    } catch (error) {
      setIsStartupEnabledDraft(!shouldEnable);
      throw error;
    }
  }, [archiveLimitDraft, isStartupEnabledDraft, requestSettings, saveManagementSettings]);

  /** 警告ラインの入力値をバリデーションして保存する */
  const saveArchiveLimit = useCallback(async () => {
    const parsed = parseArchiveLimit(archiveLimitDraft);
    if (parsed === null) {
      throw new UserFacingError('警告ラインは 1MB～10,485,760MB (10TB) の整数で指定してください');
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
