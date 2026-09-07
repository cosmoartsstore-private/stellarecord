import { useEffect, useRef, type MouseEvent, type ReactNode } from 'react';
import { StellaIcon, stellaIconNames } from '../../../shared/components/Icons';
import styles from './SettingsModal.module.css';

interface SettingsModalProps {
  children: ReactNode;
  onClose: () => void;
}

/** アプリ設定をまとめて表示するモーダル。 */
export function SettingsModal({ children, onClose }: SettingsModalProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const titleRef = useRef<HTMLHeadingElement>(null);

  const closeDialog = () => {
    if (dialogRef.current?.open) dialogRef.current.close();
    onClose();
  };

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (!dialog.open) dialog.showModal();
    titleRef.current?.focus();

    return () => {
      if (dialog.open) dialog.close();
    };
  }, []);

  const handleBackdropClick = (event: MouseEvent<HTMLDialogElement>) => {
    if (event.target === event.currentTarget) closeDialog();
  };

  return (
    <dialog
      ref={dialogRef}
      className={styles.dialog}
      aria-labelledby="settings-title"
      onCancel={(event) => {
        event.preventDefault();
        closeDialog();
      }}
      onClick={handleBackdropClick}
    >
      <header className={styles.header}>
        {/* prettier-ignore */}
        <h2 ref={titleRef} id="settings-title" className={styles.title} tabIndex={-1}>設定</h2>
        {/* prettier-ignore */}
        <button type="button" className={styles.closeButton} onClick={closeDialog} aria-label="設定を閉じる"><StellaIcon name={stellaIconNames.close} /></button>
      </header>
      <div className={styles.body}>{children}</div>
    </dialog>
  );
}
