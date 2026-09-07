/** テーマ永続化に使用するブラウザストレージのキー */
const themeStorageKey = 'stella-record-theme';

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
  const stored = window.localStorage.getItem(themeStorageKey);
  return isThemeMode(stored) ? stored : 'light';
}

/** 現在のテーマをブラウザストレージに保存する */
export function saveTheme(themeMode: ThemeMode) {
  if (typeof window === 'undefined') {
    return;
  }
  window.localStorage.setItem(themeStorageKey, themeMode);
}
