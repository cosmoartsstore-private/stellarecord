// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { getVersion } from '@tauri-apps/api/app';
import { open } from '@tauri-apps/plugin-shell';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { installDialogBoundary } from '../test/dialog';
import { CreditButton } from './CreditModal';
import { ToastContainer } from './ToastContainer';

vi.mock('@tauri-apps/api/app', () => ({ getVersion: vi.fn().mockResolvedValue('0.9.1') }));
vi.mock('@tauri-apps/plugin-shell', () => ({ open: vi.fn().mockResolvedValue(undefined) }));
installDialogBoundary();
afterEach(cleanup);
describe('CreditButtonと通知表示', () => {
  it('クリック時だけversionを取得し、各リンクの正確なURLをOSへ渡して閉じられる', async () => {
    render(<CreditButton />);
    expect(getVersion).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'クレジット' }));
    await waitFor(() => { expect(screen.getByText('v0.9.1')).toBeTruthy(); });
    for (const [label, url] of [['lit.link', 'https://lit.link/planet_vrc'], ['X (Twitter)', 'https://x.com/planet_vrc'], ['BOOTH', 'https://cosmo-arts-store.booth.pm/']] as const) {
      fireEvent.click(screen.getByRole('button', { name: new RegExp(label.replaceAll(/[()]/g, '\\$&')) }));
      expect(open).toHaveBeenLastCalledWith(url);
    }
    fireEvent.click(screen.getByRole('button', { name: 'クレジットを閉じる' }));
    expect(screen.queryByRole('dialog')).toBeNull();
  });
  it('同じ通知文を別々に表示し、更新で削除されたidだけを消す', () => {
    const { rerender } = render(<ToastContainer toasts={[{ id: 1, msg: '同じ通知' }, { id: 2, msg: '同じ通知' }]} />);
    expect(screen.getAllByText('同じ通知')).toHaveLength(2);
    rerender(<ToastContainer toasts={[{ id: 2, msg: '残る通知' }]} />);
    expect(screen.queryByText('同じ通知')).toBeNull();
    expect(screen.getByText('残る通知')).toBeTruthy();
    rerender(<ToastContainer toasts={[]} />);
    expect(screen.queryByText('残る通知')).toBeNull();
  });
});
