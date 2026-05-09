/**
 * アーカイブ機能の共有型定義
 *
 * levels / categories の数値列挙はRustバックエンド側の定義に準拠。
 * フロントエンドでは LEVEL_KEYS / CATEGORY_KEYS 配列を介してCSSクラスにマッピングする
 */

/** ログビューアのストリーム開始時にTauri IPCから返されるメタ情報 */
export interface LogViewerMeta {
  session_id: string;
  archive_name: string;
  source_name: string;
}

/**
 * ストリーミング中にTauriイベントとして送信される1バッチ分のログ行
 *
 * Level:    0=plain  1=info  2=warning  3=error  4=debug
 * Category: 0=plain  1=world  2=travel  3=notification
 *           4=player-join  5=player-ready  6=player-left  7=video
 *           8=debug-system  9=debug-avatar  10=debug-network  11=debug-interact
 */
export interface LogViewerChunk {
  /** ファイル切替時に前回ストリームのイベントを破棄するためのセッション識別子 */
  session_id: string;
  timestamps: string[];
  levels: number[];
  categories: number[];
  raw_lines: string[];
  /** バックエンドが検出したハイライト対象キーワード（該当なしの行はnull） */
  highlights: (string | null)[];
}

/** ストリーミングチャンクを蓄積して構築するログビューア表示状態（フロントエンド専用） */
export interface LogViewerData {
  archive_name: string;
  source_name: string;
  timestamps: string[];
  levels: number[];
  categories: number[];
  raw_lines: string[];
  highlights: (string | null)[];
}

/** インポート/閲覧モーダルで選択可能なアーカイブファイル */
export interface ArchiveFileItem {
  name: string;
  size_bytes: number;
}

/** アプリ起動時の自動取り込み結果サマリ */
export interface StartupImportSummary {
  total_count: number;
}
