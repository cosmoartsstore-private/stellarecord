import { loadManagementSettings, loadStartupImportSettings } from '../../features/settings/services/settingsService';
import { listen } from '@tauri-apps/api/event';
import { beginPickerBoundary, observeNextPickerResult, pickerResults, viewerFailures, viewerStarts, releaseStreamLock } from '../../test/native-ipc/picker-boundary';
import type { LogViewerChunk } from '../../features/archive/models/types';
import expectedLightLogo from '../../assets/logo-light.png';
import expectedDarkLogo from '../../assets/logo-dark.png';
import expectedAvatar from '../../assets/avatar.jpg';

interface UiProbeInputs { database_path: string; log_archive_path: string; archive_name: string; source_name: string; raw_lines: string[]; native_picker: boolean; external_read_failure: boolean; external_stream_lock: boolean; external_stream_partial: boolean; external_paths: string[]; external_lines: string[] }
const checks: string[] = [];
const viewerDoneSessions: string[] = [];
const viewerChunks: { session_id: string; rows: number; first: string; last: string }[] = [];
let isFinished = false;
let isNativePickerEnabled = false;
let unlistenObservedDone: (() => void) | undefined;
let unlistenObservedChunk: (() => void) | undefined;

/** 本体の入口を実Browserで起動し、DOM操作・表示・実IPCの結果を専用titleへ返す。 */
function finish(ok: boolean, error = '') {
  if (isFinished) return;
  unlistenObservedDone?.(); unlistenObservedDone = undefined;
  unlistenObservedChunk?.(); unlistenObservedChunk = undefined;
  isFinished = true; document.title = 'STELLA_NATIVE_IPC_RESULT:' + JSON.stringify({ ok, error, checks, picker_results: pickerResults, viewer_failures: viewerFailures, viewer_starts: viewerStarts, viewer_done_sessions: viewerDoneSessions, viewer_chunks: viewerChunks });
}
function assert(condition: unknown, message: string): asserts condition { if (!condition) throw new Error(message); }
function equal(actual: unknown, expected: unknown, message: string) { assert(JSON.stringify(actual) === JSON.stringify(expected), `${message}: ${JSON.stringify(actual)}`); }
/** bundle内の具体的なsource assetへのbindingと、実WebViewのdecode結果を照合する。 */
async function decodedImage(scope: ParentNode, alt: string, expectedSource: string) {
  const image = scope.querySelector(`img[alt="${alt}"]`);
  assert(image instanceof HTMLImageElement, `画像要素がありません: ${alt}`);
  equal(image.src, new URL(expectedSource, document.baseURI).href, `画像のsource binding: ${alt}`);
  await image.decode();
  assert(image.complete && image.naturalWidth > 0 && image.naturalHeight > 0, `画像をdecodeできません: ${alt}`);
  equal(image.currentSrc, image.src, `decode対象の画像: ${alt}`);
}
function visible(element: Element) {
  const style = getComputedStyle(element);
  return element.getClientRects().length > 0 && style.display !== 'none' && style.visibility !== 'hidden' && !element.closest('dialog:not([open])');
}
function textExists(scope: ParentNode, text: string) { return Array.from(scope.querySelectorAll('div,p,span,h2,h3,h4,td,th')).some(element => visible(element) && element.textContent.trim() === text); }
function findButton(scope: ParentNode, label: string) {
  const buttons = Array.from(scope.querySelectorAll('button')).filter(button => visible(button) && (button.getAttribute('aria-label') ?? button.textContent.trim()) === label);
  assert(buttons.length <= 1, `操作対象が一意ではありません: ${label}`); return buttons.at(0);
}
function openDialog(titleId: string) { return document.querySelector<HTMLDialogElement>(`dialog[open][aria-labelledby="${titleId}"]`) ?? undefined; }
function dialogRemoved(titleId: string) { return !document.querySelector(`dialog[aria-labelledby="${titleId}"]`) ? true : undefined; }

/** nativeの完了によるDOM更新を待つ。時間切れは表示内容を添えて失敗し、固定sleepで成功を推測しない。 */
function waitFor<T>(label: string, read: () => T | undefined): Promise<T> {
  return new Promise((resolve, reject) => {
    const observer = new MutationObserver(poll);
    const timeout = window.setTimeout(() => { observer.disconnect(); reject(new Error(`${label}: DOM更新を確認できません。${document.body.innerText.slice(-3000)}`)); }, 10000);
    function poll() {
      try {
        const value = read();
        if (value === undefined) return;
        observer.disconnect(); window.clearTimeout(timeout); resolve(value);
      } catch (error) { observer.disconnect(); window.clearTimeout(timeout); reject(error instanceof Error ? error : new Error(String(error))); }
    }
    observer.observe(document, { childList: true, subtree: true, attributes: true, characterData: true }); poll();
  });
}
async function clickButton(label: string, scope: ParentNode = document) {
  const button = await waitFor(label, () => { const candidate = findButton(scope, label); return candidate && !candidate.disabled ? candidate : undefined; });
  button.click();
}
function setInput(id: string, value: string) {
  const input = document.getElementById(id); assert(input instanceof HTMLInputElement && !input.disabled && visible(input), `${id}: 入力できません。`);
  // Reactの値追跡を迂回してnative setterで入力し、実際のonChangeへ標準input eventを渡す。
  const descriptor = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value'); assert(typeof descriptor?.set === 'function', 'inputのnative setterがありません。');
  descriptor.set.call(input, value); input.dispatchEvent(new Event('input', { bubbles: true }));
}
async function check(name: string, action: () => Promise<void>) {
  await action(); checks.push(name);
  if (isNativePickerEnabled) document.title = 'STELLA_NATIVE_IPC_UI_CHECK:' + name;
}

async function run() {
  const inputText = document.getElementById('probe-input')?.textContent; assert(inputText, '専用のprobe入力がありません。');
  const input = JSON.parse(inputText) as UiProbeInputs;
  isNativePickerEnabled = input.native_picker;
  if (input.native_picker) beginPickerBoundary(input.external_paths, input.external_stream_lock);
  if (input.external_stream_lock || input.external_stream_partial) unlistenObservedDone = await listen<string>('log_viewer_done', event => { viewerDoneSessions.push(event.payload); });
  if (input.external_stream_partial) unlistenObservedChunk = await listen<LogViewerChunk>('log_viewer_chunk', event => {
    const chunk = event.payload; const rows = chunk.raw_lines.length;
    assert(rows > 0 && [chunk.timestamps, chunk.levels, chunk.categories, chunk.highlights].every(values => values.length === rows), '実SDKへ配信されたchunkの列数が一致しません。');
    viewerChunks.push({ session_id: chunk.session_id, rows, first: chunk.raw_lines[0], last: chunk.raw_lines.at(-1) ?? '' });
  });
  // 起動中の例外も検証結果へ返せるよう、監視登録後に本体の入口を読み込む。
  await import('../../main');
  await check('initial-consent-and-render', async () => {
    const consent = await waitFor('初回の読込確認', () => openDialog('startup-import-title'));
    assert(document.querySelector('.stella-record-root'), '本体のAppが描画されていません。');
    assert(Array.from(document.styleSheets).some(sheet => sheet.href?.endsWith('/browser-probe.css') && sheet.cssRules.length > 0), '本体のCSSが適用されていません。');
    assert(consent.textContent.includes(input.database_path) && consent.textContent.includes(input.log_archive_path), '専用Dataの保存先が表示されていません。');
    await clickButton('読み取らない', consent); await waitFor('初回確認の終了', () => dialogRemoved('startup-import-title'));
    equal(await loadStartupImportSettings(), { enabled: false, preference_set: true, log_archive_path: input.log_archive_path, database_path: input.database_path }, '読込拒否の永続化');
    const navigation = document.querySelector('nav'); assert(navigation, 'メインnavがありません。');
    assert(document.querySelector('.stella-record-root.light-theme'), '初期themeがlightではありません。');
    await decodedImage(navigation, 'STELLA RECORD', expectedLightLogo);
  });
  await check('launcher-unregister', async () => {
    await waitFor('登録済みアプリ', () => textExists(document, '検証用アプリ') && textExists(document, '保存した説明') ? true : undefined);
    await clickButton('カード表示'); await waitFor('カード表示への切替', () => findButton(document, 'カード表示')?.getAttribute('aria-pressed') === 'true' ? true : undefined);
    await clickButton('検証用アプリ の登録を解除');
    await waitFor('登録解除後の空一覧', () => textExists(document, '登録されているアプリはありません') ? true : undefined);
    assert(textExists(document, '検証用アプリ の登録を解除しました'), '登録解除の通知がありません。');
  });
  await check('settings-validation-save-and-theme', async () => {
    await clickButton('設定'); const settings = await waitFor('設定dialog', () => openDialog('settings-title'));
    await waitFor('保存済み容量の初期表示', () => document.querySelector<HTMLInputElement>('#archive-limit-mb')?.value === '127' ? true : undefined);
    setInput('archive-limit-mb', '0'); await clickButton('保存', settings);
    await waitFor('容量の拒否通知', () => textExists(document, '警告ラインは 1MB～10,485,760MB (10TB) の整数で指定してください') ? true : undefined);
    equal((await loadManagementSettings()).archive_limit_mb, 127, '不正入力後の保存保持');
    setInput('archive-limit-mb', '512'); await clickButton('保存', settings);
    await waitFor('容量の保存通知', () => textExists(document, '設定を保存しました') ? true : undefined);
    equal((await loadManagementSettings()).archive_limit_mb, 512, '画面からの容量保存');
    const theme = document.getElementById('theme-mode'); assert(theme instanceof HTMLSelectElement, 'テーマ選択がありません。');
    theme.value = 'dark'; theme.dispatchEvent(new Event('change', { bubbles: true }));
    await waitFor('テーマの永続化と反映', () => document.querySelector('.stella-record-root.dark-theme') && localStorage.getItem('stella-record-theme') === 'dark' ? true : undefined);
    const navigation = document.querySelector('nav'); assert(navigation, 'メインnavがありません。');
    await decodedImage(navigation, 'STELLA RECORD', expectedDarkLogo);
    theme.value = 'midnight'; theme.dispatchEvent(new Event('change', { bubbles: true }));
    await waitFor('midnightの永続化と反映', () => document.querySelector('.stella-record-root.midnight-theme') && localStorage.getItem('stella-record-theme') === 'midnight' ? true : undefined);
    await decodedImage(navigation, 'STELLA RECORD', expectedDarkLogo);
    theme.value = 'dark'; theme.dispatchEvent(new Event('change', { bubbles: true }));
    await waitFor('darkへ戻す', () => document.querySelector('.stella-record-root.dark-theme') ? true : undefined);
    const toggle = document.getElementById('startup-import'); assert(toggle instanceof HTMLInputElement && !toggle.disabled && !toggle.checked, '初期の自動取込状態が異なります。');
    toggle.click(); await waitFor('自動取込を許可', () => toggle.checked && !toggle.disabled && textExists(document, '次回からログの自動取り込みを有効にします') ? true : undefined);
    assert((await loadStartupImportSettings()).enabled, '許可が保存されていません。');
    toggle.click(); await waitFor('自動取込を無効化', () => !toggle.checked && !toggle.disabled && textExists(document, 'ログの自動取り込みを無効にしました') ? true : undefined);
    assert(!(await loadStartupImportSettings()).enabled, '無効化が保存されていません。');
    // close()の直後にはonCloseとAppのunmountが未完了のため、旧dialogの除去まで待って再操作する。
    await clickButton('設定を閉じる', settings); await waitFor('設定を閉じる', () => dialogRemoved('settings-title'));
    await clickButton('設定'); const reopened = await waitFor('設定の再表示', () => openDialog('settings-title'));
    equal(document.querySelector<HTMLInputElement>('#archive-limit-mb')?.value, '512', '再表示時の容量');
    equal(document.querySelector<HTMLSelectElement>('#theme-mode')?.value, 'dark', '再表示時のテーマ');
    await clickButton('設定を閉じる', reopened); await waitFor('設定の終了', () => dialogRemoved('settings-title'));
    await clickButton('クレジット'); const credit = await waitFor('credit dialog', () => openDialog('credit-title'));
    await decodedImage(credit, 'STELLA RECORD', expectedDarkLogo);
    await decodedImage(credit, 'ぷらねっと', expectedAvatar);
    await clickButton('クレジットを閉じる', credit); await waitFor('creditを閉じる', () => dialogRemoved('credit-title'));
  });
  await check('archive-selection-and-import', async () => {
    await clickButton('解析', document.querySelector('nav') ?? document); await clickButton('ログを選択');
    const selector = await waitFor('archive選択dialog', () => openDialog('archive-selector-title'));
    assert(textExists(selector, '0 / 1 件選択中') && findButton(selector, '取り込み開始')?.disabled, '空選択の開始が拒否されていません。');
    const archive = Array.from(selector.querySelectorAll('button')).find(button => button.textContent.includes(input.archive_name));
    assert(archive && visible(archive) && !archive.disabled, '専用archiveを選択できません。'); archive.click();
    await waitFor('archive選択', () => archive.getAttribute('aria-pressed') === 'true' && textExists(selector, '1 / 1 件選択中') ? true : undefined);
    await clickButton('取り込み開始', selector);
    await waitFor('実取込の完了表示', () => dialogRemoved('archive-selector-title') && textExists(document, '1件のインポートが完了しました。 (100%)') && !findButton(document, 'ログを選択')?.disabled ? true : undefined);
  });
  await check('viewer-filter-close-and-reopen', async () => {
    async function openViewer() {
      await clickButton('ログを開く'); const viewer = await waitFor('ログviewer', () => openDialog('log-viewer-title'));
      await waitFor('実streamの4行描画', () => textExists(viewer, '4 行') && input.raw_lines.every(line => textExists(viewer, line)) ? true : undefined);
      equal(document.getElementById('log-viewer-source')?.textContent, input.source_name, 'viewerのsource名'); return viewer;
    }
    const viewer = await openViewer(); await clickButton('ワールド', viewer);
    await waitFor('ワールドfilterの3行描画', () => textExists(viewer, '3 / 4 行') && input.raw_lines.slice(1).every(line => textExists(viewer, line)) && !textExists(viewer, input.raw_lines[0]) ? true : undefined);
    await clickButton('閉じる', viewer); await waitFor('viewer終了', () => dialogRemoved('log-viewer-title'));
    const reopened = await openViewer(); assert(findButton(reopened, 'ワールド')?.getAttribute('aria-pressed') === 'false', '再表示時のfilterが初期化されていません。');
    await clickButton('閉じる', reopened); await waitFor('再表示したviewer終了', () => dialogRemoved('log-viewer-title'));
  });
  await check('database-preview-and-sort', async () => {
    await clickButton('DB', document.querySelector('nav') ?? document); await clickButton('Logical'); await clickButton('visits');
    await waitFor('保存済み訪問のDB表示', () => textExists(document, '1～1 / 全 1 件') && Array.from(document.querySelectorAll('tbody td')).some(cell => cell.textContent === 'Owned World') ? true : undefined);
    for (const expected of ['ascending', 'descending', 'none']) {
      await clickButton('IDで並べ替え');
      // DB読込中にtableが置換されるため、保存した旧nodeではなく現在のheaderと保存行を照合する。
      await waitFor(`DB並べ替え結果: ${expected}`, () => document.querySelector('thead th')?.getAttribute('aria-sort') === expected && Array.from(document.querySelectorAll('tbody td')).some(cell => cell.textContent === 'Owned World') ? true : undefined);
    }
    await clickButton('apps'); await waitFor('登録解除したDB行', () => textExists(document, '0 件') && textExists(document, 'データが存在しません') ? true : undefined);
  });
  if (input.native_picker) {
    await clickButton('解析', document.querySelector('nav') ?? document); await clickButton('ログを開く');
    let viewer = await waitFor('外部選択前のviewer', () => openDialog('log-viewer-title'));
    await waitFor('外部選択前の4行', () => textExists(viewer, '4 行') && input.raw_lines.every(line => textExists(viewer, line)) ? true : undefined);
    const settingsBefore = await loadManagementSettings(); const preferenceBefore = await loadStartupImportSettings();
    const storageBefore = Object.entries(localStorage).sort(([left], [right]) => left.localeCompare(right));
    const fileName = (path: string) => path.split(/[\\/]/).at(-1) ?? '';
    async function pick(stage: string) {
      const result = observeNextPickerResult(); document.title = 'STELLA_NATIVE_IPC_PROGRESS:' + stage;
      // titleの通知をnative側へ渡してから、実pickerを呼ぶ画面の操作を開始する。
      await new Promise<void>(resolve => { requestAnimationFrame(() => { requestAnimationFrame(() => { resolve(); }); }); });
      await clickButton('ファイルを選択', viewer); return result;
    }
    async function unchangedSettings() {
      equal(await loadManagementSettings(), settingsBefore, 'picker操作前後の管理設定');
      equal(await loadStartupImportSettings(), preferenceBefore, 'picker操作前後の取込設定');
      equal(Object.entries(localStorage).sort(([left], [right]) => left.localeCompare(right)), storageBefore, 'picker操作前後の全localStorage');
    }
    await check('native-external-picker-cancel', async () => {
      equal(await pick('picker-initial-cancel'), [], '初回の実picker取消し');
      await waitFor('取消し後のarchive表示', () => findButton(viewer, 'ファイルを選択')?.disabled === false && !findButton(viewer, '選択を解除') && textExists(viewer, '4 行') && input.raw_lines.every(line => textExists(viewer, line)) ? true : undefined);
      equal(document.getElementById('log-viewer-source')?.textContent, input.source_name, '取消し後のsource名'); await unchangedSettings();
    });
    await check(input.external_read_failure ? 'native-external-picker-select-and-read-failure' : input.external_stream_lock ? 'native-external-picker-select-and-stream-open-failure' : input.external_stream_partial ? 'native-external-picker-select-and-partial-stream-failure' : 'native-external-picker-select-and-view', async () => {
      const selected = await pick('picker-select');
      const rawPath = input.external_paths[0]; const compressedPath = input.external_paths[1];
      if (input.external_stream_lock) {
        equal(selected, [rawPath], '実pickerが返した排他lock中の直接log');
        await waitFor('メタ情報返却後の実doneと0行表示', () => {
          const started = viewerStarts.find(item => item.file_path === rawPath);
          return started && viewerDoneSessions.includes(started.session_id) && textExists(viewer, '0 行') && !textExists(viewer, '読み込み中...') && findButton(viewer, fileName(rawPath))?.getAttribute('aria-current') === 'true' ? true : undefined;
        });
        equal(viewerStarts.length, 1, 'lock中の実メタ情報返却回数'); equal(viewerStarts[0].source_name, fileName(rawPath), 'lock中に返ったsource名');
        equal(viewerFailures, [], 'lock中も開始command自体は成功する契約');
        assert(!textExists(document, 'ログの読み込みを開始できませんでした'), '非同期のstream故障を開始commandの拒否へ置換しています。');
        assert(!input.raw_lines.some(line => textExists(viewer, line)) && !input.external_lines.slice(0, 3).some(line => textExists(viewer, line)), 'lock中に旧fileまたは読めないfileの行を表示しています。');
        equal(document.getElementById('log-viewer-source')?.textContent, fileName(rawPath), 'lock中の画面source名'); await unchangedSettings(); return;
      }
      if (input.external_read_failure) {
        equal(selected, [compressedPath], '実pickerが返した空archive');
        await waitFor('実読込故障の通知とviewer解除', () => dialogRemoved('log-viewer-title') && textExists(document, 'ログの読み込みを開始できませんでした') ? true : undefined);
        equal(viewerFailures, [{ file_path: compressedPath, error: `アーカイブ内にログファイルがありません: ${compressedPath}` }], '実IPCの空archive読込失敗');
        assert(findButton(document, 'ログを開く')?.disabled === false, '読込失敗後に再開操作が無効です。'); await unchangedSettings(); return;
      }
      if (input.external_stream_partial) {
        equal(selected, [compressedPath], '実pickerが返した本文途中故障の圧縮log');
        await waitFor('途中故障後の実doneと501行表示', () => {
          const started = viewerStarts.find(item => item.file_path === compressedPath);
          return started && viewerDoneSessions.includes(started.session_id) && textExists(viewer, '501 行') && !textExists(viewer, '読み込み中...') && input.external_lines.slice(0, 3).every(line => textExists(viewer, line)) && findButton(viewer, fileName(compressedPath))?.getAttribute('aria-current') === 'true' ? true : undefined;
        });
        equal(viewerStarts.length, 1, '途中故障の実メタ情報返却回数');
        equal(viewerChunks.filter(chunk => chunk.session_id === viewerStarts[0].session_id).map(chunk => chunk.rows), [500, 1], '途中故障前の実SDK chunk');
        equal(viewerFailures, [], '非同期の途中故障でも開始commandは成功する現行契約');
        assert(!textExists(document, 'ログの読み込みを開始できませんでした'), '途中故障を開始commandの拒否へ置換しています。');
        assert(!input.raw_lines.some(line => textExists(viewer, line)), '既定archiveの行が途中故障fileに混入しています。');
        equal(document.getElementById('log-viewer-source')?.textContent, fileName(rawPath), '途中故障fileの内部source名'); await unchangedSettings(); return;
      }
      equal([...selected].sort(), [...input.external_paths].sort(), '実pickerが返した2file');
      await waitFor('選択した2fileのsidebar', () => textExists(viewer, '選択ファイル') && findButton(viewer, fileName(rawPath)) && findButton(viewer, fileName(compressedPath)) ? true : undefined);
      await clickButton(fileName(rawPath), viewer);
      await waitFor('直接logの501行', () => textExists(viewer, '501 行') && input.external_lines.slice(0, 3).every(line => textExists(viewer, line)) && findButton(viewer, fileName(rawPath))?.getAttribute('aria-current') === 'true' ? true : undefined);
      equal(document.getElementById('log-viewer-source')?.textContent, fileName(rawPath), '直接logのsource名');
      await clickButton(fileName(compressedPath), viewer);
      await waitFor('圧縮logの4行', () => textExists(viewer, '4 行') && input.raw_lines.every(line => textExists(viewer, line)) && findButton(viewer, fileName(compressedPath))?.getAttribute('aria-current') === 'true' ? true : undefined);
      equal(document.getElementById('log-viewer-source')?.textContent, input.source_name, '圧縮logのsource名'); await unchangedSettings();
    });
    if (input.external_read_failure) await check('native-external-picker-retry-after-read-failure', async () => {
      // 読込失敗で本体がviewerを解除するため、同じAppから既定logを開いて再選択する。
      await clickButton('ログを開く'); viewer = await waitFor('読込故障後に再開したviewer', () => openDialog('log-viewer-title'));
      await waitFor('故障後の既定archive表示', () => !textExists(viewer, '選択ファイル') && textExists(viewer, '4 行') && input.raw_lines.every(line => textExists(viewer, line)) ? true : undefined);
      equal(await pick('picker-retry-select'), [input.external_paths[0]], '読込故障後に正常な直接logを実pickerで選択');
      await waitFor('再試行での直接logの501行', () => textExists(viewer, '501 行') && input.external_lines.slice(0, 3).every(line => textExists(viewer, line)) && findButton(viewer, fileName(input.external_paths[0]))?.getAttribute('aria-current') === 'true' ? true : undefined);
      equal(document.getElementById('log-viewer-source')?.textContent, fileName(input.external_paths[0]), '再試行後の直接logのsource名');
      equal(viewerFailures.length, 1, '再試行が過去の読込失敗を再発行しない'); await unchangedSettings();
    });
    if (input.external_stream_lock) await check('native-external-picker-retry-same-file-after-lock', async () => {
      await releaseStreamLock();
      equal(await pick('picker-retry-select'), [input.external_paths[0]], 'lock解除後に同じ直接logを実pickerで再選択');
      await waitFor('同一fileの再試行完了と501行表示', () => viewerStarts.length === 2 && viewerDoneSessions.includes(viewerStarts[1].session_id) && textExists(viewer, '501 行') && input.external_lines.slice(0, 3).every(line => textExists(viewer, line)) ? true : undefined);
      equal(viewerStarts.map(item => item.file_path), [input.external_paths[0], input.external_paths[0]], '故障と再試行で同じ実読込先');
      assert(viewerStarts[0].session_id !== viewerStarts[1].session_id, '再試行に新しいstream sessionを発行していません。'); equal(viewerFailures, [], 'メタ情報返却後故障を開始拒否へ置換しない');
      equal(document.getElementById('log-viewer-source')?.textContent, fileName(input.external_paths[0]), '再試行後の同一file表示'); await unchangedSettings();
    });
    if (input.external_stream_partial) await check('native-external-picker-retry-same-file-after-partial-read', async () => {
      // 実pickerが開いた後にnative parentが所有fileだけを修復し、同じpathを選択する。
      equal(await pick('picker-retry-select'), [input.external_paths[1]], '途中故障後の同じ圧縮logを実pickerで再選択');
      await waitFor('修復後の同一fileの実doneと502行表示', () => viewerStarts.length === 2 && viewerDoneSessions.includes(viewerStarts[1].session_id) && textExists(viewer, '502 行') && input.external_lines.slice(0, 3).every(line => textExists(viewer, line)) ? true : undefined);
      equal(viewerStarts.map(item => item.file_path), [input.external_paths[1], input.external_paths[1]], '途中故障と修復後で同じ実読込path');
      assert(viewerStarts[0].session_id !== viewerStarts[1].session_id, '修復後の再試行に新しいsessionを発行していません。');
      equal(viewerChunks.filter(chunk => chunk.session_id === viewerStarts[1].session_id).map(chunk => chunk.rows), [500, 2], '修復後の実SDK chunk');
      equal(viewerFailures, [], '修復後に開始commandの拒否がありません');
      equal(document.getElementById('log-viewer-source')?.textContent, fileName(input.external_paths[0]), '修復後の内部source名'); await unchangedSettings();
    });
    await check('native-external-picker-cancel-after-selection', async () => {
      equal(await pick('picker-selected-cancel'), [], '選択済み状態の実picker取消し');
      const isDirectSelected = input.external_read_failure || input.external_stream_lock;
      const selectedPath = input.external_paths[isDirectSelected ? 0 : 1]; const lines = isDirectSelected || input.external_stream_partial ? input.external_lines.slice(0, 3) : input.raw_lines;
      await waitFor('再取消し後の選択保持', () => findButton(viewer, fileName(selectedPath))?.getAttribute('aria-current') === 'true' && findButton(viewer, '選択を解除')?.disabled === false && textExists(viewer, input.external_stream_partial ? '502 行' : isDirectSelected ? '501 行' : '4 行') && lines.every(line => textExists(viewer, line)) ? true : undefined);
      equal(document.getElementById('log-viewer-source')?.textContent, input.external_stream_partial ? fileName(input.external_paths[0]) : isDirectSelected ? fileName(selectedPath) : input.source_name, '再取消し後のsource名'); await unchangedSettings();
    });
    await check('external-selection-clear-and-default-viewer', async () => {
      await clickButton('選択を解除', viewer);
      await waitFor('既定archiveへの切替', () => !findButton(viewer, '選択を解除') && !textExists(viewer, '選択ファイル') && textExists(viewer, '4 行') && input.raw_lines.every(line => textExists(viewer, line)) ? true : undefined);
      await unchangedSettings(); await clickButton('閉じる', viewer); await waitFor('外部選択後のviewer終了', () => dialogRemoved('log-viewer-title'));
    });
  }
  finish(true);
}

window.addEventListener('error', event => { finish(false, event.error instanceof Error ? event.error.stack ?? event.error.message : event.message); });
window.addEventListener('unhandledrejection', event => { finish(false, String(event.reason)); });
void run().catch((error: unknown) => { finish(false, error instanceof Error ? error.stack ?? error.message : String(error)); });
