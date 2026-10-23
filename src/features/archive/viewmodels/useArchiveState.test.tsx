// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { listen } from '@tauri-apps/api/event';
import type { EventCallback, UnlistenFn } from '@tauri-apps/api/event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LogViewerChunk, LogViewerMeta } from '../models/types';
import {
  launchEnhancedImport,
  launchStartupArchiveImport,
  loadArchiveFiles,
  pickLogFiles,
  startExternalLogViewerStream,
  startLogViewerStream,
} from '../services/archiveService';
import { useArchiveState } from './useArchiveState';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('../services/archiveService', () => ({
  launchEnhancedImport: vi.fn(),
  launchStartupArchiveImport: vi.fn(),
  loadArchiveFiles: vi.fn(),
  pickLogFiles: vi.fn(),
  startExternalLogViewerStream: vi.fn(),
  startLogViewerStream: vi.fn(),
}));

function createDeferred<T>() {
  let resolvePromise: (value: T) => void = () => {
    throw new Error('Promiseの初期化前にresolveされました');
  };
  let rejectPromise: (reason: unknown) => void = () => {
    throw new Error('Promiseの初期化前にrejectされました');
  };
  const promise = new Promise<T>((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;
  });
  return { promise, resolve: resolvePromise, reject: rejectPromise };
}

const listenMock = vi.mocked(listen);
const launchEnhancedImportMock = vi.mocked(launchEnhancedImport);
const launchStartupArchiveImportMock = vi.mocked(launchStartupArchiveImport);
const loadArchiveFilesMock = vi.mocked(loadArchiveFiles);
const pickLogFilesMock = vi.mocked(pickLogFiles);
const startExternalLogViewerStreamMock = vi.mocked(startExternalLogViewerStream);
const startLogViewerStreamMock = vi.mocked(startLogViewerStream);
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe('useArchiveState', () => {
  it('チャンクを100ms単位で一括反映し、完了とcloseで残timerと購読を解除する', async () => {
    vi.useFakeTimers();
    const { result } = renderHook(useArchiveState);
    await act(async () => { await result.current.openSelectedLogViewer('batch.log'); });
    const sessionId = startLogViewerStreamMock.mock.calls[0][1];
    const chunk = listenMock.mock.calls[0][1] as EventCallback<LogViewerChunk>;
    const done = listenMock.mock.calls[1][1] as EventCallback<string>;
    act(() => {
      for (const line of ['一行目', '二行目']) chunk({ event: 'log_viewer_chunk', id: 1, payload: { session_id: sessionId, timestamps: ['時刻'], levels: [1], categories: [2], raw_lines: [line], highlights: [null] } });
    });
    expect(result.current.logViewerData?.raw_lines).toEqual([]);
    act(() => { vi.advanceTimersByTime(99); });
    expect(result.current.logViewerData?.raw_lines).toEqual([]);
    act(() => { vi.advanceTimersByTime(1); });
    expect(result.current.logViewerData?.raw_lines).toEqual(['一行目', '二行目']);
    expect(result.current.logViewerData?.levels).toEqual([1, 1]);
    act(() => { done({ event: 'log_viewer_done', id: 2, payload: sessionId }); });
    expect(result.current.isLogViewerLoaded).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
    act(() => { result.current.closeLogViewer(); });
    expect(result.current.logViewerData).toBeNull();
  });
  beforeEach(() => {
    vi.resetAllMocks();
    listenMock.mockResolvedValue(vi.fn());
    launchEnhancedImportMock.mockResolvedValue(undefined);
    launchStartupArchiveImportMock.mockResolvedValue(undefined);
    loadArchiveFilesMock.mockResolvedValue([]);
    pickLogFilesMock.mockResolvedValue([]);
    startExternalLogViewerStreamMock.mockImplementation((fileKey, sessionId) =>
      Promise.resolve({
        session_id: sessionId,
        archive_name: fileKey,
        source_name: `source:${fileKey}`,
      }),
    );
    startLogViewerStreamMock.mockImplementation((fileKey, sessionId) =>
      Promise.resolve({
        session_id: sessionId,
        archive_name: fileKey,
        source_name: `source:${fileKey}`,
      }),
    );
  });

  it('listenの解決前に閉じても、解決後に購読を解除して開始コマンドを送らない', async () => {
    const pendingListen = createDeferred<UnlistenFn>();
    const unlistenChunk = vi.fn();
    listenMock.mockReturnValueOnce(pendingListen.promise);
    const { result } = renderHook(() => useArchiveState());
    let openPromise = Promise.resolve('');

    act(() => {
      openPromise = result.current.openSelectedLogViewer('old.log');
    });
    expect(listenMock).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.closeLogViewer();
    });
    expect(result.current.logViewerData).toBeNull();

    await act(async () => {
      pendingListen.resolve(unlistenChunk);
      await openPromise;
    });

    expect(unlistenChunk).toHaveBeenCalledOnce();
    expect(listenMock).toHaveBeenCalledTimes(1);
    expect(startLogViewerStreamMock).not.toHaveBeenCalled();
    expect(result.current.logViewerData).toBeNull();
  });

  it('done側listenが失敗した場合は、先に登録済みのchunk購読を解除する', async () => {
    const unlistenChunk = vi.fn();
    listenMock
      .mockResolvedValueOnce(unlistenChunk)
      .mockRejectedValueOnce(new Error('done listen failed'));
    const { result } = renderHook(() => useArchiveState());

    await act(async () => {
      await expect(result.current.openSelectedLogViewer('archive.log')).rejects.toThrow(
        'done listen failed',
      );
    });

    expect(unlistenChunk).toHaveBeenCalledOnce();
    expect(startLogViewerStreamMock).not.toHaveBeenCalled();
  });

  it('chunk側listenが失敗した場合は、開始コマンドを送らずloadingを解除する', async () => {
    listenMock.mockRejectedValueOnce(new Error('chunk listen failed'));
    const { result } = renderHook(() => useArchiveState());

    await act(async () => {
      await expect(result.current.openSelectedLogViewer('archive.log')).rejects.toThrow(
        'chunk listen failed',
      );
    });

    expect(startLogViewerStreamMock).not.toHaveBeenCalled();
    expect(result.current.logViewerData).toBeNull();
    expect(result.current.isLogViewerLoading).toBe(false);
  });

  it('開始コマンドが失敗した場合は、両方のイベント購読を解除する', async () => {
    const unlistenChunk = vi.fn();
    const unlistenDone = vi.fn();
    listenMock.mockResolvedValueOnce(unlistenChunk).mockResolvedValueOnce(unlistenDone);
    startLogViewerStreamMock.mockRejectedValueOnce(new Error('start failed'));
    const { result } = renderHook(() => useArchiveState());

    await act(async () => {
      await expect(result.current.openSelectedLogViewer('archive.log')).rejects.toThrow(
        'start failed',
      );
    });

    expect(unlistenChunk).toHaveBeenCalledOnce();
    expect(unlistenDone).toHaveBeenCalledOnce();
    expect(result.current.logViewerData).toBeNull();
    expect(result.current.isLogViewerLoading).toBe(false);
  });

  it('開始コマンド待機中に切り替えた場合は、旧購読と旧世代の状態更新を破棄する', async () => {
    const oldStart = createDeferred<LogViewerMeta>();
    const unlistenOldChunk = vi.fn();
    const unlistenOldDone = vi.fn();
    const unlistenNewChunk = vi.fn();
    const unlistenNewDone = vi.fn();
    listenMock
      .mockResolvedValueOnce(unlistenOldChunk)
      .mockResolvedValueOnce(unlistenOldDone)
      .mockResolvedValueOnce(unlistenNewChunk)
      .mockResolvedValueOnce(unlistenNewDone);
    startLogViewerStreamMock.mockReturnValueOnce(oldStart.promise);
    const { result } = renderHook(() => useArchiveState());
    let oldOpenPromise = Promise.resolve('');

    act(() => {
      oldOpenPromise = result.current.openSelectedLogViewer('old.log');
    });
    await waitFor(() => {
      expect(startLogViewerStreamMock).toHaveBeenCalledTimes(1);
    });

    const oldSessionId = startLogViewerStreamMock.mock.calls[0][1];
    const oldChunkHandler = listenMock.mock.calls[0][1] as EventCallback<LogViewerChunk>;
    const oldDoneHandler = listenMock.mock.calls[1][1] as EventCallback<string>;

    await act(async () => {
      await result.current.openSelectedLogViewer('new.log');
    });

    expect(unlistenOldChunk).toHaveBeenCalledOnce();
    expect(unlistenOldDone).toHaveBeenCalledOnce();
    expect(result.current.logViewerData).toMatchObject({
      archive_name: 'new.log',
      source_name: 'source:new.log',
      raw_lines: [],
    });

    await act(async () => {
      oldStart.resolve({
        session_id: oldSessionId,
        archive_name: 'old.log',
        source_name: 'source:old.log',
      });
      await oldOpenPromise;
      oldChunkHandler({
        event: 'log_viewer_chunk',
        id: 1,
        payload: {
          session_id: oldSessionId,
          timestamps: ['00:00:00'],
          levels: [1],
          categories: [0],
          raw_lines: ['old line'],
          highlights: [null],
        },
      });
      oldDoneHandler({
        event: 'log_viewer_done',
        id: 2,
        payload: oldSessionId,
      });
    });

    expect(result.current.logViewerData).toMatchObject({
      archive_name: 'new.log',
      source_name: 'source:new.log',
      raw_lines: [],
    });
    expect(result.current.isLogViewerLoaded).toBe(false);
  });

  it('未反映chunkをファイル切替で破棄し、旧eventと別sessionを無視して現sessionだけを完了する', async () => {
    vi.useFakeTimers();
    const unlisteners = [vi.fn(), vi.fn(), vi.fn(), vi.fn()];
    for (const unlisten of unlisteners) listenMock.mockResolvedValueOnce(unlisten);
    const { result } = renderHook(useArchiveState);
    await act(async () => { await result.current.openSelectedLogViewer('old.log'); });
    const oldSession = startLogViewerStreamMock.mock.calls[0][1];
    const oldChunk = listenMock.mock.calls[0][1] as EventCallback<LogViewerChunk>;
    const oldDone = listenMock.mock.calls[1][1] as EventCallback<string>;
    const payload = (session: string, line: string): LogViewerChunk => ({ session_id: session, timestamps: ['時刻'], levels: [2], categories: [1], raw_lines: [line], highlights: [{ start: 0, end: 2 }] });
    act(() => { oldChunk({ event: 'log_viewer_chunk', id: 1, payload: payload(oldSession, '旧未反映') }); });
    expect(vi.getTimerCount()).toBe(1);

    await act(async () => { await result.current.openSelectedLogViewer('current.log'); });
    const currentSession = startLogViewerStreamMock.mock.calls[1][1];
    const currentChunk = listenMock.mock.calls[2][1] as EventCallback<LogViewerChunk>;
    const currentDone = listenMock.mock.calls[3][1] as EventCallback<string>;
    expect(currentSession).not.toBe(oldSession);
    expect(vi.getTimerCount()).toBe(0);
    expect(unlisteners[0]).toHaveBeenCalledOnce();
    expect(unlisteners[1]).toHaveBeenCalledOnce();
    act(() => {
      oldChunk({ event: 'log_viewer_chunk', id: 2, payload: payload(oldSession, '旧遅延') });
      oldDone({ event: 'log_viewer_done', id: 3, payload: oldSession });
      currentChunk({ event: 'log_viewer_chunk', id: 4, payload: payload(oldSession, '別session') });
      currentDone({ event: 'log_viewer_done', id: 5, payload: oldSession });
      vi.advanceTimersByTime(100);
    });
    expect(result.current.logViewerData?.raw_lines).toEqual([]);
    expect(result.current.isLogViewerLoaded).toBe(false);
    expect(unlisteners[2]).not.toHaveBeenCalled();
    expect(unlisteners[3]).not.toHaveBeenCalled();

    act(() => {
      currentChunk({ event: 'log_viewer_chunk', id: 6, payload: payload(currentSession, '現在一行目') });
      currentChunk({ event: 'log_viewer_chunk', id: 7, payload: payload(currentSession, '現在二行目') });
      currentDone({ event: 'log_viewer_done', id: 8, payload: currentSession });
    });
    expect(result.current.logViewerData).toEqual({ archive_name: 'current.log', source_name: 'source:current.log', timestamps: ['時刻', '時刻'], levels: [2, 2], categories: [1, 1], raw_lines: ['現在一行目', '現在二行目'], highlights: [{ start: 0, end: 2 }, { start: 0, end: 2 }] });
    expect(result.current.isLogViewerLoaded).toBe(true);
    expect(result.current.isLogViewerLoading).toBe(false);
    expect(vi.getTimerCount()).toBe(0);
    act(() => {
      currentChunk({ event: 'log_viewer_chunk', id: 9, payload: payload(currentSession, '完了後') });
      currentDone({ event: 'log_viewer_done', id: 10, payload: currentSession });
    });
    expect(result.current.logViewerData?.raw_lines).toEqual(['現在一行目', '現在二行目']);
    expect(result.current.isLogViewerLoaded).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
    act(() => { result.current.closeLogViewer(); });
    for (const unlisten of unlisteners) expect(unlisten).toHaveBeenCalledOnce();
    expect(vi.getTimerCount()).toBe(0);
    expect(result.current.logViewerData).toBeNull();
  });

  it('closeとunmountで未反映chunkのtimerを解除し、遅延callbackで状態を再生成しない', async () => {
    vi.useFakeTimers();
    const unlisteners = [vi.fn(), vi.fn(), vi.fn(), vi.fn()];
    for (const unlisten of unlisteners) listenMock.mockResolvedValueOnce(unlisten);
    const { result, unmount } = renderHook(useArchiveState);
    await act(async () => { await result.current.openSelectedLogViewer('same.log'); });
    const oldSession = startLogViewerStreamMock.mock.calls[0][1];
    const oldChunk = listenMock.mock.calls[0][1] as EventCallback<LogViewerChunk>;
    const oldDone = listenMock.mock.calls[1][1] as EventCallback<string>;
    const payload = (session: string, line: string): LogViewerChunk => ({ session_id: session, timestamps: [''], levels: [0], categories: [0], raw_lines: [line], highlights: [null] });
    act(() => {
      oldChunk({ event: 'log_viewer_chunk', id: 1, payload: payload(oldSession, '閉じる前') });
      result.current.closeLogViewer();
      oldDone({ event: 'log_viewer_done', id: 2, payload: oldSession });
      oldChunk({ event: 'log_viewer_chunk', id: 3, payload: payload(oldSession, '閉じた後') });
      vi.advanceTimersByTime(100);
    });
    expect(vi.getTimerCount()).toBe(0);
    expect(result.current.logViewerData).toBeNull();
    expect(result.current.isLogViewerLoaded).toBe(false);
    await act(async () => { await result.current.openSelectedLogViewer('same.log'); });
    const currentSession = startLogViewerStreamMock.mock.calls[1][1];
    expect(currentSession).not.toBe(oldSession);
    const currentChunk = listenMock.mock.calls[2][1] as EventCallback<LogViewerChunk>;
    const currentDone = listenMock.mock.calls[3][1] as EventCallback<string>;
    act(() => {
      oldChunk({ event: 'log_viewer_chunk', id: 4, payload: payload(oldSession, '再開後の旧行') });
      currentChunk({ event: 'log_viewer_chunk', id: 5, payload: payload(currentSession, '解除予定') });
    });
    expect(result.current.logViewerData?.raw_lines).toEqual([]);
    expect(vi.getTimerCount()).toBe(1);
    unmount();
    act(() => {
      currentChunk({ event: 'log_viewer_chunk', id: 6, payload: payload(currentSession, 'unmount後') });
      currentDone({ event: 'log_viewer_done', id: 7, payload: currentSession });
      vi.advanceTimersByTime(100);
    });
    expect(vi.getTimerCount()).toBe(0);
    for (const unlisten of unlisteners) expect(unlisten).toHaveBeenCalledOnce();
    expect(startLogViewerStreamMock.mock.calls.map(([file]) => file)).toEqual(['same.log', 'same.log']);
  });

  it('done購読が遅延した同一fileの多重開始で、新しい要求だけを送信し古い購読を解決直後に解除する', async () => {
    const pendingDone = createDeferred<UnlistenFn>();
    const unlisteners = [vi.fn(), vi.fn(), vi.fn(), vi.fn()];
    listenMock.mockResolvedValueOnce(unlisteners[0]).mockReturnValueOnce(pendingDone.promise).mockResolvedValueOnce(unlisteners[2]).mockResolvedValueOnce(unlisteners[3]);
    const { result } = renderHook(useArchiveState);
    let oldOpen = Promise.resolve('');
    act(() => { oldOpen = result.current.openSelectedLogViewer('same.log'); });
    await waitFor(() => { expect(listenMock).toHaveBeenCalledTimes(2); });
    expect(startLogViewerStreamMock).not.toHaveBeenCalled();
    await act(async () => { await result.current.openSelectedLogViewer('same.log'); });
    expect(startLogViewerStreamMock).toHaveBeenCalledOnce();
    expect(unlisteners[0]).toHaveBeenCalledOnce();
    expect(unlisteners[1]).not.toHaveBeenCalled();
    await act(async () => { pendingDone.resolve(unlisteners[1]); await oldOpen; });
    expect(unlisteners[1]).toHaveBeenCalledOnce();
    expect(startLogViewerStreamMock).toHaveBeenCalledOnce();
    expect(result.current.logViewerData).toMatchObject({ archive_name: 'same.log', source_name: 'source:same.log', raw_lines: [] });
    expect(result.current.isLogViewerLoaded).toBe(false);
    act(() => { result.current.closeLogViewer(); });
    for (const unlisten of unlisteners) expect(unlisten).toHaveBeenCalledOnce();
  });

  it('開始要求の拒否でbufferとtimerを破棄し、同一fileへの再試行を旧失敗や旧eventが変更しない', async () => {
    vi.useFakeTimers();
    const pendingStart = createDeferred<LogViewerMeta>();
    startLogViewerStreamMock.mockReturnValueOnce(pendingStart.promise);
    const unlisteners = [vi.fn(), vi.fn(), vi.fn(), vi.fn()];
    for (const unlisten of unlisteners) listenMock.mockResolvedValueOnce(unlisten);
    const { result } = renderHook(useArchiveState);
    let rejection = Promise.resolve();
    await act(async () => {
      const open = result.current.openSelectedLogViewer('same.log');
      rejection = expect(open).rejects.toThrow('開始要求を拒否');
      await Promise.resolve();
      await Promise.resolve();
    });
    const oldSession = startLogViewerStreamMock.mock.calls[0][1];
    const oldChunk = listenMock.mock.calls[0][1] as EventCallback<LogViewerChunk>;
    const oldDone = listenMock.mock.calls[1][1] as EventCallback<string>;
    const payload = (session: string, line: string): LogViewerChunk => ({ session_id: session, timestamps: [''], levels: [0], categories: [0], raw_lines: [line], highlights: [null] });
    act(() => { oldChunk({ event: 'log_viewer_chunk', id: 1, payload: payload(oldSession, '破棄対象') }); });
    expect(vi.getTimerCount()).toBe(1);
    await act(async () => { pendingStart.reject(new Error('開始要求を拒否')); await rejection; });
    expect(vi.getTimerCount()).toBe(0);
    expect(result.current.logViewerData).toBeNull();
    expect(result.current.isLogViewerLoading).toBe(false);
    expect(unlisteners[0]).toHaveBeenCalledOnce();
    expect(unlisteners[1]).toHaveBeenCalledOnce();
    await act(async () => { await result.current.openSelectedLogViewer('same.log'); });
    const currentSession = startLogViewerStreamMock.mock.calls[1][1];
    const currentChunk = listenMock.mock.calls[2][1] as EventCallback<LogViewerChunk>;
    const currentDone = listenMock.mock.calls[3][1] as EventCallback<string>;
    act(() => {
      oldChunk({ event: 'log_viewer_chunk', id: 2, payload: payload(oldSession, '旧失敗の遅延行') });
      oldDone({ event: 'log_viewer_done', id: 3, payload: oldSession });
      currentChunk({ event: 'log_viewer_chunk', id: 4, payload: payload(currentSession, '再試行の行') });
      currentDone({ event: 'log_viewer_done', id: 5, payload: currentSession });
      vi.advanceTimersByTime(100);
    });
    expect(result.current.logViewerData).toEqual({ archive_name: 'same.log', source_name: 'source:same.log', timestamps: [''], levels: [0], categories: [0], raw_lines: ['再試行の行'], highlights: [null] });
    expect(result.current.isLogViewerLoaded).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
    for (const unlisten of unlisteners) expect(unlisten).toHaveBeenCalledOnce();
  });
});
