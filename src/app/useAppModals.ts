import { useState } from 'react';
import {
  getDeletableSourceLogs,
  deleteSourceLogs,
} from '../features/analyze/services/analyzeService';
import type { DeletableLogInfo } from '../features/analyze/models/types';
import type { ArchiveFileItem } from '../features/archive/models/types';
import { addErrorToast } from '../shared/lib/errors';

/** モーダルコーディネーターが親Appから受け取るコールバックと共有状態 */
interface UseAppModalsOptions {
  addToast: (msg: string) => void;
  archiveFiles: ArchiveFileItem[];
  openEnhancedSync: () => Promise<ArchiveFileItem[]>;
  executeEnhancedSync: (targets: string[]) => Promise<void>;
  openLogViewerSelection: () => Promise<ArchiveFileItem[]>;
  openSelectedLogViewer: (fileKey: string) => Promise<string>;
  closeLogViewer: () => void;
  setAnalyzeRunning: (v: boolean) => void;
  batchSelectedFiles: Set<string>;
  clearBatchSelection: () => void;
  selectExternalLogFiles: () => Promise<string[] | null>;
  clearExternalLogFiles: () => Promise<ArchiveFileItem[]>;
  externalFiles: string[];
}

/**
 * 全上位モーダルの開閉・確認ロジックを集約するフック
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
    selectExternalLogFiles,
    clearExternalLogFiles,
    externalFiles,
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
      addErrorToast(addToast, 'アーカイブ一覧取得', 'アーカイブ一覧を取得できませんでした', error);
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
      addErrorToast(
        addToast,
        '選択アーカイブ取り込み開始',
        '取り込みを開始できませんでした',
        error,
      );
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
        addErrorToast(
          addToast,
          '最新アーカイブログ閲覧開始',
          'ログの読み込みを開始できませんでした',
          error,
        );
      });
    } catch (error) {
      addErrorToast(addToast, 'ログビューア一覧取得', 'ログビューアを開けませんでした', error);
    }
  };

  /** モーダルを閉じずに別のファイルに切り替える */
  const handleViewerNavigateToFile = (fileKey: string) => {
    openSelectedLogViewer(fileKey).catch((error: unknown) => {
      addErrorToast(
        addToast,
        'ログビューアファイル切替',
        'ログの読み込みを開始できませんでした',
        error,
      );
    });
  };

  /** 外部ログファイルを選択し、最初に選んだファイルをビューアで開く */
  const handleSelectExternalFiles = async () => {
    try {
      const files = await selectExternalLogFiles();
      if (files === null) return;
      // 新しく追加されたファイルのうち最初のものを開く
      const newFiles = files.filter((f) => !externalFiles.includes(f));
      const target = newFiles.length > 0 ? newFiles[0] : files[0];
      openSelectedLogViewer(target).catch((error: unknown) => {
        addErrorToast(addToast, '外部ログ閲覧開始', 'ログの読み込みを開始できませんでした', error);
      });
    } catch (error) {
      addErrorToast(addToast, '外部ログファイル選択', '外部ログファイルを開けませんでした', error);
    }
  };

  /** 外部ファイル選択を解除し、既定アーカイブストアの最新を自動で開く */
  const handleClearExternalFiles = async () => {
    try {
      const files = await clearExternalLogFiles();
      if (files.length === 0) {
        addToast('アーカイブファイルが見つかりませんでした');
        return;
      }
      openSelectedLogViewer(files[0].name).catch((error: unknown) => {
        addErrorToast(
          addToast,
          '既定アーカイブログ閲覧開始',
          'ログの読み込みを開始できませんでした',
          error,
        );
      });
    } catch (error) {
      addErrorToast(
        addToast,
        '既定アーカイブフォルダ切替',
        '既定フォルダへ切り替えられませんでした',
        error,
      );
    }
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
      addErrorToast(addToast, '元ログ削除候補取得', '元ログ一覧を取得できませんでした', error);
    }
  };

  /** 選択されたソースログファイルを削除し、削除件数を通知する */
  const handleConfirmCleanup = async (fileNames: string[]) => {
    setIsCleanupModalOpen(false);
    try {
      const count = await deleteSourceLogs(fileNames);
      addToast(`元ログ ${String(count)} 件を削除しました`);
    } catch (error) {
      addErrorToast(addToast, '元ログ削除', '元ログを削除できませんでした', error);
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
    handleSelectExternalFiles,
    handleClearExternalFiles,
    handleOpenCleanup,
    handleConfirmCleanup,
    closeArchiveSelector,
    closeLogViewerModal,
    closeCleanupModal,
  };
}
