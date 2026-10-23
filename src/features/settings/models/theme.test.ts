import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { readInitialTheme, saveTheme } from './theme';

describe('readInitialTheme', () => {
  let storage: Record<string, string>;

  beforeEach(() => {
    storage = {};
    vi.stubGlobal('window', {
      localStorage: {
        getItem: (key: string) => storage[key] ?? null,
        setItem: (key: string, value: string) => {
          storage[key] = value;
        },
      },
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  test('returns light when no stored value', () => {
    expect(readInitialTheme()).toBe('light');
  });

  test('returns stored dark theme', () => {
    storage['stella-record-theme'] = 'dark';
    expect(readInitialTheme()).toBe('dark');
  });

  test('削除済みのテーマ識別子はlightとして扱う', () => {
    storage['stella-record-theme'] = 'midnight';
    expect(readInitialTheme()).toBe('light');
  });

  test('returns light for invalid stored value', () => {
    storage['stella-record-theme'] = 'neon';
    expect(readInitialTheme()).toBe('light');
  });

  test('returns light for empty stored value', () => {
    storage['stella-record-theme'] = '';
    expect(readInitialTheme()).toBe('light');
  });
});

describe('saveTheme', () => {
  let storage: Record<string, string>;

  beforeEach(() => {
    storage = {};
    vi.stubGlobal('window', {
      localStorage: {
        getItem: (key: string) => storage[key] ?? null,
        setItem: (key: string, value: string) => {
          storage[key] = value;
        },
      },
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  test('saves theme to localStorage', () => {
    saveTheme('dark');
    expect(storage['stella-record-theme']).toBe('dark');
  });

  test('overwrites previous theme', () => {
    saveTheme('dark');
    saveTheme('light');
    expect(storage['stella-record-theme']).toBe('light');
  });

  test('windowがない実行環境では保存せず、初期themeをlightにする', () => {
    vi.stubGlobal('window', undefined);
    expect(readInitialTheme()).toBe('light');
    expect(() => { saveTheme('dark'); }).not.toThrow();
    expect(storage).toEqual({});
  });
});
