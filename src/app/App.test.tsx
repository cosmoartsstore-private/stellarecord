// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type EventCallback } from '@tauri-apps/api/event';
import { getVersion } from '@tauri-apps/api/app';
import * as ReactDOM from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { StartupImportSettings } from '../features/settings/models/types';
import { installDialogBoundary } from '../test/dialog';
import App from './App';

const renderer = vi.hoisted(() => ({ createRoot: undefined as typeof import('react-dom/client').createRoot | undefined }));
vi.mock('react-dom/client', async (importOriginal) => {
  const original = await importOriginal<typeof import('react-dom/client')>();
  renderer.createRoot = original.createRoot;
  return { ...original, createRoot: vi.fn(original.createRoot) };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: vi.fn() }));
vi.mock('@tauri-apps/plugin-shell', () => ({ open: vi.fn().mockResolvedValue(undefined) }));
installDialogBoundary();
const callbacks = new Map<string, EventCallback<unknown>>();
const faults = new Set<string>();
const unlisteners: ReturnType<typeof vi.fn>[] = [];
let startup: StartupImportSettings;
let importSession = '';

beforeEach(() => {
  vi.useFakeTimers();
  vi.resetAllMocks();
  if (!renderer.createRoot) throw new Error('Reactのrendererを取得できません');
  vi.mocked(ReactDOM.createRoot).mockImplementation(renderer.createRoot);
  callbacks.clear(); faults.clear(); unlisteners.length = 0; importSession = '';
  window.localStorage.clear();
  HTMLElement.prototype.scrollIntoView = vi.fn();
  startup = { enabled: false, preference_set: true, source_log_path: 'C:\\Polaris\\Data\\archive', log_archive_path: 'C:\\Logs', database_path: 'C:\\Data\\main.db' };
  vi.mocked(getVersion).mockResolvedValue('1.0.0');
  vi.mocked(listen).mockImplementation((name, handler) => {
    callbacks.set(name, handler);
    const unlisten = vi.fn(() => { if (callbacks.get(name) === handler) callbacks.delete(name); });
    unlisteners.push(unlisten);
    return Promise.resolve(unlisten);
  });
  // IO境界だけを置き換え、App・hook・画面・serviceはproductionを接続する。
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (faults.has(command)) return Promise.reject(new Error(`取得失敗: ${command}`));
    const values = args as Record<string, unknown> | undefined;
    switch (command) {
      case 'get_management_settings': return Promise.resolve({ startup_enabled: false, startup_preference_set: true, archive_limit_mb: 800 });
      case 'get_startup_import_settings': return Promise.resolve(startup);
      case 'change_source_log_directory': return Promise.resolve('D:\\日本語 ログ');
      case 'get_storage_status': return Promise.resolve([3, 300]);
      case 'read_registry_catalog': return Promise.resolve({ apps: [{ name: 'Demo', path: 'C:\\Apps\\demo.exe', description: '説明' }] });
      case 'pick_exe_file': return Promise.resolve('C:\\Apps\\added.exe');
      case 'extract_exe_display_name': return Promise.resolve('追加アプリ');
      case 'list_archive_files': return Promise.resolve([{ name: 'a.tar.zst', size_bytes: 10 }, { name: 'b.tar.zst', size_bytes: 20 }]);
      case 'read_archive_log_viewer':
      case 'read_external_log_viewer':
        importSession = String(values?.sessionId);
        return Promise.resolve({ session_id: importSession, archive_name: values?.fileName ?? values?.filePath, source_name: '元ログ' });
      case 'pick_log_files': return Promise.resolve(['C:\\Logs\\external.txt']);
      case 'get_deletable_source_logs': return Promise.resolve({ source_log_path: startup.source_log_path, logs: [{ file_name: 'output_log.txt', size_bytes: 10 }] });
      case 'delete_source_logs': return Promise.resolve(1);
      case 'get_db_tables': return Promise.resolve([{ name: 'logs', label: 'ログ', description: '', storage: '', is_view: false }]);
      case 'get_db_table_data': return Promise.resolve({ name: 'logs', label: 'ログ', description: '', storage: '', columns: [{ name: 'value', label: '値', description: '' }], rows: [['読み込み済み']], total_rows: 501 });
      default: return Promise.resolve(undefined);
    }
  });
});
afterEach(() => { cleanup(); vi.useRealTimers(); document.documentElement.classList.remove('disable-transitions'); });
async function mount() { render(<App />); await act(async () => { await vi.advanceTimersByTimeAsync(800); }); }
async function click(name: string | RegExp) { await act(async () => { fireEvent.click(screen.getByRole('button', { name })); await vi.advanceTimersByTimeAsync(0); }); }
async function clickDialog(name: string | RegExp) { await act(async () => { fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name })); await vi.advanceTimersByTimeAsync(0); }); }
async function toggle(name: string) { await act(async () => { fireEvent.click(screen.getByRole('checkbox', { name })); await vi.advanceTimersByTimeAsync(0); }); }
function emit(name: string, payload: unknown) { callbacks.get(name)?.({ event: name, id: 1, payload }); }

describe('Appの画面からnative commandへの接続', () => {
  it('HTMLのmount先がない場合は起動を拒否する', async () => {
    vi.resetModules();
    await expect(import('../main')).rejects.toThrow('root element was not found');
    expect(invoke).not.toHaveBeenCalled();
  });

  it('実際のentry pointからStrictModeで画面を起動し、unmountで全購読を解除する', async () => {
    vi.resetModules();
    const host = document.createElement('div');
    host.id = 'root'; document.body.append(host);
    const original = renderer.createRoot;
    if (!original) throw new Error('Reactのrendererを取得できません');
    const mounted: { root: ReactDOM.Root | null } = { root: null };
    vi.mocked(ReactDOM.createRoot).mockImplementation((container, options) => {
      mounted.root = original(container, options);
      return mounted.root;
    });
    try {
      await act(async () => { await import('../main'); });
      await act(async () => { await vi.advanceTimersByTimeAsync(800); });
      expect(screen.getByRole('navigation', { name: 'メインナビゲーション' })).toBeTruthy();
      expect(screen.getByText('Demo')).toBeTruthy();
      expect(invoke).toHaveBeenCalledWith('get_startup_import_settings');
    } finally {
      await act(async () => { mounted.root?.unmount(); await vi.advanceTimersByTimeAsync(0); });
      host.remove();
    }
    expect(unlisteners.length).toBeGreaterThan(0);
    expect(unlisteners.every((unlisten) => unlisten.mock.calls.length === 1)).toBe(true);
  });
  it('ランチャーの起動・フォルダ・登録解除・再取得を正しい対象へ接続する', async () => {
    await mount();
    await click('カード表示'); await click('リスト表示');
    await click('起動');
    expect(invoke).toHaveBeenCalledWith('launch_external_app', { appPath: 'C:\\Apps\\demo.exe' });
    expect(screen.getByText('Demo を起動しました')).toBeTruthy();
    await click('ファイルの場所を開く');
    expect(invoke).toHaveBeenCalledWith('show_app_in_folder', { path: 'C:\\Apps\\demo.exe' });
    await click('Demo の登録を解除');
    expect(invoke).not.toHaveBeenCalledWith('unregister_app', { path: 'C:\\Apps\\demo.exe' });
    await clickDialog('登録を解除');
    expect(invoke).toHaveBeenCalledWith('unregister_app', { path: 'C:\\Apps\\demo.exe' });
    await click('再読み込み');
    await act(async () => { await vi.advanceTimersByTimeAsync(700); });
    expect(screen.getByRole('button', { name: '再読み込み' }).hasAttribute('disabled')).toBe(false);
    expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'read_registry_catalog')).toHaveLength(3);
  });

  it.each([['launch_external_app', '起動', 'Demo を起動できませんでした'], ['show_app_in_folder', 'ファイルの場所を開く', 'ファイルの場所を開けませんでした'], ['unregister_app', 'Demo の登録を解除', 'Demo の登録を解除できませんでした']] as const)('失敗した%sは成功通知を出さず、失敗文言を表示する', async (command, button, message) => {
    await mount(); faults.add(command); await click(button);
    if (command === 'unregister_app') await clickDialog('登録を解除');
    expect(screen.getByText(message)).toBeTruthy();
    expect(screen.queryByText('Demo を起動しました')).toBeNull();
    expect(invoke).toHaveBeenCalledWith('log_client_error', expect.anything());
  });

  it('登録をキャンセルした場合は保存せず、再度開くと未保存の入力を引き継がない', async () => {
    await mount(); await click('登録'); await clickDialog('選択');
    fireEvent.change(screen.getByLabelText('アプリ名'), { target: { value: '未保存の表示名' } });
    fireEvent.change(screen.getByLabelText('説明（任意）'), { target: { value: '未保存の補足' } });
    await clickDialog('キャンセル');
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'register_app')).toHaveLength(0);
    expect(screen.queryByText('未保存の表示名 を登録しました')).toBeNull();
    await click('登録');
    expect(screen.getByLabelText<HTMLInputElement>('アプリ名').value).toBe('');
    expect(screen.getByLabelText<HTMLInputElement>('説明（任意）').value).toBe('');
    expect(screen.getByPlaceholderText<HTMLInputElement>('アプリの実行ファイルを選択').value).toBe('');
    expect(within(screen.getByRole('dialog')).getByRole<HTMLButtonElement>('button', { name: '登録' }).disabled).toBe(true);
  });

  it('登録画面の選択内容を保存し、失敗時には入力画面を残して再試行する', async () => {
    await mount(); await click('登録'); await click('選択');
    fireEvent.change(screen.getByLabelText('説明（任意）'), { target: { value: '補足' } });
    faults.add('register_app'); await clickDialog('登録');
    expect(screen.getByText('追加アプリ を登録できませんでした')).toBeTruthy();
    expect(screen.getByRole('dialog')).toBeTruthy();
    faults.delete('register_app'); await clickDialog('登録');
    expect(invoke).toHaveBeenCalledWith('register_app', { path: 'C:\\Apps\\added.exe', name: '追加アプリ', description: '補足' });
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('登録後の一覧と起動対象を、遅れて返った起動時の空一覧で消さない', async () => {
    const original = vi.mocked(invoke).getMockImplementation();
    if (!original) throw new Error('IPC境界を取得できません');
    let completeInitial: (value: { apps: [] }) => void = () => { throw new Error('初回一覧取得前'); };
    let catalogReads = 0;
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command !== 'read_registry_catalog') return original(command, args);
      catalogReads += 1;
      return catalogReads === 1
        ? new Promise((resolve) => { completeInitial = resolve; })
        : Promise.resolve({ apps: [{ name: '追加アプリ', path: 'C:\\Apps\\added.exe', description: '登録済み' }] });
    });
    await mount();
    await click('登録'); await clickDialog('選択'); await clickDialog('登録');
    expect(screen.getByRole('heading', { name: '追加アプリ' })).toBeTruthy();
    expect(screen.getByText('追加アプリ を登録しました')).toBeTruthy();
    await act(async () => { completeInitial({ apps: [] }); await Promise.resolve(); });
    expect(screen.getByRole('heading', { name: '追加アプリ' })).toBeTruthy();
    expect(screen.queryByText('アプリはまだ登録されていません')).toBeNull();
    await click('起動');
    expect(invoke).toHaveBeenCalledWith('launch_external_app', { appPath: 'C:\\Apps\\added.exe' });
  });

  it('解除後の空一覧を、解除前に開始した再読み込みの遅い応答で復活させない', async () => {
    await mount();
    const original = vi.mocked(invoke).getMockImplementation();
    if (!original) throw new Error('IPC境界を取得できません');
    let completeOld: (value: { apps: { name: string; path: string; description: string }[] }) => void = () => { throw new Error('解除前の一覧取得前'); };
    let catalogReads = 0;
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command !== 'read_registry_catalog') return original(command, args);
      catalogReads += 1;
      return catalogReads === 1 ? new Promise((resolve) => { completeOld = resolve; }) : Promise.resolve({ apps: [] });
    });
    await click('再読み込み');
    await click('Demo の登録を解除');
    expect(invoke).not.toHaveBeenCalledWith('unregister_app', { path: 'C:\\Apps\\demo.exe' });
    await clickDialog('登録を解除');
    expect(screen.getByText('アプリはまだ登録されていません')).toBeTruthy();
    await act(async () => { completeOld({ apps: [{ name: 'Demo', path: 'C:\\Apps\\demo.exe', description: '解除前' }] }); await vi.advanceTimersByTimeAsync(700); });
    expect(screen.queryByRole('heading', { name: 'Demo' })).toBeNull();
    expect(screen.getByText('アプリはまだ登録されていません')).toBeTruthy();
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '再読み込み' }).disabled).toBe(false);
    expect(invoke).toHaveBeenCalledWith('unregister_app', { path: 'C:\\Apps\\demo.exe' });
  });

  it('設定画面から管理値と次回取り込みの回答を別々に保存し、テーマを永続化する', async () => {
    await mount(); await click('設定');
    fireEvent.click(screen.getByRole('combobox', { name: 'テーマ' }));
    fireEvent.click(screen.getByRole('option', { name: 'ダーク' }));
    await act(async () => { await vi.advanceTimersByTimeAsync(50); });
    expect(window.localStorage.getItem('stella-record-theme')).toBe('dark');
    expect(document.documentElement.classList.contains('disable-transitions')).toBe(false);
    fireEvent.click(screen.getByRole('combobox', { name: 'テーマ' }));
    fireEvent.click(screen.getByRole('option', { name: 'ダーク' }));
    fireEvent.click(screen.getByRole('combobox', { name: 'テーマ' }));
    fireEvent.click(screen.getByRole('option', { name: 'ライト' }));
    fireEvent.change(screen.getByLabelText('警告設定'), { target: { value: '1200' } });
    await click('保存');
    expect(invoke).toHaveBeenCalledWith('save_management_settings', { startupEnabled: false, archiveLimitMb: 1200 });
    await toggle('自動起動');
    expect(invoke).toHaveBeenLastCalledWith('save_management_settings', { startupEnabled: true, archiveLimitMb: 1200 });
    await toggle('自動起動');
    expect(invoke).toHaveBeenLastCalledWith('save_management_settings', { startupEnabled: false, archiveLimitMb: 1200 });
    await toggle('ログの自動取り込み');
    expect(invoke).toHaveBeenCalledWith('save_startup_import_preference', { enabled: true });
    expect(invoke).not.toHaveBeenCalledWith('launch_startup_archive_import');
    await toggle('ログの自動取り込み');
    expect(invoke).toHaveBeenCalledWith('save_startup_import_preference', { enabled: false });
    await click('設定を閉じる');
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('設定保存失敗を画面へ通知し、下書きと実際の保存状態を混同しない', async () => {
    await mount(); await click('設定'); faults.add('save_management_settings');
    await toggle('自動起動');
    expect(screen.getByText('設定を保存できませんでした')).toBeTruthy();
    expect(screen.getByRole<HTMLInputElement>('checkbox', { name: '自動起動' }).checked).toBe(false);
    await click('保存');
    expect(screen.getAllByText('設定を保存できませんでした')).toHaveLength(2);
    const dialog = screen.getByRole('dialog');
    const notification = within(dialog).getByRole('status');
    expect(notification.getAttribute('aria-live')).toBe('polite');
    expect(within(notification).getAllByText('設定を保存できませんでした')).toHaveLength(2);
    faults.add('save_startup_import_preference');
    await toggle('ログの自動取り込み');
    expect(screen.getByText('自動取り込みの設定を保存できませんでした')).toBeTruthy();
  });

  it('容量下書きは保存操作まで確定せず、自動起動の切替後も入力を保持する', async () => {
    await mount(); await click('設定');
    const input = screen.getByRole<HTMLInputElement>('spinbutton', { name: '警告設定' });
    fireEvent.change(input, { target: { value: '1200' } });
    expect(invoke).not.toHaveBeenCalledWith('save_management_settings', expect.anything());
    await toggle('自動起動');
    expect(invoke).toHaveBeenLastCalledWith('save_management_settings', { startupEnabled: true, archiveLimitMb: 800 });
    expect(input.value).toBe('1200');
    await click('保存');
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'save_management_settings')).toEqual([
      ['save_management_settings', { startupEnabled: true, archiveLimitMb: 800 }],
      ['save_management_settings', { startupEnabled: true, archiveLimitMb: 1200 }],
    ]);
    expect(input.value).toBe('1200');
  });

  it.each([true, false])('初回回答%sを保存し、今回の取り込みは許可時に1回だけ開始する', async (enabled) => {
    startup.preference_set = false;
    await mount();
    await click(enabled ? '自動で取り込む' : '自動で取り込まない');
    expect(invoke).toHaveBeenCalledWith('save_startup_import_preference', { enabled });
    expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'launch_startup_archive_import')).toHaveLength(enabled ? 1 : 0);
    expect(screen.queryByRole('dialog')).toBeNull();
    await click('解析'); await click('ランチャー');
    expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'launch_startup_archive_import')).toHaveLength(enabled ? 1 : 0);
  });

  it('初回許可の保存失敗では確認を維持して取り込まず、同じbuttonの再試行成功後だけ開始する', async () => {
    startup.preference_set = false;
    faults.add('save_startup_import_preference');
    await mount();
    await click('自動で取り込む');
    expect(screen.getByRole('dialog', { name: 'ログの自動取り込み' })).toBeTruthy();
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '自動で取り込む' }).disabled).toBe(false);
    expect(screen.getByText('自動取り込みの設定を保存できませんでした')).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith('save_startup_import_preference', { enabled: true });
    expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'launch_startup_archive_import')).toHaveLength(0);
    faults.delete('save_startup_import_preference');
    await click('自動で取り込む');
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'save_startup_import_preference')).toHaveLength(2);
    expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'launch_startup_archive_import')).toHaveLength(1);
    await click('解析'); await click('ランチャー');
    expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'launch_startup_archive_import')).toHaveLength(1);
  });

  it('許可済みの起動時取り込み失敗と保存先取得失敗を別の通知として1回表示する', async () => {
    startup.enabled = true; faults.add('launch_startup_archive_import');
    await mount();
    expect(screen.getByText('起動時取り込みを開始できませんでした')).toBeTruthy();
    await click('設定'); await click('設定を閉じる');
    expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'launch_startup_archive_import')).toHaveLength(1);
    cleanup(); faults.add('get_startup_import_settings');
    await mount();
    expect(screen.getAllByText('読み込み元と保存先を取得できませんでした')).toHaveLength(1);
    await click('設定');
    expect(screen.getAllByText('読み込み元と保存先を取得できませんでした')).toHaveLength(1);
  });

  it('DBタブで実際のhookを使い、選択・ページ・sortをnativeへ渡す', async () => {
    await mount(); await click('DB');
    expect(screen.getByText('読み込み済み')).toBeTruthy();
    await click('次のページ');
    expect(invoke).toHaveBeenCalledWith('get_db_table_data', { tableName: 'logs', page: 1, sortColumn: undefined, sortDir: undefined });
    await click('値で並べ替え');
    expect(invoke).toHaveBeenCalledWith('get_db_table_data', { tableName: 'logs', page: 0, sortColumn: 'value', sortDir: 'asc' });
    await click('ログ'); await click('次のページ'); await click('前のページ');
  });

  it('設定画面から読込元を変更して再表示でき、取り込み許可と保存先を維持する', async () => {
    await mount(); await click('設定');
    expect(screen.getByText(startup.source_log_path)).toBeTruthy();
    await click('ログの読み込み元を変更');
    expect(invoke).toHaveBeenCalledWith('change_source_log_directory');
    expect(screen.getByText('D:\\日本語 ログ')).toBeTruthy();
    expect(screen.getByText(startup.log_archive_path)).toBeTruthy();
    expect(screen.getByText(startup.database_path)).toBeTruthy();
    expect(screen.getByText('ログの読み込み元を変更しました。次回の取り込みから反映されます。')).toBeTruthy();
    expect(invoke).not.toHaveBeenCalledWith('save_startup_import_preference', expect.anything());
    expect(invoke).not.toHaveBeenCalledWith('launch_startup_archive_import');
    await click('設定を閉じる'); await click('設定');
    expect(screen.getByText('D:\\日本語 ログ')).toBeTruthy();
  });

  it('Polarisがない初回画面から読込元を選択しても、許可するまでは取り込まない', async () => {
    startup.preference_set = false; startup.source_log_path = '';
    await mount();
    expect(screen.getByText('フォルダを選択してください')).toBeTruthy();
    await click('ログの読み込み元を変更');
    expect(screen.getByText('D:\\日本語 ログ')).toBeTruthy();
    expect(invoke).not.toHaveBeenCalledWith('launch_startup_archive_import');
    expect(screen.getByRole('dialog', { name: 'ログの自動取り込み' })).toBeTruthy();
    await click('自動で取り込む');
    expect(invoke).toHaveBeenCalledWith('save_startup_import_preference', { enabled: true });
    expect(invoke).toHaveBeenCalledWith('launch_startup_archive_import');
  });

  it('読込元の変更失敗を通知し、同じボタンから再試行できる', async () => {
    faults.add('change_source_log_directory');
    await mount(); await click('設定'); await click('ログの読み込み元を変更');
    expect(screen.getByText(startup.source_log_path)).toBeTruthy();
    expect(screen.getByText('ログの読み込み元を変更できませんでした')).toBeTruthy();
    faults.delete('change_source_log_directory');
    await click('ログの読み込み元を変更');
    expect(screen.getByText('D:\\日本語 ログ')).toBeTruthy();
  });

  it('選択したarchiveを取り込み、進捗・停止・終了を反映して元ログの削除へ接続する', async () => {
    await mount(); await click('解析'); await click('アーカイブ容量を更新'); await click('ログを選択');
    await click('すべて選択'); await click('取り込み開始');
    expect(invoke).toHaveBeenCalledWith('launch_enhanced_import', { fileNames: ['a.tar.zst', 'b.tar.zst'] });
    act(() => { emit('analyze-progress', { status: '取り込み中', progress: '1 / 2', is_running: true }); });
    expect(screen.getByText('1 / 2')).toBeTruthy();
    await click('停止');
    expect(invoke).toHaveBeenCalledWith('cancel_analyze');
    act(() => { emit('analyze-finished', null); });
    await click('元ログの削除'); await click('1 件を削除');
    expect(invoke).toHaveBeenCalledWith('delete_source_logs', { sourceLogPath: startup.source_log_path, fileNames: ['output_log.txt'] });
  });

  it('取込中に別tabへ移動しても進捗と完了を保持し、戻って同じ対象を再操作できる', async () => {
    await mount(); await click('解析'); await click('ログを選択');
    await click('すべて選択'); await click('取り込み開始');
    expect(invoke).toHaveBeenCalledWith('launch_enhanced_import', { fileNames: ['a.tar.zst', 'b.tar.zst'] });
    act(() => { emit('analyze-progress', { status: '開始済み', progress: '0 / 2', is_running: true }); });
    await click('DB');
    expect(screen.getByRole('button', { name: 'DB' }).getAttribute('aria-current')).toBe('page');
    act(() => { emit('analyze-progress', { status: '別tabで継続中', progress: '1 / 2', is_running: true }); });
    await click('解析');
    expect(screen.getByRole('progressbar', { name: 'ログ取り込み' }).getAttribute('aria-valuenow')).toBe('50');
    expect(screen.getByText('別tabで継続中')).toBeTruthy();
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'ログを選択' }).disabled).toBe(true);
    await click('ランチャー');
    act(() => {
      emit('analyze-progress', { status: '別tabで完了', progress: '2 / 2', is_running: false });
      emit('analyze-finished', null);
    });
    await click('解析');
    expect(screen.getByText('別tabで完了 (2 / 2)')).toBeTruthy();
    expect(screen.queryByRole('progressbar', { name: 'ログ取り込み' })).toBeNull();
    expect(screen.queryByRole('button', { name: '停止' })).toBeNull();
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'ログを選択' }).disabled).toBe(false);
    await click('ログを選択'); await click('すべて選択'); await click('取り込み開始');
    expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'launch_enhanced_import')).toEqual([
      ['launch_enhanced_import', { fileNames: ['a.tar.zst', 'b.tar.zst'] }],
      ['launch_enhanced_import', { fileNames: ['a.tar.zst', 'b.tar.zst'] }],
    ]);
  });

  it('アーカイブ0件でもビューアを開き、外部ログの選択・解除・キャンセルを使える', async () => {
    const nativeInvoke = vi.mocked(invoke).getMockImplementation();
    if (!nativeInvoke) throw new Error('native境界の実装がありません');
    vi.mocked(invoke).mockImplementation((command, args) => command === 'list_archive_files' ? Promise.resolve([]) : nativeInvoke(command, args));
    await mount(); await click('解析'); await click('ログを開く');
    const viewer = screen.getByRole('dialog', { name: 'ログビューア' });
    expect(within(viewer).getByText('ログがありません。')).toBeTruthy();
    expect(within(viewer).getAllByRole<HTMLButtonElement>('button').filter((button) => !button.disabled).map((button) => button.textContent)).toEqual(['ログを選択', 'キャンセル']);
    expect(invoke).not.toHaveBeenCalledWith('read_archive_log_viewer', expect.anything());
    await clickDialog('ログを選択');
    expect(invoke).toHaveBeenCalledWith('read_external_log_viewer', { filePath: 'C:\\Logs\\external.txt', sessionId: importSession });
    act(() => { emit('log_viewer_done', importSession); });
    expect(within(viewer).getByRole<HTMLInputElement>('searchbox').disabled).toBe(false);
    await clickDialog('選択を解除');
    expect(within(viewer).getByText('ログがありません。')).toBeTruthy();
    expect(callbacks.has('log_viewer_chunk')).toBe(false);
    expect(callbacks.has('log_viewer_done')).toBe(false);
    await clickDialog('キャンセル');
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('最新アーカイブの開始失敗後もビューアを保持し、正常な過去ログを選び直せる', async () => {
    faults.add('read_archive_log_viewer');
    await mount(); await click('解析'); await click('ログを開く');
    const viewer = screen.getByRole('dialog', { name: 'ログビューア' });
    expect(within(viewer).getByText('表示するログを選択してください。')).toBeTruthy();
    expect(within(viewer).getByText('ログの読み込みを開始できませんでした')).toBeTruthy();
    faults.delete('read_archive_log_viewer');
    await clickDialog(/b.tar.zst/);
    expect(invoke).toHaveBeenLastCalledWith('read_archive_log_viewer', { fileName: 'b.tar.zst', sessionId: importSession });
    act(() => { emit('log_viewer_done', importSession); });
    expect(within(viewer).getByRole<HTMLInputElement>('searchbox').disabled).toBe(false);
    await clickDialog('閉じる');
  });

  it('ログビューアにnativeの同一sessionイベントだけを反映し、閉じた後は購読を解除する', async () => {
    await mount(); await click('解析'); await click('ログを開く');
    expect(importSession).not.toBe('');
    expect(invoke).toHaveBeenCalledWith('read_archive_log_viewer', { fileName: 'a.tar.zst', sessionId: importSession });
    act(() => {
      emit('log_viewer_chunk', { session_id: '旧session', timestamps: [], levels: [], categories: [], raw_lines: ['旧行'], highlights: [] });
      emit('log_viewer_chunk', { session_id: importSession, timestamps: ['時刻'], levels: [1], categories: [0], raw_lines: ['現在の行'], highlights: [null] });
      emit('log_viewer_done', importSession);
    });
    expect(screen.getByText('1 行')).toBeTruthy();
    await clickDialog('ログを選択');
    expect(invoke).toHaveBeenCalledWith('read_external_log_viewer', { filePath: 'C:\\Logs\\external.txt', sessionId: importSession });
    expect(screen.getByRole('button', { name: '選択を解除' }).hasAttribute('disabled')).toBe(true);
    act(() => { emit('log_viewer_done', importSession); });
    await click('選択を解除');
    expect(invoke).toHaveBeenLastCalledWith('read_archive_log_viewer', { fileName: 'a.tar.zst', sessionId: importSession });
    act(() => { emit('log_viewer_done', importSession); });
    await click(/b.tar.zst/);
    expect(invoke).toHaveBeenLastCalledWith('read_archive_log_viewer', { fileName: 'b.tar.zst', sessionId: importSession });
    await click('閉じる');
    expect(callbacks.has('log_viewer_chunk')).toBe(false);
    expect(callbacks.has('log_viewer_done')).toBe(false);
    expect(unlisteners.filter((callback) => callback.mock.calls.length === 1)).toHaveLength(8);
  });

  it('cleanup確認はキャンセル時に削除せず、再度開ける', async () => {
    await mount(); await click('解析'); await click('元ログの削除'); await click('キャンセル');
    expect(invoke).not.toHaveBeenCalledWith('delete_source_logs', expect.anything());
    expect(screen.queryByRole('dialog')).toBeNull();
    await click('元ログの削除');
    expect(screen.getByRole('button', { name: '1 件を削除' })).toBeTruthy();
  });
});
