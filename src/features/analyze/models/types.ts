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

/** アーカイブ内容と現在内容が一致し、削除対象となるソースログファイル */
export interface DeletableLogInfo {
  file_name: string;
  size_bytes: number;
}
