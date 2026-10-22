import { byteUnitBytes } from '../constants';

type ByteUnit = keyof typeof byteUnitBytes;

/** 指定した単位と小数桁でバイト数を表示用文字列に変換する。 */
export function formatByteUnit(bytes: number, unit: ByteUnit, fractionDigits: number) {
  return `${(bytes / byteUnitBytes[unit]).toFixed(fractionDigits)} ${unit}`;
}

/** バイト数をファイルサイズ表示（GB / MB / KB / B）へ変換する。 */
export function formatFileSize(bytes: number) {
  if (bytes >= byteUnitBytes.GB) {
    return formatByteUnit(bytes, 'GB', 2);
  }
  if (bytes >= byteUnitBytes.MB) {
    return formatByteUnit(bytes, 'MB', 2);
  }
  if (bytes >= byteUnitBytes.KB) {
    return formatByteUnit(bytes, 'KB', 1);
  }
  return `${String(bytes)} B`;
}

/** ストレージメーター用にMB単位の整数へ丸めて表示する。 */
export function formatRoundedMegabytes(bytes: number) {
  return `${String(Math.round(bytes / byteUnitBytes.MB))} MB`;
}
