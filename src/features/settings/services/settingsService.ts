import { invoke } from '@tauri-apps/api/core';
import type { ManagementSettings, StartupImportSettings } from '../models/types';

/** 永続化された管理設定を読み込む */
export const loadManagementSettings = () => invoke<ManagementSettings>('get_management_settings');

/** 自動起動とアーカイブ容量の設定をBackend経由で保存する */
export const saveManagementSettings = (startupEnabled: boolean, archiveLimitMb: number) => invoke('save_management_settings', { startupEnabled, archiveLimitMb });

/** 起動時ログ取り込みの許可状態と保存先を読み込む */
export const loadStartupImportSettings = () => invoke<StartupImportSettings>('get_startup_import_settings');

/** 読込元を選択して保存する。キャンセル時はnullを返す */
export const changeSourceLogDirectory = () => invoke<string | null>('change_source_log_directory');

/** 起動時ログ取り込みについてユーザーが選択した許可状態を保存する */
export const saveStartupImportPreference = (enabled: boolean) => invoke('save_startup_import_preference', { enabled });
