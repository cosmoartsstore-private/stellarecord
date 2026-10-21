import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { UserFacingError, addErrorToast, reportClientError } from './errors';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.resetAllMocks(); vi.mocked(invoke).mockResolvedValue(undefined); });

describe('利用者向けエラーと内部ログ', () => {
  it('入力値エラーは元の文言を表示し、内部ログへ送らない', () => {
    const addToast = vi.fn();
    const failure = new UserFacingError('容量は整数で指定してください');
    expect(failure.name).toBe('UserFacingError');
    addErrorToast(addToast, '容量保存', '保存できませんでした', failure);
    expect(addToast).toHaveBeenCalledExactlyOnceWith('容量は整数で指定してください');
    expect(invoke).not.toHaveBeenCalled();
  });

  it('予期しないエラーは短い通知と詳細ログを分ける', () => {
    const addToast = vi.fn();
    const failure = new Error('内部パス C:/Data/events.db');
    failure.stack = 'Error: 内部パス\n  at worker\t(location)';
    addErrorToast(addToast, '設定保存', '設定を保存できませんでした', failure);
    expect(addToast).toHaveBeenCalledExactlyOnceWith('設定を保存できませんでした');
    expect(invoke).toHaveBeenCalledWith('log_client_error', { message: '設定保存: Error: 内部パス at worker (location)' });
  });

  it('stackがないErrorも名前とメッセージを保持する', () => {
    const failure = new TypeError('型が不正');
    failure.stack = undefined;
    reportClientError('取得', failure);
    expect(invoke).toHaveBeenCalledWith('log_client_error', { message: '取得: TypeError: 型が不正' });
  });

  it.each([
    [' \t詳細\n文字列  ', '監査: 詳細 文字列'], [null, '監査: null'], [undefined, '監査: undefined'],
    [true, '監査: true'], [42, '監査: 42'], [3n, '監査: 3'], [Symbol('detail'), '監査: Symbol(detail)'],
    [{ code: 'IO', retry: false }, '監査: {"code":"IO","retry":false}'],
  ])('unknown値をログ用の1行に正規化する: %s', (value, expected) => {
    reportClientError('監査', value);
    expect(invoke).toHaveBeenCalledWith('log_client_error', { message: expected });
  });

  it('関数と循環参照もログに変換し、上限4000文字を維持する', () => {
    function handler() { return undefined; }
    reportClientError('関数', handler);
    expect(vi.mocked(invoke).mock.calls[0]?.[1]).toHaveProperty('message', expect.stringContaining('handler'));
    const circular: { self?: unknown } = {};
    circular.self = circular;
    reportClientError('循環', circular);
    expect(invoke).toHaveBeenCalledWith('log_client_error', { message: '循環: [unserializable object]' });
    reportClientError('長文', 'x'.repeat(5000));
    expect(invoke).toHaveBeenLastCalledWith('log_client_error', { message: '長文: ' + 'x'.repeat(3996) });
  });

  it('詳細ログの送信失敗でも通知を表示し、非同期例外を伝播しない', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error('送信拒否'));
    const addToast = vi.fn();
    expect(() => { addErrorToast(addToast, '保存', '保存できませんでした', new Error('IO')); }).not.toThrow();
    await Promise.resolve();
    expect(addToast).toHaveBeenCalledExactlyOnceWith('保存できませんでした');
  });
});
