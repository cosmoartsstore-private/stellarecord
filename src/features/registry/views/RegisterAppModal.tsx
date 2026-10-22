import { getMsg } from '../../../messages/getMsg';
import { getFileNameFromPath } from '../../../shared/lib/pathUtils';
import { useRef, useState } from 'react';
import { closeDialog, useModalDialog } from '../../../shared/hooks/useModalDialog';
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
  const dialogRef = useModalDialog();

  // 直前に自動補完で入れた名前。ユーザーが手動編集した場合は ref と現値が乖離するため、
  // 次の exe 選択時に上書きしてよいかどうかを判定できる。
  const lastSuggestedNameRef = useRef('');
  const fileSelectionGenerationRef = useRef(0);
  const nameEditGenerationRef = useRef(0);

  /** exe ファイルを選択し、名前未入力なら表示名を自動補完するハンドラ。 */
  const handlePickFile = async () => {
    const selectionGeneration = ++fileSelectionGenerationRef.current;
    const nameEditGeneration = nameEditGenerationRef.current;
    try {
      const selected = await pickExeFile();
      if (!selected || selectionGeneration !== fileSelectionGenerationRef.current) return;
      setPath(selected);

      // 既存の値が空、または前回の補完値そのままの場合のみ自動補完で上書きする。
      const hasUserEdited = name.length > 0 && name !== lastSuggestedNameRef.current;
      if (hasUserEdited || nameEditGeneration !== nameEditGenerationRef.current) return;

      let suggested = '';
      try {
        suggested = await extractExeDisplayName(selected);
      } catch {
        const fileName = getFileNameFromPath(selected);
        suggested = fileName.replace(/\.exe$/i, '');
      }
      // 表示名の取得中に行ったEXEの再選択と手入力を優先する。
      if (selectionGeneration !== fileSelectionGenerationRef.current || nameEditGeneration !== nameEditGenerationRef.current) return;
      setName(suggested);
      lastSuggestedNameRef.current = suggested;
    } catch {
      // ファイル選択ダイアログのキャンセルやエラーは無視
    }
  };

  const canSubmit = path.length > 0 && name.trim().length > 0;

  return (
    <dialog ref={dialogRef} className={shared.modalOverlay} aria-labelledby="register-app-title" aria-describedby="register-app-description" onClose={onClose}>
      <div className={shared.modalContent}>
        <h3 id="register-app-title" tabIndex={-1} autoFocus>{getMsg('RegisterApp.title')}</h3>
        <p id="register-app-description">{getMsg('RegisterApp.description')}</p>
        <div className={styles.registerForm}>
          <label className={styles.formField}>
            <span>{getMsg('RegisterApp.executableLabel')}</span>
            <div className={styles.filePickerRow}>
              <input type="text" className={styles.formInput} value={path} readOnly placeholder={getMsg('RegisterApp.executablePlaceholder')} />
              <button type="button" className={shared.btn} onClick={() => {
                  void handlePickFile();
                }}>{getMsg('RegisterApp.browse')}</button>
            </div>
          </label>
          <label className={styles.formField}>
            <span>{getMsg('RegisterApp.nameLabel')}</span>
            <input type="text" className={styles.formInput} value={name} onChange={(e) => {
                nameEditGenerationRef.current += 1;
                setName(e.target.value);
              }} placeholder={getMsg('RegisterApp.namePlaceholder')} />
          </label>
          <label className={styles.formField}>
            <span>{getMsg('RegisterApp.descriptionLabel')}</span>
            <input type="text" className={styles.formInput} value={description} onChange={(e) => {
                setDescription(e.target.value);
              }} placeholder={getMsg('RegisterApp.descriptionPlaceholder')} />
          </label>
        </div>
        <div className={shared.modalActions}>
          <button type="button" className={shared.btn} onClick={closeDialog}>{getMsg('common.cancel')}</button>
          <button type="button" className={`${shared.btn} ${shared.primary}`} disabled={!canSubmit} onClick={() => {
              onConfirm(path, name.trim(), description);
            }}>{getMsg('common.register')}</button>
        </div>
      </div>
    </dialog>
  );
}
