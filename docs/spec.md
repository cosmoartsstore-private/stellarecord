# 機能仕様書 — 設計判断と技術的見どころ

> 本書はポートフォリオ用に、STELLA RECORD の機能仕様を**「なぜそう設計したか」「どこに技術的見どころがあるか」**の観点でまとめたもの。
> 単純なスペック羅列ではなく、設計者の意思決定プロセスを追体験できる構成にしている。

---

## 目次

1. [プロダクト概要と解決した課題](#1-プロダクト概要と解決した課題)
2. [アーキテクチャ全体像と設計方針](#2-アーキテクチャ全体像と設計方針)
3. [機能別 設計の見どころ](#3-機能別-設計の見どころ)
   1. [取り込みパイプライン](#31-取り込みパイプライン-savepoint--キャンセル可能設計)
   2. [ストリーミングログビューア](#32-ストリーミングログビューア-3層バッファリング)
   3. [VRChat ログパーサ](#33-vrchat-ログパーサ-行単位ステートマシン)
   4. [Win32 アイコン抽出](#34-win32-アイコン抽出-高解像度フォールバック)
   5. [単一インスタンスガード](#35-単一インスタンスガード-mutex-寿命の設計)
4. [並行性と整合性の設計](#4-並行性と整合性の設計)
5. [セキュリティ設計](#5-セキュリティ設計)
6. [規模と計測値](#6-規模と計測値)
7. [既知の制約と今後の改善余地](#7-既知の制約と今後の改善余地)

---

## 1. プロダクト概要と解決した課題

### 解決した課題

VRChat の `output_log_*.txt` は以下の問題を抱えている：

| 課題 | 影響 |
|---|---|
| **ログが古い順に削除される** | 数ヶ月前の訪問履歴が失われる |
| **非構造化テキスト** | 「先月この人と会ったワールド」を後から検索できない |
| **数百 MB に肥大化** | ディスク容量を圧迫 |
| **複数行にまたがる通知** | 単純な grep では追えない |

### アプローチ

```
                  ┌─────────────┐
   生ログ ───────▶│  圧縮保管    │──▶  .tar.zst（zstd lv3, 約 90% 削減）
  (output_log_*) │             │
                  ├─────────────┤
                  │ 行単位パーサ │──▶  正規化 SQLite（9 テーブル + 3 ビュー）
                  ├─────────────┤
                  │ ストリーム閲覧│──▶  仮想スクロール（10 万行超対応）
                  └─────────────┘
```

**3 つを 1 つのアプリで束ねた**のがプロダクト価値の核。各機能単体は既存ツール（7-Zip, grep, VRCX）でも実現可能だが、**「圧縮しつつ構造化しつつ閲覧可能」**を 1 ワークフローで完結させる点が他に存在しない。

### スコープ定義の判断

| 項目 | 採用 | 不採用 | 不採用の理由 |
|---|---|---|---|
| 対象 OS | Windows のみ | macOS/Linux 対応 | VRChat ユーザーの大半が Windows、開発リソース集中 |
| 配信形式 | ローカルデスクトップアプリ | Web SaaS | ログにプライバシー情報が含まれるためローカル完結が前提 |
| 認証 | なし（シングルユーザー） | アカウント機能 | パーソナルツールとして設計、複数アカウント切替は非対象 |
| 通知連携 | なし | Discord Webhook 等 | スコープを「保管と閲覧」に絞り、配信は別ツールに委譲 |

---

## 2. アーキテクチャ全体像と設計方針

### レイヤ構成

```
┌─────────────────────────────────────────────────────────────┐
│ WebView (Edge / Chromium) — UI レイヤ                        │
│  React 19 + TypeScript 5.9                                   │
│  ├ app/         ルーティング・モーダル統括                    │
│  ├ features/    機能別 MVVM (views / viewmodels / services)  │
│  │   ├ analyze, archive, database, registry, settings        │
│  │   └ 相互参照禁止 (ESLint no-restricted-imports で強制)    │
│  └ shared/      共通基盤 (Icons, Toast, 型, ユーティリティ)  │
└──────────────────────┬──────────────────────────────────────┘
                       │ Tauri IPC (invoke / event)
                       │ 22 コマンド / 4 イベント
┌──────────────────────┴──────────────────────────────────────┐
│ Rust Backend — ビジネスロジック・OS 連携                     │
│  ├ commands/    IPC ハンドラ (薄い層、検証 → 委譲のみ)       │
│  ├ analyze/     ログ解析パイプライン (純粋ロジック)          │
│  ├ config.rs    Windows レジストリ I/O                       │
│  ├ platform.rs  Win32 API (Mutex, アイコン, ダイアログ)      │
│  └ utils.rs     ロガー / エラー整形 / イベント送信           │
└──────────────────────┬──────────────────────────────────────┘
                       │
        ┌──────────────┴────────────────┐
        ▼                               ▼
   SQLite (WAL)                    .tar.zst Archive
   Data/db/stellarecord.db          Data/archive/
```

### 設計原則と意思決定

#### 原則 1: 「IPC ハンドラは薄く、ロジックは純粋関数で」

```rust
// commands/import.rs — IPC 層は検証 → 委譲のみ
#[tauri::command]
pub fn launch_enhanced_import(file_names: Vec<String>, ...) -> Result<String, String> {
    let db_path = get_db_path()?;
    let target_paths = validate_paths(file_names)?;
    std::thread::spawn(move || {
        analyze::run_enhanced_import_batch(&db_path, &target_paths, ...)  // ← ロジックは別モジュール
    });
    Ok(format!("{}件のアーカイブ同期を開始しました。", total))
}
```

**意図**: IPC レイヤは Tauri 依存だが、`analyze::*` は Tauri に依存しない純粋関数群にすることで、将来 CLI 化やテスト容易性を確保。

#### 原則 2: 「Rust 側で `unwrap` / `panic` を一切許さない」

```toml
[workspace.lints.clippy]
unwrap_used  = "deny"
expect_used  = "deny"
panic        = "deny"
```

**意図**: 運用中のクラッシュは「DB 整合性破壊 + UI フリーズ + ユーザーが何も保存できない」最悪シナリオを招く。コンパイル時に `Result` 伝播を強制し、エラーは必ず**ログ退避 + UI 通知**の 2 経路に流れるよう設計。

唯一の例外は `parser.rs::compile_regex` で、これは**固定パターンのバグ**を起動時に即座に検知する意図的な panic（`#[allow(clippy::panic)]` 明示）。

#### 原則 3: 「フロントエンドの feature 間相互参照を禁止」

```js
// eslint.config.js
'no-restricted-imports': [
  'error',
  { patterns: [
    'src/features/analyze/**',  // archive から analyze を import 禁止
    'src/features/archive/**',
    ...
  ]}
]
```

**意図**: feature が肥大化したときの相互依存スパゲッティ化を防ぐ。共通化したくなったら `shared/` に上げることを強制。

#### 原則 4: 「ユーザーデータは絶対に失わない」

| 段階 | 保護策 |
|---|---|
| アンインストール時 | NSIS スクリプトで `Data/archive/` `Data/db/` を保護対象としてスキップ |
| 取り込み失敗時 | savepoint で個別ファイルロールバック、外側 transaction で全体ロールバック |
| 取り込みキャンセル時 | 外側 transaction を rollback → DB を完全に元の状態に戻す |
| 多重起動時 | Mutex で起動阻止し、複数プロセスから DB を書き換える事故を防止 |

---

## 3. 機能別 設計の見どころ

### 3.1 取り込みパイプライン — savepoint + キャンセル可能設計

**課題**: ログを 1 件ずつ取り込む途中で 5 件目でエラーが起きたら？ユーザーがキャンセルしたら？

**設計**:

```
外側 transaction
 ├─ savepoint sp_1 → parse_and_import → commit
 ├─ savepoint sp_2 → parse_and_import → エラー → rollback (このファイルのみ捨てる)
 ├─ savepoint sp_3 → parse_and_import → ユーザーがキャンセル → rollback
 │                                                          ↓
 │                                              外側 transaction も rollback
 │                                              ↓
 │                                          DB は取り込み前と完全に同じ状態
 └─ ...
```

**コード**: `src-tauri/src/analyze/mod.rs::run_diff_import`

```rust
let mut main_tx = main_conn.transaction()?;
for (idx, log_path) in log_files.iter().enumerate() {
    if let Err(err) = ensure_not_canceled(cancel_status) {
        rollback_outer_transaction(main_tx, "解析中断時")?;
        return Err(err);
    }
    let main_sp = main_tx.savepoint()?;
    match parse_and_import(&main_sp, log_path, ...) {
        Ok(()) => main_sp.commit()?,
        Err(err) if err.to_string() == ANALYZE_CANCELED_MESSAGE => {
            rollback_savepoint(main_sp, "解析中断時")?;
            rollback_outer_transaction(main_tx, "解析中断時")?;
            return Err(ANALYZE_CANCELED_MESSAGE.to_string());
        }
        Err(err) => {
            rollback_savepoint(main_sp, "ファイル単位ロールバック時")?;  // 個別失敗は許容
            log_err(&format!("エラー ({filename}): {err}"));
        }
    }
}
main_tx.commit()?;
```

**見どころ**:

- **失敗の粒度を 2 段階で制御**: 「1 ファイルだけ捨てる」（savepoint）と「全部やり直し」（outer transaction）を使い分け
- **キャンセルは協調的**: `Arc<AtomicBool>` を行ループの先頭でポーリングし、応答性 5000 行 = ~50ms 以内に停止
- **冪等性**: `sessions.log_name` を UNIQUE にし、再取り込み時は `SELECT EXISTS` で skip → ユーザーが何度実行しても安全
- **進捗ストリーミング**: バックグラウンドスレッドから `analyze-progress` イベントを送出、UI 側で 100ms 単位に集約してレンダリング負荷を分散

### 3.2 ストリーミングログビューア — 3 層バッファリング

**課題**: 数十 MB の `.tar.zst`（展開すると数百 MB）を即座に閲覧したい。ただし全行メモリに乗せたら UI が固まる。

**設計**: **Rust 側で 500 行ごとに IPC イベント送出 → React 側で 100ms バッファ → react-virtual で仮想スクロール** の 3 段構え。

```
[Rust] tar.zst 解凍 → BufReader::lines()
                            │
                            ▼  500 行たまったら
                       app.emit("log_viewer_chunk", { session_id, ...500 行 })
                            │
                            ▼  Tauri IPC
[React] listen("log_viewer_chunk")
        if (payload.session_id !== currentSessionId) return;  // ← 古いファイルのイベント破棄
        pendingChunksRef.current.push(payload);
        if (!flushTimerRef.current) {
            flushTimerRef.current = setTimeout(flushChunks, 100);  // ← 100ms 集約
        }
        │
        ▼  100ms 経過
        chunks.reduce(appendChunk, prev) → setState
        │
        ▼
[react-virtual]  画面に映る行のみ DOM 化（10 行 overscan）
```

**見どころ**:

- **`session_id` による競合排除**: ユーザーが連続でファイル切替したとき、古いファイルのチャンクが到着しても session_id 不一致で破棄。古いファイルの行が新しいファイルに混ざる事故を防ぐ
- **`flushSync` で初期状態を強制反映**: 新セッション開始時に `flushSync(() => setLogViewerData(emptyViewerData()))` で前のデータを確実にクリアしてから listen 開始
- **チャンク内ペイロード設計**: `raw_lines: string[]`, `levels: u8[]`, `categories: u8[]`, `highlights: (string|null)[]` を**並列配列**にしてシリアライズコストを最小化（オブジェクト配列より JSON サイズ約 40% 削減）
- **仮想スクロール**: `@tanstack/react-virtual` で表示行数に比例しないレンダリングコスト。10 万行ログでもスクロール時の DOM 数は常に ~30 行

### 3.3 VRChat ログパーサ — 行単位ステートマシン

**課題**: VRChat ログは**非構造化テキスト + 時系列依存**。例えば「Joining wrld_xxx」の行は、**直前の「Entering Room: <World>」行が同じセッションで出ている前提**でワールド名と紐付ける必要がある。

**設計**: 23 種類の正規表現 + ループ内変数による有限ステートマシン。

```rust
// src-tauri/src/analyze/mod.rs::parse_and_import_reader (抜粋)
let mut current_ts: Option<NaiveDateTime> = None;       // 最後に観測した時刻
let mut current_visit_id: Option<i64> = None;           // 現在のワールド訪問 ID
let mut pending_room_name: Option<String> = None;       // Joining 前に観測したワールド名

for line_result in reader.lines() {
    let Ok(line) = line_result else { continue };       // 不正 UTF-8 は skip 継続

    if let Some(caps) = RE_TIME.captures(&line) {       // タイムスタンプ更新
        current_ts = parse_dt(caps.get(1).unwrap().as_str());
    }
    if let Some(caps) = RE_ENTERING.captures(&line) {   // ワールド名候補を蓄積
        pending_room_name = Some(caps.get(1).unwrap().as_str().to_string());
    }
    if let Some(caps) = RE_JOINING.captures(&line) {    // 候補と組み合わせて確定
        if let Some(room_name) = pending_room_name.as_ref() {
            // INSERT INTO visits ...
            current_visit_id = Some(last_insert_id);
        }
    }
    if let Some(caps) = RE_PLAYER_JOIN.captures(&line) {
        if let Some(visit_id) = current_visit_id {       // 現在の訪問に紐付け
            // INSERT INTO with_users ...
        }
    }
    // ... 以下 OnPlayerLeft, Notification, Screenshot, OSC, Subscription ...
}
```

**見どころ**:

- **LazyLock + コンパイル時パニック**: `static RE_TIME: LazyLock<Regex> = LazyLock::new(...)` で全パターンを 1 度だけコンパイル。不正パターンは起動時にプロセス停止（早期検知）
- **ステート変数の最小化**: 5 つの `Option` のみで時系列依存を解決。ボトムアップに DDD すると Aggregate になるが、**ログ処理速度を優先して手続き的に書く判断**
- **エンコーディング耐性**: 非 UTF-8 行は `continue` で skip して継続（mod 系プラグインが Shift-JIS で書き込むケースに対応）
- **マルチライン通知の検出**: `Received Notification: <Notification ...>` が複数行にまたがるケースは別途レンジブロック検出ロジックで一塊として扱う（`detect_range_block_start` / `RangeBlockKind::Notification`）

**実装規模**: パーサ本体 889 行、正規表現 23 種、対応イベント 10 種。

### 3.4 Win32 アイコン抽出 — 高解像度フォールバック

**課題**: ランチャーで EXE を登録するとき、Windows エクスプローラ並の高品質アイコンを表示したい。

**設計**: **`SHGetImageList(SHIL_JUMBO)` で 256×256 取得 → 失敗時に `ExtractIconExW` で 32×32 フォールバック → HICON を PNG にエンコードして DB に BLOB 保存**。

```rust
// src-tauri/src/platform.rs::extract_exe_icon_png
pub fn extract_exe_icon_png(exe_path: &Path) -> Option<Vec<u8>> {
    extract_icon_jumbo(exe_path)        // ← 256×256 (Jumbo image list)
        .or_else(|| extract_icon_legacy(exe_path))  // ← 32×32 fallback
}

fn extract_icon_jumbo(exe_path: &Path) -> Option<Vec<u8>> {
    let com_ok = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let result = (|| {
        let mut file_info = SHFILEINFOW::default();
        let ret = unsafe { SHGetFileInfoW(..., SHGFI_SYSICONINDEX) };
        let image_list: IImageList = unsafe { SHGetImageList(SHIL_JUMBO) }.ok()?;
        let hicon: HICON = unsafe { image_list.GetIcon(file_info.iIcon, 1) }.ok()?;
        let png = hicon_to_png(hicon);  // ← GDI で DIB に展開、BGRA→RGBA 変換、image crate で PNG エンコード
        unsafe { DestroyIcon(hicon); }
        png
    })();
    if com_ok { unsafe { CoUninitialize(); } }
    result
}
```

**見どころ**:

- **COM の正しい初期化と解放**: `CoInitializeEx` の戻り値（S_OK / S_FALSE / 失敗）を判定し、成功時のみ `CoUninitialize` をペアで呼ぶ。「すでに初期化済み」のケースでも正しくバランスを取る
- **HICON → PNG 変換の手書き**: `GetDIBits` で 32bit RGBA DIB として読み出し、`chunks_exact_mut(4)` で BGRA→RGBA を swap、`image` crate で PNG エンコード。中間ファイルなし、メモリのみで完結
- **リソースリーク防止**: `DeleteObject` / `DeleteDC` / `DestroyIcon` の呼び忘れがないようにクロージャ + cleanup_bitmaps パターンで早期 return でも解放
- **失敗時の優雅な縮退**: アイコン取得失敗時は `None` を返し、UI 側でフォールバックアイコン（SVG）を表示

### 3.5 単一インスタンスガード — Mutex 寿命の設計

**課題**: 2 つのプロセスから同じ DB に書き込まれたら破壊される。確実な多重起動防止が必要。

**設計**: `CreateMutexW` で名前付き Mutex を作成し、`GetLastError() == ERROR_ALREADY_EXISTS` を多重起動シグナルとして使う。

```rust
// src-tauri/src/platform.rs::ensure_single_instance
let mutex = unsafe { CreateMutexW(None, true, PCWSTR(mutex_name.as_ptr())) }?;
if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
    std::process::exit(0);
}
let _ = ManuallyDrop::new(mutex);  // ← プロセス終了まで保持
```

**見どころ（過去の誤指摘との対比）**:

最初のコードレビューで「`_mutex` がスコープを抜けると Drop されて Mutex が解放される」と誤指摘されたが、調査した結果以下の事実を確認した：

- `windows` crate v0.58 の `HANDLE` は `#[repr(transparent)] pub struct HANDLE(pub *mut c_void)` で **`Copy` を実装し `Drop` を実装していない**
- そのため `_mutex` がスコープを抜けても `CloseHandle` は呼ばれない
- Windows カーネルはプロセスのハンドルテーブルで Mutex を管理し、**明示的に `CloseHandle` しない限りプロセス終了までカーネルオブジェクトは生存**

**結論**: ハンドルの Drop 挙動を正しく理解した上で、`ManuallyDrop` で意図を明示し、プロセス終了まで Mutex を保持する設計を確立した。

---

## 4. 並行性と整合性の設計

### スレッドモデル

| スレッド | 役割 | 寿命 |
|---|---|---|
| メインスレッド（Tauri runtime） | IPC ハンドラ実行、UI イベント送信 | プロセス全寿命 |
| 取り込みワーカー | `run_diff_import` / `run_enhanced_import_batch` | 取り込み 1 回ごと |
| ログビューアワーカー | tar.zst 解凍 + 行分類 + チャンク送信 | ファイル 1 つごと |

ワーカー間で共有する状態は以下のみ：

| 共有状態 | 同期方法 |
|---|---|
| キャンセルフラグ | `Arc<AtomicBool>`（Tauri State として管理） |
| SQLite DB | WAL モード + UI レベルの排他制御（`isAnalyzeRunning`） |
| Tauri AppHandle | `Clone` で各ワーカーに配布 |

### SQLite の並行性

- **WAL モード**: 読み取りと書き込みが並行可能
- **書き込みは常に 1 スレッドのみ**: UI が `isAnalyzeRunning` で取り込みボタンを disabled にし、複数取り込みワーカーの起動を防ぐ
- **読み取りはログビューアと並行可能**: アーカイブ閲覧中に取り込みを開始しても SQLITE_BUSY は発生しない

### キャンセル可能設計

```
ユーザーが「停止」ボタンクリック
       │
       ▼  IPC invoke("cancel_analyze")
   cancel_status.0.store(true, Ordering::SeqCst)
       │
       ▼  バックグラウンドスレッドが次のチェックポイントで検知
   for line in reader.lines() {
       if cancel_status.load(Ordering::SeqCst) {
           return Err(analyze_cancel_sqlite_err());
       }
       ...
   }
       │
       ▼  外側 transaction を rollback
   DB は取り込み前と完全に同じ状態
```

**意図**: チャネルや async ランタイムを導入せず、`AtomicBool` 1 個でクロススレッド連携を成立させる。応答時間は約 50ms（5000 行 ÷ 100k 行/秒）。

---

## 5. セキュリティ設計

| 観点 | 対策 |
|---|---|
| **CSP** | `default-src 'self'; script-src 'self'; img-src 'self' asset: https: data:; style-src 'self' 'unsafe-inline'; connect-src 'self' http://localhost:* ipc:;` — リモートスクリプト・WebSocket を完全遮断 |
| **Tauri Capabilities** | `core:default`, `shell:default`, `shell:allow-open` のみ。`fs` プラグインは型定義のみで I/O は Rust 経由 |
| **SQL インジェクション** | テーブル名・カラム名は `is_ascii_alphanumeric() \|\| '_'` で検証してから `format!` 補間、値は全て `params!` バインド |
| **パストラバーサル** | 外部ログ閲覧 (`read_external_log_viewer`) は `matches_external_log_format` で `output_log_*` 接頭辞 + `.txt`/`.tar.zst` 拡張子を検証 |
| **クラッシュ抑制** | clippy で `unwrap_used / expect_used / panic = deny`、`install_panic_hook` で運用ログへ退避 |
| **多重起動** | `Local\StellaRecord_SingleInstance` の名前付き Mutex |
| **インストール先制限** | NSIS で Program Files / WINDIR への配置を拒否（書き込み権限の問題を回避） |
| **依存脆弱性** | `npm audit fix` を都度実施、最新メジャー追随（直近のコミット履歴に明記） |

---

## 6. 規模と計測値

### コード規模

| 言語 | 行数 | ファイル数 |
|---|---|---|
| Rust | 4,825 | 14 |
| TypeScript / TSX | 3,211 | 40 |
| CSS Modules | 3,301 | 30+ |
| **合計** | **約 11,300 行** | **54 ソース + 30+ CSS** |

### IPC 規模

- IPC コマンド: **22 個**
- Tauri イベント: **4 種類**（`analyze-progress`, `analyze-finished`, `log_viewer_chunk`, `log_viewer_done`）

### データベース

- テーブル: **9 個**
- ビュー: **3 個**
- インデックス: **8 個**
- 検出可能なログイベント種別: **10 種類**
- 正規表現パターン: **23 種類**

### 想定パフォーマンス

| 項目 | 値 | 計測方法 |
|---|---|---|
| 圧縮率 | 約 90%（zstd lv3） | 典型的な VRChat ログ |
| ログビューア初期描画 | 100ms 以内（最初のチャンク） | sessionId 採番 → 最初の `log_viewer_chunk` 到達まで |
| スクロール fps | 60fps（10 万行） | react-virtual の overscan 10 + チャンク 500 行 |
| キャンセル応答時間 | ~50ms | 5000 行/cancel check ÷ ~100k 行/秒 |
| Mutex 起動阻止 | 即時（< 10ms） | `CreateMutexW` 失敗で `std::process::exit(0)` |

> 数値はローカル開発環境（Windows 11, Ryzen 7 + NVMe SSD）での実測ベース。本番環境では構成により変動。

---

## 7. 既知の制約と今後の改善余地

| 項目 | 現状 | 改善方向 |
|---|---|---|
| ログビューアの非 UTF-8 耐性 | 不正バイト行で打ち切られる（`map_while(Result::ok)`） | 取り込み側と同じ `continue` パターンに統一 |
| ストリーミング配列の concat | `Array.concat` で O(n²) 累積コピー | `push(...chunk)` に変更し O(n) 化 |
| ログ閲覧 modal の Esc 閉じ | 未対応 | `useEffect` で `keydown` リスナー追加 |
| パストラバーサル耐性 | `read_archive_log_viewer` は absolute path で escape 可能 | `output_log_*.tar.zst` 形式の検証を追加 |
| マイグレーション時の重複ドロップ | `INSERT OR IGNORE` でサイレント | `log_info` で件数を記録 |
| 多言語化 | 日本語固定 | i18n キー化（リソース外出し） |
| クロスプラットフォーム | Windows のみ | `#[cfg(windows)]` 隔離は済んでいるため、macOS 対応時はダイアログ/レジストリ層のみ書き換え |

> 上記制約は [`fix-task.md`](../fix-task.md) と過去の調査ログに基づき、影響範囲と修正コストを評価した結果、現バージョンでは見送った項目。
