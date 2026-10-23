import { getMsg } from '../../../messages/getMsg';
import shared from '../../../shared/styles/shared.module.css';
import styles from './DataLocationList.module.css';

interface DataLocationListProps {
  sourceLogPath: string;
  logArchivePath: string;
  databasePath: string;
  isLoading?: boolean;
  isSourceLogChangeDisabled?: boolean;
  onChangeSourceLogDirectory?: () => void;
}

/** ログ読込元と、ログアーカイブ・DBの実効保存先を表示する。 */
export function DataLocationList({ sourceLogPath, logArchivePath, databasePath, isLoading = false, isSourceLogChangeDisabled = false, onChangeSourceLogDirectory }: DataLocationListProps) {
  const displayPath = (path: string) => {
    if (isLoading) return getMsg('common.loading');
    return path.length > 0 ? path : getMsg('DataLocations.unavailable');
  };

  return (
    <dl className={styles.list} aria-busy={isLoading}>
      <div className={styles.item}>
        <dt>{getMsg('DataLocations.sourceLogs')}</dt>
        <dd className={styles.editablePath}>
          <span className={styles.sourcePath} title={sourceLogPath}>{isLoading ? getMsg('common.loading') : sourceLogPath || getMsg('DataLocations.sourceUnavailable')}</span>
          {onChangeSourceLogDirectory && <button type="button" className={`${shared.btn} ${styles.changeButton}`} onClick={onChangeSourceLogDirectory} disabled={isLoading || isSourceLogChangeDisabled} aria-label={getMsg('DataLocations.changeSourceLogDirectory')}>{getMsg('common.change')}</button>}
        </dd>
      </div>
      <div className={styles.item}>
        <dt>{getMsg('DataLocations.logs')}</dt>
        <dd title={logArchivePath}>{displayPath(logArchivePath)}</dd>
      </div>
      <div className={styles.item}>
        <dt>{getMsg('DataLocations.database')}</dt>
        <dd title={databasePath}>{displayPath(databasePath)}</dd>
      </div>
    </dl>
  );
}
