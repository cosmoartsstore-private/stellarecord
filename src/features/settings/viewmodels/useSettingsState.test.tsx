// @vitest-environment jsdom

import { act, renderHook } from '@testing-library/react';
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
    vi.useRealTimers();
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
});
