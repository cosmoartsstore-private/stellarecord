import { getMsg } from '../../../messages/getMsg';
import { archiveCapacity } from '../../../shared/constants';
import { useCallback, useEffect, useRef, useState } from 'react';
import type { ManagementSettings } from '../models/types';
import { loadManagementSettings as loadManagementSettingsCommand, saveManagementSettings as saveManagementSettingsCommand } from '../services/settingsService';
import { UserFacingError } from '../../../shared/lib/errors';

/** Backend設定読込前の容量値と画面の下書きで共有する既定値。 */
const defaultArchiveLimitMb = archiveCapacity.DEFAULT_MB;
const defaultManagementSettings: ManagementSettings = { startup_enabled: false, startup_preference_set: false, archive_limit_mb: defaultArchiveLimitMb };

const maxArchiveLimitMb = archiveCapacity.MAX_MB;

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
  const [archiveLimitDraft, setArchiveLimitDraftState] = useState(String(defaultArchiveLimitMb));
  const [isStartupEnabledDraft, setIsStartupEnabledDraft] = useState(false);
  const persistedSettingsRef = useRef<ManagementSettings | null>(null);
  const settingsLoadPromiseRef = useRef<Promise<ManagementSettings> | null>(null);
  const isDraftSynchronizedRef = useRef(false);
  const archiveDraftEditGenerationRef = useRef(0);
  const archiveDraftSyncGenerationRef = useRef(0);

  const setArchiveLimitDraft = useCallback((draft: string) => {
    archiveDraftEditGenerationRef.current += 1;
    setArchiveLimitDraftState(draft);
  }, []);

  /** 初回取得を呼び出し元間で共有し、永続設定を一度だけ読み込む */
  const requestSettings = useCallback(() => {
    settingsLoadPromiseRef.current ??= loadManagementSettingsCommand()
      .then((settings) => {
        persistedSettingsRef.current = settings;
        isDraftSynchronizedRef.current = false;
        archiveDraftSyncGenerationRef.current = 0;
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
    async (startupEnabled: boolean, archiveLimitMb: number, draftGeneration: number) => {
      await saveManagementSettingsCommand(startupEnabled, archiveLimitMb);
      const settings: ManagementSettings = { startup_enabled: startupEnabled, startup_preference_set: true, archive_limit_mb: archiveLimitMb };
      persistedSettingsRef.current = settings;
      isDraftSynchronizedRef.current = false;
      archiveDraftSyncGenerationRef.current = draftGeneration;
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
    // 読込開始後・保存要求後の入力を、古い永続値の応答で上書きしない。
    if (archiveDraftEditGenerationRef.current === archiveDraftSyncGenerationRef.current) {
      setArchiveLimitDraftState(String(managementSettings.archive_limit_mb));
    }
    setIsStartupEnabledDraft(managementSettings.startup_enabled);
    isDraftSynchronizedRef.current = persistedSettingsRef.current === managementSettings;
  }, [managementSettings]);

  /** 自動起動のON/OFFを切り替えて即座に保存する（失敗時はロールバック） */
  const toggleStartup = useCallback(async () => {
    const shouldEnable = !isStartupEnabledDraft;
    const canUseArchiveLimitDraft = isDraftSynchronizedRef.current;
    const draftGeneration = archiveDraftEditGenerationRef.current;
    setIsStartupEnabledDraft(shouldEnable);
    try {
      const persistedSettings = persistedSettingsRef.current ?? (await requestSettings());
      const limitMb =
        (canUseArchiveLimitDraft ? parseArchiveLimit(archiveLimitDraft) : null) ??
        persistedSettings.archive_limit_mb;
      await saveManagementSettings(shouldEnable, limitMb, draftGeneration);
    } catch (error) {
      setIsStartupEnabledDraft(!shouldEnable);
      throw error;
    }
  }, [archiveLimitDraft, isStartupEnabledDraft, requestSettings, saveManagementSettings]);

  /** 警告ラインの入力値をバリデーションして保存する */
  const saveArchiveLimit = useCallback(async () => {
    const parsed = parseArchiveLimit(archiveLimitDraft);
    if (parsed === null) {
      throw new UserFacingError(getMsg('SettingsState.invalidArchiveLimit'));
    }
    const canUseStartupDraft = isDraftSynchronizedRef.current;
    const draftGeneration = archiveDraftEditGenerationRef.current;
    const persistedSettings = persistedSettingsRef.current ?? (await requestSettings());
    await saveManagementSettings(canUseStartupDraft ? isStartupEnabledDraft : persistedSettings.startup_enabled, parsed, draftGeneration);
  }, [archiveLimitDraft, isStartupEnabledDraft, requestSettings, saveManagementSettings]);

  return { archiveLimitDraft, isStartupEnabledDraft, setArchiveLimitDraft, toggleStartup, saveArchiveLimit };
}
