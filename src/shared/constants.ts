/** アプリ全体で表示する製品名と制作者名。 */
export const appDisplayName = 'STELLA RECORD';
export const publisherName = 'CosmoArtsStore';

/** 保存設定と画面表示に共通する2進単位。 */
export const byteUnitBytes = { KB: 1024, MB: 1024 * 1024, GB: 1024 * 1024 * 1024 } as const;

/** 保存済み設定と画面の下書きで共通のアーカイブ容量契約。 */
export const archiveCapacity = { DEFAULT_MB: 300, MAX_MB: 10_485_760 } as const;

/** 端末内で保持する表示設定のキー。 */
export const storageKeys = { THEME: 'stella-record-theme' } as const;

/** アプリ共通の通知を表示する既定時間。 */
export const defaultToastDurationMs = 3_000;
