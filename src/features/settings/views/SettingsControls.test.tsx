// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsControls } from './SettingsControls';
import { DataLocationList } from './DataLocationList';

beforeEach(() => { HTMLElement.prototype.scrollIntoView = vi.fn(); });
afterEach(cleanup);

function settingsProps() {
  return { themeMode: 'dark' as const, archiveLimitDraft: '300', isStartupEnabledDraft: false, isStartupImportEnabled: true, isStartupImportLoading: false, sourceLogPath: 'C:/Polaris/Data/archive', logArchivePath: 'C:/Data/logs', databasePath: 'C:/Data/events.db', isDataLocationLoading: false, isSourceLogChangeDisabled: false, onChangeSourceLogDirectory: vi.fn(), onThemeModeChange: vi.fn(), onArchiveLimitDraftChange: vi.fn(), onSaveArchiveLimit: vi.fn(), onToggleStartup: vi.fn(), onToggleStartupImport: vi.fn() };
}

describe('設定画面の操作', () => {
  it('テーマ・startup・取り込み許可を別のcallbackへ渡す', () => {
    const props = settingsProps();
    render(<SettingsControls {...props} />);
    fireEvent.click(screen.getByRole('combobox', { name: 'テーマ' }));
    fireEvent.click(screen.getByRole('option', { name: 'ライト' }));
    expect(props.onThemeModeChange).toHaveBeenCalledWith('light');
    fireEvent.click(screen.getByRole('checkbox', { name: '自動起動' }));
    expect(props.onToggleStartup).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole('checkbox', { name: 'ログの自動取り込み' }));
    expect(props.onToggleStartupImport).toHaveBeenCalledOnce();
    expect(screen.getByText('C:/Data/events.db')).toBeTruthy();
    expect(screen.getByText('C:/Data/logs')).toBeTruthy();
  });

  it('容量入力を8文字まで受け付け、Enterと保存ボタンで保存を要求する', () => {
    const props = settingsProps();
    render(<SettingsControls {...props} />);
    const input = screen.getByRole('spinbutton', { name: '警告設定' });
    fireEvent.change(input, { target: { value: '10485760' } });
    expect(props.onArchiveLimitDraftChange).toHaveBeenLastCalledWith('10485760');
    fireEvent.change(input, { target: { value: '123456789' } });
    expect(props.onArchiveLimitDraftChange).toHaveBeenCalledOnce();
    fireEvent.keyDown(input, { key: 'ArrowUp' });
    expect(props.onSaveArchiveLimit).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: 'Enter' });
    fireEvent.click(screen.getByRole('button', { name: '保存' }));
    expect(props.onSaveArchiveLimit).toHaveBeenCalledTimes(2);
  });

  it('取り込み設定の取得中は切替を無効化し、取得後は許可状態を表示する', () => {
    const props = settingsProps();
    const { rerender } = render(<SettingsControls {...props} isStartupImportLoading />);
    const input = screen.getByRole<HTMLInputElement>('checkbox', { name: 'ログの自動取り込み' });
    expect(input.disabled).toBe(true);
    act(() => { input.click(); });
    expect(props.onToggleStartupImport).not.toHaveBeenCalled();
    rerender(<SettingsControls {...props} isStartupEnabledDraft isStartupImportEnabled={false} />);
    expect(screen.getByRole<HTMLInputElement>('checkbox', { name: '自動起動' }).checked).toBe(true);
    expect(screen.getByRole<HTMLInputElement>('checkbox', { name: 'ログの自動取り込み' }).checked).toBe(false);
  });

  it('保存先の取得中と取得失敗を区別し、取得後は元のpathを保持する', () => {
    const { rerender } = render(<DataLocationList sourceLogPath="" logArchivePath="" databasePath="" isLoading />);
    expect(screen.getAllByText('読み込み中…')).toHaveLength(3);
    rerender(<DataLocationList sourceLogPath="" logArchivePath="" databasePath="C:/Data/events.db" />);
    expect(screen.getByText('保存先を取得できません')).toBeTruthy();
    expect(screen.getByText('フォルダを選択してください')).toBeTruthy();
    expect(screen.getByText('C:/Data/events.db').getAttribute('title')).toBe('C:/Data/events.db');
  });

  it('読込元の変更を専用callbackへ渡し、選択・保存中は変更ボタンを無効にする', () => {
    const props = settingsProps();
    const { rerender } = render(<SettingsControls {...props} />);
    expect(screen.getByText(props.sourceLogPath)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'ログの読み込み元を変更' }));
    expect(props.onChangeSourceLogDirectory).toHaveBeenCalledOnce();
    expect(props.onToggleStartupImport).not.toHaveBeenCalled();
    rerender(<SettingsControls {...props} isSourceLogChangeDisabled />);
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'ログの読み込み元を変更' }).disabled).toBe(true);
  });
});
