import { describe, expect, test } from 'vitest';
import { formatStorageMeter } from './storageFormat';

describe('formatStorageMeter', () => {
  test('formats as MB when forceGb is false', () => {
    expect(formatStorageMeter(1024 * 1024 * 100, false)).toBe('100 MB');
  });

  test('formats zero as MB', () => {
    expect(formatStorageMeter(0, false)).toBe('0 MB');
  });

  test('rounds MB to integer', () => {
    expect(formatStorageMeter(1024 * 1024 * 50 + 500000, false)).toBe('50 MB');
  });

  test('formats as GB when forceGb is true', () => {
    expect(formatStorageMeter(1024 * 1024 * 1024, true)).toBe('1.00 GB');
  });

  test('formats fractional GB', () => {
    expect(formatStorageMeter(1024 * 1024 * 1024 * 2.5, true)).toBe('2.50 GB');
  });

  test('formats small value as GB when forced', () => {
    expect(formatStorageMeter(1024 * 1024 * 100, true)).toBe('0.10 GB');
  });
});
