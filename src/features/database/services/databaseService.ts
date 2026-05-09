import { invoke } from '@tauri-apps/api/core';
import type { DbTableSummary, TableData } from '../models/types';

/** 指定テーブルの1ページ分（500行）のデータを取得する */
export const loadDbTableData = (
  tableName: string,
  page = 0,
  sortColumn?: string,
  sortDir?: 'asc' | 'desc',
) =>
  invoke<TableData>('get_db_table_data', { tableName, page, sortColumn, sortDir });

/** プレビュー可能なテーブル一覧（カタログ）を取得する */
export const loadDbTables = () => invoke<DbTableSummary[]>('get_db_tables');
