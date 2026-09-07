import { StellaIcon, stellaIconNames } from '../../../shared/components/Icons';
import { formatStorageMeter } from '../../../shared/lib/storageFormat';
import shared from '../../../shared/styles/shared.module.css';
import type { StorageStatus } from '../models/types';
import styles from './AnalyzeSection.module.css';

/** 解析ダッシュボードのProps — 状態は親コンポーネントが保持し、このコンポーネントは表示に専念する */
interface AnalyzeSectionProps {
  storageStatus: StorageStatus;
  isAnalyzeRunning: boolean;
  analyzeProgress: string;
  analyzeStatus: string;
  onRefreshStorage: () => void;
  onOpenEnhancedSync: () => void;
  onOpenLogViewer: () => void;
  onCancelSync: () => void;
  onOpenCleanup: () => void;
}

/** バックエンドの `完了件数/総件数` 表記を 0～100 の進捗率へ変換する。 */
function parseAnalyzeProgress(progress: string): number {
  const slash = progress.indexOf('/');
  if (slash === -1) return 0;
  const done = Number(progress.slice(0, slash));
  const total = Number(progress.slice(slash + 1));
  if (!Number.isFinite(done) || !Number.isFinite(total) || total <= 0) return 0;
  return Math.min(100, Math.max(0, Math.round((done / total) * 100)));
}

/** 解析メインダッシュボード — ストレージメーター・アーカイブ同期・ログビューア・クリーンアップ */
export function AnalyzeSection({
  storageStatus,
  isAnalyzeRunning,
  analyzeProgress,
  analyzeStatus,
  onRefreshStorage,
  onOpenEnhancedSync,
  onOpenLogViewer,
  onCancelSync,
  onOpenCleanup,
}: AnalyzeSectionProps) {
  // 上限が10GBを超える場合はGB表示に切り替えて可読性を確保
  const isGbMeterPreferred = storageStatus.limit >= 10 * 1024 * 1024 * 1024;
  const storagePercent = Number.isFinite(storageStatus.percent)
    ? Math.min(100, Math.max(0, storageStatus.percent))
    : 0;
  const analyzePercent = parseAnalyzeProgress(analyzeProgress);

  return (
    <div className={`${styles.root} ${shared.viewContainer}`}>
      <header className={styles.hero}>
        <h1 className={styles.heroTitle}>解析管理</h1>
      </header>

      <section className={`${shared.card} ${styles.primaryCard}`}>
        <div className={styles.homeSectionHead}>
          <h3 className={styles.sectionMiniTitle}>ストレージ管理</h3>
        </div>

        <div className={styles.dbStorageCard}>
          <div className={styles.storageHeader}>
            <div className={styles.storageTitle}>
              アーカイブ容量
              <button
                type="button"
                className={styles.btnRefreshStorage}
                onClick={onRefreshStorage}
                aria-label="アーカイブ容量を更新"
              >
                <StellaIcon name={stellaIconNames.refresh} />
              </button>
            </div>
            <div className={styles.storageStats}>
              {formatStorageMeter(storageStatus.current, isGbMeterPreferred)} /{' '}
              {formatStorageMeter(storageStatus.limit, isGbMeterPreferred)} (
              {storageStatus.percent.toFixed(1)}%)
            </div>
          </div>
          <div
            className={styles.storageTrack}
            role="progressbar"
            aria-label="アーカイブ容量"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={storagePercent}
            aria-valuetext={`${storageStatus.percent.toFixed(1)}% 使用中`}
          >
            <div
              className={`${styles.storageFill} ${storageStatus.percent > 90 ? styles.storageWarning : ''}`}
              // インライン width でアニメーションする進捗バーを制御する
              style={{ width: `${String(storagePercent)}%` }}
            />
          </div>
        </div>
      </section>

      <div className={shared.card}>
        <div className={styles.analyzeBlockHeader}>
          <div className={styles.analyzeBlockHeadCopy}>
            <h3 className={styles.sectionMiniTitle}>ログデータ取込・ビューア</h3>
          </div>
          <button
            type="button"
            className={`${shared.btn} ${shared.danger}`}
            onClick={onOpenCleanup}
          >
            元ログの削除
          </button>
        </div>

        <div className={styles.syncGrid}>
          <div className={styles.syncCard}>
            <h4>復元</h4>
            <p>圧縮済みログからデータを再復元します</p>
            <button
              type="button"
              className={`${shared.btn} ${shared.primary}`}
              style={{ width: '100%' }}
              onClick={onOpenEnhancedSync}
              disabled={isAnalyzeRunning}
            >
              ログを選択
            </button>
          </div>
          <div className={styles.syncCard}>
            <h4>ログビューア</h4>
            <p>圧縮済みログを閲覧します</p>
            <button
              type="button"
              className={shared.btn}
              style={{ width: '100%' }}
              onClick={onOpenLogViewer}
            >
              ログを開く
            </button>
          </div>
        </div>

        {/* 進捗パネル — analyzeRunning が true の間のみ表示 */}
        {isAnalyzeRunning && (
          <div className={styles.progressContainer}>
            <div className={styles.progressInfo}>
              <span>取り込み中…</span>
              <span>{analyzeProgress}</span>
            </div>
            <div
              className={styles.progressTrack}
              role="progressbar"
              aria-label="ログ取り込み"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={analyzePercent}
              aria-valuetext={analyzeProgress || '進捗を計算中'}
            >
              <div
                className={styles.progressFill}
                style={{ width: `${String(analyzePercent)}%` }}
              />
            </div>
            <div className={styles.progressStatusRow}>
              <p className={styles.sectionMiniCopy}>{analyzeStatus}</p>
              <button type="button" className={styles.progressCancel} onClick={onCancelSync}>
                停止
              </button>
            </div>
          </div>
        )}
        {!isAnalyzeRunning && analyzeStatus.length > 0 && (
          <div className={styles.progressContainer} aria-live="polite">
            <div className={styles.progressStatusRow}>
              <p className={styles.sectionMiniCopy}>
                {analyzeStatus}
                {analyzeProgress.length > 0 ? ` (${analyzeProgress})` : ''}
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
