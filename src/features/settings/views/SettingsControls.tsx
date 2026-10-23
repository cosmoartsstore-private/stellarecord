import { getMsg } from '../../../messages/getMsg';
import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { archiveCapacity } from '../../../shared/constants';
import type { ThemeMode } from '../models/theme';
import shared from '../../../shared/styles/shared.module.css';
import { DataLocationList } from './DataLocationList';
import styles from './SettingsControls.module.css';

const themeOptions: { value: ThemeMode; label: string }[] = [{ value: 'light', label: getMsg('SettingsControls.lightTheme') }, { value: 'dark', label: getMsg('SettingsControls.darkTheme') }];

/** 選択中のテーマと候補のフォーカスを分け、確定操作でだけ配色を変更する。 */
function ThemeSelect({ themeMode, onChange }: { themeMode: ThemeMode; onChange: (themeMode: ThemeMode) => void }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [isOpen, setIsOpen] = useState(false);
  const [activeTheme, setActiveTheme] = useState(themeMode);
  const selectedOption = themeOptions.find((option) => option.value === themeMode);

  useEffect(() => {
    if (!isOpen) return;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (event.target instanceof Node && !containerRef.current?.contains(event.target)) setIsOpen(false);
    };
    document.addEventListener('pointerdown', closeOnOutsidePointer);
    return () => { document.removeEventListener('pointerdown', closeOnOutsidePointer); };
  }, [isOpen]);

  useEffect(() => {
    if (isOpen) document.getElementById(`theme-option-${activeTheme}`)?.scrollIntoView({ block: 'nearest' });
  }, [activeTheme, isOpen]);

  const selectTheme = (value: ThemeMode) => {
    onChange(value);
    setIsOpen(false);
    buttonRef.current?.focus();
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    const activeIndex = themeOptions.findIndex((option) => option.value === activeTheme);
    switch (event.key) {
      case 'ArrowDown':
      case 'ArrowUp': {
        event.preventDefault();
        if (isOpen && event.altKey && event.key === 'ArrowUp') {
          selectTheme(activeTheme);
          return;
        }
        const nextIndex = Math.max(0, Math.min(themeOptions.length - 1, activeIndex + (event.key === 'ArrowDown' ? 1 : -1)));
        setActiveTheme(isOpen ? themeOptions[nextIndex].value : themeMode);
        setIsOpen(true);
        return;
      }
      case 'Home':
      case 'End':
        event.preventDefault();
        setActiveTheme(event.key === 'Home' ? 'light' : 'dark');
        setIsOpen(true);
        return;
      case 'Enter':
      case ' ':
        event.preventDefault();
        if (isOpen) selectTheme(activeTheme);
        else { setActiveTheme(themeMode); setIsOpen(true); }
        return;
      case 'Escape':
        if (isOpen) {
          event.preventDefault();
          event.stopPropagation();
          setIsOpen(false);
        }
        return;
      case 'Tab':
        if (isOpen) { onChange(activeTheme); setIsOpen(false); }
        return;
      default: {
        if (event.key.length !== 1 || event.altKey || event.ctrlKey || event.metaKey) return;
        const match = themeOptions.find((option) => option.label.startsWith(event.key) || option.value.startsWith(event.key.toLowerCase()));
        if (match) { event.preventDefault(); setActiveTheme(match.value); setIsOpen(true); }
      }
    }
  };

  return (
    <div ref={containerRef} className={styles.themePicker}>
      <button
        ref={buttonRef}
        id="theme-mode"
        type="button"
        role="combobox"
        className={styles.themeSelect}
        aria-labelledby="theme-mode-label"
        aria-describedby="theme-mode-description"
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-controls={isOpen ? 'theme-options' : undefined}
        aria-activedescendant={isOpen ? `theme-option-${activeTheme}` : undefined}
        onBlur={(event) => {
          if (!containerRef.current?.contains(event.relatedTarget)) setIsOpen(false);
        }}
        onKeyDown={handleKeyDown}
        onClick={() => {
          setActiveTheme(themeMode);
          setIsOpen(!isOpen);
        }}
      >
        <span>{selectedOption?.label}</span>
        <span className={styles.themeChevron} aria-hidden="true" />
      </button>
      {isOpen && (
        <ul id="theme-options" className={styles.themeOptions} role="listbox" aria-labelledby="theme-mode-label">
          {themeOptions.map((option) => (
            <li key={option.value} role="presentation">
              <button
                id={`theme-option-${option.value}`}
                type="button"
                role="option"
                className={styles.themeOption}
                tabIndex={-1}
                aria-selected={option.value === themeMode}
                data-highlighted={option.value === activeTheme}
                onPointerMove={() => { setActiveTheme(option.value); }}
                onMouseDown={(event) => { event.preventDefault(); }}
                onClick={() => { selectTheme(option.value); }}
              >
                <span>{option.label}</span>
                <span className={styles.themeCheck} aria-hidden="true">{option.value === themeMode ? '✓' : ''}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

interface SettingsControlsProps {
  themeMode: ThemeMode;
  archiveLimitDraft: string;
  isStartupEnabledDraft: boolean;
  isStartupImportEnabled: boolean;
  isStartupImportLoading: boolean;
  sourceLogPath: string;
  logArchivePath: string;
  databasePath: string;
  isDataLocationLoading: boolean;
  isSourceLogChangeDisabled: boolean;
  onChangeSourceLogDirectory: () => void;
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
    <div className={`${styles.switchControl} ${isDisabled ? styles.switchDisabled : ''}`}>
      <label className={styles.switchToggle} htmlFor={id}>
        <input id={id} className={styles.switchInput} type="checkbox" checked={isChecked} disabled={isDisabled} onChange={onChange} aria-labelledby={labelledBy} aria-describedby={describedBy} />
        <span className={styles.switchTrack} aria-hidden="true"><span className={styles.switchThumb} /></span>
      </label>
      <span className={styles.switchState} aria-hidden="true">
        {isChecked ? getMsg('common.on') : getMsg('common.off')}
      </span>
    </div>
  );
}

/** 設定を用途別の単一列にまとめ、各項目の説明と操作を対応付けて描画する。 */
export function SettingsControls(props: SettingsControlsProps) {
  const { themeMode, archiveLimitDraft, isStartupEnabledDraft, isStartupImportEnabled, isStartupImportLoading, sourceLogPath, logArchivePath, databasePath, isDataLocationLoading, isSourceLogChangeDisabled, onChangeSourceLogDirectory, onThemeModeChange, onArchiveLimitDraftChange, onSaveArchiveLimit, onToggleStartup, onToggleStartupImport } = props;

  return (
    <div className={styles.settingsRoot}>
      <section className={styles.section} aria-labelledby="appearance-settings-title">
        <h3 id="appearance-settings-title" className={styles.sectionTitle}>{getMsg('SettingsControls.appearance')}</h3>
        <div className={styles.settingGroup}>
          <div className={styles.settingRow}>
            <div className={styles.settingCopy}>
              <span id="theme-mode-label" className={styles.settingName}>{getMsg('SettingsControls.theme')}</span>
              <p id="theme-mode-description" className={styles.settingDescription}>{getMsg('SettingsControls.themeDescription')}</p>
            </div>
            <ThemeSelect themeMode={themeMode} onChange={onThemeModeChange} />
          </div>
        </div>
      </section>

      <section className={styles.section} aria-labelledby="startup-settings-title">
        <h3 id="startup-settings-title" className={styles.sectionTitle}>{getMsg('common.launch')}</h3>
        <div className={styles.settingGroup}>
          <div className={styles.settingRow}>
            <div className={styles.settingCopy}>
              <span id="windows-startup-label" className={styles.settingName}>{getMsg('SettingsControls.startup')}</span>
              <p id="windows-startup-description" className={styles.settingDescription}>{getMsg('SettingsControls.startupDescription')}</p>
            </div>
            <SettingSwitch id="windows-startup" isChecked={isStartupEnabledDraft} labelledBy="windows-startup-label" describedBy="windows-startup-description" onChange={onToggleStartup} />
          </div>
          <div className={styles.settingRow}>
            <div className={styles.settingCopy}>
              <span id="startup-import-label" className={styles.settingName}>{getMsg('SettingsControls.startupImport')}</span>
              <p id="startup-import-description" className={styles.settingDescription}>{getMsg('SettingsControls.startupImportDescription')}</p>
            </div>
            <SettingSwitch id="startup-import" isChecked={isStartupImportEnabled} isDisabled={isStartupImportLoading} labelledBy="startup-import-label" describedBy="startup-import-description" onChange={onToggleStartupImport} />
          </div>
        </div>
      </section>

      <section className={styles.section} aria-labelledby="storage-settings-title">
        <h3 id="storage-settings-title" className={styles.sectionTitle}>{getMsg('SettingsControls.storage')}</h3>
        <div className={styles.settingGroup}>
          <div className={styles.settingRow}>
            <div className={styles.settingCopy}>
              <label className={styles.settingName} htmlFor="archive-limit-mb">{getMsg('SettingsControls.capacityWarning')}</label>
              <p id="archive-limit-description" className={styles.settingDescription}>{getMsg('SettingsControls.capacityWarningDescription')}</p>
            </div>
            <div className={styles.numberControl}>
              <input id="archive-limit-mb" className={styles.numberInput} type="number" inputMode="numeric" min={1} max={archiveCapacity.MAX_MB} step={1} value={archiveLimitDraft} aria-describedby="archive-limit-description archive-limit-unit" onChange={(event) => {
                  const value = event.target.value;
                  if (value.length <= 8) onArchiveLimitDraftChange(value);
                }} onKeyDown={(event) => {
                  if (event.key === 'Enter') onSaveArchiveLimit();
                }} />
              <span id="archive-limit-unit" className={styles.numberUnit}>MB</span>
              <button type="button" className={`${shared.btn} ${styles.saveButton}`} onClick={onSaveArchiveLimit}>{getMsg('common.save')}</button>
            </div>
          </div>
          <div className={`${styles.settingRow} ${styles.locationRow}`}>
            <h4 className={styles.settingName}>{getMsg('SettingsControls.dataLocations')}</h4>
            <DataLocationList sourceLogPath={sourceLogPath} logArchivePath={logArchivePath} databasePath={databasePath} isLoading={isDataLocationLoading} isSourceLogChangeDisabled={isSourceLogChangeDisabled} onChangeSourceLogDirectory={onChangeSourceLogDirectory} />
          </div>
        </div>
      </section>
    </div>
  );
}
