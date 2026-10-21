// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { emptyTableData } from '../models/types';
import { DatabaseSection } from './DatabaseSection';

afterEach(cleanup);
const tables = [
  { name: 'logs', label: 'ログ', description: '取り込み済みログ', storage: 'db', is_view: false },
  { name: 'visits', label: '訪問', description: '', storage: 'db', is_view: true },
];
function props() {
  return { dbTables: tables, currentTable: 'logs', tableData: { ...emptyTableData, name: 'logs', columns: [{ name: 'value', label: '値', description: '' }], rows: [['<script>ログ</script>']], total_rows: 1001 }, isDbLoading: false, currentPage: 0, totalPages: 3, pageSize: 500, sortState: null, onSelectTable: vi.fn(), onGoToPage: vi.fn(), onToggleSort: vi.fn() };
}

describe('DatabaseSection', () => {
  it('テーブルとビューを表示し、表示名だけを切り替えて物理名を要求へ渡す', () => {
    const input = props();
    render(<DatabaseSection {...input} />);
    expect(screen.getByRole('heading', { name: 'ログ' })).toBeTruthy();
    expect(screen.getByText('取り込み済みログ')).toBeTruthy();
    expect(screen.getByText('ビュー')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'ログ' }).getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(screen.getByRole('button', { name: '訪問' }));
    expect(input.onSelectTable).toHaveBeenCalledWith('visits');
    fireEvent.click(screen.getByRole('button', { name: 'Logical' }));
    fireEvent.click(screen.getByRole('button', { name: 'logs' }));
    expect(input.onSelectTable).toHaveBeenLastCalledWith('logs');
    expect(screen.getByRole('button', { name: 'Physical' }).getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByRole('cell', { name: '<script>ログ</script>' }).querySelector('script')).toBeNull();
  });

  it('ページ境界と読み込み中の操作を制限し、表示範囲を実際の件数で止める', () => {
    const input = props();
    const { rerender } = render(<DatabaseSection {...input} />);
    expect(screen.getByText('1～500 / 全 1001 件')).toBeTruthy();
    act(() => { screen.getByRole<HTMLButtonElement>('button', { name: '前のページ' }).click(); });
    expect(input.onGoToPage).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '次のページ' }));
    expect(input.onGoToPage).toHaveBeenCalledWith(1);
    rerender(<DatabaseSection {...input} currentPage={2} />);
    expect(screen.getByText('1001～1001 / 全 1001 件')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: '前のページ' }));
    expect(input.onGoToPage).toHaveBeenLastCalledWith(1);
    act(() => { screen.getByRole<HTMLButtonElement>('button', { name: '次のページ' }).click(); });
    expect(input.onGoToPage).toHaveBeenCalledTimes(2);
    rerender(<DatabaseSection {...input} currentPage={1} isDbLoading />);
    expect(screen.getByText('読み込み中...')).toBeTruthy();
    expect(screen.queryByRole('table')).toBeNull();
    for (const name of ['前のページ', '次のページ']) act(() => { screen.getByRole<HTMLButtonElement>('button', { name }).click(); });
    expect(input.onGoToPage).toHaveBeenCalledTimes(2);
  });

  it('columnのソート表示と要求の物理名が一致する', () => {
    const input = props();
    const { rerender } = render(<DatabaseSection {...input} />);
    expect(screen.getByRole('columnheader').getAttribute('aria-sort')).toBe('none');
    fireEvent.click(screen.getByRole('button', { name: '値で並べ替え' }));
    expect(input.onToggleSort).toHaveBeenCalledWith('value');
    rerender(<DatabaseSection {...input} sortState={{ column: 'value', dir: 'asc' }} />);
    expect(screen.getByRole('columnheader').getAttribute('aria-sort')).toBe('ascending');
    rerender(<DatabaseSection {...input} sortState={{ column: 'value', dir: 'desc' }} />);
    expect(screen.getByRole('columnheader').getAttribute('aria-sort')).toBe('descending');
  });

  it('未選択・空テーブル・カタログ欠落を区別し、空データからページ操作を出さない', () => {
    const input = props();
    const { rerender } = render(<DatabaseSection {...input} dbTables={[]} currentTable="" tableData={emptyTableData} />);
    expect(screen.getByText('表示できるテーブルがまだありません')).toBeTruthy();
    expect(screen.getByText('テーブルを選択してください')).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'DB プレビュー' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: '次のページ' })).toBeNull();
    rerender(<DatabaseSection {...input} tableData={{ ...input.tableData, label: '独自ラベル', rows: [], total_rows: 0 }} />);
    expect(screen.getByText('データが存在しません')).toBeTruthy();
    expect(screen.getByRole('heading', { name: '独自ラベル' })).toBeTruthy();
    expect(screen.getByText('0 件')).toBeTruthy();
  });
});
