import type { AppCard, RegistryCatalog } from '../models/types';
import shared from '../../../shared/styles/shared.module.css';
import { StellaIcon, stellaIconNames } from '../../../shared/components/Icons';
import { LauncherFallbackIcon } from './RegistryIcons';
import styles from './RegistrySection.module.css';

/** ランチャーパネルのProps */
interface RegistrySectionProps {
  registryApps: RegistryCatalog;
  launcherViewMode: 'list' | 'card';
  isReloading: boolean;
  onSetLauncherViewMode: (viewMode: 'list' | 'card') => void;
  onLaunchApp: (app: AppCard) => void;
  onOpenFolder: (app: AppCard) => void;
  onUnregisterApp: (app: AppCard) => void;
  onRegisterApp: () => void;
  onReload: () => void;
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

  /** Base64 PNGアイコンを描画する（未設定時はフォールバックアイコン） */
  const renderLauncherIcon = (app: AppCard) => {
    if (app.icon_data) {
      // icon_dataは登録元アプリがDBにBase64 PNGとして格納
      return <img src={`data:image/png;base64,${app.icon_data}`} alt="" />;
    }
    return <LauncherFallbackIcon />;
  };

  return (
    <div className={`${styles.root} ${shared.viewContainer}`}>
      <div className={shared.sectionHeader}>
        <div className={styles.sectionHeaderRow}>
          <h2>ランチャー</h2>
          <div className={styles.headerActions}>
            <div className={styles.launcherViewSwitch}>
              <button
                className={`${styles.launcherViewButton} ${launcherViewMode === 'list' ? styles.active : ''}`}
                onClick={() => { onSetLauncherViewMode('list'); }}
                aria-label="リスト表示"
              >
                <StellaIcon name={stellaIconNames.list} />
              </button>
              <button
                className={`${styles.launcherViewButton} ${launcherViewMode === 'card' ? styles.active : ''}`}
                onClick={() => { onSetLauncherViewMode('card'); }}
                aria-label="カード表示"
              >
                <StellaIcon name={stellaIconNames.grid} />
              </button>
            </div>
            <button
              className={`${shared.btn} ${shared.primary}`}
              onClick={onRegisterApp}
            >
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

        {allApps.length > 0 && launcherViewMode === 'list' && (
          <div className={styles.launcherList}>
            {allApps.map((app) => (
              <article key={app.name} className={styles.launcherListItem}>
                <div className={styles.launcherListMain}>
                  <div className={styles.launcherListIcon}>
                    {renderLauncherIcon(app)}
                  </div>
                  <div className={styles.launcherListCopy}>
                    <h4>{app.name}</h4>
                    <p>{app.description}</p>
                  </div>
                </div>
                <div className={styles.launcherListActions}>
                  <button
                    className={`${shared.btn} ${shared.primary}`}
                    onClick={() => {
                      onLaunchApp(app);
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
                    onClick={() => { onUnregisterApp(app); }}
                    aria-label="登録解除"
                  >
                    <StellaIcon name={stellaIconNames.trash} />
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}

        {allApps.length > 0 && launcherViewMode === 'card' && (
          <div className={styles.launcherCardGrid}>
            {allApps.map((app) => (
              <article key={app.name} className={styles.launcherCardLarge}>
                <div className={styles.launcherCardLargeIcon}>
                  {renderLauncherIcon(app)}
                </div>
                <div className={styles.launcherCardLargeCopy}>
                  <h4>{app.name}</h4>
                  <p>{app.description}</p>
                </div>
                <div className={styles.launcherCardLargeActions}>
                  <button
                    className={`${shared.btn} ${shared.primary} ${styles.launcherLaunchButton}`}
                    onClick={() => {
                      onLaunchApp(app);
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
                    onClick={() => { onUnregisterApp(app); }}
                    aria-label="登録解除"
                  >
                    <StellaIcon name={stellaIconNames.trash} />
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
