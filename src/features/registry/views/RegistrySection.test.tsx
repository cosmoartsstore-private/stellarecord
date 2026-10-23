// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { LauncherViewMode } from '../models/types';
import { RegistrySection } from './RegistrySection';

afterEach(cleanup);
const app = { name: 'Alpha', description: '', path: 'C:/Alpha/Alpha.exe' };

function registryProps() {
  return { registryApps: { apps: [app] }, launcherViewMode: 'list' as LauncherViewMode, isReloading: false, onSetLauncherViewMode: vi.fn(), onLaunchApp: vi.fn(), onOpenFolder: vi.fn(), onUnregisterApp: vi.fn(), onRegisterApp: vi.fn(), onReload: vi.fn() };
}

describe('ランチャー操作', () => {
  it.each(['list', 'card'] as const)('対象アプリを起動・フォルダ表示・登録解除のcallbackへ渡す: %s', (view) => {
    const props = registryProps();
    render(<RegistrySection {...props} launcherViewMode={view} />);
    expect(screen.getByText('説明なし')).toBeTruthy();
    const item = within(screen.getByRole('article'));
    fireEvent.click(item.getByRole('button', { name: '起動' }));
    fireEvent.click(item.getByRole('button', { name: 'ファイルの場所を開く' }));
    fireEvent.click(item.getByRole('button', { name: 'Alpha の登録を解除' }));
    expect(props.onLaunchApp).toHaveBeenCalledWith(app);
    expect(props.onOpenFolder).toHaveBeenCalledWith(app);
    expect(props.onUnregisterApp).toHaveBeenCalledWith(app);
  });

  it('表示形式・登録・再読み込みを操作でき、再読み込み中は二重要求しない', () => {
    const props = registryProps();
    const { rerender } = render(<RegistrySection {...props} />);
    fireEvent.click(screen.getByRole('button', { name: 'カード表示' }));
    fireEvent.click(screen.getByRole('button', { name: 'リスト表示' }));
    expect(props.onSetLauncherViewMode.mock.calls).toEqual([['card'], ['list']]);
    fireEvent.click(screen.getByRole('button', { name: '登録' }));
    fireEvent.click(screen.getByRole('button', { name: '再読み込み' }));
    expect(props.onRegisterApp).toHaveBeenCalledOnce();
    expect(props.onReload).toHaveBeenCalledOnce();
    rerender(<RegistrySection {...props} isReloading />);
    fireEvent.click(screen.getByRole('button', { name: '再読み込み' }));
    expect(props.onReload).toHaveBeenCalledOnce();
  });

  it('未登録を表示し、登録済みアイコンと説明をそのまま表示する', () => {
    const props = registryProps();
    const { rerender } = render(<RegistrySection {...props} registryApps={{ apps: [] }} />);
    expect(screen.getByText('アプリはまだ登録されていません')).toBeTruthy();
    rerender(<RegistrySection {...props} registryApps={{ apps: [{ ...app, description: '画像管理', icon_data: 'YWJj' }] }} />);
    expect(screen.getByText('画像管理')).toBeTruthy();
    expect(screen.getByRole('article').querySelector('img')?.getAttribute('src')).toBe('data:image/png;base64,YWJj');
  });
});
