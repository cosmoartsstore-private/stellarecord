import { getMsg } from '../../../messages/getMsg';
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { renderHighlightedBody, splitLogLine } from '../models/logFormat';
import { parseArchiveDate } from '../models/archiveFormat';
import { formatFileSize } from '../../../shared/lib/byteFormat';
import { getFileNameFromPath } from '../../../shared/lib/pathUtils';
import { StellaIcon, stellaIconNames } from '../../../shared/components/Icons';
import { closeDialog, closeDialogOnBackdrop, useModalDialog } from '../../../shared/hooks/useModalDialog';
import type { ArchiveFileItem, LogViewerData } from '../models/types';
import shared from '../../../shared/styles/shared.module.css';
import styles from './LogViewerModal.module.css';

const zoomMin = 0.5;
const zoomMax = 2.5;

/** フィルタチップ定義 — クリックで仮想リストを該当行に絞り込む */
const CHIPS: { key: string; label: string; colorClass: string; matchKeys?: string[] }[] = [{ key: 'world', label: getMsg('LogViewer.worldCategory'), colorClass: styles.legendWorld }, { key: 'notification', label: getMsg('LogViewer.notificationCategory'), colorClass: styles.legendNotification }, { key: 'player', label: getMsg('LogViewer.playerCategory'), colorClass: styles.legendPlayerJoin, matchKeys: ['player-join', 'player-left'] }, { key: 'video', label: getMsg('LogViewer.videoCategory'), colorClass: styles.legendVideo }, { key: 'photo', label: getMsg('LogViewer.photoCategory'), colorClass: styles.legendPhoto }, { key: 'warning', label: getMsg('LogViewer.warningCategory'), colorClass: styles.legendWarning }, { key: 'error', label: getMsg('LogViewer.errorCategory'), colorClass: styles.legendError }, { key: 'debug', label: getMsg('LogViewer.debugCategory'), colorClass: styles.legendDebug }];

const categoryClassMap: Record<string, string> = { world: styles.categoryWorld, notification: styles.categoryNotification, 'player-join': styles.categoryPlayerJoin, 'player-left': styles.categoryPlayerLeft, video: styles.categoryVideo, photo: styles.categoryPhoto };

const levelClassMap: Record<string, string> = { error: styles.levelError, warning: styles.levelWarning, debug: styles.levelDebug, plain: styles.levelPlain, info: styles.levelInfo };
const zoomStep = 0.1;
const baseLineHeight = 22;
const unselectedViewerData: LogViewerData = { archive_name: '', source_name: '', timestamps: [], levels: [], categories: [], raw_lines: [], highlights: [] };

/** Backendの数値ログレベルをCSSクラスキー文字列にマッピング */
const levelKeys = ['plain', 'info', 'warning', 'error', 'debug'] as const;
/**
 * Backendの数値カテゴリを CSS クラスキー文字列にマッピング。
 * Rust 側 `encode_log_category_u8` の番号と必ず一致させること。
 */
const categoryKeys = [
  'plain',
  'world',
  'notification',
  'player-join',
  'player-left',
  'video',
  'photo',
] as const;

interface LogViewerModalProps {
  logViewerData: LogViewerData | null;
  archiveFiles: ArchiveFileItem[];
  /** ユーザーが選択した外部ログファイルの絶対パス一覧 */
  externalFiles: string[];
  isLoading: boolean;
  isLoaded: boolean;
  onNavigateToFile: (fileKey: string) => void;
  onPickExternalFiles: () => void;
  onClearExternalFiles: () => void;
  onClose: () => void;
}

/** 検索は表示分類に依存せず、読み込み済みのファイル全体を明示操作で調べる。 */
function LogSearchControls({ rawLines, isLoaded, dialogRef, onJump }: {
  rawLines: string[];
  isLoaded: boolean;
  dialogRef: React.RefObject<HTMLDialogElement | null>;
  onJump: (index: number) => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [searchText, setSearchText] = useState('');
  const [result, setResult] = useState<{ indices: number[]; position: number } | null>(null);
  const hasMatches = (result?.indices.length ?? 0) > 0;

  const clearSearch = useCallback(() => {
    setSearchText('');
    setResult(null);
    inputRef.current?.focus();
  }, []);

  const findMatch = useCallback((direction: number) => {
    if (!isLoaded) return;
    if (result) {
      if (result.indices.length === 0) return;
      const position = (result.position + direction + result.indices.length) % result.indices.length;
      const index = result.indices.at(position);
      if (index === undefined) return;
      setResult({ ...result, position });
      onJump(index);
      return;
    }
    const query = searchText.trim().toLowerCase();
    if (!query) return;
    const indices: number[] = [];
    for (const [index, line] of rawLines.entries()) {
      if (line.toLowerCase().includes(query)) indices.push(index);
    }
    const position = direction < 0 ? Math.max(indices.length - 1, 0) : 0;
    setResult({ indices, position });
    const index = indices.at(position);
    if (index !== undefined) onJump(index);
  }, [isLoaded, result, searchText, rawLines, onJump]);

  // ブラウザ標準の検索は仮想リストの画面外を検索できないため、ダイアログ内で置き換える。
  useEffect(() => {
    const dialog = dialogRef.current;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.isComposing) return;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'f') {
        event.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      } else if (event.key === 'F3' || (event.key === 'Enter' && event.shiftKey && event.target === inputRef.current)) {
        event.preventDefault();
        findMatch(event.shiftKey ? -1 : 1);
      } else if (event.key === 'Escape' && event.target === inputRef.current && (searchText || result)) {
        event.preventDefault();
        clearSearch();
      }
    };
    dialog?.addEventListener('keydown', handleKeyDown);
    return () => { dialog?.removeEventListener('keydown', handleKeyDown); };
  }, [dialogRef, findMatch, clearSearch, searchText, result]);

  const status = result
    ? hasMatches
      ? getMsg('LogViewer.searchPosition', { current: result.position + 1, total: result.indices.length })
      : getMsg('LogViewer.searchEmpty')
    : '';

  return (
    <form className={styles.searchBar} role="search" onSubmit={(event) => { event.preventDefault(); findMatch(1); }}>
      <input ref={inputRef} id="log-viewer-search" type="search" className={styles.searchInput} value={searchText} aria-label={getMsg('LogViewer.searchLabel')} placeholder={getMsg('LogViewer.searchPlaceholder')} autoComplete="off" spellCheck={false} disabled={!isLoaded} aria-describedby="log-viewer-search-status" onChange={(event) => { setSearchText(event.target.value); setResult(null); }} />
      <span id="log-viewer-search-status" className={styles.searchStatus} role="status">{status}</span>
      <button type="submit" className={styles.searchButton} disabled={!isLoaded || !searchText.trim()} aria-label={getMsg('LogViewer.searchSubmit')}><StellaIcon name={stellaIconNames.search} className={styles.searchIcon} aria-hidden="true" /></button>
    </form>
  );
}

/**
 * サイドバーナビ・カテゴリ/ログレベルフィルタ付きのフルスクリーンログビューアモーダル
 * 10万行超のレンダリングに @tanstack/react-virtual を使用
 */
export function LogViewerModal({ logViewerData: selectedData, archiveFiles, externalFiles, isLoading, isLoaded, onNavigateToFile, onPickExternalFiles, onClearExternalFiles, onClose }: LogViewerModalProps) {
  // ファイル未選択・開始失敗時も、同じダイアログ内で別のログを選べるようにする。
  const logViewerData = selectedData ?? unselectedViewerData;
  const listRef = useRef<HTMLDivElement>(null);
  const [activeFilters, setActiveFilters] = useState<string[]>([]);
  const [zoomLevel, setZoomLevel] = useState(1);
  const [jumpTarget, setJumpTarget] = useState<{ archiveName: string; index: number } | null>(null);
  const dialogRef = useModalDialog();

  const hasExternalFiles = externalFiles.length > 0;
  const hasNoLogs = !selectedData && archiveFiles.length === 0 && !hasExternalFiles;

  // ファイル切替時にスクロール位置のみリセット（フィルタは維持）
  useLayoutEffect(() => {
    if (listRef.current) {
      listRef.current.scrollTop = 0;
      listRef.current.scrollLeft = 0;
    }
  }, [logViewerData.archive_name]);

  // フィルタ変更時にスクロールをリセットして結果の先頭から表示
  useLayoutEffect(() => {
    if (listRef.current) {
      listRef.current.scrollTop = 0;
      listRef.current.scrollLeft = 0;
    }
  }, [activeFilters]);

  // Ctrl+ホイールでログリストをズーム
  const handleWheel = useCallback((e: React.WheelEvent<HTMLDivElement>) => {
    if (!selectedData || !e.ctrlKey) return;
    e.preventDefault();
    setZoomLevel((prev) => {
      const next = prev + (e.deltaY < 0 ? zoomStep : -zoomStep);
      return Math.round(Math.min(zoomMax, Math.max(zoomMin, next)) * 100) / 100;
    });
  }, [selectedData]);

  // 選択した分類のいずれかに一致する行を表示する。入退室は参加・退出の両方を含む。
  const activeMatchKeys = useMemo(() => {
    if (activeFilters.length === 0) return null;
    return new Set(CHIPS.filter((chip) => activeFilters.includes(chip.key)).flatMap((chip) => chip.matchKeys ?? [chip.key]));
  }, [activeFilters]);

  // フィルタに一致する行インデックスのサブセットを事前計算
  const filteredIndices = useMemo(() => {
    if (!activeMatchKeys) return null;
    const indices: number[] = [];
    for (let i = 0; i < logViewerData.raw_lines.length; i++) {
      const levelKey = levelKeys[logViewerData.levels[i] ?? 0] ?? 'plain';
      const categoryKey = categoryKeys[logViewerData.categories[i] ?? 0] ?? 'plain';
      if (activeMatchKeys.has(levelKey) || activeMatchKeys.has(categoryKey)) {
        indices.push(i);
      }
    }
    return indices;
  }, [activeMatchKeys, logViewerData]);

  const displayCount = filteredIndices ? filteredIndices.length : logViewerData.raw_lines.length;

  const estimatedLineHeight = Math.round(baseLineHeight * zoomLevel);
  const getLineKey = useCallback((index: number) => filteredIndices?.[index] ?? index, [filteredIndices]);

  const virtualizer = useVirtualizer({ count: displayCount, getScrollElement: () => listRef.current, getItemKey: getLineKey, estimateSize: () => estimatedLineHeight, overscan: 10 });

  const jumpToSearchMatch = useCallback((index: number) => {
    setActiveFilters([]);
    setJumpTarget({ archiveName: logViewerData.archive_name, index });
  }, [logViewerData.archive_name]);

  // 分類解除後の全件数を仮想リストへ反映してから、元の行番号へ移動する。
  useLayoutEffect(() => {
    if (jumpTarget?.archiveName !== logViewerData.archive_name) return;
    listRef.current?.scrollTo({ left: 0 });
    virtualizer.scrollToIndex(jumpTarget.index, { align: 'center' });
  }, [jumpTarget, logViewerData.archive_name, virtualizer]);

  // ズーム変更時に仮想レイアウトを再計算
  useEffect(() => {
    virtualizer.measure();
  }, [zoomLevel, virtualizer]);

  const zoomFontSize = `${(0.8 * zoomLevel).toFixed(2)}rem`;

  /** カテゴリ/ログレベル色分けとキーワードハイライト付きで1行を描画する */
  const renderLine = (i: number, key: string | number, extraStyle?: React.CSSProperties) => {
    const rawLine = logViewerData.raw_lines[i] ?? '';
    const levelKey = levelKeys[logViewerData.levels[i] ?? 0] ?? 'plain';
    const categoryKey = categoryKeys[logViewerData.categories[i] ?? 0] ?? 'plain';
    const highlight = logViewerData.highlights[i] ?? null;
    const parts = splitLogLine(rawLine);
    // 動画の指定者名は色付けせず、URL・タイトルだけを強調する。
    const bodyClassName = categoryKey === 'video' && highlight ? undefined : categoryClassMap[categoryKey];
    const highlightClassName = categoryKey === 'video' ? styles.videoValue : styles.highlight;
    const isTarget = jumpTarget?.archiveName === logViewerData.archive_name && jumpTarget.index === i;
    return (
      <div key={key} style={{ ...extraStyle, fontSize: zoomFontSize }} className={`${styles.line} ${isTarget ? styles.lineTarget : ''}`}>
        {parts ? (
          <>{parts.prefix}<span className={levelClassMap[levelKey]}>{parts.level}</span>{parts.separator}<span className={bodyClassName}>{renderHighlightedBody(parts.body, highlight, highlightClassName, styles.identifier)}</span></>
        ) : (
          <span className={styles.levelDebug}>{renderHighlightedBody(rawLine, highlight, highlightClassName, styles.identifier)}</span>
        )}
      </div>
    );
  };

  /** サイドバーに表示するファイル一覧（アーカイブ + 外部ファイル統合） */
  const sidebarFiles = hasExternalFiles ? externalFiles : archiveFiles.map((f) => f.name);
  const sidebarCount = sidebarFiles.length + (hasExternalFiles ? archiveFiles.length : 0);

  return (
    <dialog ref={dialogRef} className={`${styles.root} ${shared.modalOverlay} ${shared.fullscreen}`} aria-labelledby="log-viewer-title" aria-describedby="log-viewer-source" onClose={onClose} onMouseDown={closeDialogOnBackdrop}>
      <div className={`${styles.content} ${shared.modalContent}`}>
        {/* ── サイドバー ── */}
        <aside className={styles.sidebar}>
          <div className={styles.sidebarHeader}>
            <span className={styles.sidebarTitle}>{getMsg('LogViewer.filesHeading')}</span>
            <span className={styles.sidebarCount}>{sidebarCount}{getMsg('LogViewer.fileCountSuffix')}</span>
          </div>
          <div className={styles.sidebarFolderSwitcher}>
            <button type="button" className={styles.sidebarFolderButton} onClick={onPickExternalFiles} disabled={isLoading}>{getMsg('LogViewer.selectFile')}</button>
            {hasExternalFiles && (
              <button type="button" className={styles.sidebarFolderButton} onClick={onClearExternalFiles} disabled={isLoading}>{getMsg('LogViewer.clearSelection')}</button>
            )}
          </div>
          <div className={styles.sidebarList}>
            {hasExternalFiles && (
              <>
                <div className={styles.sidebarDivider}>{getMsg('LogViewer.selectedFiles')}</div>
                {externalFiles.map((filePath) => {
                  const isActive = filePath === logViewerData.archive_name;
                  const name = getFileNameFromPath(filePath);
                  const date = parseArchiveDate(name);
                  return (
                    <button key={filePath} type="button" className={`${styles.sidebarItem} ${isActive ? styles.sidebarItemActive : ''}`} onClick={() => {
                        if (!isActive && !isLoading) onNavigateToFile(filePath);
                      }} disabled={isLoading && !isActive} aria-current={isActive ? 'true' : undefined}>
                      <span className={styles.sidebarItemDate}>{date ?? name}</span>
                    </button>
                  );
                })}
                <div className={styles.sidebarDivider}>{getMsg('LogViewer.archivesHeading')}</div>
              </>
            )}
            {archiveFiles.length === 0 && !hasExternalFiles ? (
              <div className={styles.sidebarEmpty}>{getMsg('LogViewer.archiveEmpty')}</div>
            ) : (
              archiveFiles.map((file) => {
                const isActive = file.name === logViewerData.archive_name;
                const date = parseArchiveDate(file.name);
                return (
                  <button key={file.name} type="button" className={`${styles.sidebarItem} ${isActive ? styles.sidebarItemActive : ''}`} onClick={() => {
                      if (!isActive && !isLoading) onNavigateToFile(file.name);
                    }} disabled={isLoading && !isActive} aria-current={isActive ? 'true' : undefined}>
                    <span className={styles.sidebarItemDate}>{date ?? file.name}</span>
                    <span className={styles.sidebarItemSize}>{formatFileSize(file.size_bytes)}</span>
                  </button>
                );
              })
            )}
          </div>
        </aside>

        {/* ── メインコンテンツ ── */}
        <div className={styles.main}>
          <div className={styles.mainHeader}>
            <div className={styles.mainHeaderCopy}>
              <h3 id="log-viewer-title" className={styles.mainTitle} tabIndex={-1} autoFocus>{getMsg('common.logViewer')}</h3>
              <p id="log-viewer-source" className={styles.mainSub}>{logViewerData.source_name}</p>
            </div>
            <div className={styles.mainHeaderMeta}>
              <span className={`${styles.lineCount} ${selectedData && !isLoaded ? styles.lineCountLoading : ''}`}>
                {activeFilters.length > 0
                  ? getMsg('LogViewer.filteredLineCount', { filtered: String(filteredIndices?.length ?? 0), total: String(logViewerData.raw_lines.length) })
                  : getMsg('LogViewer.lineCount', { count: String(logViewerData.raw_lines.length) })}
              </span>
              {zoomLevel !== 1 && (
                <span className={styles.zoomBadge}>{Math.round(zoomLevel * 100)}%</span>
              )}
            </div>
          </div>

          <div className={styles.legend}>
            {CHIPS.map((chip) => (
              <button key={chip.key} type="button" className={`${styles.legendItem} ${chip.colorClass} ${activeFilters.includes(chip.key) ? styles.legendItemActive : ''}`} disabled={!selectedData} aria-pressed={activeFilters.includes(chip.key)} onClick={() => {
                  setActiveFilters((prev) => prev.includes(chip.key) ? prev.filter((key) => key !== chip.key) : [...prev, chip.key]);
                }}>
                {chip.label}
              </button>
            ))}
            <button type="button" className={styles.filterClear} disabled={!selectedData || activeFilters.length === 0} aria-label={getMsg('LogViewer.clearFiltersLabel')} onClick={() => { setActiveFilters([]); }}>{getMsg('LogViewer.clearFilters')}</button>
          </div>

          <LogSearchControls key={logViewerData.archive_name} rawLines={logViewerData.raw_lines} isLoaded={selectedData !== null && isLoaded} dialogRef={dialogRef} onJump={jumpToSearchMatch} />

          <div className={styles.logList} ref={listRef} onWheel={handleWheel}>
            {!selectedData && <p className={styles.logEmpty} role="status">{getMsg(hasNoLogs ? 'LogViewer.logsEmpty' : 'LogViewer.selectLog')}</p>}
            {isLoading && (
              <div className={styles.loadingOverlay}>
                <div className={styles.loadingSpinner} />
                <span className={styles.loadingText}>{getMsg('common.loadingAscii')}</span>
              </div>
            )}

            <div style={{ height: `${String(virtualizer.getTotalSize())}px`,
                minWidth: '100%',
                position: 'relative', }}>
              {virtualizer.getVirtualItems().map((virtualItem) => {
                const actualIndex = filteredIndices
                  ? (filteredIndices[virtualItem.index] ?? virtualItem.index)
                  : virtualItem.index;
                return renderLine(actualIndex, actualIndex, { position: 'absolute', top: 0, left: 0, minWidth: '100%', width: 'max-content', transform: `translateY(${String(virtualItem.start)}px)`, });
              })}
            </div>
          </div>

          <div className={shared.modalActions}>
            <button type="button" className={shared.btn} onClick={closeDialog}>{getMsg(hasNoLogs ? 'common.cancel' : 'common.close')}</button>
          </div>
        </div>
      </div>
    </dialog>
  );
}
