# STELLARECORD 調査済み: 問題なしと判定した項目

調査日: 2026-05-06
対象: stellarecord (Tauri v2 + React, VRChat ログ管理デスクトップアプリ)

このファイルは過去の調査で「問題なし」と確認した項目をまとめたもの。
次回の調査で同じ箇所を再検証する手間を省くためのリファレンス。

---

## 1. シングルインスタンス Mutex は正常に動作する

**ファイル:** `src-tauri/src/platform.rs:42-67`
**過去の誤指摘:** `_mutex` が `#[cfg(windows)]` ブロック終了時にドロップされ、Mutex が即座に解放される → 多重起動防止が機能しない

**実際の動作:**
- `CreateMutexW` の戻り値は `windows::Win32::Foundation::HANDLE`
- `windows` crate v0.58 の `HANDLE` は `#[repr(transparent)] pub struct HANDLE(pub *mut c_void)` で、`Copy` trait を実装し `Drop` を**実装していない**
- `_mutex` がスコープを抜けても `CloseHandle` は呼ばれない
- Windows カーネルはプロセスのハンドルテーブルでオブジェクトを管理しており、明示的に `CloseHandle` しない限りプロセス終了までカーネルオブジェクトは生存する
- したがって Mutex はプロセスの生存期間中ずっと保持され、**シングルインスタンスガードは正しく機能する**

**確認方法:** `windows` crate のソースで `HANDLE` に `impl Drop` がないことを確認。`Cargo.toml` で `windows = "0.58"` を使用。

---

## 2. launch_external_app / open_folder のパス検証なしは実質リスク低

**ファイル:**
- `src-tauri/src/commands/polaris.rs:11` (`launch_external_app`)
- `src-tauri/src/commands/archive.rs:1091-1093` (`open_folder`)

**過去の誤指摘:** フロントエンドから任意パスを受け取り検証なしで実行 → 任意コード実行の脆弱性

**実際のリスク評価:**
- Tauri v2 の IPC コマンドはローカル WebView からのみ呼び出し可能
- CSP で `connect-src 'self' http://localhost:* ipc:` に制限されており、外部サイトからのスクリプト注入経路がない
- `launch_external_app` の呼び出し元は `registryApps`（DB の `apps` テーブルから読み込んだ登録済みアプリのパス）のみ
- `open_folder` は `app.path.substring(0, app.path.lastIndexOf('\\'))` で親ディレクトリを計算した結果を渡す
- リモートコンテンツを読み込まないローカルデスクトップアプリであり、WebView 侵害の実用的な攻撃シナリオが極めて乏しい

**結論:** セキュリティベストプラクティスとしてはバリデーション追加が望ましいが、現状のアーキテクチャでは実質的な脅威にならない。

---

## 3. read_recent_lines のメモリ消費は実用上問題なし

**ファイル:** `src-tauri/src/utils.rs:226-262`

**過去の誤指摘:** VRChat ログ（数百MB）を全行メモリに読み込んでから末尾のみ使用 → OOM の危険

**実際の呼び出し元:**
- `commands/polaris.rs:35` で `utils::read_recent_lines(&log_path, 100)` として呼ばれる
- `log_path` は `Polaris/info.log`（Polaris アプリケーション自身のログファイル）
- VRChat のソースログ（`output_log_*.txt`、数十〜数百MB）には使われていない
- Polaris の `info.log` は通常数KB〜数MB程度

**結論:** 対象ファイルが小さいため実用上問題にならない。VRChat ソースログの読み取りは別の関数（`open_source_log_for_read` + `read_to_end` 等）で行われる。

---

## 4. SQLite 同時アクセスは WAL モード + UI 制御で保護済み

**ファイル:** `src-tauri/src/commands/import.rs`, `archive.rs`

**過去の誤指摘:** 複数スレッドが個別の Connection を開き、SQLITE_BUSY エラーの原因になる

**実際の保護メカニズム:**
- `analyze/db.rs:183` で `PRAGMA journal_mode = WAL` を設定 → WAL モードは読み取りの並行性を保証
- UI 側で `isAnalyzeRunning` 状態を管理し、インポート中は別のインポートを開始できない
- `AnalyzeCancelStatus` は単一の `Arc<AtomicBool>` で、同時に1つのインポートスレッドのみが走る設計
- `read_archive_log_viewer` は読み取り専用。WAL モードでは書き込み中の読み取りは正常に動作する

**結論:** アプリケーションレベルで書き込みの多重実行が防止されており、WAL モードが読み取りの並行性を保証するため、SQLITE_BUSY は発生しない。

---

## 5. SQL 構築の format! は適切にバリデーション済み

**ファイル:** `src-tauri/src/commands/database.rs:386-397`

**過去の誤指摘:** `format!` による SQL 構築はインジェクションリスクがある

**実際の防御:**
- `table_name`: `sanitize_table_name()` (L226-235) で ASCII 英数字と `_` のみに制限
- `sort_column`: `col.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')` (L388) で検証
- `PAGE_SIZE`: `const u32 = 500` (L358)
- `offset`: `page.unwrap_or(0) * PAGE_SIZE` で `u32` 演算のみ
- `sort_dir`: `match` で `"asc"` / `"DESC"` のリテラルのみに制限 (L389-391)

**結論:** 全ての動的値が適切にバリデーションまたは型安全性で保護されている。

---

## 6. dedup_by のソート順は同一テキストでは正しく動作する

**ファイル:** `src-tauri/src/commands/archive.rs:724-725`

```rust
markers.sort_by(|left, right| right.text.len().cmp(&left.text.len()));
markers.dedup_by(|left, right| left.category == right.category && left.text == right.text);
```

**過去の誤指摘:** テキスト長でソートした後の `dedup_by` が重複を見逃す可能性

**実際の動作:**
- `dedup_by` は連続する重複のみ除去する
- ソートキーは `text.len()` (降順)
- 重複の条件は `category == category && text == text`
- 同一の `text` は必ず同一の `len()` を持つため、length ソート後は同一テキストのマーカーが隣接する
- 同一 text + 同一 category のマーカーは必ず連続するため、`dedup_by` で正しく除去される

**結論:** 同一テキストは同一長であるという性質により、このソート+dedup パターンは正しく動作する。

---

## 7. TOCTOU 競合 (ソースログ削除) は実質リスクなし

**ファイル:** `src-tauri/src/commands/archive.rs:1135-1160`

**過去の誤指摘:** アーカイブ存在確認と削除の間にアーカイブが消える → データ消失

**実際のリスク:**
- シングルユーザーのデスクトップアプリケーション
- アーカイブを削除する外部プロセスは通常存在しない
- `delete_source_logs` はユーザーの明示的な操作（UI のクリーンアップモーダル）でのみ呼ばれる
- `ensure_single_instance` で多重起動が防止されている

**結論:** 理論的な競合だが、実運用上の発生確率は極めて低い。

---

## 8. get_polaris_status の System::new() は許容範囲

**ファイル:** `src-tauri/src/platform.rs:94-101`

**過去の誤指摘:** 毎回 `System::new()` で全プロセスを列挙するのは無駄

**実際の呼び出し頻度:**
- `commands/polaris.rs:17` (`get_polaris_status` コマンド) - UI から明示的に呼ばれる
- `commands/polaris.rs:44` (`start_polaris` コマンド内の事前チェック) - ボタンクリック時のみ
- ポーリングや定期実行ではなく、ユーザー操作時のみ実行される

**結論:** 呼び出し頻度が低いため、`System` のキャッシュは不要。

---

## 9. config::load_registry_catalog の rows.flatten() はログ記録済み

**ファイル:** `src-tauri/src/config.rs:220`

**過去の誤指摘:** `rows.flatten()` でエラーが無視される

**実際の動作:**
- `query_map` のクロージャ内で個々の行をデシリアライズしている (L195-217)
- `query_map` 自体の失敗は直前の `match` (L211-217) でハンドリングされ、エラー時は空カタログを返す
- `flatten()` で無視されるのは個別行の読み取りエラーのみ
- アプリカタログはベストエフォートで読み込む設計であり、1行の破損で全体が表示不能になるよりも合理的

**結論:** 設計上の意図的な選択。個別行のエラーはユーザー体験を損なわないために無視している。

---

## 10. インポート操作間の排他制御は UI ガードで十分

**ファイル:** `src-tauri/src/commands/import.rs:31-174`, `src/features/analyze/views/AnalyzeSection.tsx:132`

**過去の誤指摘:** `launch_enhanced_import` と `launch_startup_archive_import` が同一の `AtomicBool` を共有し、バックエンドに排他ロックがない → 同時実行で SQLITE_BUSY やデータ破損の危険

**実際の防御:**
- `AnalyzeSection.tsx:132` で `disabled={isAnalyzeRunning}` によりインポートボタンを無効化
- 起動時インポートが `setAnalyzeRunning(true)` を即座に設定し、次のレンダリングで UI に反映
- ユーザーが UI 操作で同時実行を発生させることは事実上不可能
- 起動時インポートの IPC は即座に返るため、UI ブロック中に操作するタイムウィンドウが存在しない

**結論:** UI レベルのガードが実効的に同時実行を防止しており、バックエンド側の追加ロックは不要。

---

## 11. アンマウント時のフラッシュタイマー未クリアは実質影響なし

**ファイル:** `src/features/archive/viewmodels/useArchiveState.ts:162-165`

**過去の誤指摘:** `stopStream()` が `flushTimerRef.current` をクリアしないため、アンマウント後にタイマーが発火し、アンマウント済みコンポーネントに `setState` が呼ばれる

**実際の動作:**
- React 18 ではアンマウント後の `setState` は silently ignore される（警告も出ない）
- タイマーは最大 100ms 後に1回だけ発火し、`flushChunks` は `pendingChunksRef` から空配列を取得して即座に return
- `openStreamForFile` (L81-84) ではタイマーを正しくクリアしており、ファイル切替時には問題なし
- アンマウント時の空振りタイマーによる副作用はゼロ

**結論:** React 18 のアンマウント後 setState 無視仕様により、実用上の影響はない。
