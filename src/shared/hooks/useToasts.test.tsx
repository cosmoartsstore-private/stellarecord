// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useToasts } from './useToasts';

beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { cleanup(); vi.clearAllTimers(); vi.useRealTimers(); });

describe('通知の表示期間', () => {
  it('同じ文言でも別の通知として保持し、指定期間の通知だけ削除する', () => {
    const { result } = renderHook(() => useToasts());
    act(() => { result.current.addToast('保存しました'); result.current.addToast('保存しました', 500); });
    expect(result.current.toasts).toEqual([{ id: 1, msg: '保存しました' }, { id: 2, msg: '保存しました' }]);
    act(() => { vi.advanceTimersByTime(499); });
    expect(result.current.toasts).toHaveLength(2);
    act(() => { vi.advanceTimersByTime(1); });
    expect(result.current.toasts).toEqual([{ id: 1, msg: '保存しました' }]);
    act(() => { vi.advanceTimersByTime(2500); });
    expect(result.current.toasts).toEqual([]);
    act(() => { result.current.addToast('次の通知', 0); });
    expect(result.current.toasts).toEqual([{ id: 3, msg: '次の通知' }]);
    act(() => { vi.runOnlyPendingTimers(); });
    expect(result.current.toasts).toEqual([]);
  });
});
