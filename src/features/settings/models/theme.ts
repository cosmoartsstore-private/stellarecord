import type { ThemeMode } from './types';

/** テーマ永続化に使用するブラウザストレージのキー */
const themeStorageKey = 'stella-record-theme';

const VALID_THEMES: ThemeMode[] = ['light', 'dark', 'midnight'];

/** ブラウザストレージから初期テーマを読み込む（未設定時はlight） */
export function readInitialTheme(): ThemeMode {
  if (typeof window === 'undefined') {
    return 'light';
  }
  const stored = window.localStorage.getItem(themeStorageKey);
  return VALID_THEMES.includes(stored as ThemeMode) ? (stored as ThemeMode) : 'light';
}

/** 現在のテーマをブラウザストレージに保存する */
export function saveTheme(themeMode: ThemeMode) {
  if (typeof window === 'undefined') {
    return;
  }
  window.localStorage.setItem(themeStorageKey, themeMode);
}
