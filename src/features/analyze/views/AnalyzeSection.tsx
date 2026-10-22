import { getMsg } from '../../../messages/getMsg';
import { byteUnitBytes } from '../../../shared/constants';
import { StellaIcon, stellaIconNames } from '../../../shared/components/Icons';
import { formatStorageMeter } from '../../../shared/lib/storageFormat';
import shared from '../../../shared/styles/shared.module.css';
import type { StorageStatus } from '../models/types';
import styles from './AnalyzeSection.module.css';

const gbMeterThresholdBytes = 10 * byteUnitBytes.GB;

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

/** Backendの `完了件数/総件数` 表記を 0～100 の進捗率へ変換する。 */
function parseAnalyzeProgress(progress: string): number {
  const slash = progress.indexOf('/');
  if (slash === -1) return 0;
  const done = Number(progress.slice(0, slash));
  const total = Number(progress.slice(slash + 1));
  if (!Number.isFinite(done) || !Number.isFinite(total) || total <= 0) return 0;
  return Math.min(100, Math.max(0, Math.round((done / total) * 100)));
}

/** 解析メインダッシュボード — ストレージメーター・アーカイブ同期・ログビューア・クリーンアップ */
export function AnalyzeSection({ storageStatus, isAnalyzeRunning, analyzeProgress, analyzeStatus, onRefreshStorage, onOpenEnhancedSync, onOpenLogViewer, onCancelSync, onOpenCleanup }: AnalyzeSectionProps) {
  // 上限が10GBを超える場合はGB表示に切り替えて可読性を確保
  const isGbMeterPreferred = storageStatus.limit >= gbMeterThresholdBytes;
  const storagePercent = Number.isFinite(storageStatus.percent)
    ? Math.min(100, Math.max(0, storageStatus.percent))
    : 0;
  const analyzePercent = parseAnalyzeProgress(analyzeProgress);

  return (
    <div className={`${styles.root} ${shared.viewContainer}`}>
      <header className={styles.hero}>
        <h1 className={styles.heroTitle}>{getMsg('AnalyzeSection.title')}</h1>
      </header>

      <section className={`${shared.card} ${styles.primaryCard}`}>
        <div className={styles.homeSectionHead}>
          <h3 className={styles.sectionMiniTitle}>{getMsg('AnalyzeSection.storageHeading')}</h3>
        </div>

        <div className={styles.dbStorageCard}>
          <div className={styles.storageHeader}>
            <div className={styles.storageTitle}>{getMsg('AnalyzeSection.archiveCapacity')}<button type="button" className={styles.btnRefreshStorage} onClick={onRefreshStorage} aria-label={getMsg('AnalyzeSection.refreshArchiveCapacity')}><StellaIcon name={stellaIconNames.refresh} /></button>
            </div>
            <div className={styles.storageStats}>
              {formatStorageMeter(storageStatus.current, isGbMeterPreferred)} /{' '}
              {formatStorageMeter(storageStatus.limit, isGbMeterPreferred)} (
              {storageStatus.percent.toFixed(1)}%)
            </div>
          </div>
          <div className={styles.storageTrack} role="progressbar" aria-label={getMsg('AnalyzeSection.archiveCapacity')} aria-valuemin={0} aria-valuemax={100} aria-valuenow={storagePercent} aria-valuetext={getMsg('AnalyzeSection.capacityUsage', { percent: storageStatus.percent.toFixed(1) })}>
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
            <h3 className={styles.sectionMiniTitle}>{getMsg('AnalyzeSection.logsHeading')}</h3>
          </div>
          <button type="button" className={`${shared.btn} ${shared.danger}`} onClick={onOpenCleanup}>{getMsg('AnalyzeSection.deleteSourceLogs')}</button>
        </div>

        <div className={styles.syncGrid}>
          <div className={styles.syncCard}>
            <h4>{getMsg('common.restore')}</h4>
            <p>{getMsg('AnalyzeSection.restoreDescription')}</p>
            <button type="button" className={`${shared.btn} ${shared.primary}`} style={{ width: '100%' }} onClick={onOpenEnhancedSync} disabled={isAnalyzeRunning}>{getMsg('AnalyzeSection.selectLogs')}</button>
          </div>
          <div className={styles.syncCard}>
            <h4>{getMsg('common.logViewer')}</h4>
            <p>{getMsg('AnalyzeSection.viewerDescription')}</p>
            <button type="button" className={shared.btn} style={{ width: '100%' }} onClick={onOpenLogViewer}>{getMsg('AnalyzeSection.openLog')}</button>
          </div>
        </div>

        {/* 進捗パネル — analyzeRunning が true の間のみ表示 */}
        {isAnalyzeRunning && (
          <div className={styles.progressContainer}>
            <div className={styles.progressInfo}>
              <span>{getMsg('AnalyzeSection.importing')}</span>
              <span>{analyzeProgress}</span>
            </div>
            <div className={styles.progressTrack} role="progressbar" aria-label={getMsg('AnalyzeSection.importProgress')} aria-valuemin={0} aria-valuemax={100} aria-valuenow={analyzePercent} aria-valuetext={analyzeProgress || getMsg('AnalyzeSection.progressPending')}>
              <div className={styles.progressFill} style={{ width: `${String(analyzePercent)}%` }} />
            </div>
            <div className={styles.progressStatusRow}>
              <p className={styles.sectionMiniCopy}>{analyzeStatus}</p>
              <button type="button" className={styles.progressCancel} onClick={onCancelSync}>{getMsg('common.stop')}</button>
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
