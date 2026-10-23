import { getMsg } from '../messages/getMsg';
import { useState } from 'react';
import { getDeletableSourceLogs, deleteSourceLogs } from '../features/analyze/services/analyzeService';
import type { SourceLogCandidates } from '../features/analyze/models/types';
import type { ArchiveFileItem } from '../features/archive/models/types';
import { addErrorToast, type AddToast } from '../shared/lib/errors';

/** モーダルコーディネーターが親Appから受け取るコールバックと共有状態 */
interface UseAppModalsOptions {
  addToast: AddToast;
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
  const { addToast, openEnhancedSync, executeEnhancedSync, openLogViewerSelection, openSelectedLogViewer, closeLogViewer, setAnalyzeRunning, batchSelectedFiles, clearBatchSelection, selectExternalLogFiles, clearExternalLogFiles, externalFiles } = options;

  const [isArchiveSelectorVisible, setIsArchiveSelectorVisible] = useState(false);
  const [isLogViewerModalVisible, setIsLogViewerModalVisible] = useState(false);
  const [isCleanupModalOpen, setIsCleanupModalOpen] = useState(false);
  const [cleanupCandidates, setCleanupCandidates] = useState<SourceLogCandidates | null>(null);

  /** インポート可能なアーカイブを取得してバッチインポート選択モーダルを開く */
  const handleOpenEnhancedSync = async () => {
    try {
      const files = await openEnhancedSync();
      if (files.length === 0) {
        addToast(getMsg('AppModals.importableArchiveMissing'));
        return;
      }
      setIsArchiveSelectorVisible(true);
    } catch (error) {
      addErrorToast(addToast, getMsg('Operations.loadArchiveList'), getMsg('AppModals.archiveListLoadFailed'), error);
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
      addErrorToast(addToast, getMsg('Operations.importSelectedArchives'), getMsg('AppModals.importStartFailed'), error);
    }
  };

  /** ログビューアモーダルを開き、最新のアーカイブファイルを自動選択する */
  const handleOpenLogViewer = async () => {
    try {
      const files = await openLogViewerSelection();
      setIsLogViewerModalVisible(true);
      const firstFile = files.at(0);
      if (!firstFile) return;
      openSelectedLogViewer(firstFile.name).catch((error: unknown) => {
        addErrorToast(addToast, getMsg('Operations.openLatestArchive'), getMsg('AppModals.logReadStartFailed'), error);
      });
    } catch (error) {
      // 管理ストアを読めなくても、外部ログを選択する操作は利用できるようにする。
      setIsLogViewerModalVisible(true);
      addErrorToast(addToast, getMsg('Operations.loadLogViewerList'), getMsg('AppModals.logViewerOpenFailed'), error);
    }
  };

  /** モーダルを閉じずに別のファイルに切り替える */
  const handleViewerNavigateToFile = (fileKey: string) => {
    openSelectedLogViewer(fileKey).catch((error: unknown) => {
      addErrorToast(addToast, getMsg('Operations.switchLogViewerFile'), getMsg('AppModals.logReadStartFailed'), error);
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
        addErrorToast(addToast, getMsg('Operations.openExternalLog'), getMsg('AppModals.logReadStartFailed'), error);
      });
    } catch (error) {
      addErrorToast(addToast, getMsg('Operations.selectExternalLog'), getMsg('AppModals.externalLogOpenFailed'), error);
    }
  };

  /** 外部ファイル選択を解除し、既定アーカイブストアの最新を自動で開く */
  const handleClearExternalFiles = async () => {
    try {
      const files = await clearExternalLogFiles();
      const firstFile = files.at(0);
      if (!firstFile) return;
      openSelectedLogViewer(firstFile.name).catch((error: unknown) => {
        addErrorToast(addToast, getMsg('Operations.openDefaultArchive'), getMsg('AppModals.logReadStartFailed'), error);
      });
    } catch (error) {
      addErrorToast(addToast, getMsg('Operations.switchDefaultArchiveFolder'), getMsg('AppModals.defaultFolderSwitchFailed'), error);
    }
  };

  /** 削除対象のソースログを取得してクリーンアップ確認モーダルを開く */
  const handleOpenCleanup = async () => {
    try {
      const candidates = await getDeletableSourceLogs();
      if (candidates.logs.length === 0) {
        addToast(getMsg('AppModals.deletableSourceLogMissing'));
        return;
      }
      setCleanupCandidates(candidates);
      setIsCleanupModalOpen(true);
    } catch (error) {
      addErrorToast(addToast, getMsg('Operations.loadSourceLogCandidates'), getMsg('AppModals.sourceLogListLoadFailed'), error);
    }
  };

  /** 選択されたソースログファイルを削除し、削除件数を通知する */
  const handleConfirmCleanup = async (fileNames: string[]) => {
    if (!cleanupCandidates) return;
    setIsCleanupModalOpen(false);
    try {
      const count = await deleteSourceLogs(cleanupCandidates.source_log_path, fileNames);
      addToast(getMsg('AppModals.sourceLogsDeleted', { count: String(count) }));
    } catch (error) {
      addErrorToast(addToast, getMsg('PolarisCleanup.title'), getMsg('AppModals.sourceLogsDeleteFailed'), error);
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
    deletableLogs: cleanupCandidates?.logs ?? [],
    cleanupSourceLogPath: cleanupCandidates?.source_log_path ?? '',
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
