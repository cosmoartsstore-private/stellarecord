/** WindowsとPOSIXの区切りを考慮し、末尾のファイル名部分を返す。 */
export function getFileNameFromPath(fullPath: string): string {
  const separator = Math.max(fullPath.lastIndexOf('\\'), fullPath.lastIndexOf('/'));
  return fullPath.slice(separator + 1);
}
