import { useCallback, useEffect, useRef, useState } from 'react';
import type { MouseEvent } from 'react';

/**
 * ファイル名のSet管理フック — Shift範囲選択・Ctrl個別トグル・ドラッグ選択に対応
 * インポートモードとビューアモードの両方で共用する
 */
export function useArchiveSelection(archiveFiles: string[]) {
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [lastSelected, setLastSelected] = useState<string | null>(null);

  /** マウスボタン押下中かどうか（ドラッグ選択用） */
  const isDraggingSelect = useRef(false);
  /** ドラッグ中の操作モード（追加 or 解除） */
  const dragMode = useRef<'select' | 'deselect'>('select');

  // リスト外でのmouseupも検知してドラッグ状態をリセット
  useEffect(() => {
    const handleMouseUp = () => {
      isDraggingSelect.current = false;
    };
    window.addEventListener('mouseup', handleMouseUp);
    return () => {
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }, []);

  /** 選択状態を完全にリセットする */
  const clearSelection = useCallback(() => {
    setSelectedFiles(new Set());
    setLastSelected(null);
    isDraggingSelect.current = false;
  }, []);

  /**
   * mousedown('down') / mouseenter('enter') の統合ハンドラ
   * Shift範囲選択・Ctrl個別トグル・プレーンクリック・ドラッグ選択を処理する
   */
  const handleFileAction = useCallback(
    (event: MouseEvent, file: string, type: 'down' | 'enter') => {
      if (type === 'down') {
        if (event.shiftKey && lastSelected) {
          const startIdx = archiveFiles.indexOf(lastSelected);
          const endIdx = archiveFiles.indexOf(file);
          if (startIdx !== -1 && endIdx !== -1) {
            const min = Math.min(startIdx, endIdx);
            const max = Math.max(startIdx, endIdx);
            const range = archiveFiles.slice(min, max + 1);
            setSelectedFiles((prev) => {
              const next = new Set(prev);
              for (const name of range) {
                next.add(name);
              }
              return next;
            });
          }
          return;
        }

        isDraggingSelect.current = true;
        if (event.ctrlKey || event.metaKey) {
          dragMode.current = selectedFiles.has(file) ? 'deselect' : 'select';
          setSelectedFiles((prev) => {
            const next = new Set(prev);
            if (dragMode.current === 'select') {
              next.add(file);
            } else {
              next.delete(file);
            }
            return next;
          });
        } else {
          if (selectedFiles.has(file)) {
            dragMode.current = 'deselect';
            setSelectedFiles((prev) => {
              const next = new Set(prev);
              next.delete(file);
              return next;
            });
          } else {
            dragMode.current = 'select';
            setSelectedFiles((prev) => {
              const next = new Set(prev);
              next.add(file);
              return next;
            });
          }
        }
        setLastSelected(file);
        return;
      }

      // ドラッグ中にカーソルが通過したアイテムを選択/解除
      if (isDraggingSelect.current) {
        setSelectedFiles((prev) => {
          const next = new Set(prev);
          if (dragMode.current === 'select') {
            next.add(file);
          } else {
            next.delete(file);
          }
          return next;
        });
        setLastSelected(file);
      }
    },
    [archiveFiles, lastSelected, selectedFiles],
  );

  /** 全選択と全解除をトグルする */
  const handleSelectAll = useCallback(() => {
    if (selectedFiles.size === archiveFiles.length) {
      clearSelection();
      return;
    }
    setSelectedFiles(new Set(archiveFiles));
  }, [archiveFiles, clearSelection, selectedFiles.size]);

  return {
    selectedFiles,
    clearSelection,
    handleFileAction,
    handleSelectAll,
  };
}
