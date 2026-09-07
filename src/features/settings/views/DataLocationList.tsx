import styles from './DataLocationList.module.css';

interface DataLocationListProps {
  logArchivePath: string;
  databasePath: string;
  isLoading?: boolean;
}

/** ログアーカイブとDBの実効保存先を同じ表記で表示する。 */
export function DataLocationList({
  logArchivePath,
  databasePath,
  isLoading = false,
}: DataLocationListProps) {
  const displayPath = (path: string) => {
    if (isLoading) return '読み込み中…';
    return path.length > 0 ? path : '保存先を取得できません';
  };

  return (
    <dl className={styles.list} aria-busy={isLoading}>
      <div className={styles.item}>
        <dt>ログ保存先</dt>
        <dd title={logArchivePath}>{displayPath(logArchivePath)}</dd>
      </div>
      <div className={styles.item}>
        <dt>DB保存先</dt>
        <dd title={databasePath}>{displayPath(databasePath)}</dd>
      </div>
    </dl>
  );
}
