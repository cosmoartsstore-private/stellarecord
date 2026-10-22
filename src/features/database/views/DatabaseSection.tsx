import { getMsg } from '../../../messages/getMsg';
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
  pageSize: number;
  sortState: SortState | null;
  onSelectTable: (tableName: string) => void;
  onGoToPage: (page: number) => void;
  onToggleSort: (columnName: string) => void;
}

/** サイドバーテーブル選択とページネーション付きの読み取り専用DBプレビュー */
export function DatabaseSection({ dbTables, currentTable, tableData, isDbLoading, currentPage, totalPages, pageSize, sortState, onSelectTable, onGoToPage, onToggleSort }: DatabaseSectionProps) {
  const [isPhysicalNames, setIsPhysicalNames] = useState(false);

  const selectedTableSummary = dbTables.find((table) => table.name === currentTable) ?? null;

  const rangeStart = tableData.total_rows > 0 ? currentPage * pageSize + 1 : 0;
  const rangeEnd = Math.min((currentPage + 1) * pageSize, tableData.total_rows);

  const tables = dbTables.filter((t) => !t.is_view);
  const views = dbTables.filter((t) => t.is_view);

  /** サイドバーに表示する 1 テーブル分のボタンを描画する。 */
  const renderSidebarItem = (table: DbTableSummary) => {
    const isActive = currentTable === table.name;
    return (
      <button key={table.name} type="button" onClick={() => {
          onSelectTable(table.name);
        }} aria-pressed={isActive} className={`${styles.tableItem} ${isActive ? styles.tableItemActive : ''}`}>
        <span className={styles.tableItemLabel}>{isPhysicalNames ? table.name : table.label}</span>
      </button>
    );
  };

  return (
    <div className={`${styles.root} ${shared.viewContainer}`}>
      <div className={shared.sectionHeader}>
        <h2>{getMsg('DatabaseSection.title')}</h2>
      </div>

      <div className={styles.layout}>
        <div className={styles.sidebar}>
          <div className={styles.sidebarTop}>
            <div className={styles.sidebarLabel}>{getMsg('DatabaseSection.tableList')}</div>
            <button type="button" className={styles.nameToggle} onClick={() => {
                setIsPhysicalNames((prev) => !prev);
              }} aria-pressed={isPhysicalNames}>
              {isPhysicalNames ? 'Physical' : 'Logical'}
            </button>
          </div>
          {dbTables.length === 0 && (
            <div className={styles.sidebarEmpty}>{getMsg('DatabaseSection.emptyCatalog')}</div>
          )}
          <div className={styles.tableList}>
            {tables.map(renderSidebarItem)}
            {views.length > 0 && (
              <>
                <div className={styles.sidebarDivider}>{getMsg('DatabaseSection.viewBadge')}</div>
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
                    : getMsg('DatabaseSection.tableLabel')}
              </h3>
              {selectedTableSummary && selectedTableSummary.description.length > 0 && (
                <span className={styles.headerDesc}>{selectedTableSummary.description}</span>
              )}
              <span className={styles.headerMeta}>
                {tableData.total_rows > 0
                  ? getMsg('DatabaseSection.rowRange', { start: String(rangeStart), end: String(rangeEnd), total: String(tableData.total_rows) })
                  : getMsg('DatabaseSection.emptyRange')}
              </span>
            </div>
            {tableData.total_rows > 0 && (
              <div className={styles.pagination}>
                <button type="button" className={shared.btn} onClick={() => {
                    onGoToPage(currentPage - 1);
                  }} disabled={isDbLoading || currentPage === 0} aria-label={getMsg('DatabaseSection.previousPage')}>
                  ←
                </button>
                <span className={styles.paginationInfo}>
                  {String(currentPage + 1)} / {String(totalPages)}
                </span>
                <button type="button" className={shared.btn} onClick={() => {
                    onGoToPage(currentPage + 1);
                  }} disabled={isDbLoading || currentPage >= totalPages - 1} aria-label={getMsg('DatabaseSection.nextPage')}>
                  →
                </button>
              </div>
            )}
          </div>

          <div className={styles.tableWrap}>
            {isDbLoading && <div className={styles.loadingState}>{getMsg('common.loadingAscii')}</div>}

            {!isDbLoading && (
              <table className={styles.table}>
                <thead>
                  <tr>
                    {tableData.columns.map((column) => {
                      const isSorted = sortState?.column === column.name;
                      return (
                        <th key={column.name} className={styles.sortableHeader} aria-sort={
                            isSorted
                              ? sortState.dir === 'asc'
                                ? 'ascending'
                                : 'descending'
                              : 'none'
                          }>
                          <button type="button" className={styles.sortButton} onClick={() => {
                              onToggleSort(column.name);
                            }} aria-label={getMsg('DatabaseSection.sortColumn', { label: column.label })}>
                            <span className={styles.columnLabel}>
                              {column.label}
                              <span aria-hidden="true" className={`${styles.sortArrow} ${isSorted ? styles.sortArrowActive : ''}`}>
                                {isSorted ? (sortState.dir === 'asc' ? '▲' : '▼') : '▲'}
                              </span>
                            </span>
                          </button>
                        </th>
                      );
                    })}
                  </tr>
                </thead>
                <tbody>
                  {tableData.columns.length === 0 && (
                    <tr>
                      <td colSpan={1} className={styles.empty}>{getMsg('DatabaseSection.selectTable')}</td>
                    </tr>
                  )}
                  {tableData.columns.length > 0 && tableData.rows.length === 0 && (
                    <tr>
                      <td colSpan={tableData.columns.length} className={styles.empty}>{getMsg('DatabaseSection.noData')}</td>
                    </tr>
                  )}
                  {tableData.rows.map((row, rowIndex) => (
                    <tr key={`${tableData.name}-${String(rowIndex)}`}>
                      {row.map((cell, cellIndex) => (
                        <td key={`${tableData.name}-${String(rowIndex)}-${String(cellIndex)}`}>{cell}</td>
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
