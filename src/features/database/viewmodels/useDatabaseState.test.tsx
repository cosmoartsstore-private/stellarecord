// @vitest-environment jsdom

import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DbTableSummary, TableData } from '../models/types';
import { loadDbTableData, loadDbTables } from '../services/databaseService';
import { useDatabaseState } from './useDatabaseState';

vi.mock('../services/databaseService', () => ({
  loadDbTableData: vi.fn(),
  loadDbTables: vi.fn(),
}));

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
    loadDbTablesMock.mockResolvedValue([]);
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
