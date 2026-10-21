// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AnalyzeSection } from './AnalyzeSection';

afterEach(cleanup);

function analyzeProps() {
  return { storageStatus: { current: 1024, limit: 2048, percent: 50 }, isAnalyzeRunning: false, analyzeProgress: '', analyzeStatus: '', onRefreshStorage: vi.fn(), onOpenEnhancedSync: vi.fn(), onOpenLogViewer: vi.fn(), onCancelSync: vi.fn(), onOpenCleanup: vi.fn() };
}

describe('解析画面', () => {
  it('容量更新・復元・閲覧・削除を別々の操作へ接続する', () => {
    const props = analyzeProps();
    render(<AnalyzeSection {...props} />);
    fireEvent.click(screen.getByRole('button', { name: 'アーカイブ容量を更新' }));
    fireEvent.click(screen.getByRole('button', { name: 'ログを選択' }));
    fireEvent.click(screen.getByRole('button', { name: 'ログを開く' }));
    fireEvent.click(screen.getByRole('button', { name: '元ログの削除' }));
    expect(props.onRefreshStorage).toHaveBeenCalledOnce();
    expect(props.onOpenEnhancedSync).toHaveBeenCalledOnce();
    expect(props.onOpenLogViewer).toHaveBeenCalledOnce();
    expect(props.onOpenCleanup).toHaveBeenCalledOnce();
    expect(screen.queryByRole('progressbar', { name: 'ログ取り込み' })).toBeNull();
  });

  it.each([['1/4', 25], ['4/2', 100], ['-1/2', 0], ['1/0', 0], ['x/4', 0], ['1/x', 0], ['待機中', 0], ['', 0]])('取り込み進捗を表示範囲へ収め、処理中の再取込を禁止する: %s', (progress, percent) => {
    const props = analyzeProps();
    render(<AnalyzeSection {...props} isAnalyzeRunning analyzeProgress={progress} analyzeStatus="読込中" />);
    expect(screen.getByRole('progressbar', { name: 'ログ取り込み' }).getAttribute('aria-valuenow')).toBe(String(percent));
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'ログを選択' }).disabled).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: '停止' }));
    expect(props.onCancelSync).toHaveBeenCalledOnce();
    expect(screen.getByText('読込中')).toBeTruthy();
  });

  it.each([[-20, 0], [150, 100], [Number.NaN, 0], [Number.POSITIVE_INFINITY, 0]])('不正・範囲外の容量割合も描画幅を0～100へ収める: %s', (percent, expected) => {
    render(<AnalyzeSection {...analyzeProps()} storageStatus={{ current: 1024, limit: 20 * 1024 ** 3, percent }} />);
    const meter = screen.getByRole('progressbar', { name: 'アーカイブ容量' });
    expect(meter.getAttribute('aria-valuenow')).toBe(String(expected));
    expect(meter.querySelector<HTMLElement>('div')?.style.width).toBe(`${String(expected)}%`);
  });

  it.each(['', '20/20'])('停止後の状態はprogress有無に応じて表示し、中断操作を残さない: %s', (progress) => {
    render(<AnalyzeSection {...analyzeProps()} analyzeStatus="完了" analyzeProgress={progress} />);
    expect(screen.getByText(progress ? '完了 (20/20)' : '完了')).toBeTruthy();
    expect(screen.queryByRole('button', { name: '停止' })).toBeNull();
  });
});
