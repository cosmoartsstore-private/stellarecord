import { getMsg } from '../../../messages/getMsg';
import shared from '../../../shared/styles/shared.module.css';
import { preventDialogCancel, useModalDialog } from '../../../shared/hooks/useModalDialog';
import { DataLocationList } from './DataLocationList';
import styles from './StartupImportConsentDialog.module.css';

interface StartupImportConsentDialogProps {
  sourceLogPath: string;
  logArchivePath: string;
  databasePath: string;
  isSaving: boolean;
  onChangeSourceLogDirectory?: () => void;
  onAllow: () => void;
  onDecline: () => void;
}

/** 初回起動時に、ログ取り込みの保存先と継続実行について確認する。 */
export function StartupImportConsentDialog(props: StartupImportConsentDialogProps) {
  const { sourceLogPath, logArchivePath, databasePath, isSaving, onChangeSourceLogDirectory, onAllow, onDecline } = props;
  const dialogRef = useModalDialog();

  return (
    <dialog ref={dialogRef} className={shared.modalOverlay} aria-labelledby="startup-import-title" aria-describedby="startup-import-description startup-import-note" onCancel={preventDialogCancel}>
      <section className={`${shared.modalContent} ${styles.dialog}`}>
        <h3 id="startup-import-title">{getMsg('StartupImportConsent.title')}</h3>
        <p id="startup-import-description" className={styles.description}>{getMsg('StartupImportConsent.description')}</p>

        <div className={styles.locationPanel}><DataLocationList sourceLogPath={sourceLogPath} logArchivePath={logArchivePath} databasePath={databasePath} isSourceLogChangeDisabled={isSaving} onChangeSourceLogDirectory={onChangeSourceLogDirectory} /></div>

        <p id="startup-import-note" className={styles.note}>{getMsg('StartupImportConsent.details')}</p>

        <div className={shared.modalActions}>
          <button type="button" className={shared.btn} onClick={onDecline} disabled={isSaving} autoFocus>{getMsg('StartupImportConsent.decline')}</button>
          <button type="button" className={`${shared.btn} ${shared.primary}`} onClick={onAllow} disabled={isSaving}>{getMsg('StartupImportConsent.allow')}</button>
        </div>
      </section>
    </dialog>
  );
}
