import { useCallback, useEffect, useState } from 'react';
import type { StartupImportSettings } from '../models/types';
import {
  loadStartupImportSettings,
  saveStartupImportPreference as saveStartupImportPreferenceCommand,
} from '../services/settingsService';

/** 初回確認と保存先表示で共有する、起動時ログ取り込み設定を管理する。 */
export function useStartupImportSettings() {
  const [settings, setSettings] = useState<StartupImportSettings | null>(null);
  const [loadError, setLoadError] = useState<unknown>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    let isMounted = true;
    loadStartupImportSettings()
      .then((loaded) => {
        if (!isMounted) return;
        setSettings(loaded);
        setLoadError(null);
      })
      .catch((error: unknown) => {
        if (!isMounted) return;
        setLoadError(error);
      })
      .finally(() => {
        if (isMounted) setIsLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, []);

  const saveStartupImportPreference = useCallback(async (enabled: boolean) => {
    setIsSaving(true);
    try {
      await saveStartupImportPreferenceCommand(enabled);
      setSettings((current) =>
        current
          ? {
              ...current,
              enabled,
              preference_set: true,
            }
          : current,
      );
    } finally {
      setIsSaving(false);
    }
  }, []);

  return {
    startupImportSettings: settings,
    startupImportSettingsError: loadError,
    isStartupImportSettingsLoading: isLoading,
    isStartupImportPreferenceSaving: isSaving,
    saveStartupImportPreference,
  };
}
