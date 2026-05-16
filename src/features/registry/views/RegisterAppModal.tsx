import { useRef, useState } from 'react';
import shared from '../../../shared/styles/shared.module.css';
import styles from './RegistrySection.module.css';
import { extractExeDisplayName, pickExeFile } from '../services/registryService';

interface RegisterAppModalProps {
  onClose: () => void;
  onConfirm: (path: string, name: string, description: string) => void;
}

/** 外部アプリ（exe）の実行ファイルパス・表示名・説明を入力して登録するモーダル。 */
export function RegisterAppModal({ onClose, onConfirm }: RegisterAppModalProps) {
  const [path, setPath] = useState('');
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');

  // 直前に自動補完で入れた名前。ユーザーが手動編集した場合は ref と現値が乖離するため、
  // 次の exe 選択時に上書きしてよいかどうかを判定できる。
  const lastSuggestedNameRef = useRef('');

  /** exe ファイルを選択し、名前未入力なら表示名を自動補完するハンドラ。 */
  const handlePickFile = async () => {
    try {
      const selected = await pickExeFile();
      if (!selected) return;
      setPath(selected);

      // 既存の値が空、または前回の補完値そのままの場合のみ自動補完で上書きする。
      const hasUserEdited = name.length > 0 && name !== lastSuggestedNameRef.current;
      if (hasUserEdited) return;

      let suggested = '';
      try {
        suggested = await extractExeDisplayName(selected);
      } catch {
        const fileName = selected.split(/[\\/]/).pop() ?? '';
        suggested = fileName.replace(/\.exe$/i, '');
      }
      setName(suggested);
      lastSuggestedNameRef.current = suggested;
    } catch {
      // ファイル選択ダイアログのキャンセルやエラーは無視
    }
  };

  const canSubmit = path.length > 0 && name.trim().length > 0;

  return (
    <div className={shared.modalOverlay}>
      <div className={shared.modalContent}>
        <h3>アプリを登録</h3>
        <div className={styles.registerForm}>
          <label className={styles.formField}>
            <span>実行ファイル</span>
            <div className={styles.filePickerRow}>
              <input
                type="text"
                className={styles.formInput}
                value={path}
                readOnly
                placeholder="ファイルを選択してください"
              />
              <button
                className={shared.btn}
                onClick={() => { void handlePickFile(); }}
              >
                参照
              </button>
            </div>
          </label>
          <label className={styles.formField}>
            <span>アプリ名</span>
            <input
              type="text"
              className={styles.formInput}
              value={name}
              onChange={(e) => { setName(e.target.value); }}
              placeholder="表示名を入力"
            />
          </label>
          <label className={styles.formField}>
            <span>説明（任意）</span>
            <input
              type="text"
              className={styles.formInput}
              value={description}
              onChange={(e) => { setDescription(e.target.value); }}
              placeholder="アプリの説明"
            />
          </label>
        </div>
        <div className={shared.modalActions}>
          <button className={shared.btn} onClick={onClose}>
            キャンセル
          </button>
          <button
            className={`${shared.btn} ${shared.primary}`}
            disabled={!canSubmit}
            onClick={() => { onConfirm(path, name.trim(), description); }}
          >
            登録
          </button>
        </div>
      </div>
    </div>
  );
}
