/** ダッシュボードメーター用のアーカイブ使用量と上限 */
export interface StorageStatus {
  current: number;
  limit: number;
  percent: number;
}

/** RustBackendが発行する解析進捗ペイロード */
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

/** 削除候補を取得した時点の読込元と対象ログ */
export interface SourceLogCandidates {
  source_log_path: string;
  logs: DeletableLogInfo[];
}
