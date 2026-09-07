import type { AppCard, LauncherViewMode, RegistryCatalog } from '../models/types';
import shared from '../../../shared/styles/shared.module.css';
import { StellaIcon, stellaIconNames } from '../../../shared/components/Icons';
import styles from './RegistrySection.module.css';

/** ランチャーパネルのProps */
interface RegistrySectionProps {
  registryApps: RegistryCatalog;
  launcherViewMode: LauncherViewMode;
  isReloading: boolean;
  onSetLauncherViewMode: (viewMode: LauncherViewMode) => void;
  onLaunchApp: (app: AppCard) => void;
  onOpenFolder: (app: AppCard) => void;
  onUnregisterApp: (app: AppCard) => void;
  onRegisterApp: () => void;
  onReload: () => void;
}

interface LauncherItemProps {
  app: AppCard;
  viewMode: LauncherViewMode;
  onLaunch: (app: AppCard) => void;
  onOpenFolder: (app: AppCard) => void;
  onUnregister: (app: AppCard) => void;
}

/** アイコン未登録時に表示するプレースホルダー */
function LauncherFallbackIcon() {
  return (
    <svg viewBox="0 0 24 24" className="icon-svg">
      <path d="M16,9H19L14,16L9,9H12V5H16M11,2H13V4H11V2M15,19V17H17V19H15M11,19V17H13V19H11M7,19V17H9V19H7Z" />
    </svg>
  );
}

/** 表示形式に応じた1件分のランチャー項目 */
function LauncherItem({ app, viewMode, onLaunch, onOpenFolder, onUnregister }: LauncherItemProps) {
  const isList = viewMode === 'list';
  const icon = (
    <div className={isList ? styles.launcherListIcon : styles.launcherCardLargeIcon}>
      {app.icon_data ? (
        <img src={`data:image/png;base64,${app.icon_data}`} alt="" />
      ) : (
        <LauncherFallbackIcon />
      )}
    </div>
  );
  const copy = (
    <div className={isList ? styles.launcherListCopy : styles.launcherCardLargeCopy}>
      <h4>{app.name}</h4>
      <p className={app.description ? undefined : styles.noDescription}>
        {app.description || '説明なし'}
      </p>
    </div>
  );

  return (
    <article className={isList ? styles.launcherListItem : styles.launcherCardLarge}>
      {isList ? (
        <div className={styles.launcherListMain}>
          {icon}
          {copy}
        </div>
      ) : (
        <>
          {icon}
          {copy}
        </>
      )}
      <div className={isList ? styles.launcherListActions : styles.launcherCardLargeActions}>
        <button
          className={
            isList
              ? `${shared.btn} ${shared.primary}`
              : `${shared.btn} ${shared.primary} ${styles.launcherLaunchButton}`
          }
          onClick={() => {
            onLaunch(app);
          }}
        >
          起動
        </button>
        <button
          className={shared.btn}
          onClick={() => {
            onOpenFolder(app);
          }}
        >
          フォルダを開く
        </button>
        <button
          className={styles.deleteButton}
          onClick={() => {
            onUnregister(app);
          }}
          aria-label="登録解除"
        >
          <StellaIcon name={stellaIconNames.trash} />
        </button>
      </div>
    </article>
  );
}

/** リスト/カード切替・リロード・アプリ操作を備えたランチャーパネル */
export function RegistrySection({
  registryApps,
  launcherViewMode,
  isReloading,
  onSetLauncherViewMode,
  onLaunchApp,
  onOpenFolder,
  onUnregisterApp,
  onRegisterApp,
  onReload,
}: RegistrySectionProps) {
  const allApps = registryApps.apps;

  return (
    <div className={`${styles.root} ${shared.viewContainer}`}>
      <div className={shared.sectionHeader}>
        <div className={styles.sectionHeaderRow}>
          <h2>ランチャー</h2>
          <div className={styles.headerActions}>
            <div className={styles.launcherViewSwitch}>
              <button
                className={`${styles.launcherViewButton} ${launcherViewMode === 'list' ? styles.active : ''}`}
                onClick={() => {
                  onSetLauncherViewMode('list');
                }}
                aria-label="リスト表示"
              >
                <StellaIcon name={stellaIconNames.list} />
              </button>
              <button
                className={`${styles.launcherViewButton} ${launcherViewMode === 'card' ? styles.active : ''}`}
                onClick={() => {
                  onSetLauncherViewMode('card');
                }}
                aria-label="カード表示"
              >
                <StellaIcon name={stellaIconNames.grid} />
              </button>
            </div>
            <button className={`${shared.btn} ${shared.primary}`} onClick={onRegisterApp}>
              <StellaIcon name={stellaIconNames.plus} />
              登録
            </button>
            <button
              className={`${shared.btn} ${styles.reloadButton}`}
              onClick={onReload}
              disabled={isReloading}
            >
              <span className={`${styles.reloadIcon} ${isReloading ? styles.reloadIconSpin : ''}`}>
                <StellaIcon name={stellaIconNames.refresh} />
              </span>
              再読込
            </button>
          </div>
        </div>
      </div>
      <div className={`${shared.card} ${styles.section}`}>
        {allApps.length === 0 && (
          <div className={styles.launcherEmptyState}>登録されているアプリはありません</div>
        )}

        {allApps.length > 0 && (
          <div
            className={launcherViewMode === 'list' ? styles.launcherList : styles.launcherCardGrid}
          >
            {allApps.map((app) => (
              <LauncherItem
                key={app.path}
                app={app}
                viewMode={launcherViewMode}
                onLaunch={onLaunchApp}
                onOpenFolder={onOpenFolder}
                onUnregister={onUnregisterApp}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
