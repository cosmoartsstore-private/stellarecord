// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { installDialogBoundary } from '../../../test/dialog';
import { PolarisCleanupModal } from './PolarisCleanupModal';

installDialogBoundary();
afterEach(cleanup);
const logs = [{ file_name: 'output_log_a.txt', size_bytes: 1024 }, { file_name: 'output_log_b.txt', size_bytes: 2048 }];

describe('元ログの削除の確認', () => {
  it('初期全選択から解除・再選択でき、選択したファイルだけを削除へ渡す', () => {
    const confirm = vi.fn();
    render(<PolarisCleanupModal sourceLogPath="D:/Logs" logs={logs} onClose={vi.fn()} onConfirm={confirm} />);
    expect(screen.getByText(/2 \/ 2 件 選択中/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /output_log_a.txt/ }));
    expect(screen.getByText(/1 \/ 2 件 選択中/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: '1 件を削除' }));
    expect(confirm).toHaveBeenCalledWith(['output_log_b.txt']);
    fireEvent.click(screen.getByRole('button', { name: /output_log_a.txt/ }));
    fireEvent.click(screen.getByRole('button', { name: 'すべて解除' }));
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '0 件を削除' }).disabled).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'すべて選択' }));
    fireEvent.click(screen.getByRole('button', { name: '2 件を削除' }));
    expect(confirm).toHaveBeenLastCalledWith(['output_log_a.txt', 'output_log_b.txt']);
  });

  it('空一覧では削除できず、キャンセルは削除を呼ばない', () => {
    const confirm = vi.fn();
    const close = vi.fn();
    render(<PolarisCleanupModal sourceLogPath="D:/Logs" logs={[]} onClose={close} onConfirm={confirm} />);
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '0 件を削除' }).disabled).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'キャンセル' }));
    expect(close).toHaveBeenCalledOnce();
    expect(confirm).not.toHaveBeenCalled();
  });
});
