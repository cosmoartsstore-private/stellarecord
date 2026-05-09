import { useState } from 'react';
import type { DeletableLogInfo } from '../models/types';
import shared from '../../../shared/styles/shared.module.css';
import styles from './PolarisCleanupModal.module.css';

/** クリーンアップモーダルのProps — 削除可能なログのみ受け取る */
interface PolarisCleanupModalProps {
  logs: DeletableLogInfo[];
  onClose: () => void;
  onConfirm: (fileNames: string[]) => void;
}

/** バイト数を B / KB / MB の読みやすい文字列に変換する */
function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${String(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** アーカイブ済みソースログの削除確認モーダル */
export function PolarisCleanupModal({ logs, onClose, onConfirm }: PolarisCleanupModalProps) {
  // 一括削除が一般的なため、初期状態で全ファイルを選択
  const [selected, setSelected] = useState<Set<string>>(new Set(logs.map((l) => l.file_name)));

  const allSelected = selected.size === logs.length;

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

  const toggleAll = () => {
    setSelected(allSelected ? new Set() : new Set(logs.map((l) => l.file_name)));
  };

  const totalSize = logs
    .filter((l) => selected.has(l.file_name))
    .reduce((sum, l) => sum + l.size_bytes, 0);

  return (
    <div className={shared.modalOverlay}>
      <button className={shared.modalBackdrop} onClick={onClose} />
      <div className={`${shared.modalContent} ${styles.content}`}>
        <div className={styles.header}>
          <div className={styles.warningIcon}>⚠</div>
          <div>
            <h3 className={styles.title}>元ログ削除</h3>
            <p className={styles.subtitle}>
              Polaris側に存在する元ログを削除します
              <br />
              <span className={styles.dangerText}>※削除した場合、元には戻せません</span>
            </p>
          </div>
        </div>

        <div className={styles.notice}>
          対象データはSTELLA RECORD側に圧縮バックアップされているため、削除後もログビューアから閲覧できます
        </div>

        <div className={styles.listHeader}>
          <button className={styles.toggleAllBtn} onClick={toggleAll}>
            {allSelected ? 'すべて解除' : 'すべて選択'}
          </button>
          <span className={styles.listMeta}>
            {String(selected.size)} / {String(logs.length)} 件 選択中 —{' '}
            {formatBytes(totalSize)}
          </span>
        </div>

        <div className={styles.fileList}>
          {logs.map((log) => {
            const isSelected = selected.has(log.file_name);
            return (
              <button
                key={log.file_name}
                type="button"
                className={`${styles.fileRow} ${isSelected ? styles.fileRowSelected : ''}`}
                onClick={() => { toggle(log.file_name); }}
              >
                <div className={`${styles.checkbox} ${isSelected ? styles.checkboxChecked : ''}`}>
                  <svg viewBox="0 0 12 10" className={styles.checkIcon}>
                    <polyline points="1.5 5 4.5 8 10.5 2" />
                  </svg>
                </div>
                <span className={styles.fileName}>{log.file_name}</span>
                <span className={styles.fileSize}>{formatBytes(log.size_bytes)}</span>
              </button>
            );
          })}
        </div>

        <div className={shared.modalActions}>
          <button className={shared.btn} onClick={onClose}>
            キャンセル
          </button>
          <button
            className={`${shared.btn} ${shared.danger} ${shared.wipe}`}
            disabled={selected.size === 0}
            onClick={() => {
              onConfirm(Array.from(selected));
            }}
          >
            {String(selected.size)} 件を削除
          </button>
        </div>
      </div>
    </div>
  );
}
