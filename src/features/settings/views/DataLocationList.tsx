import { getMsg } from '../../../messages/getMsg';
import styles from './DataLocationList.module.css';

interface DataLocationListProps {
  logArchivePath: string;
  databasePath: string;
  isLoading?: boolean;
}

/** ログアーカイブとDBの実効保存先を同じ表記で表示する。 */
export function DataLocationList({ logArchivePath, databasePath, isLoading = false }: DataLocationListProps) {
  const displayPath = (path: string) => {
    if (isLoading) return getMsg('common.loading');
    return path.length > 0 ? path : getMsg('DataLocations.unavailable');
  };

  return (
    <dl className={styles.list} aria-busy={isLoading}>
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
