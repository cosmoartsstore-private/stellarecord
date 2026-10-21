/** 実SDKへ同じ引数を渡し、検証用pickerの返却値と外部ログの読込先だけを照合する。 */
import { invoke as realInvoke, type InvokeArgs, type InvokeOptions } from '../../../node_modules/@tauri-apps/api/core.js';
export * from '../../../node_modules/@tauri-apps/api/core.js';

export const pickerResults: string[][] = [];
export const viewerFailures: { file_path: unknown; error: unknown }[] = [];
export const viewerStarts: { file_path: string; session_id: string; source_name: string }[] = [];
let paths: Set<string> | undefined;
let observer: ((result: { paths: string[] } | { error: unknown }) => void) | undefined;
let controlledPath: string | undefined;
let lockStage: 'disabled' | 'pending' | 'held' | 'released' = 'disabled';
export function beginPickerBoundary(ownedPaths: string[], controlStreamLock = false) {
  if (paths || ownedPaths.length !== 2 || new Set(ownedPaths).size !== 2) throw new Error('専用pickerの入力が不正です。');
  paths = new Set(ownedPaths);
  if (controlStreamLock) { controlledPath = ownedPaths[0]; lockStage = 'pending'; }
}

/** 実pickerの選択後だけ専用fileをlockし、native側の取得・解除完了を待つ。SDKの応答は置換しない。 */
function controlStreamLock(stage: string): Promise<void> {
  const target = window as Window & { __STELLA_NATIVE_STREAM_LOCK_READY__?: () => void };
  if (target.__STELLA_NATIVE_STREAM_LOCK_READY__) throw new Error('専用stream lockの待機が重複しています。');
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => { delete target.__STELLA_NATIVE_STREAM_LOCK_READY__; reject(new Error('専用stream lockのnative完了を確認できません。')); }, 10000);
    target.__STELLA_NATIVE_STREAM_LOCK_READY__ = () => { window.clearTimeout(timeout); delete target.__STELLA_NATIVE_STREAM_LOCK_READY__; resolve(); };
    document.title = 'STELLA_NATIVE_IPC_PROGRESS:' + stage;
  });
}
export async function releaseStreamLock() {
  if (lockStage !== 'held') throw new Error('解除対象の専用stream lockがありません。');
  await controlStreamLock('stream-lock-release-before-retry'); lockStage = 'released';
}
export function observeNextPickerResult(): Promise<string[]> {
  if (!paths || observer) throw new Error('pickerの返却監視が未登録または重複しています。');
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => { observer = undefined; reject(new Error('実pickerの返却を確認できません。')); }, 10000);
    observer = result => { observer = undefined; window.clearTimeout(timeout); if ('error' in result) reject(result.error instanceof Error ? result.error : new Error(String(result.error))); else resolve(result.paths); };
  });
}

/** 返却値を置換せず、専用領域の外へ読込みが進む前に検証を失敗させる。 */
export async function invoke<T>(command: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  if (!paths) throw new Error('専用pickerの読込先が未登録です。');
  const values = args as Record<string, unknown> | undefined;
  if (command === 'read_external_log_viewer' && !paths.has(String(values?.filePath))) throw new Error('外部ログの読込先が専用領域の外です。');
  if (command === 'read_external_log_viewer' && lockStage === 'pending') {
    if (values?.filePath !== controlledPath) throw new Error('専用stream lockの対象fileが一致しません。');
    await controlStreamLock('stream-lock-acquire-before-read'); lockStage = 'held';
  }
  try {
    const result = await realInvoke<T>(command, args, options);
    if (command === 'read_external_log_viewer') {
      const meta = result as { source_name: string };
      viewerStarts.push({ file_path: String(values?.filePath), session_id: String(values?.sessionId), source_name: meta.source_name });
    }
    if (command === 'pick_log_files') {
      if (!isStringArray(result)) throw new Error('実pickerの返却型が不正です。');
      pickerResults.push(result);
      if (!result.every(value => paths?.has(value))) throw new Error('実pickerのpathと専用入力が一致しません。' + JSON.stringify({ actual: result, owned: [...paths] }));
      observer?.({ paths: result });
    }
    return result;
  } catch (error) {
    if (command === 'pick_log_files') observer?.({ error });
    if (command === 'read_external_log_viewer') viewerFailures.push({ file_path: values?.filePath, error });
    throw error;
  }
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item: unknown) => typeof item === 'string');
}
