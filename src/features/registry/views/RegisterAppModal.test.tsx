// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { installDialogBoundary } from '../../../test/dialog';
import { extractExeDisplayName, pickExeFile } from '../services/registryService';
import { RegisterAppModal } from './RegisterAppModal';

vi.mock('../services/registryService', () => ({ extractExeDisplayName: vi.fn(), pickExeFile: vi.fn() }));
installDialogBoundary();
beforeEach(() => { vi.resetAllMocks(); vi.mocked(pickExeFile).mockResolvedValue('C:/Apps/Sample.exe'); vi.mocked(extractExeDisplayName).mockResolvedValue('Sample'); });
afterEach(cleanup);

describe('アプリ登録入力', () => {
  it('実行ファイルを選択して名前を補完し、登録時は名前だけtrimする', async () => {
    const confirm = vi.fn();
    render(<RegisterAppModal onClose={vi.fn()} onConfirm={confirm} />);
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '登録' }).disabled).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: '参照' }));
    await waitFor(() => { expect(screen.getByLabelText<HTMLInputElement>('アプリ名').value).toBe('Sample'); });
    fireEvent.change(screen.getByLabelText('アプリ名'), { target: { value: '  独自名  ' } });
    fireEvent.change(screen.getByLabelText('説明（任意）'), { target: { value: '  説明  ' } });
    fireEvent.click(screen.getByRole('button', { name: '登録' }));
    expect(confirm).toHaveBeenCalledWith('C:/Apps/Sample.exe', '独自名', '  説明  ');
    vi.mocked(pickExeFile).mockResolvedValueOnce('C:/Apps/Other.exe');
    fireEvent.click(screen.getByRole('button', { name: '参照' }));
    await waitFor(() => { expect(screen.getByPlaceholderText<HTMLInputElement>('ファイルを選択してください').value).toBe('C:/Apps/Other.exe'); });
    expect(screen.getByLabelText<HTMLInputElement>('アプリ名').value).toBe('  独自名  ');
    expect(extractExeDisplayName).toHaveBeenCalledOnce();
  });

  it('自動補完のままなら再選択で更新し、表示名取得失敗時はexe名を使用する', async () => {
    render(<RegisterAppModal onClose={vi.fn()} onConfirm={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: '参照' }));
    await waitFor(() => { expect(screen.getByLabelText<HTMLInputElement>('アプリ名').value).toBe('Sample'); });
    vi.mocked(pickExeFile).mockResolvedValueOnce('C:\\Apps\\Other.EXE');
    vi.mocked(extractExeDisplayName).mockRejectedValueOnce(new Error('VersionInfoなし'));
    fireEvent.click(screen.getByRole('button', { name: '参照' }));
    await waitFor(() => { expect(screen.getByLabelText<HTMLInputElement>('アプリ名').value).toBe('Other'); });
  });

  it.each(['resolve', 'reject'] as const)('名前取得待ちに手入力した表示名を、遅れた%sで上書きしない', async (outcome) => {
    let resolveName: (value: string) => void = () => { throw new Error('表示名取得前'); };
    let rejectName: (reason: Error) => void = () => { throw new Error('表示名取得前'); };
    vi.mocked(extractExeDisplayName).mockReturnValueOnce(new Promise((resolve, reject) => { resolveName = resolve; rejectName = reject; }));
    const confirm = vi.fn();
    render(<RegisterAppModal onClose={vi.fn()} onConfirm={confirm} />);
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: '参照' })); await Promise.resolve(); });
    expect(screen.getByPlaceholderText<HTMLInputElement>('ファイルを選択してください').value).toBe('C:/Apps/Sample.exe');
    fireEvent.change(screen.getByLabelText('アプリ名'), { target: { value: '手入力した表示名' } });
    await act(async () => { if (outcome === 'resolve') resolveName('取得した表示名'); else rejectName(new Error('表示名取得拒否')); await Promise.resolve(); });
    expect(screen.getByLabelText<HTMLInputElement>('アプリ名').value).toBe('手入力した表示名');
    fireEvent.click(screen.getByRole('button', { name: '登録' }));
    expect(confirm).toHaveBeenCalledExactlyOnceWith('C:/Apps/Sample.exe', '手入力した表示名', '');
  });

  it.each(['resolve', 'reject'] as const)('選び直したEXEの名前と登録対象を、前のEXEの遅れた%sで変更しない', async (outcome) => {
    let resolveOldName: (value: string) => void = () => { throw new Error('以前の表示名取得前'); };
    let rejectOldName: (reason: Error) => void = () => { throw new Error('以前の表示名取得前'); };
    vi.mocked(extractExeDisplayName).mockReturnValueOnce(new Promise((resolve, reject) => { resolveOldName = resolve; rejectOldName = reject; }));
    const confirm = vi.fn();
    render(<RegisterAppModal onClose={vi.fn()} onConfirm={confirm} />);
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: '参照' })); await Promise.resolve(); });
    vi.mocked(pickExeFile).mockResolvedValueOnce('C:/Apps/Current.exe');
    vi.mocked(extractExeDisplayName).mockResolvedValueOnce('現在のアプリ');
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: '参照' })); await Promise.resolve(); });
    expect(screen.getByLabelText<HTMLInputElement>('アプリ名').value).toBe('現在のアプリ');
    await act(async () => { if (outcome === 'resolve') resolveOldName('以前のアプリ'); else rejectOldName(new Error('以前の表示名取得拒否')); await Promise.resolve(); });
    expect(screen.getByLabelText<HTMLInputElement>('アプリ名').value).toBe('現在のアプリ');
    expect(screen.getByPlaceholderText<HTMLInputElement>('ファイルを選択してください').value).toBe('C:/Apps/Current.exe');
    fireEvent.click(screen.getByRole('button', { name: '登録' }));
    expect(confirm).toHaveBeenCalledExactlyOnceWith('C:/Apps/Current.exe', '現在のアプリ', '');
  });

  it.each([null, new Error('選択拒否')])('選択キャンセル・失敗では入力を変えない: %s', async (outcome) => {
    if (outcome instanceof Error) vi.mocked(pickExeFile).mockRejectedValueOnce(outcome);
    else vi.mocked(pickExeFile).mockResolvedValueOnce(outcome);
    const close = vi.fn();
    render(<RegisterAppModal onClose={close} onConfirm={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: '参照' }));
    await waitFor(() => { expect(pickExeFile).toHaveBeenCalledOnce(); });
    expect(extractExeDisplayName).not.toHaveBeenCalled();
    expect(screen.getByRole<HTMLButtonElement>('button', { name: '登録' }).disabled).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'キャンセル' }));
    expect(close).toHaveBeenCalledOnce();
  });
});
