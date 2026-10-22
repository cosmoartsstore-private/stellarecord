import { getMsg } from '../../../messages/getMsg';
import { useState } from 'react';
import { StellaIcon, stellaIconNames } from '../../../shared/components/Icons';
import { closeDialog, closeDialogOnBackdrop, useModalDialog } from '../../../shared/hooks/useModalDialog';
import { formatFileSize } from '../../../shared/lib/byteFormat';
import type { DeletableLogInfo } from '../models/types';
import shared from '../../../shared/styles/shared.module.css';
import styles from './PolarisCleanupModal.module.css';

/** クリーンアップモーダルのProps — アーカイブ内容と一致した元ログのみ受け取る */
interface PolarisCleanupModalProps {
  logs: DeletableLogInfo[];
  onClose: () => void;
  onConfirm: (fileNames: string[]) => void;
}

/** アーカイブ内容と一致したソースログの削除確認モーダル */
export function PolarisCleanupModal({ logs, onClose, onConfirm }: PolarisCleanupModalProps) {
  // 一括削除が一般的なため、初期状態で全ファイルを選択
  const [selected, setSelected] = useState<Set<string>>(new Set(logs.map((l) => l.file_name)));
  const dialogRef = useModalDialog();

  const isAllSelected = selected.size === logs.length;

  /** 指定ファイル名の選択状態を1件分だけトグルする。 */
  const toggle = (fileName: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(fileName)) {
        next.delete(fileName);
      } else {
        next.add(fileName);
      }
      return next;
    });
  };

  /** 全件選択と全件解除を切り替える。 */
  const toggleAll = () => {
    setSelected(isAllSelected ? new Set() : new Set(logs.map((l) => l.file_name)));
  };

  const totalSize = logs.filter((l) => selected.has(l.file_name)).reduce((sum, l) => sum + l.size_bytes, 0);

  return (
    <dialog ref={dialogRef} className={shared.modalOverlay} aria-labelledby="polaris-cleanup-title" aria-describedby="polaris-cleanup-description polaris-cleanup-notice" onClose={onClose} onMouseDown={closeDialogOnBackdrop}>
      <div className={`${shared.modalContent} ${styles.content}`}>
        <div className={styles.header}>
          <div className={styles.warningIcon}><StellaIcon name={stellaIconNames.alert} /></div>
          <div>
            <h3 id="polaris-cleanup-title" className={styles.title} tabIndex={-1} autoFocus>{getMsg('PolarisCleanup.title')}</h3>
            <p id="polaris-cleanup-description" className={styles.subtitle}>{getMsg('PolarisCleanup.description')}<br />
              <span className={styles.dangerText}>{getMsg('PolarisCleanup.irreversible')}</span>
            </p>
          </div>
        </div>

        <div id="polaris-cleanup-notice" className={styles.notice}>{getMsg('PolarisCleanup.archiveRetained')}</div>

        <div className={styles.listHeader}>
          <button type="button" className={styles.toggleAllBtn} onClick={toggleAll}>
            {isAllSelected ? getMsg('common.clearSelection') : getMsg('common.selectAll')}
          </button>
          <span className={styles.listMeta}>
            {String(selected.size)} / {String(logs.length)}{getMsg('PolarisCleanup.selectionSuffix')}{formatFileSize(totalSize)}
          </span>
        </div>

        <div className={styles.fileList}>
          {logs.map((log) => {
            const isSelected = selected.has(log.file_name);
            return (
              <button key={log.file_name} type="button" aria-pressed={isSelected} className={`${styles.fileRow} ${isSelected ? styles.fileRowSelected : ''}`} onClick={() => {
                  toggle(log.file_name);
                }}>
                <span className={`${styles.checkbox} ${isSelected ? styles.checkboxChecked : ''}`}>
                  <svg aria-hidden="true" viewBox="0 0 12 10" className={styles.checkIcon}><polyline points="1.5 5 4.5 8 10.5 2" /></svg>
                </span>
                <span className={styles.fileName}>{log.file_name}</span>
                <span className={styles.fileSize}>{formatFileSize(log.size_bytes)}</span>
              </button>
            );
          })}
        </div>

        <div className={shared.modalActions}>
          <button type="button" className={shared.btn} onClick={closeDialog}>{getMsg('common.cancel')}</button>
          <button type="button" className={`${shared.btn} ${shared.danger} ${shared.wipe}`} disabled={selected.size === 0} onClick={() => {
              onConfirm(Array.from(selected));
            }}>
            {String(selected.size)}{getMsg('PolarisCleanup.deleteCountSuffix')}</button>
        </div>
      </div>
    </dialog>
  );
}
