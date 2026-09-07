import type { ThemeMode } from '../models/theme';
import shared from '../../../shared/styles/shared.module.css';
import { DataLocationList } from './DataLocationList';
import styles from './SettingsControls.module.css';

const themeOptions: { value: ThemeMode; label: string }[] = [
  { value: 'light', label: 'ライト' },
  { value: 'dark', label: 'ダーク' },
  { value: 'midnight', label: 'ミッドナイト' },
];

interface SettingsControlsProps {
  themeMode: ThemeMode;
  archiveLimitDraft: string;
  isStartupEnabledDraft: boolean;
  isStartupImportEnabled: boolean;
  isStartupImportLoading: boolean;
  logArchivePath: string;
  databasePath: string;
  isDataLocationLoading: boolean;
  onThemeModeChange: (themeMode: ThemeMode) => void;
  onArchiveLimitDraftChange: (value: string) => void;
  onSaveArchiveLimit: () => void;
  onToggleStartup: () => void;
  onToggleStartupImport: () => void;
}

interface SettingSwitchProps {
  id: string;
  isChecked: boolean;
  isDisabled?: boolean;
  labelledBy: string;
  describedBy: string;
  onChange: () => void;
}

/** 即時反映される二値設定を、状態名付きのスイッチとして表示する。 */
function SettingSwitch(props: SettingSwitchProps) {
  const { id, isChecked, isDisabled = false, labelledBy, describedBy, onChange } = props;

  return (
    <label className={styles.switchControl} htmlFor={id}>
      <input
        id={id}
        className={styles.switchInput}
        type="checkbox"
        checked={isChecked}
        disabled={isDisabled}
        onChange={onChange}
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
      />
      {/* prettier-ignore */}
      <span className={styles.switchTrack} aria-hidden="true">
        <span className={styles.switchThumb} />
      </span>
      {/* prettier-ignore */}
      <span className={styles.switchState} aria-hidden="true">
        {isChecked ? 'オン' : 'オフ'}
      </span>
    </label>
  );
}

/** 設定を用途別の単一列にまとめ、各項目の説明と操作を対応付けて描画する。 */
export function SettingsControls(props: SettingsControlsProps) {
  // prettier-ignore
  const { themeMode, archiveLimitDraft, isStartupEnabledDraft, isStartupImportEnabled, isStartupImportLoading, logArchivePath, databasePath, isDataLocationLoading, onThemeModeChange, onArchiveLimitDraftChange, onSaveArchiveLimit, onToggleStartup, onToggleStartupImport } = props;

  return (
    <div className={styles.settingsRoot}>
      <section className={styles.section} aria-labelledby="appearance-settings-title">
        {/* prettier-ignore */}
        <h3 id="appearance-settings-title" className={styles.sectionTitle}>外観</h3>
        <div className={styles.settingGroup}>
          <div className={styles.settingRow}>
            <div className={styles.settingCopy}>
              {/* prettier-ignore */}
              <label className={styles.settingName} htmlFor="theme-mode">テーマ</label>
              {/* prettier-ignore */}
              <p id="theme-mode-description" className={styles.settingDescription}>アプリ全体の配色を選択します。</p>
            </div>
            <select
              id="theme-mode"
              className={styles.themeSelect}
              value={themeMode}
              onChange={(event) => {
                onThemeModeChange(event.target.value as ThemeMode);
              }}
              aria-describedby="theme-mode-description"
            >
              {themeOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>
        </div>
      </section>

      <section className={styles.section} aria-labelledby="startup-settings-title">
        {/* prettier-ignore */}
        <h3 id="startup-settings-title" className={styles.sectionTitle}>起動</h3>
        <div className={styles.settingGroup}>
          <div className={styles.settingRow}>
            <div className={styles.settingCopy}>
              {/* prettier-ignore */}
              <label id="windows-startup-label" className={styles.settingName} htmlFor="windows-startup">自動起動</label>
              {/* prettier-ignore */}
              <p id="windows-startup-description" className={styles.settingDescription}>Windows へのサインイン時に StellaRecord を起動します。</p>
            </div>
            <SettingSwitch
              id="windows-startup"
              isChecked={isStartupEnabledDraft}
              labelledBy="windows-startup-label"
              describedBy="windows-startup-description"
              onChange={onToggleStartup}
            />
          </div>
          <div className={styles.settingRow}>
            <div className={styles.settingCopy}>
              {/* prettier-ignore */}
              <label id="startup-import-label" className={styles.settingName} htmlFor="startup-import">ログの自動取り込み</label>
              {/* prettier-ignore */}
              <p id="startup-import-description" className={styles.settingDescription}>次回の起動から Polaris のログを読み取り、ログ保存先と DB に保存します。</p>
            </div>
            <SettingSwitch
              id="startup-import"
              isChecked={isStartupImportEnabled}
              isDisabled={isStartupImportLoading}
              labelledBy="startup-import-label"
              describedBy="startup-import-description"
              onChange={onToggleStartupImport}
            />
          </div>
        </div>
      </section>

      <section className={styles.section} aria-labelledby="storage-settings-title">
        {/* prettier-ignore */}
        <h3 id="storage-settings-title" className={styles.sectionTitle}>ストレージ</h3>
        <div className={styles.settingGroup}>
          <div className={styles.settingRow}>
            <div className={styles.settingCopy}>
              {/* prettier-ignore */}
              <label className={styles.settingName} htmlFor="archive-limit-mb">容量の警告ライン</label>
              {/* prettier-ignore */}
              <p id="archive-limit-description" className={styles.settingDescription}>アーカイブ容量の警告基準を MB 単位で指定します。</p>
            </div>
            <div className={styles.numberControl}>
              <input
                id="archive-limit-mb"
                className={styles.numberInput}
                type="number"
                inputMode="numeric"
                min={1}
                max={10485760}
                step={1}
                value={archiveLimitDraft}
                aria-describedby="archive-limit-description archive-limit-unit"
                onChange={(event) => {
                  const value = event.target.value;
                  if (value.length <= 8) onArchiveLimitDraftChange(value);
                }}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') onSaveArchiveLimit();
                }}
              />
              <span id="archive-limit-unit" className={styles.numberUnit}>
                MB
              </span>
              {/* prettier-ignore */}
              <button type="button" className={`${shared.btn} ${styles.saveButton}`} onClick={onSaveArchiveLimit}>保存</button>
            </div>
          </div>
          <div className={`${styles.settingRow} ${styles.locationRow}`}>
            <h4 className={styles.settingName}>データの保存先</h4>
            {/* prettier-ignore */}
            <DataLocationList logArchivePath={logArchivePath} databasePath={databasePath} isLoading={isDataLocationLoading} />
          </div>
        </div>
      </section>
    </div>
  );
}
