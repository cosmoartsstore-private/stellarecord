// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { installDialogBoundary } from '../../../test/dialog';
import { SettingsModal } from './SettingsModal';
import { StartupImportConsentDialog } from './StartupImportConsentDialog';

installDialogBoundary();
afterEach(cleanup);

describe('設定dialog', () => {
  it('表示時にタイトルへfocusし、内部操作では閉じず、閉じるボタンで通知する', () => {
    const close = vi.fn();
    render(<SettingsModal onClose={close}><button type="button">内部操作</button></SettingsModal>);
    const dialog = screen.getByRole<HTMLDialogElement>('dialog', { name: '設定' });
    expect(dialog.open).toBe(true);
    expect(document.activeElement).toBe(screen.getByRole('heading', { name: '設定' }));
    fireEvent.mouseDown(screen.getByRole('button', { name: '内部操作' }));
    expect(close).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '設定を閉じる' }));
    expect(dialog.open).toBe(false);
    expect(close).toHaveBeenCalledOnce();
  });

  it('設定dialogの背景で閉じる', () => {
    const close = vi.fn();
    render(<SettingsModal onClose={close}>設定内容</SettingsModal>);
    fireEvent.mouseDown(screen.getByRole('dialog', { name: '設定' }));
    expect(close).toHaveBeenCalledOnce();
  });

  it('初回取り込みは許可と拒否を区別し、保存中とEscapeでは回答を変えない', () => {
    const props = { logArchivePath: 'C:/Data/logs', databasePath: 'C:/Data/events.db', isSaving: false, onAllow: vi.fn(), onDecline: vi.fn() };
    const { rerender } = render(<StartupImportConsentDialog {...props} />);
    const dialog = screen.getByRole('dialog', { name: '起動時のログ取り込み' });
    fireEvent.click(screen.getByRole('button', { name: '読み取らない' }));
    fireEvent.click(screen.getByRole('button', { name: '読み取りを許可' }));
    expect(props.onDecline).toHaveBeenCalledOnce();
    expect(props.onAllow).toHaveBeenCalledOnce();
    const cancel = new Event('cancel', { bubbles: false, cancelable: true });
    fireEvent(dialog, cancel);
    expect(cancel.defaultPrevented).toBe(true);
    rerender(<StartupImportConsentDialog {...props} isSaving />);
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '読み取らない' }).disabled).toBe(true);
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '読み取りを許可' }).disabled).toBe(true);
    expect(screen.getByText('C:/Data/events.db')).toBeTruthy();
  });
});
