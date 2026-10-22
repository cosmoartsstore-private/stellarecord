import rawMessages from './messages.ja.properties?raw';

export type MessageParams = Readonly<Record<string, string | number>>;

const messageKeyPattern = /^[A-Za-z][A-Za-z0-9]*(?:\.[A-Za-z][A-Za-z0-9]*)+$/;
const placeholderPattern = /\{([A-Za-z][A-Za-z0-9_]*)\}/g;
const literalOpenBrace = '\uE000';
const literalCloseBrace = '\uE001';

function findSeparator(line: string): number {
  let isEscaped = false;

  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    if (isEscaped) {
      isEscaped = false;
      continue;
    }
    if (character === '\\') {
      isEscaped = true;
      continue;
    }
    if (character === '=' || character === ':') return index;
  }

  return -1;
}

function unescapePropertyValue(value: string): string {
  return value.replaceAll(/\\(n|r|t|\\|=|:)/g, (_, escaped: string) => {
    if (escaped === 'n') return '\n';
    if (escaped === 'r') return '\r';
    if (escaped === 't') return '\t';
    return escaped;
  });
}

/** UTF-8 の文言resourceを検証し、参照用catalogへ変換する。 */
function parseMessages(source: string): ReadonlyMap<string, string> {
  const messages = new Map<string, string>();

  for (const [lineIndex, rawLine] of source.split(/\r?\n/).entries()) {
    const line = rawLine.trimStart();
    if (line.length === 0 || line.startsWith('#') || line.startsWith('!')) continue;

    const separatorIndex = findSeparator(line);
    if (separatorIndex < 1) {
      throw new Error(`messages.ja.properties:${String(lineIndex + 1)} の形式が不正です。`);
    }

    const key = line.slice(0, separatorIndex).trim();
    if (!messageKeyPattern.test(key)) {
      throw new Error(`messages.ja.properties:${String(lineIndex + 1)} のキー「${key}」が不正です。`);
    }
    if (messages.has(key)) {
      throw new Error(`messages.ja.properties:${String(lineIndex + 1)} のキー「${key}」が重複しています。`);
    }

    messages.set(key, unescapePropertyValue(line.slice(separatorIndex + 1)));
  }

  return messages;
}

const messages = parseMessages(rawMessages);

/** 文言キーを解決し、`{name}` 形式の変数を埋め込む。`{{` と `}}` は波括弧として出力する。 */
export function getMsg(key: string, params: MessageParams = {}): string {
  const template = messages.get(key);
  if (template === undefined) throw new Error(`文言キー「${key}」が定義されていません。`);

  const escapedTemplate = template.split('{{').join(literalOpenBrace).split('}}').join(literalCloseBrace);
  const resolved = escapedTemplate.replaceAll(placeholderPattern, (_, name: string) => {
    if (!(name in params)) {
      throw new Error(`文言キー「${key}」の変数「${name}」が指定されていません。`);
    }
    return String(params[name]);
  });

  return resolved.split(literalOpenBrace).join('{').split(literalCloseBrace).join('}');
}
