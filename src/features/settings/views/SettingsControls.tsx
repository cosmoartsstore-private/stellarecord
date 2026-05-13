import { StellaIcon, stellaIconNames } from '../../../shared/components/Icons';
import shared from '../../../shared/styles/shared.module.css';
import styles from './SettingsControls.module.css';

/** 警告ライン入力 + 自動起動トグルの設定コントロール */
interface SettingsControlsProps {
  archiveLimitDraft: string;
  isStartupEnabledDraft: boolean;
  onArchiveLimitDraftChange: (value: string) => void;
  onSaveArchiveLimit: () => void;
  onToggleStartup: () => void;
}

/** ストレージ警告ライン（MB）と自動起動のインライン設定ウィジェット */
export function SettingsControls({
  archiveLimitDraft,
  isStartupEnabledDraft,
  onArchiveLimitDraftChange,
  onSaveArchiveLimit,
  onToggleStartup,
}: SettingsControlsProps) {
  return (
    <>
      <label className={styles.inlineSettingRow}>
        <span className={styles.inlineSettingLabel}>警告ライン</span>
        <input
          className={styles.inlineNumberInput}
          type="number"
          min={1}
          max={10485760}
          step={1}
          value={archiveLimitDraft}
          onChange={(e) => {
            onArchiveLimitDraftChange(e.target.value);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') onSaveArchiveLimit();
          }}
        />
        <span className={styles.inlineUnit}>MB</span>
        <button className={shared.btn} onClick={onSaveArchiveLimit}>
          保存
        </button>
      </label>
      <div className={styles.inlineToggleRow}>
        <button
          className={`${styles.autoStartBtn} ${isStartupEnabledDraft ? styles.autoStartBtnOn : ''}`}
          onClick={onToggleStartup}
        >
          <StellaIcon name={stellaIconNames.power} />
          <span>自動起動</span>
        </button>
      </div>
    </>
  );
}
