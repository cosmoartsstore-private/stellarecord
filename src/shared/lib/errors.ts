import { invoke } from '@tauri-apps/api/core';

type AddToast = (msg: string) => void;

const maxClientErrorLogLength = 4000;

function ignoreClientLogError(): void {
  return undefined;
}

/** 入力値エラーなど、そのまま画面表示してよい文言を持つエラー。 */
export class UserFacingError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'UserFacingError';
  }
}

/** unknown エラーを内部ログ用の文字列に正規化する。 */
function normalizeErrorDetail(error: unknown): string {
  if (error instanceof Error) {
    return error.stack ?? `${error.name}: ${error.message}`;
  }
  if (typeof error === 'string') {
    return error;
  }
  if (
    error === null ||
    error === undefined ||
    typeof error === 'function' ||
    typeof error === 'symbol' ||
    typeof error === 'bigint' ||
    typeof error === 'number' ||
    typeof error === 'boolean'
  ) {
    return String(error);
  }
  try {
    return JSON.stringify(error);
  } catch {
    return '[unserializable object]';
  }
}

/** フロントエンドで捕捉した詳細エラーを既存の Rust ログへ送る。 */
export function reportClientError(context: string, error: unknown): void {
  const detail = normalizeErrorDetail(error).replaceAll(/\s+/g, ' ').trim();
  const message = `${context}: ${detail}`.slice(0, maxClientErrorLogLength);
  void invoke('log_client_error', { message }).catch(ignoreClientLogError);
}

/** 詳細を内部ログに残し、ユーザーには安定した短い文言だけを表示する。 */
export function addErrorToast(
  addToast: AddToast,
  context: string,
  userMessage: string,
  error: unknown,
): void {
  if (error instanceof UserFacingError) {
    addToast(error.message);
    return;
  }
  reportClientError(context, error);
  addToast(userMessage);
}
