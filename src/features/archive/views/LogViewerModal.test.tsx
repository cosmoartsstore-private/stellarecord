// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { installDialogBoundary } from '../../../test/dialog';
import { LogViewerModal } from './LogViewerModal';

installDialogBoundary();
beforeEach(() => {
  // jsdomのlayout境界だけを補い、productionのvirtualizerと絞り込みを実行する。
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(400);
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(800);
});
afterEach(cleanup);
function props() {
  return { logViewerData: { archive_name: 'archive-a', source_name: '元ログ', timestamps: [], levels: [1, 2, 3, 4, 1, 1, 1, 1], categories: [1, 0, 0, 0, 3, 4, 5, 6], raw_lines: ['ワールドの本文', '警告の本文', 'エラーの本文', 'デバッグの本文', '入室の本文', '退室の本文', '動画の本文', '写真の本文'].map((body, index) => `2026.10.22 12:00:00 ${['Log', 'Warning', 'Error', 'Debug'][index] ?? 'Log'} - ${body}`), highlights: [{ start: 5, end: 7 }, null, null, null, null, null, null, null] }, archiveFiles: [{ name: 'archive-a', size_bytes: 1024 }, { name: 'archive-b', size_bytes: 2048 }], externalFiles: [], isLoading: false, isLoaded: true, onNavigateToFile: vi.fn(), onPickExternalFiles: vi.fn(), onClearExternalFiles: vi.fn(), onClose: vi.fn() };
}

describe('LogViewerModal', () => {
  it('ログがない場合は理由を表示し、キャンセルと外部ログ選択だけを操作できる', () => {
    const input = props();
    render(<LogViewerModal {...input} archiveFiles={[]} logViewerData={null} isLoaded={false} />);
    const message = screen.getByText('ログがありません。');
    expect(screen.getAllByRole<HTMLButtonElement>('button').filter((button) => !button.disabled).map((button) => button.textContent)).toEqual(['ログを選択', 'キャンセル']);
    expect(screen.getByRole<HTMLInputElement>('searchbox').disabled).toBe(true);
    expect(screen.queryByText('読み込み中…')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'ワールド' }));
    expect(screen.getByRole('button', { name: 'ワールド' }).getAttribute('aria-pressed')).toBe('false');
    if (!message.parentElement) throw new Error('ログ表示領域がありません');
    fireEvent.wheel(message.parentElement, { deltaY: -1, ctrlKey: true });
    expect(screen.queryByText('110%')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'ログを選択' }));
    expect(input.onPickExternalFiles).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole('button', { name: 'キャンセル' }));
    expect(input.onClose).toHaveBeenCalledOnce();
  });

  it('アーカイブ0件から外部ログを開くと検索と分類を使え、選択解除で空表示へ戻る', () => {
    const input = props();
    const { rerender } = render(<LogViewerModal {...input} archiveFiles={[]} logViewerData={null} isLoaded={false} />);
    const externalPath = 'C:/Logs/outside.txt';
    rerender(<LogViewerModal {...input} archiveFiles={[]} externalFiles={[externalPath]} logViewerData={{ ...input.logViewerData, archive_name: externalPath }} />);
    expect(screen.queryByText('ログがありません。')).toBeNull();
    expect(screen.getByRole<HTMLInputElement>('searchbox').disabled).toBe(false);
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'ワールド' }).disabled).toBe(false);
    fireEvent.click(screen.getByRole('button', { name: 'ワールド' }));
    expect(screen.getByText('1 / 8 行')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: '選択を解除' }));
    expect(input.onClearExternalFiles).toHaveBeenCalledOnce();
    rerender(<LogViewerModal {...input} archiveFiles={[]} logViewerData={null} isLoaded={false} />);
    expect(screen.getByText('ログがありません。')).toBeTruthy();
    expect(screen.getAllByRole<HTMLButtonElement>('button').filter((button) => !button.disabled).map((button) => button.textContent)).toEqual(['ログを選択', 'キャンセル']);
  });

  it('読取開始に失敗しても別のアーカイブと外部ログを選択できる', () => {
    const input = props();
    render(<LogViewerModal {...input} logViewerData={null} isLoaded={false} />);
    expect(screen.getByText('表示するログを選択してください。')).toBeTruthy();
    expect(screen.queryByText('ログがありません。')).toBeNull();
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'ワールド' }).disabled).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: /archive-b/ }));
    expect(input.onNavigateToFile).toHaveBeenCalledWith('archive-b');
    fireEvent.click(screen.getByRole('button', { name: 'ログを選択' }));
    expect(input.onPickExternalFiles).toHaveBeenCalledOnce();
  });

  it('分類を複数選択すると和集合を表示し、クリアで全行へ戻る', async () => {
    const { container } = render(<LogViewerModal {...props()} />);
    await waitFor(() => { expect(container.querySelector('mark')?.textContent).toBe('本文'); });
    expect(screen.getByText('8 行')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: '入退室' }));
    expect(screen.getByText('2 / 8 行')).toBeTruthy();
    expect(screen.getByText('入室の本文')).toBeTruthy();
    expect(screen.getByText('退室の本文')).toBeTruthy();
    expect(screen.queryByText('警告の本文')).toBeNull();
    for (const [name, body] of [['警告', '警告の本文'], ['エラー', 'エラーの本文'], ['デバッグ', 'デバッグの本文'], ['動画', '動画の本文'], ['写真撮影', '写真の本文']] as const) {
      fireEvent.click(screen.getByRole('button', { name }));
      expect(screen.getByText(body)).toBeTruthy();
      expect(screen.getByText('入室の本文')).toBeTruthy();
    }
    expect(screen.getByText('7 / 8 行')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: '入退室' }));
    expect(screen.getByText('5 / 8 行')).toBeTruthy();
    expect(screen.queryByText('入室の本文')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: '分類フィルターの選択をクリア' }));
    expect(screen.getByText('8 行')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'デバッグ' }).getAttribute('aria-pressed')).toBe('false');
  });

  it('外部パスとarchive名を区別し、現在ファイルやloading時には切り替え要求を出さない', () => {
    const input = props();
    const { rerender } = render(<LogViewerModal {...input} externalFiles={['C:\\Logs\\outside.txt', '/logs/other.txt']} />);
    fireEvent.click(screen.getByRole('button', { name: 'outside.txt' }));
    expect(input.onNavigateToFile).toHaveBeenLastCalledWith('C:\\Logs\\outside.txt');
    fireEvent.click(screen.getByRole('button', { name: 'other.txt' }));
    expect(input.onNavigateToFile).toHaveBeenLastCalledWith('/logs/other.txt');
    fireEvent.click(screen.getByRole('button', { name: /archive-b/ }));
    expect(input.onNavigateToFile).toHaveBeenLastCalledWith('archive-b');
    fireEvent.click(screen.getByRole('button', { name: /archive-a/ }));
    expect(input.onNavigateToFile).toHaveBeenCalledTimes(3);
    fireEvent.click(screen.getByRole('button', { name: 'ログを選択' }));
    fireEvent.click(screen.getByRole('button', { name: '選択を解除' }));
    expect(input.onPickExternalFiles).toHaveBeenCalledTimes(1);
    expect(input.onClearExternalFiles).toHaveBeenCalledTimes(1);
    rerender(<LogViewerModal {...input} isLoading isLoaded={false} />);
    expect(screen.getByText('読み込み中…')).toBeTruthy();
    act(() => { screen.getByRole<HTMLButtonElement>('button', { name: /archive-b/ }).click(); });
    expect(input.onNavigateToFile).toHaveBeenCalledTimes(3);
    fireEvent.click(screen.getByRole('button', { name: '閉じる' }));
    expect(input.onClose).toHaveBeenCalledTimes(1);
  });

  it('Ctrl+wheelのzoomを範囲内で変更し、file切替は保持、閉じて再表示すると初期値へ戻る', async () => {
    const input = props();
    const { container, rerender, unmount } = render(<LogViewerModal {...input} />);
    await waitFor(() => { expect(screen.getByText('警告の本文')).toBeTruthy(); });
    const body = screen.getByText('警告の本文');
    const list = body.parentElement?.parentElement;
    if (!list) throw new Error('ログのscroll領域がありません');
    fireEvent.wheel(list, { deltaY: -1 });
    expect(screen.queryByText('110%')).toBeNull();
    for (let i = 0; i < 20; i++) fireEvent.wheel(list, { deltaY: -1, ctrlKey: true });
    expect(screen.getByText('250%')).toBeTruthy();
    for (let i = 0; i < 30; i++) fireEvent.wheel(list, { deltaY: 1, ctrlKey: true });
    expect(screen.getByText('50%')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: '警告' }));
    rerender(<LogViewerModal {...input} logViewerData={{ ...input.logViewerData, archive_name: 'archive-b' }} />);
    expect(screen.getByText('50%')).toBeTruthy();
    expect(screen.getByText('1 / 8 行')).toBeTruthy();
    expect(screen.getByText('警告の本文')).toBeTruthy();
    expect(container.querySelector('mark')).toBeNull();
    unmount();
    render(<LogViewerModal {...input} logViewerData={{ ...input.logViewerData, archive_name: 'archive-b' }} />);
    await waitFor(() => { expect(screen.getByText('警告の本文')).toBeTruthy(); });
    expect(screen.queryByText('50%')).toBeNull();
    expect(screen.getByText('警告の本文').parentElement?.style.fontSize).toBe('0.8rem');
    expect(screen.getByText('8 行')).toBeTruthy();
    expect(screen.getByRole('button', { name: '警告' }).getAttribute('aria-pressed')).toBe('false');
    const reopenedList = screen.getByText('警告の本文').parentElement?.parentElement;
    if (!reopenedList) throw new Error('再表示したscroll領域がありません');
    fireEvent.wheel(reopenedList, { deltaY: -1, ctrlKey: true });
    expect(screen.getByText('110%')).toBeTruthy();
  });

  it('空archiveと欠けた分類値はそのまま本文を表示し、存在しない強調を生成しない', async () => {
    const input = props();
    render(<LogViewerModal {...input} archiveFiles={[]} logViewerData={{ ...input.logViewerData, raw_lines: ['分類なし', '範囲外'], levels: [99], categories: [99], highlights: [] }} />);
    expect(screen.getByText('アーカイブがありません')).toBeTruthy();
    await waitFor(() => { expect(screen.getByText('分類なし')).toBeTruthy(); });
    expect(screen.getByText('範囲外')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: '通知' }));
    expect(screen.getByText('0 / 2 行')).toBeTruthy();
    expect(screen.queryByText('分類なし')).toBeNull();
  });
});
