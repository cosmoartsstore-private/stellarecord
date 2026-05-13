import { useState } from 'react';
import type { DbTableSummary, TableData } from '../models/types';
import type { SortState } from '../viewmodels/useDatabaseState';
import shared from '../../../shared/styles/shared.module.css';
import styles from './DatabaseSection.module.css';

/** DBプレビューパネルのProps */
interface DatabaseSectionProps {
  dbTables: DbTableSummary[];
  currentTable: string;
  tableData: TableData;
  isDbLoading: boolean;
  currentPage: number;
  totalPages: number;
  sortState: SortState | null;
  onSelectTable: (tableName: string) => void;
  onGoToPage: (page: number) => void;
  onToggleSort: (columnName: string) => void;
}

/** サイドバーテーブル選択とページネーション付きの読み取り専用DBプレビュー */
export function DatabaseSection({
  dbTables,
  currentTable,
  tableData,
  isDbLoading,
  currentPage,
  totalPages,
  sortState,
  onSelectTable,
  onGoToPage,
  onToggleSort,
}: DatabaseSectionProps) {
  const [showPhysicalNames, setShowPhysicalNames] = useState(false);

  const selectedTableSummary = dbTables.find((table) => table.name === currentTable) ?? null;

  const PAGE_SIZE = 500;
  const rangeStart = tableData.total_rows > 0 ? currentPage * PAGE_SIZE + 1 : 0;
  const rangeEnd = Math.min((currentPage + 1) * PAGE_SIZE, tableData.total_rows);

  const tables = dbTables.filter((t) => !t.is_view);
  const views = dbTables.filter((t) => t.is_view);

  const renderSidebarItem = (table: DbTableSummary) => {
    const isActive = currentTable === table.name;
    return (
      <button
        key={table.name}
        type="button"
        onClick={() => { onSelectTable(table.name); }}
        className={`${styles.tableItem} ${isActive ? styles.tableItemActive : ''}`}

      >
        <span className={styles.tableItemLabel}>
          {showPhysicalNames ? table.name : table.label}
        </span>
      </button>
    );
  };

  return (
    <div className={`${styles.root} ${shared.viewContainer}`}>
      <div className={shared.sectionHeader}>
        <h2>DBプレビュー</h2>
      </div>

      <div className={styles.layout}>
        <div className={styles.sidebar}>
          <div className={styles.sidebarTop}>
            <div className={styles.sidebarLabel}>テーブル一覧</div>
            <button
              type="button"
              className={styles.nameToggle}
              onClick={() => { setShowPhysicalNames((prev) => !prev); }}
            >
              {showPhysicalNames ? 'Physical' : 'Logical'}
            </button>
          </div>
          {dbTables.length === 0 && (
            <div className={styles.sidebarEmpty}>表示できるテーブルがまだありません</div>
          )}
          <div className={styles.tableList}>
            {tables.map(renderSidebarItem)}
            {views.length > 0 && (
              <>
                <div className={styles.sidebarDivider}>ビュー</div>
                {views.map(renderSidebarItem)}
              </>
            )}
          </div>
        </div>

        <div className={styles.content}>
          <div className={styles.contentHeader}>
            <div className={styles.contentHeading}>
              <h3>
                {tableData.label.length > 0
                  ? tableData.label
                  : selectedTableSummary && selectedTableSummary.label.length > 0
                    ? selectedTableSummary.label
                    : 'DB プレビュー'}
              </h3>
              {selectedTableSummary && selectedTableSummary.description.length > 0 && (
                <span className={styles.headerDesc}>{selectedTableSummary.description}</span>
              )}
              <span className={styles.headerMeta}>
                {tableData.total_rows > 0
                  ? `${String(rangeStart)}～${String(rangeEnd)} / 全 ${String(tableData.total_rows)} 件`
                  : '0 件'}
              </span>
            </div>
            {tableData.total_rows > 0 && (
              <div className={styles.pagination}>
                <button
                  type="button"
                  className={shared.btn}
                  onClick={() => { onGoToPage(currentPage - 1); }}
                  disabled={isDbLoading || currentPage === 0}
                >
                  ←
                </button>
                <span className={styles.paginationInfo}>
                  {String(currentPage + 1)} / {String(totalPages)}
                </span>
                <button
                  type="button"
                  className={shared.btn}
                  onClick={() => { onGoToPage(currentPage + 1); }}
                  disabled={isDbLoading || currentPage >= totalPages - 1}
                >
                  →
                </button>
              </div>
            )}
          </div>

          <div className={styles.tableWrap}>
            {isDbLoading && <div className={styles.loadingState}>読み込み中...</div>}

            {!isDbLoading && (
              <table className={styles.table}>
                <thead>
                  <tr>
                    {tableData.columns.map((column) => {
                      const isSorted = sortState?.column === column.name;
                      return (
                        <th
                          key={column.name}
                          className={styles.sortableHeader}
                          onClick={() => { onToggleSort(column.name); }}
                        >
                          <div className={styles.columnLabel}>
                            {column.label}
                            <span className={`${styles.sortArrow} ${isSorted ? styles.sortArrowActive : ''}`}>
                              {isSorted ? (sortState.dir === 'asc' ? '▲' : '▼') : '▲'}
                            </span>
                          </div>
                        </th>
                      );
                    })}
                  </tr>
                </thead>
                <tbody>
                  {tableData.columns.length === 0 && (
                    <tr>
                      <td colSpan={1} className={styles.empty}>
                        テーブルを選択してください
                      </td>
                    </tr>
                  )}
                  {tableData.columns.length > 0 && tableData.rows.length === 0 && (
                    <tr>
                      <td colSpan={tableData.columns.length} className={styles.empty}>
                        データが存在しません
                      </td>
                    </tr>
                  )}
                  {tableData.rows.map((row, rowIndex) => (
                    <tr key={`${tableData.name}-${String(rowIndex)}`}>
                      {row.map((cell, cellIndex) => (
                        <td key={`${tableData.name}-${String(rowIndex)}-${String(cellIndex)}`}>
                          {cell}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
