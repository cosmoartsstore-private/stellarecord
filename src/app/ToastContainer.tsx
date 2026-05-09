import type { ToastItem } from '../shared/models/types';
import { StellaIcon, stellaIconNames } from '../shared/components/Icons';
import styles from './App.module.css';

interface ToastContainerProps {
  toasts: ToastItem[];
}

/** 画面下部に固定表示されるトースト通知スタックを描画する */
export function ToastContainer({ toasts }: ToastContainerProps) {
  return (
    <div className={styles.toastContainer}>
      {toasts.map((toast) => (
        <div key={toast.id} className={styles.toast}>
          <div className={styles.toastIcon}>
            <StellaIcon name={stellaIconNames.info} />
          </div>
          <div className={styles.toastMsg}>{toast.msg}</div>
        </div>
      ))}
    </div>
  );
}
