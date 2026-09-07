import { useCallback, useRef, useState } from 'react';
import type { DbTableSummary, TableData } from '../models/types';
import { emptyTableData } from '../models/types';
import { loadDbTableData, loadDbTables } from '../services/databaseService';
import { addErrorToast, type AddToast } from '../../../shared/lib/errors';

/** ソート状態（カラム名と方向） */
export interface SortState {
  column: string;
  dir: 'asc' | 'desc';
}

/**
 * 読み取り専用DBプレビューと破壊的DB操作を管理するフック
 *
 * テーブルカタログ・ページネーション・ソート・破壊的アクションの
 * 確認と実行手順を一元管理する
 */
export function useDatabaseState(addToast: AddToast) {
  const [dbTables, setDbTables] = useState<DbTableSummary[]>([]);
  const [currentTable, setCurrentTable] = useState('');
  const [tableData, setTableData] = useState<TableData>(emptyTableData);
  const [isDbLoading, setIsDbLoading] = useState(false);
  const [currentPage, setCurrentPage] = useState(0);
  const [sortState, setSortState] = useState<SortState | null>(null);
  const requestGenerationRef = useRef(0);
  // バックエンド側 (src-tauri/src/commands/database.rs の PAGE_SIZE) と一致させる必要がある
  const pageSize = 500;

  /** 同じ要求世代に属するテーブルデータだけをプレビュー状態へ反映する */
  const loadTableDataForRequest = useCallback(
    async (requestGeneration: number, tableName: string, page = 0, sort?: SortState | null) => {
      if (requestGenerationRef.current !== requestGeneration) return;
      setCurrentTable(tableName);
      setCurrentPage(page);
      setIsDbLoading(true);
      try {
        const data = await loadDbTableData(tableName, page, sort?.column, sort?.dir);
        if (requestGenerationRef.current === requestGeneration) {
          setTableData(data);
        }
      } catch (error) {
        if (requestGenerationRef.current === requestGeneration) {
          addErrorToast(addToast, 'DBテーブルデータ取得', 'データを読み込めませんでした', error);
        }
      } finally {
        if (requestGenerationRef.current === requestGeneration) {
          setIsDbLoading(false);
        }
      }
    },
    [addToast],
  );

  /** 指定テーブルの1ページ分を取得してプレビュー状態を更新する */
  const loadTableData = useCallback(
    async (tableName: string, page = 0, sort?: SortState | null) => {
      const requestGeneration = requestGenerationRef.current + 1;
      requestGenerationRef.current = requestGeneration;
      await loadTableDataForRequest(requestGeneration, tableName, page, sort);
    },
    [loadTableDataForRequest],
  );

  /** 現在選択中のテーブル内で指定ページに移動する */
  const goToPage = useCallback(
    async (page: number) => {
      if (!currentTable) return;
      await loadTableData(currentTable, page, sortState);
    },
    [currentTable, loadTableData, sortState],
  );

  /** カラムのソートを切り替える（昇順→降順→解除） */
  const toggleSort = useCallback(
    async (columnName: string) => {
      let next: SortState | null;
      if (sortState?.column === columnName) {
        next = sortState.dir === 'asc' ? { column: columnName, dir: 'desc' } : null;
      } else {
        next = { column: columnName, dir: 'asc' };
      }
      setSortState(next);
      if (currentTable) {
        await loadTableData(currentTable, 0, next);
      }
    },
    [currentTable, loadTableData, sortState],
  );

  /** テーブルカタログを再取得し、プレビュー対象を自動選択する */
  const loadDatabaseCatalog = useCallback(
    async (preferredTableName?: string) => {
      const requestGeneration = requestGenerationRef.current + 1;
      requestGenerationRef.current = requestGeneration;
      setIsDbLoading(true);
      try {
        const tables = await loadDbTables();
        if (requestGenerationRef.current !== requestGeneration) return;
        setDbTables(tables);
        if (tables.length === 0) {
          setCurrentTable('');
          setTableData(emptyTableData);
          return;
        }
        // 優先順位: 明示指定 > 現在の選択を維持 > 先頭テーブル
        const nextTableName =
          preferredTableName ??
          (tables.some((table) => table.name === currentTable) ? currentTable : tables[0]?.name);
        if (nextTableName) {
          setSortState(null);
          await loadTableDataForRequest(requestGeneration, nextTableName);
        }
      } catch (error) {
        if (requestGenerationRef.current !== requestGeneration) return;
        addErrorToast(addToast, 'DBカタログ取得', 'DB一覧を取得できませんでした', error);
        setDbTables([]);
        setCurrentTable('');
        setTableData(emptyTableData);
      } finally {
        if (requestGenerationRef.current === requestGeneration) {
          setIsDbLoading(false);
        }
      }
    },
    [addToast, currentTable, loadTableDataForRequest],
  );

  const totalPages = Math.ceil(tableData.total_rows / pageSize) || 1;

  return {
    dbTables,
    currentTable,
    tableData,
    isDbLoading,
    currentPage,
    totalPages,
    pageSize,
    sortState,
    loadTableData,
    goToPage,
    toggleSort,
    loadDatabaseCatalog,
  };
}
