/**
 * アーカイブ操作のTauri IPCラッパー
 *
 * 各関数はRust側の `#[tauri::command]` と1対1対応。ストリーミングログビューアは
 * 2フェーズ構成: startLogViewerStream がメタ情報を同期的に返し、バックエンドが
 * log_viewer_chunk / log_viewer_done イベントを非同期に発行する
 */
import { invoke } from '@tauri-apps/api/core';
import type { ArchiveFileItem, LogViewerMeta, StartupImportSummary } from '../models/types';

/** アーカイブディレクトリ内の .tar.zst ファイル一覧を取得する */
export const loadArchiveFiles = () => invoke<ArchiveFileItem[]>('list_archive_files');

/** 選択されたアーカイブファイルのバッチインポートを開始する */
export const launchEnhancedImport = (fileNames: string[]) =>
  invoke('launch_enhanced_import', { fileNames });

/** ストリーミングログビューアセッションを開始する（チャンクはTauriイベント経由で到着） */
export const startLogViewerStream = (fileName: string, sessionId: string) =>
  invoke<LogViewerMeta>('read_archive_log_viewer', { fileName, sessionId });

/** ネイティブダイアログでユーザーに外部フォルダを選択させる（キャンセル時はnull） */
export const pickLogFolder = () => invoke<string | null>('pick_log_folder');

/** 外部フォルダ内の output_log_*.txt / *.tar.zst を一覧取得する */
export const loadExternalLogFiles = (folderPath: string) =>
  invoke<ArchiveFileItem[]>('list_external_log_files', { folderPath });

/** 外部フォルダのログファイルに対するストリーミングログビューアセッションを開始する */
export const startExternalLogViewerStream = (
  folderPath: string,
  fileName: string,
  sessionId: string,
) =>
  invoke<LogViewerMeta>('read_external_log_viewer', {
    folderPath,
    fileName,
    sessionId,
  });

/** 起動時の一回限りのアーカイブ取り込みを実行し、件数サマリを返す */
export const launchStartupArchiveImport = () =>
  invoke<StartupImportSummary>('launch_startup_archive_import');
