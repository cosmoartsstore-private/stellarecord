import shared from '../../../shared/styles/shared.module.css';
import { useModalDialog } from '../../../shared/hooks/useModalDialog';
import { DataLocationList } from './DataLocationList';
import styles from './StartupImportConsentDialog.module.css';

interface StartupImportConsentDialogProps {
  logArchivePath: string;
  databasePath: string;
  isSaving: boolean;
  onAllow: () => void;
  onDecline: () => void;
}

/** 初回起動時に、ログ取り込みの保存先と継続実行について確認する。 */
export function StartupImportConsentDialog(props: StartupImportConsentDialogProps) {
  const { logArchivePath, databasePath, isSaving, onAllow, onDecline } = props;
  const { dialogRef, handleCancel } = useModalDialog();

  return (
    <dialog
      ref={dialogRef}
      className={shared.modalOverlay}
      aria-labelledby="startup-import-title"
      aria-describedby="startup-import-description startup-import-note"
      onCancel={handleCancel}
    >
      <section className={`${shared.modalContent} ${styles.dialog}`}>
        <h3 id="startup-import-title">起動時のログ取り込み</h3>
        {/* prettier-ignore */}
        <p id="startup-import-description" className={styles.description}>StellaRecord は起動時に Polaris のログをログ保存先へコピーし、その内容を読み取って DB へ保存します。この読み取りを許可しますか。</p>

        <div className={styles.locationPanel}>
          <DataLocationList logArchivePath={logArchivePath} databasePath={databasePath} />
        </div>

        {/* prettier-ignore */}
        <p id="startup-import-note" className={styles.note}>許可すると、次回以降もアプリ起動時に自動で取り込みます。ログと DB を StellaRecord から外部サーバーへ送信することはありません。</p>

        <div className={shared.modalActions}>
          {/* prettier-ignore */}
          <button type="button" className={shared.btn} onClick={onDecline} disabled={isSaving} autoFocus>読み取らない</button>
          {/* prettier-ignore */}
          <button type="button" className={`${shared.btn} ${shared.primary}`} onClick={onAllow} disabled={isSaving}>読み取りを許可</button>
        </div>
      </section>
    </dialog>
  );
}
