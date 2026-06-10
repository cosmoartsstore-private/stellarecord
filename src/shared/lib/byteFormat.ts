/** バイト表示で使用する2進単位の基準値。 */
export const byteUnitBytes = {
  KB: 1024,
  MB: 1024 * 1024,
  GB: 1024 * 1024 * 1024,
} as const;

type ByteUnit = keyof typeof byteUnitBytes;

/** 指定した単位と小数桁でバイト数を表示用文字列に変換する。 */
export function formatByteUnit(bytes: number, unit: ByteUnit, fractionDigits: number) {
  return `${(bytes / byteUnitBytes[unit]).toFixed(fractionDigits)} ${unit}`;
}

/** ストレージメーター用にMB単位の整数へ丸めて表示する。 */
export function formatRoundedMegabytes(bytes: number) {
  return `${String(Math.round(bytes / byteUnitBytes.MB))} MB`;
}
