import { describe, expect, test } from 'vitest';
import { formatFileSize } from './byteFormat';

describe('formatFileSize', () => {
  test('バイト単位で表示する', () => {
    expect(formatFileSize(0)).toBe('0 B');
    expect(formatFileSize(500)).toBe('500 B');
  });

  test('キロバイト単位で表示する', () => {
    expect(formatFileSize(1_024)).toBe('1.0 KB');
    expect(formatFileSize(1_536)).toBe('1.5 KB');
  });

  test('メガバイト単位で表示する', () => {
    expect(formatFileSize(1_048_576)).toBe('1.00 MB');
    expect(formatFileSize(5_767_168)).toBe('5.50 MB');
  });

  test('ギガバイト単位で表示する', () => {
    expect(formatFileSize(1_073_741_824)).toBe('1.00 GB');
    expect(formatFileSize(2_684_354_560)).toBe('2.50 GB');
  });

  test('各単位の境界直前では小さい単位を使う', () => {
    expect(formatFileSize(1_023)).toBe('1023 B');
    expect(formatFileSize(1_048_575)).toBe('1024.0 KB');
    expect(formatFileSize(1_073_741_823)).toBe('1024.00 MB');
  });
});
