// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AnalyzeProgressEvent } from '../models/types';
import { cancelAnalyze, loadStorageStatus, onAnalyzeFinished, onAnalyzeProgress } from '../services/analyzeService';
import { useAnalyzeState } from './useAnalyzeState';

vi.mock('../services/analyzeService', () => ({ cancelAnalyze: vi.fn(), loadStorageStatus: vi.fn(), onAnalyzeFinished: vi.fn(), onAnalyzeProgress: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));

let progressHandler: (payload: AnalyzeProgressEvent) => void;
let finishedHandler: () => void;
const disposeProgress = vi.fn();
const disposeFinished = vi.fn();

beforeEach(() => {
  vi.useFakeTimers();
  vi.resetAllMocks();
  vi.mocked(invoke).mockResolvedValue(undefined);
  vi.mocked(loadStorageStatus).mockResolvedValue([50, 200]);
  vi.mocked(cancelAnalyze).mockResolvedValue(undefined);
  vi.mocked(onAnalyzeProgress).mockImplementation((handler) => { progressHandler = handler; return Promise.resolve(disposeProgress); });
  vi.mocked(onAnalyzeFinished).mockImplementation((handler) => { finishedHandler = handler; return Promise.resolve(disposeFinished); });
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe('解析と容量表示', () => {
  it('初期値・初回取得・30秒ポーリングと上限超過の表示を確認する', async () => {
    const { result } = renderHook(() => useAnalyzeState(vi.fn()));
    expect(result.current.storageStatus).toEqual({ current: 0, limit: 0, percent: 0 });
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(result.current.storageStatus).toEqual({ current: 50, limit: 200, percent: 25 });
    vi.mocked(loadStorageStatus).mockResolvedValueOnce([300, 200]);
    await act(async () => { await vi.advanceTimersByTimeAsync(30000); });
    expect(result.current.storageStatus).toEqual({ current: 300, limit: 200, percent: 100 });
    expect(loadStorageStatus).toHaveBeenCalledTimes(2);
  });

  it('容量上限0は0%とし、取得失敗では最後の表示を保持する', async () => {
    const { result } = renderHook(() => useAnalyzeState(vi.fn()));
    vi.mocked(loadStorageStatus).mockResolvedValueOnce([75, 0]);
    await act(async () => { await result.current.pollStorage(); });
    expect(result.current.storageStatus).toEqual({ current: 75, limit: 0, percent: 0 });
    vi.mocked(loadStorageStatus).mockRejectedValueOnce(new Error('取得拒否'));
    await act(async () => { await result.current.pollStorage(); });
    expect(result.current.storageStatus).toEqual({ current: 75, limit: 0, percent: 0 });
  });

  it.each(['success', 'failure'] as const)('新しい容量取得の%s後に古いpollが返っても、最後の有効値を上書きしない', async (outcome) => {
    const { result } = renderHook(() => useAnalyzeState(vi.fn()));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(result.current.storageStatus).toEqual({ current: 50, limit: 200, percent: 25 });
    let finishOld: ((value: [number, number]) => void) | undefined;
    const oldResponse = new Promise<[number, number]>((resolve) => { finishOld = resolve; });
    vi.mocked(loadStorageStatus).mockReturnValueOnce(oldResponse);
    let oldPoll: Promise<void> | undefined;
    act(() => { oldPoll = result.current.pollStorage(); });
    if (outcome === 'success') vi.mocked(loadStorageStatus).mockResolvedValueOnce([300, 600]);
    else vi.mocked(loadStorageStatus).mockRejectedValueOnce(new Error('新しい要求の取得拒否'));
    await act(async () => { await result.current.pollStorage(); });
    const expected = outcome === 'success' ? { current: 300, limit: 600, percent: 50 } : { current: 50, limit: 200, percent: 25 };
    expect(result.current.storageStatus).toEqual(expected);
    await act(async () => { finishOld?.([10, 100]); await oldPoll; });
    expect(result.current.storageStatus).toEqual(expected);
  });

  it('進捗・空の進捗・停止イベントと完了イベントを区別する', async () => {
    const { result } = renderHook(() => useAnalyzeState(vi.fn()));
    act(() => { progressHandler({ status: '解析中', progress: '10/20', is_running: true }); });
    expect([result.current.analyzeRunning, result.current.analyzeStatus, result.current.analyzeProgress]).toEqual([true, '解析中', '10/20']);
    act(() => { progressHandler({ status: '次工程', progress: '', is_running: true }); });
    expect(result.current.analyzeProgress).toBe('10/20');
    await act(async () => { progressHandler({ status: '停止', progress: '', is_running: false }); await Promise.resolve(); });
    expect(result.current.analyzeRunning).toBe(false);
    expect(loadStorageStatus).toHaveBeenCalledTimes(1);
    act(() => { result.current.setAnalyzeRunning(true); });
    await act(async () => { finishedHandler(); await Promise.resolve(); });
    expect(result.current.analyzeRunning).toBe(false);
    expect(loadStorageStatus).toHaveBeenCalledTimes(2);
  });

  it('中断要求の成功と失敗を通知し、要求だけでは実行状態を解除しない', async () => {
    const addToast = vi.fn();
    const { result } = renderHook(() => useAnalyzeState(addToast));
    act(() => { result.current.setAnalyzeRunning(true); });
    await act(async () => { await result.current.handleCancelSync(); });
    expect(addToast).toHaveBeenLastCalledWith('取り込みの停止を要求しました');
    expect(result.current.analyzeRunning).toBe(true);
    vi.mocked(cancelAnalyze).mockRejectedValueOnce(new Error('送信拒否'));
    await act(async () => { await result.current.handleCancelSync(); });
    expect(addToast).toHaveBeenLastCalledWith('停止要求を送信できませんでした');
    expect(result.current.analyzeRunning).toBe(true);
  });

  it('unmountで初回タイマー・interval・イベントの購読をすべて解除する', async () => {
    const { unmount } = renderHook(() => useAnalyzeState(vi.fn()));
    await act(async () => { unmount(); await Promise.resolve(); });
    expect(disposeProgress).toHaveBeenCalledOnce();
    expect(disposeFinished).toHaveBeenCalledOnce();
    expect(vi.getTimerCount()).toBe(0);
    await vi.advanceTimersByTimeAsync(60000);
    expect(loadStorageStatus).not.toHaveBeenCalled();
  });
});
