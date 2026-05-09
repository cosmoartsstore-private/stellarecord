/** ダッシュボードメーター用のアーカイブ使用量と上限 */
export interface StorageStatus {
  current: number;
  limit: number;
  percent: number;
}

/** Rustバックエンドが発行する解析進捗ペイロード */
export interface AnalyzeProgressEvent {
  status: string;
  progress: string;
  is_running: boolean;
}

/** アーカイブ済みが確認され削除対象となるソースログファイル */
export interface DeletableLogInfo {
  file_name: string;
  size_bytes: number;
}
