import type { ReactNode } from 'react';

/** highlightText の最初の出現箇所を <mark> で囲んで返す（該当なしはプレーンテキスト） */
export function renderHighlightedBody(
  body: string,
  highlightText?: string | null,
  highlightClassName = '',
): ReactNode {
  if (!highlightText) {
    return body;
  }

  const matchIndex = body.indexOf(highlightText);
  if (matchIndex === -1) {
    return body;
  }

  return (
    <>
      {body.slice(0, matchIndex)}
      <mark className={highlightClassName}>
        {body.slice(matchIndex, matchIndex + highlightText.length)}
      </mark>
      {body.slice(matchIndex + highlightText.length)}
    </>
  );
}
