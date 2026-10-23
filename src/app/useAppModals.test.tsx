// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { deleteSourceLogs, getDeletableSourceLogs } from '../features/analyze/services/analyzeService';
import { useAppModals } from './useAppModals';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../features/analyze/services/analyzeService', () => ({ deleteSourceLogs: vi.fn(), getDeletableSourceLogs: vi.fn() }));
const files = [{ name: 'latest.tar.zst', size_bytes: 100 }, { name: 'old.tar.zst', size_bytes: 50 }];

function renderModals() {
  const options = { addToast: vi.fn(), archiveFiles: files, openEnhancedSync: vi.fn().mockResolvedValue(files), executeEnhancedSync: vi.fn().mockResolvedValue(undefined), openLogViewerSelection: vi.fn().mockResolvedValue(files), openSelectedLogViewer: vi.fn().mockResolvedValue('session'), closeLogViewer: vi.fn(), setAnalyzeRunning: vi.fn(), batchSelectedFiles: new Set(['old.tar.zst']), clearBatchSelection: vi.fn(), selectExternalLogFiles: vi.fn().mockResolvedValue(['old.txt', 'new.txt']), clearExternalLogFiles: vi.fn().mockResolvedValue(files), externalFiles: ['old.txt'] };
  return { ...renderHook(() => useAppModals(options)), options };
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(invoke).mockResolvedValue(undefined);
  vi.mocked(getDeletableSourceLogs).mockResolvedValue({ source_log_path: 'D:/Logs', logs: [{ file_name: 'output_log_a.txt', size_bytes: 100 }] });
  vi.mocked(deleteSourceLogs).mockResolvedValue(1);
});
afterEach(cleanup);

describe('画面からモーダル操作への接続', () => {
  it('取り込み対象をスナップショットして閉じ、選択解除後も対象を保持する', async () => {
    const { result, options } = renderModals();
    await act(async () => { await result.current.handleOpenEnhancedSync(); });
    expect(result.current.isArchiveSelectorVisible).toBe(true);
    options.clearBatchSelection.mockImplementation(() => { options.batchSelectedFiles.clear(); });
    await act(async () => { await result.current.handleConfirmImport(); });
    expect(result.current.isArchiveSelectorVisible).toBe(false);
    expect(options.batchSelectedFiles.size).toBe(0);
    expect(options.executeEnhancedSync).toHaveBeenCalledWith(['old.tar.zst']);
    expect(options.setAnalyzeRunning).toHaveBeenCalledWith(true);
  });

  it('対象がない場合もビューアだけは開き、取り込みと削除は対象なしを通知する', async () => {
    const { result, options } = renderModals();
    options.openEnhancedSync.mockResolvedValueOnce([]);
    options.openLogViewerSelection.mockResolvedValueOnce([]);
    vi.mocked(getDeletableSourceLogs).mockResolvedValueOnce({ source_log_path: 'D:/Logs', logs: [] });
    await act(async () => { await result.current.handleOpenEnhancedSync(); await result.current.handleOpenLogViewer(); await result.current.handleOpenCleanup(); });
    expect([result.current.isArchiveSelectorVisible, result.current.isLogViewerModalVisible, result.current.isCleanupModalOpen]).toEqual([false, true, false]);
    expect(options.addToast.mock.calls).toEqual([['取り込めるアーカイブがありません'], ['削除できる元ログがありません']]);
    expect(options.openSelectedLogViewer).not.toHaveBeenCalled();
  });

  it('ログ一覧の先頭を開き、ファイル切替と閉じる操作を接続する', async () => {
    const { result, options } = renderModals();
    await act(async () => { await result.current.handleOpenLogViewer(); });
    expect(result.current.isLogViewerModalVisible).toBe(true);
    expect(options.openSelectedLogViewer).toHaveBeenCalledWith('latest.tar.zst');
    act(() => { result.current.handleViewerNavigateToFile('old.tar.zst'); });
    expect(options.openSelectedLogViewer).toHaveBeenLastCalledWith('old.tar.zst');
    act(() => { result.current.closeLogViewerModal(); });
    expect(result.current.isLogViewerModalVisible).toBe(false);
    expect(options.closeLogViewer).toHaveBeenCalledOnce();
    expect(options.clearBatchSelection).toHaveBeenCalledOnce();
  });

  it('外部選択のキャンセル、新しいファイル、既存ファイルだけの場合を区別する', async () => {
    const { result, options } = renderModals();
    options.selectExternalLogFiles.mockResolvedValueOnce(null);
    await act(async () => { await result.current.handleSelectExternalFiles(); });
    expect(options.openSelectedLogViewer).not.toHaveBeenCalled();
    await act(async () => { await result.current.handleSelectExternalFiles(); });
    expect(options.openSelectedLogViewer).toHaveBeenLastCalledWith('new.txt');
    options.selectExternalLogFiles.mockResolvedValueOnce(['old.txt']);
    await act(async () => { await result.current.handleSelectExternalFiles(); });
    expect(options.openSelectedLogViewer).toHaveBeenLastCalledWith('old.txt');
    await act(async () => { await result.current.handleClearExternalFiles(); });
    expect(options.openSelectedLogViewer).toHaveBeenLastCalledWith('latest.tar.zst');
    options.clearExternalLogFiles.mockResolvedValueOnce([]);
    options.openSelectedLogViewer.mockClear();
    await act(async () => { await result.current.handleClearExternalFiles(); });
    expect(options.openSelectedLogViewer).not.toHaveBeenCalled();
    expect(options.addToast).not.toHaveBeenCalled();
  });

  it('削除確認には候補を表示し、確認したファイルと削除件数だけを通知する', async () => {
    const { result, options } = renderModals();
    await act(async () => { await result.current.handleOpenCleanup(); });
    expect(result.current.isCleanupModalOpen).toBe(true);
    expect(result.current.deletableLogs).toEqual([{ file_name: 'output_log_a.txt', size_bytes: 100 }]);
    await act(async () => { await result.current.handleConfirmCleanup(['output_log_a.txt']); });
    expect(result.current.isCleanupModalOpen).toBe(false);
    expect(deleteSourceLogs).toHaveBeenCalledWith('D:/Logs', ['output_log_a.txt']);
    expect(options.addToast).toHaveBeenCalledWith('元ログ 1 件を削除しました');
    await act(async () => { await result.current.handleOpenCleanup(); });
    act(() => { result.current.closeCleanupModal(); });
    expect(result.current.isCleanupModalOpen).toBe(false);
    expect(deleteSourceLogs).toHaveBeenCalledOnce();
    await act(async () => { await result.current.handleOpenEnhancedSync(); });
    act(() => { result.current.closeArchiveSelector(); });
    expect(result.current.isArchiveSelectorVisible).toBe(false);
    expect(options.clearBatchSelection).toHaveBeenCalledOnce();
  });

  it('取得・実行・削除の失敗をそれぞれの通知へ変換し、取り込み中表示を解除する', async () => {
    const { result, options } = renderModals();
    const failure = new Error('native拒否');
    options.openEnhancedSync.mockRejectedValueOnce(failure);
    options.executeEnhancedSync.mockRejectedValueOnce(failure);
    options.openLogViewerSelection.mockRejectedValueOnce(failure);
    options.selectExternalLogFiles.mockRejectedValueOnce(failure);
    options.clearExternalLogFiles.mockRejectedValueOnce(failure);
    vi.mocked(getDeletableSourceLogs).mockRejectedValueOnce(failure);
    vi.mocked(deleteSourceLogs).mockRejectedValueOnce(failure);
    await act(async () => {
      await result.current.handleOpenEnhancedSync();
      await result.current.handleConfirmImport();
      await result.current.handleOpenLogViewer();
      await result.current.handleSelectExternalFiles();
      await result.current.handleClearExternalFiles();
      await result.current.handleOpenCleanup();
    });
    await act(async () => { await result.current.handleOpenCleanup(); });
    await act(async () => {
      await result.current.handleConfirmCleanup(['output_log_a.txt']);
    });
    expect(options.addToast.mock.calls).toEqual([['アーカイブ一覧を取得できませんでした'], ['取り込みを開始できませんでした'], ['ログビューアを開けませんでした'], ['外部ログファイルを開けませんでした'], ['既定フォルダへ切り替えられませんでした'], ['元ログの一覧を取得できませんでした'], ['元ログを削除できませんでした']]);
    expect(options.setAnalyzeRunning.mock.calls).toEqual([[true], [false]]);
    expect(result.current.isCleanupModalOpen).toBe(false);
    expect(result.current.isLogViewerModalVisible).toBe(true);
  });

  it('非同期の閲覧開始失敗を入口ごとの通知へ変換する', async () => {
    const { result, options } = renderModals();
    options.openSelectedLogViewer.mockRejectedValue(new Error('読込拒否'));
    await act(async () => {
      await result.current.handleOpenLogViewer();
      result.current.handleViewerNavigateToFile('old.tar.zst');
      await result.current.handleSelectExternalFiles();
      await result.current.handleClearExternalFiles();
    });
    expect(options.addToast.mock.calls).toEqual(Array.from({ length: 4 }, () => ['ログの読み込みを開始できませんでした']));
    expect(result.current.isLogViewerModalVisible).toBe(true);
  });
});
