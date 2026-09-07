import { useEffect, useRef, type MouseEvent, type SyntheticEvent } from 'react';

/**
 * ネイティブ dialog をモーダルとして表示する。
 * 初期フォーカスは各ダイアログの autoFocus 属性で指定する。
 */
export function useModalDialog() {
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    if (!dialogRef.current?.open) dialogRef.current?.showModal();
    document.getElementById(dialogRef.current?.getAttribute('aria-labelledby') ?? '')?.focus();
  }, []);

  return dialogRef;
}

/** 操作元を含む最寄りの dialog を閉じ、ブラウザ標準のフォーカス復元を実行する。 */
export function closeDialog(event: MouseEvent<HTMLElement>) {
  event.currentTarget.closest('dialog')?.close();
}

/** dialog 本体の背景が押された場合だけ閉じる。 */
export function closeDialogOnBackdrop(event: MouseEvent<HTMLDialogElement>) {
  if (event.target === event.currentTarget) event.currentTarget.close();
}

/** 必須回答ダイアログの Escape によるキャンセルを抑止する。 */
export function preventDialogCancel(event: SyntheticEvent<HTMLDialogElement>) {
  event.preventDefault();
}
