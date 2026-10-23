import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AnalyzeProgressEvent } from '../../features/analyze/models/types';
import type { LogViewerChunk, LogViewerMeta } from '../../features/archive/models/types';
import * as analyze from '../../features/analyze/services/analyzeService';
import * as archive from '../../features/archive/services/archiveService';
import * as database from '../../features/database/services/databaseService';
import * as registry from '../../features/registry/services/registryService';
import * as settings from '../../features/settings/services/settingsService';

interface ProbeInputs {
  database_path: string; source_log_path: string; log_archive_path: string; owned_exe: string; archive_name: string; archive_bytes: number;
  source_name: string; raw_lines: string[]; external_path: string; external_lines: string[]; invalid_external_path: string;
}
const checks: string[] = [];

/** Browserでのassert結果だけを専用titleへ返し、commandの返却値と区別する。 */
function finish(ok: boolean, error = '') { document.title = 'STELLA_NATIVE_IPC_RESULT:' + JSON.stringify({ ok, error, checks }); }
function assert(condition: unknown, message: string): asserts condition { if (!condition) throw new Error(message); }
function equal(actual: unknown, expected: unknown, message: string) { assert(JSON.stringify(actual) === JSON.stringify(expected), `${message}: ${JSON.stringify(actual)}`); }
async function rejected(action: () => Promise<unknown>, expected?: string) {
  try { await action(); } catch (error) { if (expected !== undefined) equal(String(error), expected, '拒否メッセージが異なります'); return; }
  throw new Error('失敗すべきcommandが成功しました。');
}
async function check(name: string, action: () => Promise<void>) { await action(); checks.push(name); }

async function run() {
  const text = document.getElementById('probe-input')?.textContent; assert(text, '専用のprobe入力がありません。');
  const input = JSON.parse(text) as ProbeInputs;
  await check('settings-roundtrip', async () => {
    equal(await settings.loadManagementSettings(), { startup_enabled: false, startup_preference_set: false, archive_limit_mb: 127 }, '初期の管理設定');
    await rejected(() => settings.saveManagementSettings(true, 0), '警告ラインは 1MB～10,485,760MB (10TB) の整数で指定してください');
    equal((await settings.loadManagementSettings()).archive_limit_mb, 127, '拒否後の容量保持');
    await settings.saveManagementSettings(false, 512);
    equal(await settings.loadManagementSettings(), { startup_enabled: false, startup_preference_set: true, archive_limit_mb: 512 }, '保存後の管理設定');
    equal(await settings.loadStartupImportSettings(), { enabled: false, preference_set: false, source_log_path: input.source_log_path, log_archive_path: input.log_archive_path, database_path: input.database_path }, '起動時取込の初期設定');
    await settings.saveStartupImportPreference(true); assert((await settings.loadStartupImportSettings()).enabled, '取込許可が保存されていません。');
    await settings.saveStartupImportPreference(false);
    equal(await settings.loadStartupImportSettings(), { enabled: false, preference_set: true, source_log_path: input.source_log_path, log_archive_path: input.log_archive_path, database_path: input.database_path }, '取込拒否の保存');
  });
  await check('catalog-and-database', async () => {
    await registry.registerApp(input.owned_exe, ' 登録用アプリ ', '説明 "保持"');
    const catalog = await registry.loadRegistryCatalog(); const registered = catalog.apps.find(app => app.path === input.owned_exe);
    assert(registered, '登録したアプリがcatalogにありません。'); equal([registered.name, registered.description], ['登録用アプリ', '説明 "保持"'], 'アプリの保存文言');
    assert((await database.loadDbTables()).some(table => table.name === 'apps'), 'DB tableの一覧にappsがありません。');
    const page = await database.loadDbTableData('apps', 0, 'name', 'desc'); equal(page.total_rows, 1, '登録したDB行数');
    equal(page.rows[0][page.columns.findIndex(column => column.name === 'path')], input.owned_exe, 'DBの保存path');
    await rejected(() => registry.registerApp(input.owned_exe, '重複', ''), '同じパスのアプリが既に登録されています。');
    await registry.unregisterApp(input.owned_exe); equal((await database.loadDbTableData('apps')).total_rows, 0, '登録解除後のDB');
    assert(!(await registry.loadRegistryCatalog()).apps.some(app => app.path === input.owned_exe), '解除したアプリが残っています。');
  });
  await check('archive-and-storage', async () => {
    equal((await archive.loadArchiveFiles()).map(file => [file.name, file.size_bytes]), [[input.archive_name, input.archive_bytes]], 'archive一覧の型と値');
    equal(await analyze.loadStorageStatus(), [input.archive_bytes, 512 * 1024 * 1024], '容量tupleの返却');
    await analyze.cancelAnalyze();
  });
  await check('invalid-ipc-arguments', async () => {
    await rejected(() => invoke('save_management_settings', { startup_enabled: true, archive_limit_mb: 900 }));
    equal((await settings.loadManagementSettings()).archive_limit_mb, 512, '引数拒否後の保存保持');
    await rejected(() => database.loadDbTableData('unknown'));
    equal((await database.loadDbTableData('apps')).total_rows, 0, '失敗後のDB再読込');
  });
  await check('import-events-and-unlisten', async () => {
    const progress: AnalyzeProgressEvent[] = []; let finished = 0; let resolveFinished: (() => void) | undefined;
    const unlistenProgress = await analyze.onAnalyzeProgress(payload => progress.push(payload)); let isProgressSubscribed = true;
    const unlistenFinished = await analyze.onAnalyzeFinished(() => { finished++; resolveFinished?.(); });
    async function importOnce() {
      const done = new Promise<void>(resolve => { resolveFinished = resolve; });
      equal(await archive.launchEnhancedImport([input.archive_name]), '1 件のログの取り込みを開始しました。', '取り込みの開始返却');
      await done; resolveFinished = undefined;
    }
    try {
      await importOnce(); equal(finished, 1, '最初の完了event'); assert(progress.some(payload => payload.is_running), '実行中の進捗がありません。');
      equal(progress.at(-1), { status: '1 件のログの取り込みが完了しました。', progress: '100%', is_running: false }, '最終進捗の型と値');
      equal((await database.loadDbTableData('sessions')).total_rows, 1, '取り込みの永続化');
      const count = progress.length; unlistenProgress(); isProgressSubscribed = false;
      await importOnce(); equal(finished, 2, '再取り込みの完了event'); equal(progress.length, count, '解除後の進捗callback');
      equal((await database.loadDbTableData('sessions')).total_rows, 1, '再取り込みでの重複抑止');
    } finally { if (isProgressSubscribed) unlistenProgress(); unlistenFinished(); }
  });
  async function viewer(session: string, action: () => Promise<LogViewerMeta>, expected: string[]) {
    const chunks: LogViewerChunk[] = []; let resolveDone: (() => void) | undefined;
    const done = new Promise<void>(resolve => { resolveDone = resolve; });
    const unlistenChunk = await listen<LogViewerChunk>('log_viewer_chunk', event => { if (event.payload.session_id === session) chunks.push(event.payload); });
    const unlistenDone = await listen<string>('log_viewer_done', event => { if (event.payload === session) resolveDone?.(); });
    try {
      const meta = await action(); equal(meta.session_id, session, 'viewerのsession'); await done;
      equal(chunks.flatMap(chunk => chunk.raw_lines), expected, 'viewerの全行と順序');
      assert(chunks.every(chunk => chunk.timestamps.length === chunk.raw_lines.length && chunk.levels.length === chunk.raw_lines.length
        && chunk.categories.length === chunk.raw_lines.length && chunk.highlights.length === chunk.raw_lines.length), 'viewerの各配列の長さが異なります。');
      return { meta, chunks };
    } finally { unlistenChunk(); unlistenDone(); }
  }
  await check('archive-viewer-events', async () => {
    const { meta, chunks } = await viewer('archive-browser', () => archive.startLogViewerStream(input.archive_name, 'archive-browser'), input.raw_lines);
    equal(meta.source_name, input.source_name, 'archive元ログ名'); equal(meta.archive_name, input.archive_name, 'archiveのメタ情報'); equal(chunks.length, 1, 'archiveのchunk数');
    equal(chunks[0].categories, [0, 1, 0, 1], '実DBから取得したカテゴリ'); equal(chunks[0].highlights[1], { start: '[Behaviour] Entering Room: '.length, end: '[Behaviour] Entering Room: Owned World'.length }, '保存済みワールドのhighlight');
  });
  await check('external-viewer-events', async () => {
    const { meta, chunks } = await viewer('external-browser', () => archive.startExternalLogViewerStream(input.external_path, 'external-browser'), input.external_lines);
    equal(meta.archive_name, input.external_path, '外部ログのメタ情報');
    equal(chunks.map(chunk => chunk.raw_lines.length), [500, 1], '外部ログのchunk境界');
    await rejected(() => archive.startLogViewerStream('output_log_2026-10-07_00-00-00.txt.tar.zst', 'missing-browser'), 'ファイルが見つかりません: output_log_2026-10-07_00-00-00.txt.tar.zst');
    await rejected(() => archive.startExternalLogViewerStream(input.invalid_external_path, 'invalid-browser'), '対応していないログ形式です: external.txt');
  });
  finish(true);
}

window.addEventListener('error', event => { finish(false, event.error instanceof Error ? event.error.stack ?? event.error.message : event.message); });
window.addEventListener('unhandledrejection', event => { finish(false, String(event.reason)); });
void run().catch((error: unknown) => { finish(false, error instanceof Error ? error.stack ?? error.message : String(error)); });
