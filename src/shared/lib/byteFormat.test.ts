import { describe, expect, test } from 'vitest';
import { byteUnitBytes, formatFileSize } from './byteFormat';

describe('formatFileSize', () => {
  test('バイト単位で表示する', () => {
    expect(formatFileSize(0)).toBe('0 B');
    expect(formatFileSize(500)).toBe('500 B');
  });

  test('キロバイト単位で表示する', () => {
    expect(formatFileSize(byteUnitBytes.KB)).toBe('1.0 KB');
    expect(formatFileSize(1.5 * byteUnitBytes.KB)).toBe('1.5 KB');
  });

  test('メガバイト単位で表示する', () => {
    expect(formatFileSize(byteUnitBytes.MB)).toBe('1.00 MB');
    expect(formatFileSize(5.5 * byteUnitBytes.MB)).toBe('5.50 MB');
  });

  test('ギガバイト単位で表示する', () => {
    expect(formatFileSize(byteUnitBytes.GB)).toBe('1.00 GB');
    expect(formatFileSize(2.5 * byteUnitBytes.GB)).toBe('2.50 GB');
  });

  test('各単位の境界直前では小さい単位を使う', () => {
    expect(formatFileSize(byteUnitBytes.KB - 1)).toBe('1023 B');
    expect(formatFileSize(byteUnitBytes.MB - 1)).toBe('1024.0 KB');
    expect(formatFileSize(byteUnitBytes.GB - 1)).toBe('1024.00 MB');
  });
});
