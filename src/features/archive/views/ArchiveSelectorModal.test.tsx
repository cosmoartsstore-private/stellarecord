// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { installDialogBoundary } from '../../../test/dialog';
import { useArchiveSelection } from '../viewmodels/useArchiveSelection';
import { ArchiveSelectorModal } from './ArchiveSelectorModal';

installDialogBoundary();
afterEach(cleanup);
const files = [{ name: 'a.tar.zst', size_bytes: 1024 }, { name: 'b.tar.zst', size_bytes: 2048 }, { name: 'c.tar.zst', size_bytes: 4096 }];

describe('アーカイブ選択画面から選択処理への接続', () => {
  it('マウス・ドラッグ・キーボードと全選択の結果を確認する', () => {
    function Selector() {
      const selection = useArchiveSelection(files.map((file) => file.name));
      return <ArchiveSelectorModal archiveFiles={files} selectedFiles={selection.selectedFiles} onClose={vi.fn()} onSelectAll={selection.handleSelectAll} onFileAction={selection.handleFileAction} onConfirm={vi.fn()} />;
    }
    render(<Selector />);
    const first = screen.getByRole('button', { name: /a.tar.zst/ });
    const second = screen.getByRole('button', { name: /b.tar.zst/ });
    fireEvent.mouseDown(first, { ctrlKey: true });
    fireEvent.mouseEnter(second);
    expect(first.getAttribute('aria-pressed')).toBe('true');
    expect(second.getAttribute('aria-pressed')).toBe('true');
    fireEvent.mouseUp(window);
    fireEvent.click(first, { detail: 1 });
    expect(first.getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(first, { detail: 0 });
    expect(first.getAttribute('aria-pressed')).toBe('false');
    fireEvent.click(screen.getByRole('button', { name: 'すべて選択' }));
    expect(screen.getByText('3 / 3 件選択中')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'すべて解除' }));
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '取り込み開始' }).disabled).toBe(true);
  });

  it.each([['ArrowDown', 1], ['ArrowRight', 1], ['ArrowUp', 0], ['ArrowLeft', 0], ['Home', 0], ['End', 2], ['Escape', 0]])('方向キーで一覧内のfocusを移動する: %s', (key, expectedIndex) => {
    render(<ArchiveSelectorModal archiveFiles={files} selectedFiles={new Set(['a.tar.zst'])} onClose={vi.fn()} onSelectAll={vi.fn()} onFileAction={vi.fn()} onConfirm={vi.fn()} />);
    const items = files.map((file) => screen.getByRole('button', { name: new RegExp(file.name) }));
    items[0]?.focus();
    fireEvent.keyDown(items[0] ?? document.body, { key });
    expect(document.activeElement).toBe(items[expectedIndex]);
    items[2]?.focus();
    fireEvent.keyDown(items[2] ?? document.body, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(items[2]);
  });

  it('選択対象がある時だけ取込を要求し、キャンセルは取込を呼ばない', () => {
    const confirm = vi.fn();
    const close = vi.fn();
    const props = { archiveFiles: files, selectedFiles: new Set(['b.tar.zst']), onClose: close, onSelectAll: vi.fn(), onFileAction: vi.fn(), onConfirm: confirm };
    const { rerender } = render(<ArchiveSelectorModal {...props} />);
    fireEvent.click(screen.getByRole('button', { name: '取り込み開始' }));
    expect(confirm).toHaveBeenCalledOnce();
    rerender(<ArchiveSelectorModal {...props} archiveFiles={[]} selectedFiles={new Set()} />);
    expect(screen.getByText('バックアップフォルダ内にアーカイブファイルが見つかりません')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'キャンセル' }));
    expect(close).toHaveBeenCalledOnce();
    expect(confirm).toHaveBeenCalledOnce();
  });
});
