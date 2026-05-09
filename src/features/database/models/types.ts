/** DB画面に表示するプレビュー可能なテーブル情報 */
export interface DbTableSummary {
  name: string;
  label: string;
  description: string;
  storage: string;
  is_view: boolean;
}

/** プレビューカラムのUI表示名とヘルプテキスト */
export interface DbColumnMeta {
  name: string;
  label: string;
  description: string;
}

/** バックエンドから返されるテーブルプレビューのペイロード */
export interface TableData {
  name: string;
  label: string;
  description: string;
  storage: string;
  columns: DbColumnMeta[];
  rows: string[][];
  total_rows: number;
}

/** テーブル未選択時に使用する空のフォールバックデータ */
export const emptyTableData: TableData = {
  name: '',
  label: '',
  description: '',
  storage: '',
  columns: [],
  rows: [],
  total_rows: 0,
};
