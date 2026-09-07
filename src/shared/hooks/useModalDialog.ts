import {
  useCallback,
  useEffect,
  useRef,
  type MouseEvent,
  type RefObject,
  type SyntheticEvent,
} from 'react';

interface ModalDialogOptions {
  onClose?: () => void;
  initialFocusRef?: RefObject<HTMLElement | null>;
  shouldCloseOnBackdrop?: boolean;
}

/**
 * ネイティブ dialog の表示、フォーカス移動、Escape、背景クリックを統一する。
 * onClose がない必須回答ダイアログでは Escape を抑止する。
 */
export function useModalDialog(options: ModalDialogOptions = {}) {
  const { onClose, initialFocusRef, shouldCloseOnBackdrop = false } = options;
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (!dialog.open) dialog.showModal();
    initialFocusRef?.current?.focus();

    return () => {
      if (dialog.open) dialog.close();
    };
  }, [initialFocusRef]);

  const closeDialog = useCallback(() => {
    if (dialogRef.current?.open) dialogRef.current.close();
    onClose?.();
  }, [onClose]);

  const handleCancel = useCallback(
    (event: SyntheticEvent<HTMLDialogElement>) => {
      event.preventDefault();
      if (onClose) closeDialog();
    },
    [closeDialog, onClose],
  );

  const handleBackdropMouseDown = useCallback(
    (event: MouseEvent<HTMLDialogElement>) => {
      if (shouldCloseOnBackdrop && event.target === event.currentTarget) closeDialog();
    },
    [closeDialog, shouldCloseOnBackdrop],
  );

  return { dialogRef, closeDialog, handleCancel, handleBackdropMouseDown };
}
