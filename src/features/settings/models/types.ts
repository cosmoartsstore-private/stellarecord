/** 設定画面で編集される管理設定 */
export interface ManagementSettings {
  startup_enabled: boolean;
  /** ユーザーが自動起動設定を1回以上明示的に操作したかどうか */
  startup_preference_set: boolean;
  archive_limit_mb: number;
}

/** 起動時ログ取り込みの許可状態と、利用者へ表示する保存先 */
export interface StartupImportSettings {
  enabled: boolean;
  preference_set: boolean;
  log_archive_path: string;
  database_path: string;
}
