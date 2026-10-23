// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DbTableSummary, TableData } from '../models/types';
import { loadDbTableData, loadDbTables } from '../services/databaseService';
import { useDatabaseState } from './useDatabaseState';

vi.mock('../services/databaseService', () => ({
  loadDbTableData: vi.fn(),
  loadDbTables: vi.fn(),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(cleanup);

function createDeferred<T>() {
  let resolvePromise: (value: T) => void = () => {
    throw new Error('Promiseの初期化前にresolveされました');
  };
  const promise = new Promise<T>((resolve) => {
    resolvePromise = resolve;
  });
  return { promise, resolve: resolvePromise };
}

function createTableData(name: string, value: string): TableData {
  return {
    name,
    label: name,
    description: '',
    storage: '',
    columns: [{ name: 'value', label: '値', description: '' }],
    rows: [[value]],
    total_rows: 1,
  };
}

const loadDbTableDataMock = vi.mocked(loadDbTableData);
const loadDbTablesMock = vi.mocked(loadDbTables);

describe('useDatabaseState', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(invoke).mockResolvedValue(undefined);
    loadDbTablesMock.mockResolvedValue([]);
  });

  it('未選択時のページ・ソート操作では取得せず、カタログの優先指定と既存選択を維持する', async () => {
    const tables = ['logs', 'visits'].map((name) => ({ name, label: name, description: '', storage: '', is_view: false }));
    loadDbTablesMock.mockResolvedValue(tables);
    loadDbTableDataMock.mockImplementation((name) => Promise.resolve(createTableData(name, name)));
    const { result } = renderHook(() => useDatabaseState(vi.fn()));
    await act(async () => { await result.current.goToPage(1); await result.current.toggleSort('value'); });
    expect(loadDbTableDataMock).not.toHaveBeenCalled();
    await act(async () => { await result.current.loadDatabaseCatalog('visits'); });
    expect(result.current.currentTable).toBe('visits');
    expect(result.current.dbTables).toEqual(tables);
    await act(async () => { await result.current.loadDatabaseCatalog(); });
    expect(loadDbTableDataMock).toHaveBeenLastCalledWith('visits', 0, undefined, undefined);
    loadDbTablesMock.mockResolvedValue(tables.slice(0, 1));
    await act(async () => { await result.current.loadDatabaseCatalog(); });
    expect(result.current.currentTable).toBe('logs');
  });

  it('500件単位のページとソートの昇順・降順・解除を同じテーブルへ渡す', async () => {
    loadDbTableDataMock.mockImplementation((name) => Promise.resolve({ ...createTableData(name, '値'), total_rows: 1001 }));
    const { result } = renderHook(() => useDatabaseState(vi.fn()));
    await act(async () => { await result.current.loadTableData('logs'); });
    expect(result.current.totalPages).toBe(3);
    await act(async () => { await result.current.toggleSort('value'); });
    expect(result.current.sortState).toEqual({ column: 'value', dir: 'asc' });
    await act(async () => { await result.current.goToPage(2); });
    expect(loadDbTableDataMock).toHaveBeenLastCalledWith('logs', 2, 'value', 'asc');
    expect(result.current.currentPage).toBe(2);
    await act(async () => { await result.current.toggleSort('value'); });
    expect(result.current.sortState).toEqual({ column: 'value', dir: 'desc' });
    expect(result.current.currentPage).toBe(0);
    await act(async () => { await result.current.toggleSort('value'); });
    expect(result.current.sortState).toBeNull();
    expect(loadDbTableDataMock).toHaveBeenLastCalledWith('logs', 0, undefined, undefined);
  });

  it('テーブル取得失敗では確定済みデータを保ち、カタログ失敗では全選択状態を消す', async () => {
    const addToast = vi.fn();
    loadDbTableDataMock.mockResolvedValue(createTableData('logs', '保存済み'));
    const { result } = renderHook(() => useDatabaseState(addToast));
    await act(async () => { await result.current.loadTableData('logs', 1, { column: 'value', dir: 'desc' }); });
    loadDbTableDataMock.mockRejectedValueOnce(new Error('取得不能'));
    await act(async () => { await result.current.loadTableData('visits'); });
    expect(result.current.currentTable).toBe('logs');
    expect(result.current.tableData.rows).toEqual([['保存済み']]);
    expect(result.current.currentPage).toBe(1);
    expect(addToast).toHaveBeenCalledWith('テーブルのデータを読み込めませんでした');
    loadDbTablesMock.mockRejectedValueOnce(new Error('カタログ不能'));
    await act(async () => { await result.current.loadDatabaseCatalog(); });
    expect(result.current).toMatchObject({ currentTable: '', currentPage: 0, sortState: null, dbTables: [], isDbLoading: false, totalPages: 1 });
    expect(result.current.tableData.total_rows).toBe(0);
    expect(addToast).toHaveBeenLastCalledWith('テーブル一覧を取得できませんでした');
  });

  it('空のカタログは古いページ・ソート・行を残さない', async () => {
    loadDbTableDataMock.mockResolvedValue(createTableData('logs', '旧データ'));
    const { result } = renderHook(() => useDatabaseState(vi.fn()));
    await act(async () => { await result.current.loadTableData('logs', 2, { column: 'value', dir: 'asc' }); });
    await act(async () => { await result.current.loadDatabaseCatalog(); });
    expect(result.current).toMatchObject({ currentTable: '', currentPage: 0, sortState: null, isDbLoading: false, dbTables: [] });
    expect(result.current.tableData.rows).toEqual([]);
  });

  it('先に開始した取得が後から完了しても、最新データとloadingを上書きしない', async () => {
    const oldResponse = createDeferred<TableData>();
    const latestResponse = createDeferred<TableData>();
    loadDbTableDataMock
      .mockReturnValueOnce(oldResponse.promise)
      .mockReturnValueOnce(latestResponse.promise);
    const addToast = vi.fn();
    const { result } = renderHook(() => useDatabaseState(addToast));
    let oldRequest = Promise.resolve();
    let latestRequest = Promise.resolve();

    act(() => {
      oldRequest = result.current.loadTableData('old_table');
    });
    act(() => {
      latestRequest = result.current.loadTableData('latest_table');
    });

    expect(result.current.currentTable).toBe('latest_table');
    expect(result.current.isDbLoading).toBe(true);

    await act(async () => {
      oldResponse.resolve(createTableData('old_table', 'old'));
      await oldRequest;
    });

    expect(result.current.tableData.name).toBe('');
    expect(result.current.currentTable).toBe('latest_table');
    expect(result.current.isDbLoading).toBe(true);

    await act(async () => {
      latestResponse.resolve(createTableData('latest_table', 'latest'));
      await latestRequest;
    });

    expect(result.current.tableData).toMatchObject({
      name: 'latest_table',
      rows: [['latest']],
    });
    expect(result.current.isDbLoading).toBe(false);
    expect(addToast).not.toHaveBeenCalled();
  });

  it('古いカタログ応答が進行中のテーブル取得状態を上書きしない', async () => {
    const catalogResponse = createDeferred<DbTableSummary[]>();
    const tableResponse = createDeferred<TableData>();
    loadDbTablesMock.mockReturnValueOnce(catalogResponse.promise);
    loadDbTableDataMock.mockReturnValueOnce(tableResponse.promise);
    const { result } = renderHook(() => useDatabaseState(vi.fn()));
    let catalogRequest = Promise.resolve();
    let tableRequest = Promise.resolve();

    act(() => {
      catalogRequest = result.current.loadDatabaseCatalog();
    });
    act(() => {
      tableRequest = result.current.loadTableData('latest_table');
    });

    await act(async () => {
      catalogResponse.resolve([
        {
          name: 'old_table',
          label: '旧テーブル',
          description: '',
          storage: '',
          is_view: false,
        },
      ]);
      await catalogRequest;
    });

    expect(result.current.currentTable).toBe('latest_table');
    expect(result.current.tableData.name).toBe('');
    expect(result.current.isDbLoading).toBe(true);

    await act(async () => {
      tableResponse.resolve(createTableData('latest_table', 'latest'));
      await tableRequest;
    });

    expect(result.current.tableData.name).toBe('latest_table');
    expect(result.current.isDbLoading).toBe(false);
  });
});
