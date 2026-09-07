import { describe, expect, test } from 'vitest';
import { parseArchiveDate } from './archiveFormat';

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
