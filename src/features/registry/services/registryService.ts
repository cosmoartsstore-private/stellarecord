import { invoke } from '@tauri-apps/api/core';
import type { RegistryCatalog } from '../models/types';

/** 登録済みアプリの実行ファイルを起動する */
export const launchExternalApp = (appPath: string) => invoke('launch_external_app', { appPath });

/** 指定パスをOSのファイルエクスプローラで開く */
export const openFolder = (path: string) => invoke('open_folder', { path });

/** ランチャー画面用のアプリレジストリカタログを取得する */
export const loadRegistryCatalog = () => invoke<RegistryCatalog>('read_registry_catalog');

/** ネイティブファイルダイアログで exe ファイルを選択する */
export const pickExeFile = () => invoke<string | null>('pick_exe_file');

/** サードパーティアプリをランチャーに登録する */
export const registerApp = (path: string, name: string, description: string) =>
  invoke('register_app', { path, name, description });

/** サードパーティアプリの登録を解除する */
export const unregisterApp = (name: string) => invoke('unregister_app', { name });
