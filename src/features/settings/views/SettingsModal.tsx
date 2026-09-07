import type { ReactNode } from 'react';
import { StellaIcon, stellaIconNames } from '../../../shared/components/Icons';
import {
  closeDialog,
  closeDialogOnBackdrop,
  useModalDialog,
} from '../../../shared/hooks/useModalDialog';
import styles from './SettingsModal.module.css';

interface SettingsModalProps {
  children: ReactNode;
  onClose: () => void;
}

/** アプリ設定をまとめて表示するモーダル。 */
export function SettingsModal({ children, onClose }: SettingsModalProps) {
  const dialogRef = useModalDialog();

  return (
    <dialog
      ref={dialogRef}
      className={styles.dialog}
      aria-labelledby="settings-title"
      onClose={onClose}
      onMouseDown={closeDialogOnBackdrop}
    >
      <header className={styles.header}>
        {/* prettier-ignore */}
        <h2 id="settings-title" className={styles.title} tabIndex={-1} autoFocus>設定</h2>
        {/* prettier-ignore */}
        <button type="button" className={styles.closeButton} onClick={closeDialog} aria-label="設定を閉じる"><StellaIcon name={stellaIconNames.close} /></button>
      </header>
      <div className={styles.body}>{children}</div>
    </dialog>
  );
}
