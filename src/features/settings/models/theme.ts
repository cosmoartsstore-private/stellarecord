import { storageKeys } from '../../../shared/constants';

/** アプリがサポートするテーマの切替順 */
export const themeModes = ['light', 'dark', 'midnight'] as const;

export type ThemeMode = (typeof themeModes)[number];

function isThemeMode(value: string | null): value is ThemeMode {
  return themeModes.some((theme) => theme === value);
}

/** ブラウザストレージから初期テーマを読み込む（未設定時はlight） */
export function readInitialTheme(): ThemeMode {
  if (typeof window === 'undefined') {
    return 'light';
  }
  const stored = window.localStorage.getItem(storageKeys.THEME);
  return isThemeMode(stored) ? stored : 'light';
}

/** 現在のテーマをブラウザストレージに保存する */
export function saveTheme(themeMode: ThemeMode) {
  if (typeof window === 'undefined') {
    return;
  }
  window.localStorage.setItem(storageKeys.THEME, themeMode);
}
