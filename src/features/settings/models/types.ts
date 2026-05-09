/** 設定画面で編集される管理設定 */
export interface ManagementSettings {
  startup_enabled: boolean;
  /** ユーザーが自動起動設定を1回以上明示的に操作したかどうか */
  startup_preference_set: boolean;
  archive_limit_mb: number;
}

/** サポートされる3つのテーマバリアント */
export type ThemeMode = 'light' | 'dark' | 'midnight';
