import { useCallback, useEffect, useState } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { StorageStatus } from '../models/types';
import {
  cancelAnalyze,
  loadStorageStatus,
  onAnalyzeFinished,
  onAnalyzeProgress,
} from '../services/analyzeService';
import { addErrorToast } from '../../../shared/lib/errors';

type AddToast = (msg: string) => void;

/** ストレージメーターの初期表示用ゼロ値 */
const emptyStorageStatus: StorageStatus = {
  current: 0,
  limit: 0,
  percent: 0,
};

/**
 * 解析の進捗管理・ストレージポーリング・キャンセル制御を統括するフック
 *
 * Tauriイベント購読を一元管理し、画面側は返却されたコールバックと状態を
 * 宣言的にバインドするだけで済むようにしている
 */
export function useAnalyzeState(addToast: AddToast) {
  const [isAnalyzeRunning, setAnalyzeRunning] = useState(false);
  const [analyzeProgress, setAnalyzeProgress] = useState('');
  const [analyzeStatus, setAnalyzeStatus] = useState('');
  const [storageStatus, setStorageStatus] = useState<StorageStatus>(emptyStorageStatus);

  /** バックエンドからアーカイブサイズを取得する（タイマーおよび同期完了時に呼ばれる） */
  const pollStorage = useCallback(async () => {
    try {
      const [current, limit] = await loadStorageStatus();
      const percent = limit > 0 ? Math.min(100, (current / limit) * 100) : 0;
      setStorageStatus({ current, limit, percent });
    } catch {
      // ポーリング失敗は一時的なため、直前の値を保持する
    }
  }, []);

  // Tauriプッシュイベントの購読とストレージポーリングタイマーの開始
  useEffect(() => {
    const initialLoadTimer = window.setTimeout(() => {
      void pollStorage();
    }, 0);
    const storageInterval = window.setInterval(pollStorage, 30000);
    const unlistenFns: Promise<UnlistenFn>[] = [];

    unlistenFns.push(
      onAnalyzeProgress((payload) => {
        setAnalyzeStatus(payload.status);
        if (payload.progress) setAnalyzeProgress(payload.progress);
        setAnalyzeRunning(payload.is_running);
        if (!payload.is_running) {
          void pollStorage();
        }
      }),
    );

    unlistenFns.push(
      onAnalyzeFinished(() => {
        setAnalyzeRunning(false);
        void pollStorage();
      }),
    );

    return () => {
      window.clearTimeout(initialLoadTimer);
      window.clearInterval(storageInterval);
      for (const unlisten of unlistenFns) {
        void unlisten.then((dispose) => {
          dispose();
        });
      }
    };
  }, [pollStorage]);

  /** 解析の中断を要求する（実際の停止タイミングはバックエンドが決定する） */
  const handleCancelSync = useCallback(async () => {
    try {
      await cancelAnalyze();
      addToast('解析の中断を要求しました');
    } catch (error) {
      addErrorToast(addToast, '解析停止要求', '停止要求を送信できませんでした', error);
    }
  }, [addToast]);

  return {
    analyzeRunning: isAnalyzeRunning,
    analyzeProgress,
    analyzeStatus,
    storageStatus,
    pollStorage,
    setAnalyzeRunning,
    handleCancelSync,
  };
}
