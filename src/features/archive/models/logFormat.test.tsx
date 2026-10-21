// @vitest-environment jsdom
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { renderHighlightedBody } from './logFormat';

afterEach(cleanup);
describe('renderHighlightedBody', () => {
  it.each([undefined, null, '', '未検出'])('highlightがない場合は本文を変更しない: %s', (highlight) => {
    const body = '<script>日本語 & ログ</script>';
    const { container } = render(<div>{renderHighlightedBody(body, highlight)}</div>);
    expect(container.textContent).toBe(body);
    expect(container.querySelector('mark')).toBeNull();
    expect(container.querySelector('script')).toBeNull();
  });
  it.each(['日本語 日本語', '日本語', '前日本語後'])('最初の完全一致箇所だけを強調し、本文と指定classを保つ: %s', (body) => {
    const { container } = render(<div>{renderHighlightedBody(body, '日本語', 'highlight')}</div>);
    expect(container.textContent).toBe(body);
    expect(container.querySelectorAll('mark')).toHaveLength(1);
    expect(container.querySelector('mark')?.textContent).toBe('日本語');
    expect(container.querySelector('mark')?.className).toBe('highlight');
  });
});
