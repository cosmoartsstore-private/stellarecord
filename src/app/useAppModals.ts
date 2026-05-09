import { useState } from 'react';
import { getDeletableSourceLogs, deleteSourceLogs } from '../features/analyze/services/analyzeService';
import type { DeletableLogInfo } from '../features/analyze/models/types';
import type { ArchiveFileItem } from '../features/archive/models/types';

/** モーダルコーディネーターが親Appから受け取るコールバックと共有状態 */
interface UseAppModalsOptions {
  addToast: (msg: string) => void;
  archiveFiles: ArchiveFileItem[];
  openEnhancedSync: () => Promise<ArchiveFileItem[]>;
  executeEnhancedSync: (targets: string[]) => Promise<void>;
  openLogViewerSelection: () => Promise<ArchiveFileItem[]>;
  openSelectedLogViewer: (fileName: string) => Promise<string>;
  closeLogViewer: () => void;
  setAnalyzeRunning: (v: boolean) => void;
  batchSelectedFiles: Set<string>;
  clearBatchSelection: () => void;
}

/**
 * 全トップレベルモーダルの開閉・確認ロジックを集約するフック
 *
 * App.tsxをレイアウトに専念させるため、モーダルのライフサイクル管理を
 * ここに委譲する。呼び出し側は modals.xxx のパターンで操作する
 */
export function useAppModals(options: UseAppModalsOptions) {
  const {
    addToast,
    openEnhancedSync,
    executeEnhancedSync,
    openLogViewerSelection,
    openSelectedLogViewer,
    closeLogViewer,
    setAnalyzeRunning,
    batchSelectedFiles,
    clearBatchSelection,
  } = options;

  const [isArchiveSelectorVisible, setIsArchiveSelectorVisible] = useState(false);
  const [isLogViewerModalVisible, setIsLogViewerModalVisible] = useState(false);
  const [isCleanupModalOpen, setIsCleanupModalOpen] = useState(false);
  const [deletableLogs, setDeletableLogs] = useState<DeletableLogInfo[]>([]);

  /** インポート可能なアーカイブを取得してバッチインポート選択モーダルを開く */
  const handleOpenEnhancedSync = async () => {
    try {
      const files = await openEnhancedSync();
      if (files.length === 0) {
        addToast('取り込み可能なアーカイブファイルが見つかりませんでした');
        return;
      }
      setIsArchiveSelectorVisible(true);
    } catch (error) {
      addToast('ファイル一覧取得失敗: ' + String(error));
    }
  };

  /** 選択をスナップショットしてモーダルを閉じ、バッチインポートを実行する */
  const handleConfirmImport = async () => {
    setIsArchiveSelectorVisible(false);
    const targets = [...batchSelectedFiles];
    clearBatchSelection();
    setAnalyzeRunning(true);
    try {
      await executeEnhancedSync(targets);
    } catch (error) {
      setAnalyzeRunning(false);
      addToast('強化同期エラー: ' + String(error));
    }
  };

  /** ログビューアモーダルを開き、最新のアーカイブファイルを自動選択する */
  const handleOpenLogViewer = async () => {
    try {
      const files = await openLogViewerSelection();
      if (files.length === 0) {
        addToast('アーカイブファイルが見つかりませんでした');
        return;
      }
      setIsLogViewerModalVisible(true);
      openSelectedLogViewer(files[0].name).catch((error: unknown) => {
        addToast('ログストリーム開始エラー: ' + String(error));
      });
    } catch (error) {
      addToast('ログ閲覧エラー: ' + String(error));
    }
  };

  /** モーダルを閉じずに別のアーカイブファイルに切り替える */
  const handleViewerNavigateToFile = (fileName: string) => {
    void openSelectedLogViewer(fileName);
  };

  /** 削除対象のソースログを取得してクリーンアップ確認モーダルを開く */
  const handleOpenCleanup = async () => {
    try {
      const logs = await getDeletableSourceLogs();
      if (logs.length === 0) {
        addToast('削除可能な元ログが見つかりませんでした');
        return;
      }
      setDeletableLogs(logs);
      setIsCleanupModalOpen(true);
    } catch (error) {
      addToast('元ログ一覧の取得に失敗しました: ' + String(error));
    }
  };

  /** 選択されたソースログファイルを削除し、削除件数を通知する */
  const handleConfirmCleanup = async (fileNames: string[]) => {
    setIsCleanupModalOpen(false);
    try {
      const count = await deleteSourceLogs(fileNames);
      addToast(`元ログ ${String(count)} 件を削除しました`);
    } catch (error) {
      addToast('削除に失敗しました: ' + String(error));
    }
  };

  /** アーカイブ選択モーダルを閉じてバッチ選択をクリアする */
  const closeArchiveSelector = () => {
    setIsArchiveSelectorVisible(false);
    clearBatchSelection();
  };

  /** ログビューアを閉じてストリームと選択状態を破棄する */
  const closeLogViewerModal = () => {
    setIsLogViewerModalVisible(false);
    closeLogViewer();
    clearBatchSelection();
  };

  /** クリーンアップモーダルを閉じる */
  const closeCleanupModal = () => {
    setIsCleanupModalOpen(false);
  };

  return {
    isArchiveSelectorVisible,
    isLogViewerModalVisible,
    isCleanupModalOpen,
    deletableLogs,
    handleOpenEnhancedSync,
    handleConfirmImport,
    handleOpenLogViewer,
    handleViewerNavigateToFile,
    handleOpenCleanup,
    handleConfirmCleanup,
    closeArchiveSelector,
    closeLogViewerModal,
    closeCleanupModal,
  };
}
