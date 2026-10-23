import type { ReactNode } from 'react';
import type { LogViewerHighlight } from './types';

/** Rust の RE_LOG_HEADER と同じ先頭形式を分離し、元の空白を保持する。 */
export function splitLogLine(line: string) {
  const match = /^(\d{4}\.\d{2}\.\d{2} \d{2}:\d{2}:\d{2}[ \t]+)(Debug|Warning|Error|Info|Log)([ \t]+-[ \t]*)(.*)$/s.exec(line);
  if (!match) return null;
  return { prefix: match[1], level: match[2], separator: match[3], body: match[4] };
}

/** 指定項目を強調し、ワールド ID と接続先情報は常に通常色で表示する。 */
export function renderHighlightedBody(body: string, highlight?: LogViewerHighlight | null, highlightClassName = '', identifierClassName = ''): ReactNode {
  const nodes: ReactNode[] = [];
  const appendText = (start: number, end: number) => {
    if (start === end) return;
    const rangeStart = Math.max(start, highlight?.start ?? end);
    const rangeEnd = Math.min(end, highlight?.end ?? start);
    if (rangeStart >= rangeEnd) {
      nodes.push(body.slice(start, end));
      return;
    }
    nodes.push(body.slice(start, rangeStart), <mark key={`highlight-${String(start)}`} className={highlightClassName}>{body.slice(rangeStart, rangeEnd)}</mark>, body.slice(rangeEnd, end));
  };

  let offset = 0;
  for (const match of body.matchAll(/wrld_[a-fA-F0-9-]+(?::[^\s,}"<>]*)?/g)) {
    appendText(offset, match.index);
    nodes.push(<span key={`identifier-${String(match.index)}`} className={identifierClassName}>{match[0]}</span>);
    offset = match.index + match[0].length;
  }
  appendText(offset, body.length);
  return nodes;
}
