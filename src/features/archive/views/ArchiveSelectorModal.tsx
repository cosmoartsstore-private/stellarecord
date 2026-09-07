import { useRef, type KeyboardEvent, type MouseEvent } from 'react';
import { StellaIcon, stellaIconNames } from '../../../shared/components/Icons';
import { useModalDialog } from '../../../shared/hooks/useModalDialog';
import { formatFileSize } from '../../../shared/lib/byteFormat';
import type { ArchiveFileItem } from '../models/types';
import shared from '../../../shared/styles/shared.module.css';
import styles from './ArchiveSelectorModal.module.css';

/** アーカイブ選択モーダル（バッチ取り込み専用）のProps */
interface ArchiveSelectorModalProps {
  archiveFiles: ArchiveFileItem[];
  selectedFiles: Set<string>;
  onClose: () => void;
  onSelectAll: () => void;
  /** マウスドラッグとキーボードを共用する複数選択ハンドラ */
  onFileAction: (event: MouseEvent, fileName: string, type: 'down' | 'enter' | 'keyboard') => void;
  onConfirm: () => void;
}

/**
 * .tar.zst アーカイブの取り込み対象を選択するモーダル。
 * Shift範囲選択 / Ctrl個別トグル / ドラッグ選択に対応する。
 */
export function ArchiveSelectorModal({
  archiveFiles,
  selectedFiles,
  onClose,
  onSelectAll,
  onFileAction,
  onConfirm,
}: ArchiveSelectorModalProps) {
  const titleRef = useRef<HTMLHeadingElement>(null);
  const itemRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const { dialogRef, closeDialog, handleCancel } = useModalDialog({
    onClose,
    initialFocusRef: titleRef,
  });

  /** 矢印キーと Home / End でファイル項目間のフォーカスを移動する。 */
  const handleItemKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    let nextIndex: number | null = null;
    switch (event.key) {
      case 'ArrowDown':
      case 'ArrowRight':
        nextIndex = Math.min(index + 1, archiveFiles.length - 1);
        break;
      case 'ArrowUp':
      case 'ArrowLeft':
        nextIndex = Math.max(index - 1, 0);
        break;
      case 'Home':
        nextIndex = 0;
        break;
      case 'End':
        nextIndex = archiveFiles.length - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    itemRefs.current[nextIndex]?.focus();
  };

  return (
    <dialog
      ref={dialogRef}
      className={`${styles.root} ${shared.modalOverlay} ${shared.fullscreen}`}
      aria-labelledby="archive-selector-title"
      aria-describedby="archive-selector-description"
      onCancel={handleCancel}
    >
      <div className={`${styles.content} ${shared.modalContent}`}>
        <div className={styles.header}>
          <div>
            <h3 ref={titleRef} id="archive-selector-title" tabIndex={-1}>
              復元
            </h3>
            <p id="archive-selector-description">取り込むログを選択してください</p>
          </div>
          <div className={styles.meta}>
            <span className={styles.count} aria-live="polite">
              {selectedFiles.size} / {archiveFiles.length} 件選択中
            </span>
            <button
              type="button"
              className={shared.btn}
              style={{ fontSize: '0.8rem', padding: '0.4rem 1rem' }}
              onClick={onSelectAll}
            >
              {selectedFiles.size === archiveFiles.length ? 'すべて解除' : 'すべて選択'}
            </button>
          </div>
        </div>
        <div className={styles.list}>
          {archiveFiles.length === 0 ? (
            <div className={styles.emptyState}>
              バックアップフォルダ内にアーカイブファイルが見つかりません
            </div>
          ) : (
            archiveFiles.map((file, index) => (
              <button
                key={file.name}
                ref={(element) => {
                  itemRefs.current[index] = element;
                }}
                type="button"
                aria-pressed={selectedFiles.has(file.name)}
                className={`${styles.item} ${selectedFiles.has(file.name) ? styles.itemSelected : ''}`}
                onMouseDown={(event) => {
                  onFileAction(event, file.name, 'down');
                }}
                onMouseEnter={(event) => {
                  onFileAction(event, file.name, 'enter');
                }}
                onClick={(event) => {
                  if (event.detail === 0) onFileAction(event, file.name, 'keyboard');
                }}
                onKeyDown={(event) => {
                  handleItemKeyDown(event, index);
                }}
              >
                <span
                  className={`${styles.checkbox} ${selectedFiles.has(file.name) ? styles.checkboxChecked : ''}`}
                >
                  <svg aria-hidden="true" viewBox="0 0 12 10" className={styles.checkIcon}>
                    <polyline points="1.5 5 4.5 8 10.5 2" />
                  </svg>
                </span>
                <span className={styles.icon}>
                  <StellaIcon name={stellaIconNames.folder} />
                </span>
                <span className={styles.metaBlock}>
                  <span className={styles.name}>{file.name}</span>
                  <span className={styles.size}>{formatFileSize(file.size_bytes)}</span>
                </span>
              </button>
            ))
          )}
        </div>
        <div className={shared.modalActions}>
          <button type="button" className={shared.btn} onClick={closeDialog}>
            キャンセル
          </button>
          <button
            type="button"
            className={`${shared.btn} ${shared.primary}`}
            disabled={selectedFiles.size === 0}
            onClick={onConfirm}
          >
            取り込み開始
          </button>
        </div>
      </div>
    </dialog>
  );
}
