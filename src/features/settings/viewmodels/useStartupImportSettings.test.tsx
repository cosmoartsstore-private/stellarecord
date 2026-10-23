// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { StartupImportSettings } from '../models/types';
import { changeSourceLogDirectory, loadStartupImportSettings, saveStartupImportPreference } from '../services/settingsService';
import { useStartupImportSettings } from './useStartupImportSettings';

vi.mock('../services/settingsService', () => ({ changeSourceLogDirectory: vi.fn(), loadStartupImportSettings: vi.fn(), saveStartupImportPreference: vi.fn() }));
const settings: StartupImportSettings = { enabled: false, preference_set: false, source_log_path: 'C:/Polaris/Data/archive', log_archive_path: 'C:/Data/logs', database_path: 'C:/Data/events.db' };

beforeEach(() => { vi.resetAllMocks(); vi.mocked(loadStartupImportSettings).mockResolvedValue(settings); vi.mocked(saveStartupImportPreference).mockResolvedValue(undefined); });
afterEach(cleanup);

describe('起動時取り込み設定', () => {
  it('取得中を表示し、取得完了後に許可状態と保存先を返す', async () => {
    const { result } = renderHook(() => useStartupImportSettings());
    expect(result.current.isStartupImportSettingsLoading).toBe(true);
    expect(result.current.startupImportSettings).toBeNull();
    await act(async () => { await Promise.resolve(); });
    expect(result.current.isStartupImportSettingsLoading).toBe(false);
    expect(result.current.startupImportSettings).toEqual(settings);
    expect(result.current.startupImportSettingsError).toBeNull();
  });

  it('取得失敗を保持してloadingを解除する', async () => {
    const failure = new Error('取得拒否');
    vi.mocked(loadStartupImportSettings).mockRejectedValueOnce(failure);
    const { result } = renderHook(() => useStartupImportSettings());
    await act(async () => { await Promise.resolve(); });
    expect(result.current.startupImportSettingsError).toBe(failure);
    expect(result.current.startupImportSettings).toBeNull();
    expect(result.current.isStartupImportSettingsLoading).toBe(false);
  });

  it.each([true, false])('回答の保存完了までsavingを表示し、選択と回答済み状態だけ更新する: %s', async (enabled) => {
    const { result } = renderHook(() => useStartupImportSettings());
    await act(async () => { await Promise.resolve(); });
    let finishSave: () => void = () => { throw new Error('保存前'); };
    vi.mocked(saveStartupImportPreference).mockReturnValueOnce(new Promise((resolve) => { finishSave = () => { resolve(undefined); }; }));
    let saving = Promise.resolve();
    act(() => { saving = result.current.saveStartupImportPreference(enabled); });
    expect(result.current.isStartupImportPreferenceSaving).toBe(true);
    expect(result.current.startupImportSettings).toEqual(settings);
    await act(async () => { finishSave(); await saving; });
    expect(saveStartupImportPreference).toHaveBeenCalledWith(enabled);
    expect(result.current.startupImportSettings).toEqual({ ...settings, enabled, preference_set: true });
    expect(result.current.isStartupImportPreferenceSaving).toBe(false);
  });

  it('保存失敗では許可状態を変更せず、savingを解除して元の例外を返す', async () => {
    const { result } = renderHook(() => useStartupImportSettings());
    await act(async () => { await Promise.resolve(); });
    const failure = new Error('保存拒否');
    vi.mocked(saveStartupImportPreference).mockRejectedValueOnce(failure);
    await act(async () => { await expect(result.current.saveStartupImportPreference(true)).rejects.toBe(failure); });
    expect(result.current.startupImportSettings).toEqual(settings);
    expect(result.current.isStartupImportPreferenceSaving).toBe(false);
  });

  it('設定が未取得でも保存は完了し、未取得状態のまま保持する', async () => {
    vi.mocked(loadStartupImportSettings).mockReturnValueOnce(new Promise(() => undefined));
    const { result } = renderHook(() => useStartupImportSettings());
    await act(async () => { await result.current.saveStartupImportPreference(true); });
    expect(result.current.startupImportSettings).toBeNull();
    expect(result.current.isStartupImportPreferenceSaving).toBe(false);
  });

  it('フォルダの選択・保存が終わるまで待ち、読込元だけを更新する', async () => {
    const { result } = renderHook(() => useStartupImportSettings());
    await act(async () => { await Promise.resolve(); });
    let finishChange: (path: string) => void = () => { throw new Error('選択前'); };
    vi.mocked(changeSourceLogDirectory).mockReturnValueOnce(new Promise((resolve) => { finishChange = resolve; }));
    let changing: Promise<string | null> = Promise.resolve(null);
    act(() => { changing = result.current.changeSourceLogDirectory(); });
    expect(result.current.isChangingSourceLogDirectory).toBe(true);
    expect(result.current.startupImportSettings).toEqual(settings);
    await act(async () => { finishChange('D:/日本語 ログ'); await changing; });
    expect(result.current.startupImportSettings).toEqual({ ...settings, source_log_path: 'D:/日本語 ログ' });
    expect(result.current.isChangingSourceLogDirectory).toBe(false);
    expect(saveStartupImportPreference).not.toHaveBeenCalled();
  });

  it.each(['cancel', 'reject'])('フォルダ変更の中止・失敗では元の設定を保持する: %s', async (outcome) => {
    const { result } = renderHook(() => useStartupImportSettings());
    await act(async () => { await Promise.resolve(); });
    const failure = new Error('保存拒否');
    if (outcome === 'cancel') vi.mocked(changeSourceLogDirectory).mockResolvedValueOnce(null);
    else vi.mocked(changeSourceLogDirectory).mockRejectedValueOnce(failure);
    await act(async () => {
      if (outcome === 'cancel') await expect(result.current.changeSourceLogDirectory()).resolves.toBeNull();
      else await expect(result.current.changeSourceLogDirectory()).rejects.toBe(failure);
    });
    expect(result.current.startupImportSettings).toEqual(settings);
    expect(result.current.isChangingSourceLogDirectory).toBe(false);
  });

  it.each(['resolve', 'reject'])('unmount後の取得結果でstateを変更しない: %s', async (outcome) => {
    let completeLoad: () => void = () => { throw new Error('取得前'); };
    vi.mocked(loadStartupImportSettings).mockReturnValueOnce(new Promise((resolve, reject) => {
      completeLoad = () => { if (outcome === 'resolve') resolve(settings); else reject(new Error('取得拒否')); };
    }));
    const { result, unmount } = renderHook(() => useStartupImportSettings());
    unmount();
    await act(async () => { completeLoad(); await Promise.resolve(); });
    expect(result.current.startupImportSettings).toBeNull();
    expect(result.current.startupImportSettingsError).toBeNull();
    expect(result.current.isStartupImportSettingsLoading).toBe(true);
  });
});
