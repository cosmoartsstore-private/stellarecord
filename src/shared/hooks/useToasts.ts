import { useCallback, useRef, useState } from 'react';
import type { ToastItem } from '../models/types';

/** 一時トースト通知の追加と自動削除を管理するフック */
export function useToasts() {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const nextId = useRef(0);

  /** トーストを追加し、duration ms 後に自動削除する */
  const addToast = useCallback((msg: string, duration = 3000) => {
    const id = ++nextId.current;
    setToasts((prev) => [...prev, { id, msg }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((toast) => toast.id !== id));
    }, duration);
  }, []);

  return { toasts, addToast };
}
