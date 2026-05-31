import { describe, expect, test } from 'vitest';
import type { SectionId } from './section';

describe('SectionId', () => {
  test('accepts valid section ids', () => {
    const sections: SectionId[] = ['registry', 'analyze', 'database'];
    expect(sections).toHaveLength(3);
  });

  test('type guard works at runtime', () => {
    const validIds = ['registry', 'analyze', 'database'] as const;
    const isValid = (id: string): id is SectionId =>
      (validIds as readonly string[]).includes(id);

    expect(isValid('registry')).toBe(true);
    expect(isValid('analyze')).toBe(true);
    expect(isValid('database')).toBe(true);
    expect(isValid('settings')).toBe(false);
    expect(isValid('')).toBe(false);
  });
});
