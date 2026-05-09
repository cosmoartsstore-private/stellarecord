import { useState } from 'react';
import shared from '../../../shared/styles/shared.module.css';
import styles from './RegistrySection.module.css';
import { pickExeFile } from '../services/registryService';

interface RegisterAppModalProps {
  onClose: () => void;
  onConfirm: (path: string, name: string, description: string) => void;
}

export function RegisterAppModal({ onClose, onConfirm }: RegisterAppModalProps) {
  const [path, setPath] = useState('');
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');

  const handlePickFile = async () => {
    try {
      const selected = await pickExeFile();
      if (selected) {
        setPath(selected);
        if (!name) {
          const fileName = selected.split('\\').pop() ?? '';
          setName(fileName.replace(/\.exe$/i, ''));
        }
      }
    } catch {
      // ダイアログのキャンセルやエラーは無視
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
