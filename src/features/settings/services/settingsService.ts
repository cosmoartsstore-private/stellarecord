import { invoke } from '@tauri-apps/api/core';
import type { ManagementSettings } from '../models/types';

/** 永続化された管理設定を読み込む */
export const loadManagementSettings = () => invoke<ManagementSettings>('get_management_settings');

/** 自動起動とアーカイブ容量の設定をバックエンド経由で保存する */
export const saveManagementSettings = (startupEnabled: boolean, archiveLimitMb: number) =>
  invoke('save_management_settings', {
    startupEnabled,
    archiveLimitMb,
  });
