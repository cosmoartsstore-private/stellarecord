import type { ToastItem } from '../shared/models/types';
import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { StellaIcon, stellaIconNames } from '../shared/components/Icons';
import styles from './App.module.css';

interface ToastContainerProps {
  toasts: ToastItem[];
}

/** 画面下部に固定表示されるトースト通知スタックを描画する */
export function ToastContainer({ toasts }: ToastContainerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [dialogHost, setDialogHost] = useState<HTMLDialogElement | null>(null);

  useEffect(() => {
    const root = containerRef.current?.closest('.stella-record-root');
    if (!root) return;

    // modal外の要素はinertになるため、通知は開いている最前面dialog内へ置く。
    // ログ行の更新を監視せず、直下のdialog追加・削除とopen属性だけを追跡する。
    const updateHost = () => {
      observer.disconnect();
      observer.observe(root, { childList: true });
      const dialogs = [...root.querySelectorAll<HTMLDialogElement>('dialog')];
      for (const dialog of dialogs) observer.observe(dialog, { attributes: true, attributeFilter: ['open'] });
      setDialogHost(dialogs.filter((dialog) => dialog.open).at(-1) ?? null);
    };
    const observer = new MutationObserver(updateHost);
    updateHost();
    return () => { observer.disconnect(); };
  }, []);

  const content = (
    <div ref={containerRef} className={styles.toastContainer} role="status" aria-live="polite" aria-atomic="false" aria-relevant="additions">
      {toasts.map((toast) => (
        <div key={toast.id} className={styles.toast}>
          <div className={styles.toastIcon}><StellaIcon name={stellaIconNames.info} /></div>
          <div className={styles.toastMsg}>{toast.msg}</div>
        </div>
      ))}
    </div>
  );
  return dialogHost ? createPortal(content, dialogHost) : content;
}
