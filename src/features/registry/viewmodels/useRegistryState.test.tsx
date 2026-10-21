// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { loadRegistryCatalog } from '../services/registryService';
import { useRegistryState } from './useRegistryState';

vi.mock('../services/registryService', () => ({ loadRegistryCatalog: vi.fn() }));
const catalog = { apps: [{ name: 'Sample', description: '登録済みアプリ', path: 'C:/sample.exe' }] };

beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); vi.mocked(loadRegistryCatalog).mockResolvedValue(catalog); });
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe('registry一覧', () => {
  it('初回の空一覧から取得結果へ更新し、再取得失敗では前の一覧を保持する', async () => {
    const { result } = renderHook(() => useRegistryState());
    expect(result.current.registryApps).toEqual({ apps: [] });
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(result.current.registryApps).toEqual(catalog);
    vi.mocked(loadRegistryCatalog).mockRejectedValueOnce(new Error('取得拒否'));
    await act(async () => { await result.current.refreshRegistry(); });
    expect(result.current.registryApps).toEqual(catalog);
  });

  it('リロード表示は最低700ms継続し、取得完了も待つ', async () => {
    const { result } = renderHook(() => useRegistryState());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    let finishLoad: (value: typeof catalog) => void = () => { throw new Error('取得前'); };
    vi.mocked(loadRegistryCatalog).mockReturnValueOnce(new Promise((resolve) => { finishLoad = resolve; }));
    let reload = Promise.resolve();
    act(() => { reload = result.current.reloadRegistry(); });
    expect(result.current.isReloading).toBe(true);
    await act(async () => { await vi.advanceTimersByTimeAsync(700); });
    expect(result.current.isReloading).toBe(true);
    await act(async () => { finishLoad({ apps: [] }); await reload; });
    expect(result.current.isReloading).toBe(false);
    expect(result.current.registryApps).toEqual({ apps: [] });
  });

  it('取得失敗でも700ms後にリロード表示を解除する', async () => {
    const { result } = renderHook(() => useRegistryState());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    vi.mocked(loadRegistryCatalog).mockRejectedValueOnce(new Error('取得拒否'));
    let reload = Promise.resolve();
    act(() => { reload = result.current.reloadRegistry(); });
    await act(async () => { await vi.advanceTimersByTimeAsync(699); });
    expect(result.current.isReloading).toBe(true);
    await act(async () => { await vi.advanceTimersByTimeAsync(1); await reload; });
    expect(result.current.isReloading).toBe(false);
    expect(result.current.registryApps).toEqual(catalog);
  });

  it('初回取得前のunmountではcommandを送らない', () => {
    const { unmount } = renderHook(() => useRegistryState());
    unmount();
    vi.runAllTimers();
    expect(loadRegistryCatalog).not.toHaveBeenCalled();
  });

  it.each(['success', 'failure'] as const)('新しい再取得の%s後に古い再取得が返っても、最後に確認できた一覧を保持する', async (outcome) => {
    const { result } = renderHook(() => useRegistryState());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    let completeOld: (value: typeof catalog) => void = () => { throw new Error('以前の一覧取得前'); };
    vi.mocked(loadRegistryCatalog).mockReturnValueOnce(new Promise((resolve) => { completeOld = resolve; }));
    let oldRefresh = Promise.resolve();
    act(() => { oldRefresh = result.current.refreshRegistry(); });
    const current = { apps: [{ name: 'Current', description: '現在の登録', path: 'C:/current.exe' }] };
    if (outcome === 'success') vi.mocked(loadRegistryCatalog).mockResolvedValueOnce(current);
    else vi.mocked(loadRegistryCatalog).mockRejectedValueOnce(new Error('最新の取得拒否'));
    await act(async () => { await result.current.refreshRegistry(); });
    await act(async () => { completeOld({ apps: [] }); await oldRefresh; });
    expect(result.current.registryApps).toEqual(outcome === 'success' ? current : catalog);
  });
});
