// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ManagementSettings } from '../models/types';
import { loadManagementSettings, saveManagementSettings } from '../services/settingsService';
import { useSettingsState } from './useSettingsState';

vi.mock('../services/settingsService', () => ({
  loadManagementSettings: vi.fn(),
  saveManagementSettings: vi.fn(),
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

const loadManagementSettingsMock = vi.mocked(loadManagementSettings);
const saveManagementSettingsMock = vi.mocked(saveManagementSettings);

describe('useSettingsState', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.resetAllMocks();
    loadManagementSettingsMock.mockResolvedValue({
      startup_enabled: false,
      startup_preference_set: true,
      archive_limit_mb: 800,
    });
    saveManagementSettingsMock.mockResolvedValue(undefined);
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it.each(['0', '-1', '1.5', '', 'NaN', 'Infinity', '10485761'])('不正な容量を保存せず入力エラーを返す: %s', async (draft) => {
    const { result } = renderHook(() => useSettingsState());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    act(() => { result.current.setArchiveLimitDraft(draft); });
    await act(async () => { await expect(result.current.saveArchiveLimit()).rejects.toThrow('警告ラインは 1MB～10,485,760MB (10TB) の整数で指定してください'); });
    expect(saveManagementSettingsMock).not.toHaveBeenCalled();
    expect(result.current.archiveLimitDraft).toBe(draft);
  });

  it.each([1, 300, 10485760])('有効な容量と現在のstartup設定を一括保存する: %s', async (limit) => {
    const { result } = renderHook(() => useSettingsState());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    act(() => { result.current.setArchiveLimitDraft(String(limit)); });
    await act(async () => { await result.current.saveArchiveLimit(); });
    expect(saveManagementSettingsMock).toHaveBeenCalledWith(false, limit);
    expect(result.current.archiveLimitDraft).toBe(String(limit));
  });

  it('startup保存の失敗では表示を元へ戻し、次の操作で再度保存できる', async () => {
    const { result } = renderHook(() => useSettingsState());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    act(() => { result.current.setArchiveLimitDraft('1200'); });
    const failure = new Error('保存拒否');
    saveManagementSettingsMock.mockRejectedValueOnce(failure);
    await act(async () => { await expect(result.current.toggleStartup()).rejects.toBe(failure); });
    expect(result.current.isStartupEnabledDraft).toBe(false);
    expect(result.current.archiveLimitDraft).toBe('1200');
    await act(async () => { await result.current.toggleStartup(); });
    expect(saveManagementSettingsMock).toHaveBeenLastCalledWith(true, 1200);
    expect(result.current.isStartupEnabledDraft).toBe(true);
    await act(async () => { await result.current.toggleStartup(); });
    expect(saveManagementSettingsMock).toHaveBeenLastCalledWith(false, 1200);
  });

  it('初回取得失敗後のstartup操作では再取得し、既定容量を誤って保存しない', async () => {
    loadManagementSettingsMock.mockRejectedValueOnce(new Error('取得拒否'));
    const { result } = renderHook(() => useSettingsState());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(result.current.archiveLimitDraft).toBe('300');
    await act(async () => { await result.current.toggleStartup(); });
    expect(loadManagementSettingsMock).toHaveBeenCalledTimes(2);
    expect(saveManagementSettingsMock).toHaveBeenCalledWith(true, 800);
    expect(result.current.archiveLimitDraft).toBe('800');
  });

  it('再取得も失敗したstartup操作は元の表示へ戻し、unmountで初回タイマーを解除する', async () => {
    const failure = new Error('取得拒否');
    loadManagementSettingsMock.mockRejectedValue(failure);
    const { result, unmount } = renderHook(() => useSettingsState());
    await act(async () => { await expect(result.current.toggleStartup()).rejects.toBe(failure); });
    expect(result.current.isStartupEnabledDraft).toBe(false);
    expect(saveManagementSettingsMock).not.toHaveBeenCalled();
    unmount();
    await vi.advanceTimersByTimeAsync(0);
    expect(loadManagementSettingsMock).toHaveBeenCalledOnce();
  });

  it('容量の保存失敗では下書きを保持して元の例外を返す', async () => {
    const { result } = renderHook(() => useSettingsState());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    act(() => { result.current.setArchiveLimitDraft('900'); });
    const failure = new Error('保存拒否');
    saveManagementSettingsMock.mockRejectedValueOnce(failure);
    await act(async () => { await expect(result.current.saveArchiveLimit()).rejects.toBe(failure); });
    expect(result.current.archiveLimitDraft).toBe('900');
    expect(result.current.isStartupEnabledDraft).toBe(false);
  });

  it('初回読込前は既定容量を表示し、取得後は永続設定へ更新する', async () => {
    const settingsResponse = createDeferred<ManagementSettings>();
    loadManagementSettingsMock.mockReturnValueOnce(settingsResponse.promise);
    const { result } = renderHook(() => useSettingsState());
    expect(result.current.archiveLimitDraft).toBe('300');
    expect(result.current.isStartupEnabledDraft).toBe(false);
    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
      settingsResponse.resolve({ startup_enabled: true, startup_preference_set: true, archive_limit_mb: 800 });
    });
    expect(result.current.archiveLimitDraft).toBe('800');
    expect(result.current.isStartupEnabledDraft).toBe(true);
  });

  it('容量下書きが不正でもstartup切替時は最後に読み込んだ容量を保存する', async () => {
    const { result } = renderHook(() => useSettingsState());

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });
    expect(result.current.archiveLimitDraft).toBe('800');

    act(() => {
      result.current.setArchiveLimitDraft('');
    });
    await act(async () => {
      await result.current.toggleStartup();
    });

    expect(saveManagementSettingsMock).toHaveBeenCalledWith(true, 800);
    expect(result.current.isStartupEnabledDraft).toBe(true);
    expect(result.current.archiveLimitDraft).toBe('800');
  });

  it('初回読み込み前のstartup切替は永続設定の取得完了を待つ', async () => {
    const settingsResponse = createDeferred<ManagementSettings>();
    loadManagementSettingsMock.mockReturnValueOnce(settingsResponse.promise);
    const { result } = renderHook(() => useSettingsState());
    let togglePromise = Promise.resolve();

    act(() => {
      togglePromise = result.current.toggleStartup();
    });

    expect(loadManagementSettingsMock).toHaveBeenCalledOnce();
    expect(saveManagementSettingsMock).not.toHaveBeenCalled();

    await act(async () => {
      settingsResponse.resolve({
        startup_enabled: false,
        startup_preference_set: true,
        archive_limit_mb: 1600,
      });
      await togglePromise;
    });

    expect(saveManagementSettingsMock).toHaveBeenCalledWith(true, 1600);
    expect(result.current.archiveLimitDraft).toBe('1600');
  });

  it('初回取得前の容量保存は取得を待ち、有効な自動起動を変更しない', async () => {
    const loaded = createDeferred<ManagementSettings>();
    loadManagementSettingsMock.mockReturnValueOnce(loaded.promise);
    const { result } = renderHook(() => useSettingsState());
    act(() => { result.current.setArchiveLimitDraft('900'); });
    let saving = Promise.resolve();
    act(() => { saving = result.current.saveArchiveLimit(); });
    expect(loadManagementSettingsMock).toHaveBeenCalledOnce();
    expect(saveManagementSettingsMock).not.toHaveBeenCalled();
    await act(async () => {
      loaded.resolve({ startup_enabled: true, startup_preference_set: true, archive_limit_mb: 800 });
      await saving;
    });
    expect(saveManagementSettingsMock).toHaveBeenCalledExactlyOnceWith(true, 900);
    expect(result.current.isStartupEnabledDraft).toBe(true);
    expect(result.current.archiveLimitDraft).toBe('900');
  });

  it('初回取得を待った容量保存の失敗でも入力を保持し、同じ値で再試行できる', async () => {
    const loaded = createDeferred<ManagementSettings>();
    loadManagementSettingsMock.mockReturnValueOnce(loaded.promise);
    const failure = new Error('容量保存拒否');
    saveManagementSettingsMock.mockRejectedValueOnce(failure);
    const { result } = renderHook(() => useSettingsState());
    act(() => { result.current.setArchiveLimitDraft('900'); });
    let outcome = Promise.resolve<unknown>(undefined);
    act(() => { outcome = result.current.saveArchiveLimit().then(() => undefined, (error: unknown) => error); });
    await act(async () => {
      loaded.resolve({ startup_enabled: true, startup_preference_set: true, archive_limit_mb: 800 });
      expect(await outcome).toBe(failure);
    });
    expect(result.current.archiveLimitDraft).toBe('900');
    expect(result.current.isStartupEnabledDraft).toBe(true);
    await act(async () => { await result.current.saveArchiveLimit(); });
    expect(saveManagementSettingsMock).toHaveBeenLastCalledWith(true, 900);
  });

  it('初回取得の遅い応答で、入力済みの容量下書きを消さない', async () => {
    const loaded = createDeferred<ManagementSettings>();
    loadManagementSettingsMock.mockReturnValueOnce(loaded.promise);
    const { result } = renderHook(() => useSettingsState());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    act(() => { result.current.setArchiveLimitDraft('1234'); });
    await act(async () => {
      loaded.resolve({ startup_enabled: true, startup_preference_set: true, archive_limit_mb: 800 });
      await loaded.promise;
    });
    expect(result.current.archiveLimitDraft).toBe('1234');
    expect(result.current.isStartupEnabledDraft).toBe(true);
    expect(saveManagementSettingsMock).not.toHaveBeenCalled();
  });

  it('容量保存待ちに編集した新しい下書きを、前の保存完了で消さない', async () => {
    const saved = createDeferred<undefined>();
    const { result } = renderHook(() => useSettingsState());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    saveManagementSettingsMock.mockReturnValueOnce(saved.promise);
    act(() => { result.current.setArchiveLimitDraft('900'); });
    let saving = Promise.resolve();
    act(() => { saving = result.current.saveArchiveLimit(); });
    act(() => { result.current.setArchiveLimitDraft('1200'); });
    await act(async () => { saved.resolve(undefined); await saving; });
    expect(saveManagementSettingsMock).toHaveBeenCalledExactlyOnceWith(false, 900);
    expect(result.current.archiveLimitDraft).toBe('1200');
    await act(async () => { await result.current.saveArchiveLimit(); });
    expect(saveManagementSettingsMock).toHaveBeenLastCalledWith(false, 1200);
  });

  it('容量保存が初回取得を待つ間の再入力も、保存完了後に保持する', async () => {
    const loaded = createDeferred<ManagementSettings>();
    loadManagementSettingsMock.mockReturnValueOnce(loaded.promise);
    const { result } = renderHook(() => useSettingsState());
    act(() => { result.current.setArchiveLimitDraft('900'); });
    let saving = Promise.resolve();
    act(() => { saving = result.current.saveArchiveLimit(); });
    act(() => { result.current.setArchiveLimitDraft('1200'); });
    await act(async () => {
      loaded.resolve({ startup_enabled: true, startup_preference_set: true, archive_limit_mb: 800 });
      await saving;
    });
    expect(saveManagementSettingsMock).toHaveBeenCalledExactlyOnceWith(true, 900);
    expect(result.current.archiveLimitDraft).toBe('1200');
    expect(result.current.isStartupEnabledDraft).toBe(true);
  });
});
