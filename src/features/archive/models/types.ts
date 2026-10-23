/**
 * アーカイブ機能の共有型定義
 *
 * levels / categories の数値列挙はRustBackend側の定義に準拠。
 * Frontendでは LEVEL_KEYS / CATEGORY_KEYS 配列を介してCSSクラスにマッピングする
 */

/** ログビューアのストリーム開始時にTauri IPCから返されるメタ情報 */
export interface LogViewerMeta {
  session_id: string;
  archive_name: string;
  source_name: string;
}

/** 本文を基準とする UTF-16 の強調範囲。end は範囲に含めない。 */
export interface LogViewerHighlight {
  start: number;
  end: number;
}

/**
 * ストリーミング中にTauriイベントとして送信される1バッチ分のログ行
 *
 * Level:    0=plain  1=info  2=warning  3=error  4=debug
 * Category: 0=plain  1=world  2=notification
 *           3=player-join  4=player-left  5=video  6=photo
 */
export interface LogViewerChunk {
  /** ファイル切替時に前回ストリームのイベントを破棄するためのセッション識別子 */
  session_id: string;
  timestamps: string[];
  levels: number[];
  categories: number[];
  raw_lines: string[];
  /** DB登録項目、または動画URL・タイトルの強調範囲（該当なしの行はnull） */
  highlights: (LogViewerHighlight | null)[];
}

/** ストリーミングチャンクを蓄積して構築するログビューア表示状態（Frontend専用） */
export interface LogViewerData {
  archive_name: string;
  source_name: string;
  timestamps: string[];
  levels: number[];
  categories: number[];
  raw_lines: string[];
  highlights: (LogViewerHighlight | null)[];
}

/** インポート/閲覧モーダルで選択可能なアーカイブファイル */
export interface ArchiveFileItem {
  name: string;
  size_bytes: number;
}
