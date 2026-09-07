// @vitest-environment jsdom

import { act, renderHook, waitFor } from '@testing-library/react';
import { listen } from '@tauri-apps/api/event';
import type { EventCallback, UnlistenFn } from '@tauri-apps/api/event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
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
  const promise = new Promise<T>((resolve) => {
    resolvePromise = resolve;
  });
  return { promise, resolve: resolvePromise };
}

const listenMock = vi.mocked(listen);
const launchEnhancedImportMock = vi.mocked(launchEnhancedImport);
const launchStartupArchiveImportMock = vi.mocked(launchStartupArchiveImport);
const loadArchiveFilesMock = vi.mocked(loadArchiveFiles);
const pickLogFilesMock = vi.mocked(pickLogFiles);
const startExternalLogViewerStreamMock = vi.mocked(startExternalLogViewerStream);
const startLogViewerStreamMock = vi.mocked(startLogViewerStream);

describe('useArchiveState', () => {
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
});
