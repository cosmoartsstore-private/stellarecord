// @vitest-environment jsdom
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { renderHighlightedBody, splitLogLine } from './logFormat';

afterEach(cleanup);
describe('renderHighlightedBody', () => {
  it.each([undefined, null])('highlightがない場合は本文を変更しない: %s', (highlight) => {
    const body = '<script>日本語 & ログ</script>';
    const { container } = render(<div>{renderHighlightedBody(body, highlight)}</div>);
    expect(container.textContent).toBe(body);
    expect(container.querySelector('mark')).toBeNull();
    expect(container.querySelector('script')).toBeNull();
  });
  it('同じ語が複数あっても指定された位置だけを強調する', () => {
    const body = '日本語 日本語';
    const { container } = render(<div>{renderHighlightedBody(body, { start: 4, end: 7 }, 'highlight')}</div>);
    expect(container.textContent).toBe(body);
    expect(container.querySelectorAll('mark')).toHaveLength(1);
    expect(container.querySelector('mark')?.textContent).toBe('日本語');
    expect(container.querySelector('mark')?.className).toBe('highlight');
    expect(container.querySelector('div')?.firstChild?.textContent).toBe('日本語 ');
  });

  it('UTF-16の範囲で絵文字の後ろを強調し、HTMLとして解釈しない', () => {
    const body = '😀日本語<script>文字</script>';
    const { container } = render(<div>{renderHighlightedBody(body, { start: 2, end: 5 }, 'highlight')}</div>);
    expect(container.textContent).toBe(body);
    expect(container.querySelector('mark')?.textContent).toBe('日本語');
    expect(container.querySelector('script')).toBeNull();
  });

  it('指定範囲に含まれていてもワールドIDと接続先は通常色にする', () => {
    const location = 'wrld_abcdef12-3456-7890-abcd-ef1234567890:17~private(usr_owner)~region(jp)';
    const body = `移動 ${location} 完了`;
    const { container } = render(<div>{renderHighlightedBody(body, { start: 0, end: body.length }, 'highlight', 'identifier')}</div>);
    expect(container.textContent).toBe(body);
    expect(container.querySelector('.identifier')?.textContent).toBe(location);
    expect([...container.querySelectorAll('mark')].map((mark) => mark.textContent).join('')).toBe('移動  完了');
    expect(container.querySelector('a')).toBeNull();
  });
});

describe('splitLogLine', () => {
  it('先頭形式を本文から分離し、空白と原文を保持する', () => {
    expect(splitLogLine('2026.03.21 02:43:48 Debug      -  本文')).toEqual({ prefix: '2026.03.21 02:43:48 ', level: 'Debug', separator: '      -  ', body: '本文' });
    expect(splitLogLine('先頭形式のない Debug 行')).toBeNull();
  });
});
