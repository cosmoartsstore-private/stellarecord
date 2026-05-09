import type { MouseEvent } from 'react';
import { StellaIcon, stellaIconNames } from '../../../shared/components/Icons';
import { formatArchiveSize } from '../models/archiveFormat';
import type { ArchiveFileItem } from '../models/types';
import shared from '../../../shared/styles/shared.module.css';
import styles from './ArchiveSelectorModal.module.css';

/** インポート/閲覧モード共用のアーカイブ選択モーダルProps */
interface ArchiveSelectorModalProps {
  archiveFiles: ArchiveFileItem[];
  selectedFiles: Set<string>;
  modalMode: 'import' | 'viewer';
  onClose: () => void;
  onSelectAll: () => void;
  /** 複数選択用のmousedown/mouseenterハンドラ（インポートモード専用） */
  onFileAction: (event: MouseEvent, fileName: string, type: 'down' | 'enter') => void;
  /** 閲覧モードで使用する単一ファイル選択コールバック */
  onSelectSingleFile: (fileName: string) => void;
  onConfirm: () => void;
}

/**
 * .tar.zst アーカイブファイルの選択モーダル
 * インポートモード: 複数選択（Shift/Ctrl/ドラッグ）対応
 * 閲覧モード: 単一選択のみ
 */
export function ArchiveSelectorModal({
  archiveFiles,
  selectedFiles,
  modalMode,
  onClose,
  onSelectAll,
  onFileAction,
  onSelectSingleFile,
  onConfirm,
}: ArchiveSelectorModalProps) {
  return (
    <div className={`${styles.root} ${shared.modalOverlay} ${shared.fullscreen}`}>
      <div className={`${styles.content} ${shared.modalContent}`}>
        <div className={styles.header}>
          <div>
            <h3>{modalMode === 'viewer' ? 'ログを閲覧' : '復元'}</h3>
            <p>
              {modalMode === 'viewer'
                ? '閲覧する .tar.zst を1件選択してください'
                : '取り込むログを選択してください'}
            </p>
          </div>
          <div className={styles.meta}>
            <span className={styles.count}>
              {selectedFiles.size} / {archiveFiles.length} 件選択中
            </span>
            {modalMode !== 'viewer' && (
              <button
                className={shared.btn}
                style={{ fontSize: '0.8rem', padding: '0.4rem 1rem' }}
                onClick={onSelectAll}
              >
                {selectedFiles.size === archiveFiles.length ? 'すべて解除' : 'すべて選択'}
              </button>
            )}
          </div>
        </div>
        {/* 閲覧モードでは単一選択用の狭いリストスタイルを適用 */}
        <div className={`${styles.list} ${modalMode === 'viewer' ? styles.viewerList : ''}`}>
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
                  if (modalMode === 'viewer') {
                    onSelectSingleFile(file.name);
                    return;
                  }

                  onFileAction(event, file.name, 'down');
                }}
                onMouseEnter={(event) => {
                  if (modalMode !== 'viewer') {
                    onFileAction(event, file.name, 'enter');
                  }
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
            className={`${shared.btn} ${modalMode === 'viewer' ? '' : shared.primary}`}
            disabled={selectedFiles.size === 0}
            onClick={onConfirm}
          >
            {modalMode === 'viewer'
              ? selectedFiles.size > 0
                ? `${String(selectedFiles.size)}件から閲覧する`
                : 'ログを開く'
              : '取り込み開始'}
          </button>
        </div>
      </div>
    </div>
  );
}
