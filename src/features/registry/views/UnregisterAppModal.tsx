import { getMsg } from '../../../messages/getMsg';
import { closeDialog, closeDialogOnBackdrop, preventDialogCancel, useModalDialog } from '../../../shared/hooks/useModalDialog';
import shared from '../../../shared/styles/shared.module.css';
import type { AppCard } from '../models/types';
import styles from './RegistrySection.module.css';

interface UnregisterAppModalProps {
  app: AppCard;
  isSubmitting: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

/** 解除する登録を確認し、処理中の重複実行と閉じる操作を抑止する。 */
export function UnregisterAppModal({ app, isSubmitting, onClose, onConfirm }: UnregisterAppModalProps) {
  const dialogRef = useModalDialog();

  return (
    <dialog
      ref={dialogRef}
      className={shared.modalOverlay}
      aria-labelledby="unregister-app-title"
      aria-describedby="unregister-app-description unregister-app-path unregister-app-note"
      aria-busy={isSubmitting}
      onClose={onClose}
      onCancel={isSubmitting ? preventDialogCancel : undefined}
      onMouseDown={isSubmitting ? undefined : closeDialogOnBackdrop}
    >
      <div className={shared.modalContent}>
        <h3 id="unregister-app-title" tabIndex={-1} autoFocus>{getMsg('UnregisterApp.title')}</h3>
        <p id="unregister-app-description">{getMsg('UnregisterApp.description', { name: app.name })}</p>
        <p id="unregister-app-path" className={styles.unregisterPath}>{app.path}</p>
        <p id="unregister-app-note">{getMsg('UnregisterApp.filesRetained')}</p>
        <div className={shared.modalActions}>
          <button type="button" className={shared.btn} onClick={closeDialog} disabled={isSubmitting}>{getMsg('common.cancel')}</button>
          <button type="button" className={`${shared.btn} ${shared.danger}`} onClick={onConfirm} disabled={isSubmitting}>{getMsg('UnregisterApp.confirm')}</button>
        </div>
      </div>
    </dialog>
  );
}
