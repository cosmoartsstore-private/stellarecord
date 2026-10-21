import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import * as analyze from '../features/analyze/services/analyzeService';
import * as archive from '../features/archive/services/archiveService';
import * as database from '../features/database/services/databaseService';
import * as registry from '../features/registry/services/registryService';
import * as settings from '../features/settings/services/settingsService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
beforeEach(() => { vi.resetAllMocks(); vi.mocked(invoke).mockResolvedValue(undefined); vi.mocked(listen).mockResolvedValue(vi.fn()); });

describe('Frontendからnativeへの契約', () => {
  it('管理設定と取り込み回答を別のcommandへ渡し、引数名と値を維持する', async () => {
    const loaded = { startup_enabled: false, startup_preference_set: true, archive_limit_mb: 800 };
    vi.mocked(invoke).mockResolvedValueOnce(loaded);
    await expect(settings.loadManagementSettings()).resolves.toEqual(loaded);
    await settings.saveManagementSettings(true, 1200);
    await settings.loadStartupImportSettings();
    await settings.saveStartupImportPreference(false);
    expect(vi.mocked(invoke).mock.calls).toEqual([['get_management_settings'], ['save_management_settings', { startupEnabled: true, archiveLimitMb: 1200 }], ['get_startup_import_settings'], ['save_startup_import_preference', { enabled: false }]]);
  });

  it('アーカイブと外部ログを別command・別引数で読み込み、session識別子を保持する', async () => {
    await archive.loadArchiveFiles();
    await archive.launchEnhancedImport(['b.tar.zst', 'a.tar.zst']);
    await archive.startLogViewerStream('a.tar.zst', 'session-a');
    vi.mocked(invoke).mockResolvedValueOnce([]);
    await expect(archive.pickLogFiles()).resolves.toEqual([]);
    await archive.startExternalLogViewerStream('C:/Logs/external.txt', 'session-b');
    await archive.launchStartupArchiveImport();
    expect(vi.mocked(invoke).mock.calls).toEqual([['list_archive_files'], ['launch_enhanced_import', { fileNames: ['b.tar.zst', 'a.tar.zst'] }], ['read_archive_log_viewer', { fileName: 'a.tar.zst', sessionId: 'session-a' }], ['pick_log_files'], ['read_external_log_viewer', { filePath: 'C:/Logs/external.txt', sessionId: 'session-b' }], ['launch_startup_archive_import']]);
  });

  it('DBプレビューの既定ページと明示sortをそのまま渡す', async () => {
    await database.loadDbTables();
    await database.loadDbTableData('players');
    await database.loadDbTableData('visits', 2, 'entered_at', 'desc');
    expect(vi.mocked(invoke).mock.calls).toEqual([['get_db_tables'], ['get_db_table_data', { tableName: 'players', page: 0, sortColumn: undefined, sortDir: undefined }], ['get_db_table_data', { tableName: 'visits', page: 2, sortColumn: 'entered_at', sortDir: 'desc' }]]);
  });

  it('アプリ登録・解除・起動をそれぞれのnative操作へ渡す', async () => {
    await registry.loadRegistryCatalog();
    await registry.pickExeFile();
    await registry.extractExeDisplayName('C:/Apps/Alpha.exe');
    await registry.registerApp('C:/Apps/Alpha.exe', 'Alpha', '画像管理');
    await registry.unregisterApp('C:/Apps/Alpha.exe');
    await registry.launchExternalApp('C:/Apps/Alpha.exe');
    await registry.openFolder('C:/Apps');
    expect(vi.mocked(invoke).mock.calls).toEqual([['read_registry_catalog'], ['pick_exe_file'], ['extract_exe_display_name', { path: 'C:/Apps/Alpha.exe' }], ['register_app', { path: 'C:/Apps/Alpha.exe', name: 'Alpha', description: '画像管理' }], ['unregister_app', { path: 'C:/Apps/Alpha.exe' }], ['launch_external_app', { appPath: 'C:/Apps/Alpha.exe' }], ['open_folder', { path: 'C:/Apps' }]]);
  });

  it('容量取得・中断・元ログ削除を区別し、削除するファイル一覧だけを渡す', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([1024, 2048]);
    await expect(analyze.loadStorageStatus()).resolves.toEqual([1024, 2048]);
    await analyze.cancelAnalyze();
    await analyze.getDeletableSourceLogs();
    vi.mocked(invoke).mockResolvedValueOnce(2);
    await expect(analyze.deleteSourceLogs(['output_log_a.txt', 'output_log_b.txt'])).resolves.toBe(2);
    expect(vi.mocked(invoke).mock.calls).toEqual([['get_storage_status'], ['cancel_analyze'], ['get_deletable_source_logs'], ['delete_source_logs', { fileNames: ['output_log_a.txt', 'output_log_b.txt'] }]]);
  });

  it('解析進捗はpayloadだけを通知し、完了購読と解除関数を保持する', async () => {
    const progress = vi.fn();
    const finished = vi.fn();
    const unlisten = vi.fn();
    vi.mocked(listen).mockResolvedValue(unlisten);
    await expect(analyze.onAnalyzeProgress(progress)).resolves.toBe(unlisten);
    await expect(analyze.onAnalyzeFinished(finished)).resolves.toBe(unlisten);
    const handler = vi.mocked(listen).mock.calls[0]?.[1];
    const payload = { status: '解析中', progress: '2/4', is_running: true };
    handler({ event: 'analyze-progress', id: 1, payload });
    expect(progress).toHaveBeenCalledWith(payload);
    expect(vi.mocked(listen).mock.calls[1]).toEqual(['analyze-finished', finished]);
  });

  it('nativeからの失敗を成功値や空配列へ置き換えない', async () => {
    const failure = new Error('native拒否');
    vi.mocked(invoke).mockRejectedValue(failure);
    await expect(registry.registerApp('C:/Apps/Alpha.exe', 'Alpha', '')).rejects.toBe(failure);
    await expect(archive.pickLogFiles()).rejects.toBe(failure);
    await expect(database.loadDbTableData('players')).rejects.toBe(failure);
  });
});
