import { describe, expect, test } from 'vitest';
import { formatArchiveSize, parseArchiveDate } from './archiveFormat';

describe('parseArchiveDate', () => {
  test('parses standard archive filename', () => {
    const result = parseArchiveDate('output_log_2025-04-30_20-15-00.txt.tar.zst');
    expect(result).toBe('2025/04/30 (水) 20:15');
  });

  test('parses filename with T separator', () => {
    const result = parseArchiveDate('output_log_2025-01-01T00-00-00.txt.tar.zst');
    expect(result).toBe('2025/01/01 (水) 00:00');
  });

  test('returns null for unrecognizable filename', () => {
    expect(parseArchiveDate('random_file.txt')).toBeNull();
  });

  test('returns null for empty string', () => {
    expect(parseArchiveDate('')).toBeNull();
  });

  test('includes correct day of week for known date', () => {
    // 2025-05-30 is a Friday
    const result = parseArchiveDate('output_log_2025-05-30_12-00-00.txt.tar.zst');
    expect(result).toBe('2025/05/30 (金) 12:00');
  });
});

describe('formatArchiveSize', () => {
  test('formats bytes', () => {
    expect(formatArchiveSize(500)).toBe('500 B');
  });

  test('formats zero bytes', () => {
    expect(formatArchiveSize(0)).toBe('0 B');
  });

  test('formats kilobytes', () => {
    expect(formatArchiveSize(1024)).toBe('1.0 KB');
    expect(formatArchiveSize(1536)).toBe('1.5 KB');
  });

  test('formats megabytes', () => {
    expect(formatArchiveSize(1024 * 1024)).toBe('1.00 MB');
    expect(formatArchiveSize(5.5 * 1024 * 1024)).toBe('5.50 MB');
  });

  test('formats gigabytes', () => {
    expect(formatArchiveSize(1024 * 1024 * 1024)).toBe('1.00 GB');
    expect(formatArchiveSize(2.5 * 1024 * 1024 * 1024)).toBe('2.50 GB');
  });

  test('boundary: just below KB threshold', () => {
    expect(formatArchiveSize(1023)).toBe('1023 B');
  });

  test('boundary: just below MB threshold', () => {
    const result = formatArchiveSize(1024 * 1024 - 1);
    expect(result).toContain('KB');
  });

  test('boundary: just below GB threshold', () => {
    const result = formatArchiveSize(1024 * 1024 * 1024 - 1);
    expect(result).toContain('MB');
  });
});
