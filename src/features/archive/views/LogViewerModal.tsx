import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { renderHighlightedBody } from '../models/logFormat';
import { formatArchiveSize, parseArchiveDate } from '../models/archiveFormat';
import type { ArchiveFileItem, LogViewerData } from '../models/types';
import shared from '../../../shared/styles/shared.module.css';
import styles from './LogViewerModal.module.css';

const ZOOM_MIN = 0.5;
const ZOOM_MAX = 2.5;

/** フィルタチップ定義 — クリックで仮想リストを該当行に絞り込む */
const CHIPS: { key: string; label: string; colorClass: string; matchKeys?: string[] }[] = [
  { key: 'world',        label: 'ワールド', colorClass: styles.legendWorld },
  { key: 'notification', label: '通知',     colorClass: styles.legendNotification },
  { key: 'player',       label: '入退室',   colorClass: styles.legendPlayerJoin, matchKeys: ['player-join', 'player-ready', 'player-left'] },
  { key: 'video',        label: '動画',     colorClass: styles.legendVideo },
  { key: 'warning',      label: '警告',     colorClass: styles.legendWarning },
  { key: 'error',        label: 'エラー',   colorClass: styles.legendError },
  { key: 'debug',        label: 'デバッグ', colorClass: styles.legendDebug },
];

const CATEGORY_CLASS_MAP: Record<string, string> = {
  world: styles.categoryWorld,
  notification: styles.categoryNotification,
  'player-join': styles.categoryPlayerJoin,
  'player-ready': styles.categoryPlayerReady,
  'player-left': styles.categoryPlayerLeft,
  video: styles.categoryVideo,
  'debug-system': styles.categoryDebugSystem,
};

const LEVEL_CLASS_MAP: Record<string, string> = {
  error: styles.levelError,
  warning: styles.levelWarning,
  debug: styles.levelDebug,
  plain: styles.levelPlain,
  info: styles.levelInfo,
};
const ZOOM_STEP = 0.1;
const BASE_LINE_HEIGHT = 22;

/** バックエンドの数値レベルをCSSクラスキー文字列にマッピング */
const LEVEL_KEYS = ['plain', 'info', 'warning', 'error', 'debug'] as const;
/**
 * バックエンドの数値カテゴリを CSS クラスキー文字列にマッピング。
 * Rust 側 `encode_log_category_u8` の番号と必ず一致させること。
 * DB 由来カテゴリ（world / notification / player_* / video）と、複数行範囲を
 * 表すレベル由来カテゴリ debug-system のみを持つ。
 */
const CATEGORY_KEYS = [
  'plain', 'world', 'notification',
  'player-join', 'player-ready', 'player-left', 'video',
  'debug-system',
] as const;

interface LogViewerModalProps {
  logViewerData: LogViewerData;
  archiveFiles: ArchiveFileItem[];
  /** 外部フォルダ閲覧中はその絶対パス、既定アーカイブストア閲覧中は null */
  externalFolderPath: string | null;
  isLoading: boolean;
  isLoaded: boolean;
  onNavigateToFile: (fileName: string) => void;
  onPickExternalFolder: () => void;
  onClearExternalFolder: () => void;
  onClose: () => void;
}

/**
 * サイドバーナビ・カテゴリ/レベルフィルタ付きのフルスクリーンログビューアモーダル
 * 10万行超のレンダリングに @tanstack/react-virtual を使用
 */
export function LogViewerModal({
  logViewerData,
  archiveFiles,
  externalFolderPath,
  isLoading,
  isLoaded,
  onNavigateToFile,
  onPickExternalFolder,
  onClearExternalFolder,
  onClose,
}: LogViewerModalProps) {
  const listRef = useRef<HTMLDivElement>(null);
  const [activeFilter, setActiveFilter] = useState<string | null>(null);
  const [zoomLevel, setZoomLevel] = useState(1);

  // ファイル切替時にフィルタとスクロール位置をリセット
  useEffect(() => {
    setActiveFilter(null);
    if (listRef.current) {
      listRef.current.scrollTop = 0;
      listRef.current.scrollLeft = 0;
    }
  }, [logViewerData.archive_name]);

  // フィルタ変更時にスクロールをリセットして結果の先頭から表示
  useEffect(() => {
    if (listRef.current) {
      listRef.current.scrollTop = 0;
      listRef.current.scrollLeft = 0;
    }
  }, [activeFilter]);

  // Ctrl+ホイールでログリストをズーム
  const handleWheel = useCallback((e: React.WheelEvent<HTMLDivElement>) => {
    if (!e.ctrlKey) return;
    e.preventDefault();
    setZoomLevel((prev) => {
      const next = prev + (e.deltaY < 0 ? ZOOM_STEP : -ZOOM_STEP);
      return Math.round(Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, next)) * 100) / 100;
    });
  }, []);

  // アクティブチップのマッチキーを解決（「入退室」のような複合カテゴリチップに対応）
  const activeMatchKeys = useMemo(() => {
    if (!activeFilter) return null;
    const chip = CHIPS.find((c) => c.key === activeFilter);
    return chip?.matchKeys ?? [activeFilter];
  }, [activeFilter]);

  // フィルタに一致する行インデックスのサブセットを事前計算
  // フィルタ未適用時はnull（全行表示）
  const filteredIndices = useMemo(() => {
    if (!activeMatchKeys) return null;
    const indices: number[] = [];
    for (let i = 0; i < logViewerData.raw_lines.length; i++) {
      const levelKey = LEVEL_KEYS[logViewerData.levels[i] ?? 0] ?? 'plain';
      const categoryKey = CATEGORY_KEYS[logViewerData.categories[i] ?? 0] ?? 'plain';
      if (activeMatchKeys.includes(levelKey) || activeMatchKeys.includes(categoryKey)) {
        indices.push(i);
      }
    }
    return indices;
  }, [activeMatchKeys, logViewerData]);

  const displayCount = filteredIndices ? filteredIndices.length : logViewerData.raw_lines.length;

  const estimatedLineHeight = Math.round(BASE_LINE_HEIGHT * zoomLevel);

  const virtualizer = useVirtualizer({
    count: displayCount,
    getScrollElement: () => listRef.current,
    estimateSize: () => estimatedLineHeight,
    overscan: 10,
  });

  // ズーム変更時に仮想レイアウトを再計算
  useEffect(() => {
    virtualizer.measure();
  }, [zoomLevel, virtualizer]);


  const zoomFontSize = `${(0.8 * zoomLevel).toFixed(2)}rem`;

  /** カテゴリ/レベル色分けとキーワードハイライト付きで1行を描画する */
  const renderLine = (i: number, key: string | number, extraStyle?: React.CSSProperties) => {
    const rawLine = logViewerData.raw_lines[i] ?? '';
    const levelKey = LEVEL_KEYS[logViewerData.levels[i] ?? 0] ?? 'plain';
    const categoryKey = CATEGORY_KEYS[logViewerData.categories[i] ?? 0] ?? 'plain';
    const highlight = logViewerData.highlights[i] ?? null;
    return (
      <div
        key={key}
        style={{ ...extraStyle, fontSize: zoomFontSize }}
        className={`${styles.line} ${CATEGORY_CLASS_MAP[categoryKey] ?? ''} ${LEVEL_CLASS_MAP[levelKey] ?? ''}`}
      >
        {renderHighlightedBody(rawLine, highlight, styles.highlight)}
      </div>
    );
  };

  return (
    <div className={`${styles.root} ${shared.modalOverlay} ${shared.fullscreen}`}>
      <div className={`${styles.content} ${shared.modalContent}`}>

        {/* ── サイドバー ── */}
        <aside className={styles.sidebar}>
          <div className={styles.sidebarHeader}>
            <span className={styles.sidebarTitle}>
              {externalFolderPath ? '外部フォルダ' : 'ログファイル'}
            </span>
            <span className={styles.sidebarCount}>{archiveFiles.length} 件</span>
          </div>
          <div className={styles.sidebarFolderSwitcher}>
            {externalFolderPath ? (
              <>
                <span
                  className={styles.sidebarFolderPath}
                  title={externalFolderPath}
                >
                  {externalFolderPath}
                </span>
                <button
                  type="button"
                  className={styles.sidebarFolderButton}
                  onClick={onClearExternalFolder}
                  disabled={isLoading}
                >
                  既定フォルダに戻す
                </button>
              </>
            ) : (
              <button
                type="button"
                className={styles.sidebarFolderButton}
                onClick={onPickExternalFolder}
                disabled={isLoading}
              >
                別のフォルダから開く
              </button>
            )}
          </div>
          <div className={styles.sidebarList}>
            {archiveFiles.length === 0 ? (
              <div className={styles.sidebarEmpty}>
                {externalFolderPath
                  ? 'output_log_*.txt / .tar.zst が見つかりません'
                  : 'アーカイブがありません'}
              </div>
            ) : (
              archiveFiles.map((file) => {
                const isActive = file.name === logViewerData.archive_name;
                const date = parseArchiveDate(file.name);
                return (
                  <button
                    key={file.name}
                    type="button"
                    className={`${styles.sidebarItem} ${isActive ? styles.sidebarItemActive : ''}`}
                    onClick={() => {
                      if (!isActive && !isLoading) onNavigateToFile(file.name);
                    }}
                    disabled={isLoading && !isActive}
                  >
                    <span className={styles.sidebarItemDate}>{date ?? file.name}</span>
                    <span className={styles.sidebarItemSize}>{formatArchiveSize(file.size_bytes)}</span>
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
              <h3 className={styles.mainTitle}>ログビューア</h3>
              <p className={styles.mainSub}>{logViewerData.source_name}</p>
            </div>
            <div className={styles.mainHeaderMeta}>
              <span className={`${styles.lineCount} ${!isLoaded ? styles.lineCountLoading : ''}`}>
                {activeFilter
                  ? `${String(filteredIndices?.length ?? 0)} / ${String(logViewerData.raw_lines.length)} 行`
                  : `${String(logViewerData.raw_lines.length)} 行`}
              </span>
              {zoomLevel !== 1 && (
                <span className={styles.zoomBadge}>{Math.round(zoomLevel * 100)}%</span>
              )}
            </div>
          </div>

          <div className={styles.legend}>
            {CHIPS.map((chip) => (
              <button
                key={chip.key}
                type="button"
                className={`${styles.legendItem} ${chip.colorClass} ${activeFilter === chip.key ? styles.legendItemActive : ''}`}
                onClick={() => {
                  setActiveFilter((prev) => (prev === chip.key ? null : chip.key));
                }}
              >
                {chip.label}
              </button>
            ))}
          </div>

          <div className={styles.logList} ref={listRef} onWheel={handleWheel}>
            {isLoading && (
              <div className={styles.loadingOverlay}>
                <div className={styles.loadingSpinner} />
                <span className={styles.loadingText}>読み込み中...</span>
              </div>
            )}

            <div style={{ height: `${String(virtualizer.getTotalSize())}px`, minWidth: '100%', position: 'relative' }}>
              {virtualizer.getVirtualItems().map((virtualItem) => {
                // フィルタ適用時は仮想リストの連番を実際の行インデックスに変換
                const actualIndex = filteredIndices
                  ? (filteredIndices[virtualItem.index] ?? virtualItem.index)
                  : virtualItem.index;
                return renderLine(actualIndex, virtualItem.index, {
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  minWidth: '100%',
                  width: 'max-content',
                  transform: `translateY(${String(virtualItem.start)}px)`,
                });
              })}
            </div>
          </div>

          <div className={shared.modalActions}>
            <button className={shared.btn} onClick={onClose}>閉じる</button>
          </div>
        </div>

      </div>
    </div>
  );
}
