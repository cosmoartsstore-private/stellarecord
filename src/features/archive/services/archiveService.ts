/**
 * アーカイブ操作のTauri IPCラッパー
 *
 * 各関数はRust側の `#[tauri::command]` と1対1対応。ストリーミングログビューアは
 * 2フェーズ構成: startLogViewerStream がメタ情報を同期的に返し、バックエンドが
 * log_viewer_chunk / log_viewer_done イベントを非同期に発行する
 */
import { invoke } from '@tauri-apps/api/core';
import type { LogViewerMeta } from '../models/types';

/** アーカイブディレクトリ内の .tar.zst ファイル一覧を取得する */
export const loadArchiveFiles = () => invoke<{ name: string; size_bytes: number }[]>('list_archive_files');

/** 選択されたアーカイブファイルのバッチインポートを開始する */
export const launchEnhancedImport = (fileNames: string[]) =>
  invoke('launch_enhanced_import', { fileNames });

/** ストリーミングログビューアセッションを開始する（チャンクはTauriイベント経由で到着） */
export const startLogViewerStream = (fileName: string, sessionId: string) =>
  invoke<LogViewerMeta>('read_archive_log_viewer', { fileName, sessionId });

/** ネイティブダイアログでユーザーにログファイルを複数選択させる（キャンセル時は空配列） */
export const pickLogFiles = () => invoke<string[]>('pick_log_files');

/** 外部ログファイルに対するストリーミングログビューアセッションを開始する */
export const startExternalLogViewerStream = (
  filePath: string,
  sessionId: string,
) =>
  invoke<LogViewerMeta>('read_external_log_viewer', {
    filePath,
    sessionId,
  });

/** 起動時の一回限りのアーカイブ取り込みを実行する（バックグラウンドでイベント経由に進捗を流す） */
export const launchStartupArchiveImport = () =>
  invoke('launch_startup_archive_import');
