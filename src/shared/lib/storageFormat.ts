import { formatByteUnit, formatRoundedMegabytes } from './byteFormat';

/**
 * 解析ダッシュボードのストレージメーター用にバイト数をフォーマットする
 *
 * 設定上限に応じてMB/GBを切り替え、大容量アーカイブでも単位変換なしで読めるようにする
 */
export function formatStorageMeter(bytes: number, forceGb: boolean) {
  if (forceGb) {
    return formatByteUnit(bytes, 'GB', 2);
  }

  return formatRoundedMegabytes(bytes);
}
