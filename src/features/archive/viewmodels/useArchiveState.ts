import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { flushSync } from 'react-dom';
import type { ArchiveFileItem, LogViewerChunk, LogViewerData } from '../models/types';
import {
  launchEnhancedImport,
  launchStartupArchiveImport,
  loadArchiveFiles,
  pickLogFiles,
  startExternalLogViewerStream,
  startLogViewerStream,
} from '../services/archiveService';

/** 受信チャンクを蓄積データにマージする（純粋関数） */
function appendChunk(data: LogViewerData, chunk: LogViewerChunk): LogViewerData {
  return {
    ...data,
    timestamps: data.timestamps.concat(chunk.timestamps),
    levels: data.levels.concat(chunk.levels),
    categories: data.categories.concat(chunk.categories),
    raw_lines: data.raw_lines.concat(chunk.raw_lines),
    highlights: data.highlights.concat(chunk.highlights),
  };
}

/** 新規ストリーム用の空のLogViewerDataを生成する */
function emptyViewerData(archiveName: string): LogViewerData {
  return {
    archive_name: archiveName,
    source_name: '',
    timestamps: [],
    levels: [],
    categories: [],
    raw_lines: [],
    highlights: [],
  };
}

/**
 * アーカイブファイル一覧とストリーミングログビューアを管理する中核フック
 *
 * ストリーミング設計:
 * 1. openStreamForFile が一意のセッションIDでTauri IPCコマンドを送信
 * 2. Rust側が .tar.zst を解凍・パースし、セッションIDタグ付きで log_viewer_chunk を発行
 * 3. チャンクは pendingChunksRef にバッファされ、100msタイマーでReact状態に一括反映
 *    （チャンク毎の再レンダリングによるバーチャライザの性能劣化を防ぐため）
 * 4. log_viewer_done でストリーム完了を検知し、残バッファを即時フラッシュ
 * 5. セッションID照合により、ファイル切替時の旧ストリームイベントを破棄
 */
export function useArchiveState() {
  const [archiveFiles, setArchiveFiles] = useState<ArchiveFileItem[]>([]);
  const [logViewerData, setLogViewerData] = useState<LogViewerData | null>(null);
  const [isLogViewerLoaded, setIsLogViewerLoaded] = useState(false);
  /** ユーザーが選択した外部ログファイルの絶対パス一覧 */
  const [externalFiles, setExternalFiles] = useState<string[]>([]);

  /** 現在のTauriイベントリスナーの解除コールバック */
  const unlistenRef = useRef<(() => void) | null>(null);

  /**
   * 外部ファイルが選択されているかどうかを同期的に判定するための ref。
   * setState は次のレンダリングまで反映されないため、選択直後に同期実行される
   * openStreamForFile が古い状態で起動してしまうことを防ぐ。
   */
  const externalFilesRef = useRef<string[]>([]);
  const updateExternalFiles = useCallback((value: string[]) => {
    externalFilesRef.current = value;
    setExternalFiles(value);
  }, []);

  /** イベントリスナーを解除してチャンク受信を停止する */
  const stopStream = useCallback(() => {
    unlistenRef.current?.();
    unlistenRef.current = null;
  }, []);

  // フラッシュ間隔の間にチャンクを蓄積し、React更新を集約する
  const pendingChunksRef = useRef<LogViewerChunk[]>([]);
  const flushTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  /** 蓄積チャンクを1回のsetLogViewerDataでReact状態に反映する */
  const flushChunks = useCallback(() => {
    flushTimerRef.current = null;
    const chunks = pendingChunksRef.current.splice(0);
    if (chunks.length === 0) return;
    setLogViewerData((prev) => (prev ? chunks.reduce(appendChunk, prev) : prev));
  }, []);

  /** 指定ファイルのストリーミングビューアセッションを開始する */
  const openStreamForFile = useCallback(
    async (fileKey: string) => {
      stopStream();
      if (flushTimerRef.current) {
        clearTimeout(flushTimerRef.current);
        flushTimerRef.current = null;
      }
      pendingChunksRef.current = [];

      flushSync(() => {
        setLogViewerData(emptyViewerData(fileKey));
        setIsLogViewerLoaded(false);
      });

      const sessionId = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;

      const unlistenChunk = await listen<LogViewerChunk>('log_viewer_chunk', (e) => {
        if (e.payload.session_id !== sessionId) return;
        pendingChunksRef.current.push(e.payload);
        flushTimerRef.current ??= setTimeout(flushChunks, 100);
      });

      const unlistenDone = await listen<string>('log_viewer_done', (e) => {
        if (e.payload !== sessionId) return;
        if (flushTimerRef.current) {
          clearTimeout(flushTimerRef.current);
          flushTimerRef.current = null;
        }
        flushChunks();
        setIsLogViewerLoaded(true);
        stopStream();
      });

      unlistenRef.current = () => {
        unlistenChunk();
        unlistenDone();
      };

      const isExternal = externalFilesRef.current.includes(fileKey);
      const meta = isExternal
        ? await startExternalLogViewerStream(fileKey, sessionId)
        : await startLogViewerStream(fileKey, sessionId);
      setLogViewerData((prev) => (prev ? { ...prev, source_name: meta.source_name } : prev));
      return meta.archive_name;
    },
    [stopStream, flushChunks],
  );

  /** バックエンドからアーカイブファイル一覧を取得して状態に格納する */
  const loadArchiveSelection = useCallback(async () => {
    const files = await loadArchiveFiles();
    setArchiveFiles(files);
    return files;
  }, []);

  /** インポートモードでアーカイブ選択画面を開く */
  const openEnhancedSync = useCallback(() => loadArchiveSelection(), [loadArchiveSelection]);

  /** 閲覧モードでアーカイブ選択画面を開く（既定アーカイブストア） */
  const openLogViewerSelection = useCallback(async () => {
    updateExternalFiles([]);
    return loadArchiveSelection();
  }, [loadArchiveSelection, updateExternalFiles]);

  /**
   * ネイティブダイアログで外部ログファイルを選択し、一覧に追加する。
   * キャンセル時は null を返し、状態は変更しない。
   * 既に選択済みのファイルは重複追加しない。
   */
  const selectExternalLogFiles = useCallback(async (): Promise<string[] | null> => {
    const paths = await pickLogFiles();
    if (paths.length === 0) return null;
    const merged = [...externalFilesRef.current];
    for (const p of paths) {
      if (!merged.includes(p)) merged.push(p);
    }
    updateExternalFiles(merged);
    return merged;
  }, [updateExternalFiles]);

  /** 外部ファイル選択を解除して既定アーカイブストアに戻す */
  const clearExternalLogFiles = useCallback(async () => {
    updateExternalFiles([]);
    return loadArchiveSelection();
  }, [loadArchiveSelection, updateExternalFiles]);

  /** 選択されたアーカイブファイルのバッチインポートを実行する */
  const executeEnhancedSync = useCallback(async (selectedFiles: string[]) => {
    await launchEnhancedImport(selectedFiles);
  }, []);

  /** 指定ファイルのログビューアを開く */
  const openSelectedLogViewer = useCallback(
    (selectedFileKey: string) => openStreamForFile(selectedFileKey),
    [openStreamForFile],
  );

  /** ストリームを破棄してログビューアを閉じる */
  const closeLogViewer = useCallback(() => {
    stopStream();
    setLogViewerData(null);
    setIsLogViewerLoaded(false);
    updateExternalFiles([]);
  }, [stopStream, updateExternalFiles]);

  /** アプリ起動時の一回限りの自動取り込みを実行する */
  const runStartupImport = useCallback(async () => {
    await launchStartupArchiveImport();
  }, []);

  // アンマウント時にイベントリスナーとフラッシュタイマーをクリーンアップ
  useEffect(() => {
    return () => {
      stopStream();
      if (flushTimerRef.current) {
        clearTimeout(flushTimerRef.current);
        flushTimerRef.current = null;
      }
    };
  }, [stopStream]);

  // 空のビューアデータがあり読み込み未完了の場合のみローディング状態
  const isLogViewerLoading =
    logViewerData !== null && logViewerData.raw_lines.length === 0 && !isLogViewerLoaded;

  return {
    archiveFiles,
    logViewerData,
    isLogViewerLoading,
    isLogViewerLoaded,
    externalFiles,
    openEnhancedSync,
    openLogViewerSelection,
    executeEnhancedSync,
    openSelectedLogViewer,
    closeLogViewer,
    runStartupImport,
    selectExternalLogFiles,
    clearExternalLogFiles,
  };
}
