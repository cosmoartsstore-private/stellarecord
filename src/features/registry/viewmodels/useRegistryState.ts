import { useCallback, useEffect, useState } from 'react';
import type { RegistryCatalog } from '../models/types';
import { loadRegistryCatalog } from '../services/registryService';

/** データ取得前に使用する空のカタログ */
const emptyRegistryCatalog: RegistryCatalog = {
  apps: [],
};

/** レジストリアプリカタログの取得・リロードを管理するフック */
export function useRegistryState() {
  const [registryApps, setRegistryApps] = useState<RegistryCatalog>(emptyRegistryCatalog);
  const [isReloading, setIsReloading] = useState(false);

  /** カタログを静かに取得する（次回の再読込で回復可能なためエラーは無視） */
  const loadRegistryState = useCallback(async () => {
    try {
      const registry = await loadRegistryCatalog();
      setRegistryApps(registry);
    } catch {
      // 次回の再読込で回復可能
    }
  }, []);

  /** リロードアイコンが最低1回転するよう700msの最小遅延付きでカタログを再取得する */
  const reloadRegistry = useCallback(async () => {
    setIsReloading(true);
    const minDelay = new Promise((r) => setTimeout(r, 700));
    try {
      await Promise.all([loadRegistryState(), minDelay]);
    } finally {
      setIsReloading(false);
    }
  }, [loadRegistryState]);

  // 初回レンダリングをブロックしないよう次ティックで遅延取得
  useEffect(() => {
    const initialLoadTimer = window.setTimeout(() => {
      void loadRegistryState();
    }, 0);

    return () => {
      window.clearTimeout(initialLoadTimer);
    };
  }, [loadRegistryState]);

  return {
    registryApps,
    isReloading,
    reloadRegistry,
    refreshRegistry: loadRegistryState,
  };
}
