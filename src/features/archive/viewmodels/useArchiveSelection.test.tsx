// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import type { MouseEvent as ReactMouseEvent } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useArchiveSelection } from './useArchiveSelection';

const files = ['a.log', 'b.log', 'c.log', 'd.log'];
const mouse = (options: MouseEventInit = {}) => new MouseEvent('mousedown', options) as unknown as ReactMouseEvent;

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

describe('アーカイブ選択', () => {
  it('通常クリックで複数選択し、選択済み項目だけを解除する', () => {
    const { result } = renderHook(() => useArchiveSelection(files));
    expect([...result.current.selectedFiles]).toEqual([]);
    act(() => { result.current.handleFileAction(mouse(), 'a.log', 'down'); });
    act(() => { result.current.handleFileAction(mouse(), 'c.log', 'down'); });
    expect([...result.current.selectedFiles]).toEqual(['a.log', 'c.log']);
    act(() => { result.current.handleFileAction(mouse(), 'a.log', 'down'); });
    expect([...result.current.selectedFiles]).toEqual(['c.log']);
  });

  it.each([{ ctrlKey: true }, { metaKey: true }])('修飾クリックの追加・解除を隣接項目へドラッグできる: %j', (modifier) => {
    const { result } = renderHook(() => useArchiveSelection(files));
    act(() => { result.current.handleFileAction(mouse(modifier), 'a.log', 'down'); });
    act(() => { result.current.handleFileAction(mouse(), 'b.log', 'enter'); });
    expect([...result.current.selectedFiles]).toEqual(['a.log', 'b.log']);
    act(() => { result.current.handleFileAction(mouse(modifier), 'a.log', 'down'); });
    act(() => { result.current.handleFileAction(mouse(), 'b.log', 'enter'); });
    expect([...result.current.selectedFiles]).toEqual([]);
  });

  it('通常クリックで開始したドラッグも選択と解除のモードを維持する', () => {
    const { result } = renderHook(() => useArchiveSelection(files));
    act(() => { result.current.handleFileAction(mouse(), 'a.log', 'down'); });
    act(() => { result.current.handleFileAction(mouse(), 'c.log', 'enter'); });
    expect([...result.current.selectedFiles]).toEqual(['a.log', 'c.log']);
    act(() => { result.current.handleFileAction(mouse(), 'c.log', 'down'); });
    act(() => { result.current.handleFileAction(mouse(), 'a.log', 'enter'); });
    expect([...result.current.selectedFiles]).toEqual([]);
  });

  it.each([['a.log', 'd.log'], ['d.log', 'a.log']])('Shiftで両端を含む範囲を追加する: %s → %s', (start, end) => {
    const { result } = renderHook(() => useArchiveSelection(files));
    act(() => { result.current.handleFileAction(mouse(), start, 'keyboard'); });
    act(() => { result.current.handleFileAction(mouse({ shiftKey: true }), end, 'down'); });
    expect([...result.current.selectedFiles].sort()).toEqual(files);
  });

  it('存在しないShift終点では選択を変更せず、開始点が一覧から消えた場合も保持する', () => {
    const { result, rerender } = renderHook(({ items }) => useArchiveSelection(items), { initialProps: { items: files } });
    act(() => { result.current.handleFileAction(mouse(), 'a.log', 'keyboard'); });
    act(() => { result.current.handleFileAction(mouse({ shiftKey: true }), 'missing.log', 'down'); });
    expect([...result.current.selectedFiles]).toEqual(['a.log']);
    rerender({ items: ['b.log', 'c.log'] });
    act(() => { result.current.handleFileAction(mouse({ shiftKey: true }), 'c.log', 'keyboard'); });
    expect([...result.current.selectedFiles]).toEqual(['a.log']);
  });

  it('開始点がないShift操作は通常選択になり、キーボードで個別解除できる', () => {
    const { result } = renderHook(() => useArchiveSelection(files));
    act(() => { result.current.handleFileAction(mouse({ shiftKey: true }), 'b.log', 'keyboard'); });
    expect([...result.current.selectedFiles]).toEqual(['b.log']);
    act(() => { result.current.handleFileAction(mouse(), 'b.log', 'keyboard'); });
    expect([...result.current.selectedFiles]).toEqual([]);
  });

  it('領域外のmouseupとclearSelectionがドラッグを終了する', () => {
    const { result } = renderHook(() => useArchiveSelection(files));
    act(() => { result.current.handleFileAction(mouse(), 'a.log', 'down'); });
    act(() => { window.dispatchEvent(new MouseEvent('mouseup')); });
    act(() => { result.current.handleFileAction(mouse(), 'b.log', 'enter'); });
    expect([...result.current.selectedFiles]).toEqual(['a.log']);
    act(() => { result.current.handleFileAction(mouse(), 'c.log', 'down'); });
    act(() => { result.current.clearSelection(); });
    act(() => { result.current.handleFileAction(mouse(), 'd.log', 'enter'); });
    expect([...result.current.selectedFiles]).toEqual([]);
  });

  it('全選択・全解除と空の一覧を処理し、unmountで購読を解除する', () => {
    const remove = vi.spyOn(window, 'removeEventListener');
    const { result, rerender, unmount } = renderHook(({ items }) => useArchiveSelection(items), { initialProps: { items: files } });
    act(() => { result.current.handleSelectAll(); });
    expect([...result.current.selectedFiles]).toEqual(files);
    act(() => { result.current.handleSelectAll(); });
    expect([...result.current.selectedFiles]).toEqual([]);
    rerender({ items: [] });
    act(() => { result.current.handleSelectAll(); });
    expect([...result.current.selectedFiles]).toEqual([]);
    unmount();
    expect(remove).toHaveBeenCalledWith('mouseup', expect.any(Function));
  });
});
