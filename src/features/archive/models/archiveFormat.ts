/**
 * アーカイブファイル名から `yyyy/mm/dd (曜日) hh:mm` 形式の表示文字列を抽出する
 *
 * ファイル名は `2025-04-30_20-15-00.tar.zst` のパターンを想定。
 * 認識可能な日付パターンが含まれない場合は null を返す
 */

/** Date.getDay() に対応する日本語曜日ラベル（0=日曜） */
const dayNames = ['日', '月', '火', '水', '木', '金', '土'] as const;

export function parseArchiveDate(fileName: string): string | null {
  const m = /(\d{4})-(\d{2})-(\d{2})[_T](\d{2})-(\d{2})-(\d{2})/.exec(fileName);
  if (!m) return null;
  const date = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  const dow = dayNames[date.getDay()];
  return `${m[1]}/${m[2]}/${m[3]} (${dow}) ${m[4]}:${m[5]}`;
}

/** バイト数を人間が読みやすいサイズラベル（GB / MB / KB / B）に変換する */
export function formatArchiveSize(sizeBytes: number) {
  if (sizeBytes >= 1024 * 1024 * 1024) {
    return `${(sizeBytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }
  if (sizeBytes >= 1024 * 1024) {
    return `${(sizeBytes / (1024 * 1024)).toFixed(2)} MB`;
  }
  if (sizeBytes >= 1024) {
    return `${(sizeBytes / 1024).toFixed(1)} KB`;
  }
  return String(sizeBytes) + ' B';
}
