import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AnalyzeProgressEvent } from '../models/types';

/** 現在のアーカイブ使用量と設定上限を取得する */
export const loadStorageStatus = () => invoke<[number, number]>('get_storage_status');

/** 実行中の解析フローにキャンセルを要求する */
export const cancelAnalyze = () => invoke('cancel_analyze');

/** 解析進捗イベントのストリームを購読する */
export const onAnalyzeProgress = (handler: (payload: AnalyzeProgressEvent) => void) =>
  listen<AnalyzeProgressEvent>('analyze-progress', (event) => {
    handler(event.payload);
  });

/** 解析完了イベントを購読する */
export const onAnalyzeFinished = (handler: () => void) => listen('analyze-finished', handler);

/** アーカイブ済みで削除可能なソースログファイル一覧を取得する */
export const getDeletableSourceLogs = () =>
  invoke<import('../models/types').DeletableLogInfo[]>('get_deletable_source_logs');

/** 指定されたソースログファイルを削除し、削除件数を返す */
export const deleteSourceLogs = (fileNames: string[]) =>
  invoke<number>('delete_source_logs', { fileNames });
