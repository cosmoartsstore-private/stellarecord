import type { MouseEvent } from 'react';
import { StellaIcon, stellaIconNames } from '../../../shared/components/Icons';
import { formatArchiveSize } from '../models/archiveFormat';
import type { ArchiveFileItem } from '../models/types';
import shared from '../../../shared/styles/shared.module.css';
import styles from './ArchiveSelectorModal.module.css';

/** アーカイブ選択モーダル（バッチ取り込み専用）のProps */
interface ArchiveSelectorModalProps {
  archiveFiles: ArchiveFileItem[];
  selectedFiles: Set<string>;
  onClose: () => void;
  onSelectAll: () => void;
  /** 複数選択用のmousedown/mouseenterハンドラ */
  onFileAction: (event: MouseEvent, fileName: string, type: 'down' | 'enter') => void;
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
  return (
    <div className={`${styles.root} ${shared.modalOverlay} ${shared.fullscreen}`}>
      <div className={`${styles.content} ${shared.modalContent}`}>
        <div className={styles.header}>
          <div>
            <h3>復元</h3>
            <p>取り込むログを選択してください</p>
          </div>
          <div className={styles.meta}>
            <span className={styles.count}>
              {selectedFiles.size} / {archiveFiles.length} 件選択中
            </span>
            <button
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
            archiveFiles.map((file) => (
              <button
                key={file.name}
                type="button"
                className={`${styles.item} ${selectedFiles.has(file.name) ? styles.itemSelected : ''}`}
                onMouseDown={(event) => {
                  onFileAction(event, file.name, 'down');
                }}
                onMouseEnter={(event) => {
                  onFileAction(event, file.name, 'enter');
                }}
              >
                <div
                  className={`${styles.checkbox} ${selectedFiles.has(file.name) ? styles.checkboxChecked : ''}`}
                >
                  <svg viewBox="0 0 12 10" className={styles.checkIcon}>
                    <polyline points="1.5 5 4.5 8 10.5 2" />
                  </svg>
                </div>
                <span className={styles.icon}>
                  <StellaIcon name={stellaIconNames.folder} />
                </span>
                <div className={styles.metaBlock}>
                  <span className={styles.name}>{file.name}</span>
                  <span className={styles.size}>{formatArchiveSize(file.size_bytes)}</span>
                </div>
              </button>
            ))
          )}
        </div>
        <div className={shared.modalActions}>
          <button className={shared.btn} onClick={onClose}>
            キャンセル
          </button>
          <button
            className={`${shared.btn} ${shared.primary}`}
            disabled={selectedFiles.size === 0}
            onClick={onConfirm}
          >
            取り込み開始
          </button>
        </div>
      </div>
    </div>
  );
}
