import { invoke } from '@tauri-apps/api/core';

interface StartupInputs { nonce: string; database_path: string; log_archive_path: string; owned_exe: string }
function assert(condition: unknown, message: string): asserts condition { if (!condition) throw new Error(message); }

// 本体mainとrunが登録したcommandだけを呼び、別Builderの検証と区別する。
async function run() {
  const text = document.getElementById('probe-input')?.textContent; assert(text, '専用の起動入力がありません。');
  const input = JSON.parse(text) as StartupInputs; let report: Record<string, unknown>;
  try {
    const preference = await invoke<{ enabled: boolean; preference_set: boolean; database_path: string; log_archive_path: string }>('get_startup_import_settings');
    assert(!preference.enabled && preference.preference_set, '保存した取り込み許可が異なります。');
    assert(preference.database_path === input.database_path && preference.log_archive_path === input.log_archive_path, `本体の実効保存先が専用領域と異なります: ${JSON.stringify({ preference, input })}`);
    const settings = await invoke<{ startup_enabled: boolean; startup_preference_set: boolean; archive_limit_mb: number }>('get_management_settings');
    assert(!settings.startup_enabled && settings.startup_preference_set && settings.archive_limit_mb === 300, '本体の管理設定が異なります。');
    const catalog = await invoke<{ apps: { name: string; description: string; path: string; icon_data: string | null }[] }>('read_registry_catalog');
    assert(catalog.apps.length === 1 && catalog.apps[0].name === '起動検証用アプリ' && catalog.apps[0].description === '起動中に保持'
      && catalog.apps[0].path === input.owned_exe && catalog.apps[0].icon_data === null, '本体のカタログ読込が異なります。');
    const rows = await invoke<{ total_rows: number }>('get_db_table_data', { tableName: 'apps', page: 0 });
    assert(rows.total_rows === 1, '本体commandから既存DBを参照できません。');
    await invoke('cancel_analyze'); await invoke('launch_startup_archive_import');
    await invoke('log_client_error', { message: '起動検証専用の記録' });
    report = { ok: true, error: '', checks: ['saved-settings', 'current-catalog', 'database-preview', 'managed-analyze-state', 'declined-startup-import', 'native-client-log'] };
  } catch (error) { report = { ok: false, error: String(error), checks: [] }; }
  const response = await fetch(`/owned-startup-result/${input.nonce}`, { method: 'POST', body: JSON.stringify(report) });
  assert(response.ok, '専用の起動結果を記録できません。');
}
void run();
