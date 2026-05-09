/**
 * 解析ダッシュボードのストレージメーター用にバイト数をフォーマットする
 *
 * 設定上限に応じてMB/GBを切り替え、大容量アーカイブでも単位変換なしで読めるようにする
 */
export function formatStorageMeter(bytes: number, forceGb: boolean) {
  if (forceGb) {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }

  return String(Math.round(bytes / (1024 * 1024))) + ' MB';
}
