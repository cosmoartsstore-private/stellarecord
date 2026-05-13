# STELLA RECORD 機能仕様書

> 本書は STELLA RECORD v1.0 の機能仕様を、画面・データフロー・IPC API・状態管理の観点でまとめたもの。
> 実装ファイル（`src/`, `src-tauri/`）と整合性を保ち、ポートフォリオ／引き継ぎ用途のリファレンスを兼ねる。

---

## 目次

1. [プロダクト概要](#1-プロダクト概要)
2. [アーキテクチャ](#2-アーキテクチャ)
3. [画面構成](#3-画面構成)
4. [機能仕様](#4-機能仕様)
   1. [ランチャー](#41-ランチャー-registry)
   2. [解析](#42-解析-analyze)
   3. [DB プレビュー](#43-db-プレビュー-database)
   4. [設定 / テーマ](#44-設定-settings)
5. [データフロー](#5-データフロー)
   1. [起動時インポート](#51-起動時インポート)
   2. [復元（手動アーカイブ取り込み）](#52-復元手動アーカイブ取り込み)
   3. [ログビューア・ストリーミング](#53-ログビューアストリーミング)
6. [IPC コマンド一覧](#6-ipc-コマンド一覧)
7. [Tauri イベント一覧](#7-tauri-イベント一覧)
8. [永続化と設定](#8-永続化と設定)
9. [ログ解析エンジン](#9-ログ解析エンジン)
10. [非機能要件](#10-非機能要件)

---

## 1. プロダクト概要

**STELLA RECORD** は VRChat の `output_log_*.txt` を恒久保管・正規化・閲覧するための Windows デスクトップアプリ。CosmoArtsStore の VRChat エコシステム（Polaris との連携）を構成する。

| 項目 | 内容 |
|---|---|
| アプリ ID | `com.cosmoartsstore.stellarecord` |
| ターゲット OS | Windows 10 / 11 |
| 単一インスタンス | `CreateMutexW` ベースの多重起動防止 |
| 配布形態 | NSIS インストーラ（カレントユーザーインストール） |
| 連携アプリ | Polaris（生ログのバックアップ／同期を担当する姉妹アプリ） |

### 提供価値

- VRChat 公式が **削除する古いログ**を `.tar.zst` で**恒久保管**
- ログ本文を SQLite に**構造化**して、ワールド訪問・同席ユーザー・通知などを横断検索可能にする
- 圧縮済みアーカイブから **ストリーミングでログビューア**を提供し、メモリ消費を抑えつつ大容量ログを描画

---

## 2. アーキテクチャ

### モジュール構成

```
src/                            React フロントエンド (TypeScript)
├── app/                        ルートコンポーネント、ルーティング、モーダル統括
├── features/                   機能単位の MVVM 構成
│   ├── analyze/                解析セクション
│   ├── archive/                アーカイブ取り込み・ログビューア
│   ├── database/               DB プレビュー
│   ├── registry/               ランチャー
│   └── settings/               設定（ストレージ上限・スタートアップ・テーマ）
└── shared/                     共通コンポーネント (Icons / Toast / 型 / ユーティリティ)

src-tauri/                      Rust バックエンド
├── src/
│   ├── lib.rs                  Tauri 起動・IPC ハンドラ登録
│   ├── commands/               IPC コマンドハンドラ
│   │   ├── archive.rs          アーカイブ／ログビューア（最大）
│   │   ├── database.rs         DB プレビュー
│   │   ├── import.rs           取り込み・キャンセル制御
│   │   ├── registry.rs         登録アプリ CRUD
│   │   └── settings.rs         管理設定 CRUD
│   ├── analyze/                ログ解析パイプライン
│   │   ├── mod.rs              行単位ステートマシン
│   │   ├── parser.rs           正規表現定義
│   │   └── db.rs               スキーマ定義 + マイグレーション
│   ├── config.rs               Windows レジストリ I/O
│   ├── platform.rs             Win32（Mutex / レジストリ / アイコン抽出 / ダイアログ）
│   ├── utils.rs                ロガー・エラー整形・イベント送信
│   └── models.rs               IPC ペイロード型
└── windows/                    NSIS インストーラスクリプト
```

### フロントエンド設計

各 feature は **MVVM 風の 3 層構成**：

```
features/<name>/
├── models/         型定義・純粋関数（フォーマッタなど）
├── viewmodels/     状態管理フック (use*State.ts)
└── views/          表示コンポーネント (*.tsx)
└── services/       Tauri IPC ラッパー
```

`eslint.config.js` の `no-restricted-imports` で **feature 間の相互参照を禁止**し、共通処理は `shared/` に集約させる方針を強制している。

### バックエンド設計

| モジュール | 役割 |
|---|---|
| `commands/` | フロントから呼ばれる `#[tauri::command]` 関数。検証 → ビジネスロジック層へ委譲 |
| `analyze/` | 純粋なログ解析ロジック。`run_diff_import` / `run_enhanced_import_batch` を公開 |
| `config.rs` | レジストリの読み書きを 1 箇所に集約 |
| `platform.rs` | Win32 API 呼び出しを `#[cfg(windows)]` で隔離 |

`Cargo.toml` の workspace lints で **`unwrap_used = deny`、`expect_used = deny`、`panic = deny`** を強制。例外は `parser.rs` の `compile_regex`（固定パターンのコンパイル失敗時のみ panic）のみで `#[allow(clippy::panic)]` で明示。

---

## 3. 画面構成

```
┌──────────────────────────────────────────────────────────────────┐
│  STELLA RECORD                                          ☀ Theme │  ← topNavigation
├──────────────────────────────────────────────────────────────────┤
│  [ランチャー]  [解析]  [DB]                                       │  ← pillNav
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│                    contentArea (renderSection)                    │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
                                                                ┌──┐
                                                                │ⓘ │ ← CreditButton
                                                                └──┘
```

| ナビ項目 | コンポーネント | 役割 |
|---|---|---|
| ランチャー | `RegistrySection` | 登録済みアプリの起動／フォルダオープン／登録解除 |
| 解析 | `AnalyzeSection` | ストレージメーター・取り込み・ログビューア入口 |
| DB | `DatabaseSection` | SQLite テーブル／ビューの読み取り専用ブラウザ |

モーダルは全て `App.tsx` 末尾でグローバル管理：

| モーダル | コンポーネント | 用途 |
|---|---|---|
| アーカイブ選択 | `ArchiveSelectorModal` | 取り込み・閲覧対象のファイルを選択 |
| ログビューア | `LogViewerModal` | 圧縮ログをストリーミング表示（フルスクリーン） |
| 元ログ削除 | `PolarisCleanupModal` | アーカイブ済み生ログを削除する確認画面 |
| アプリ登録 | `RegisterAppModal` | 外部 EXE をランチャーに登録 |
| クレジット | `CreditModal` | 作者情報 |

---

## 4. 機能仕様

### 4.1 ランチャー (registry)

**目的**：登録済みアプリ（StellaRecord 本体・任意の外部 EXE）をワンクリックで起動する。

| 機能 | 詳細 |
|---|---|
| 表示モード | リスト表示 / カード表示（state でトグル） |
| アプリカード | アイコン（Base64 PNG）／アプリ名／説明文／起動・フォルダオープン・登録解除ボタン |
| 自アプリ自動登録 | アプリ起動時に `ensure_self_app_registered` が StellaRecord 本体を `apps` テーブルに upsert（path 一致で更新） |
| 登録 | `RegisterAppModal` で EXE を選択 → `extract_exe_display_name` で表示名候補を取得 → ユーザー編集後に `register_app` |
| アイコン抽出 | `extract_exe_icon_png`：`SHGetImageList(SHIL_JUMBO)` で 256×256 を優先取得、失敗時に `ExtractIconExW` (32×32) でフォールバック。PNG 化して DB に BLOB 保存 |
| 起動 | `launch_external_app` → `CreateProcess` 相当（`CREATE_NO_WINDOW` フラグ付き） |

**フロントエンド**：`src/features/registry/`
**バックエンド**：`src-tauri/src/commands/registry.rs`

### 4.2 解析 (analyze)

**目的**：アーカイブの容量を可視化し、ログの取り込み／閲覧／クリーンアップを束ねる。

```
┌─ ストレージ管理 ─────────────────────────────────┐
│ アーカイブ容量                       ⟳            │
│ ──────────────────────────────────────            │
│ [██████████░░░░░░] 250 MB / 300 MB (83.3%)        │
│                                                   │
│ [警告ライン: 300 MB] [保存]                        │
│ [自動起動: ON]                                     │
└──────────────────────────────────────────────────┘
┌─ ログデータ取込・ビューア ──────────  [元ログ削除] ─┐
│ ┌──── 復元 ────┐  ┌── ログビューア ──┐            │
│ │ 圧縮済みログ  │  │ 圧縮済みログを   │            │
│ │ からデータ再  │  │ 閲覧します       │            │
│ │ 復元します    │  │                  │            │
│ │ [ログを選択]  │  │ [ログを開く]     │            │
│ └──────────────┘  └─────────────────┘            │
│                                                   │
│ ── 進捗パネル (取り込み中のみ表示) ──             │
│ 取り込み中… 12/40                                  │
│ [████████░░░░░░░░░]                                │
│ 処理中: output_log_2025-10-21_00-59-15.txt   [停止] │
└──────────────────────────────────────────────────┘
```

| 機能 | 詳細 |
|---|---|
| ストレージメーター | アーカイブディレクトリの再帰的合計サイズと閾値を表示。10GB 超で GB 表示に自動切替 |
| 警告ライン編集 | MB 単位の入力欄。`save_management_settings` 経由で Polaris と共有するレジストリに保存 |
| スタートアップトグル | Run キー (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run\StellaRecord`) への登録／解除 |
| 復元 | `ArchiveSelectorModal` で複数選択 → `launch_enhanced_import` でバックグラウンドスレッド開始 |
| ログビューア起動 | アーカイブストア配下のファイル選択もしくは「ファイルを選択」で任意の外部ファイルを開ける |
| 元ログ削除 | `PolarisCleanupModal` で「アーカイブ化済み」生ログを一覧表示し、選択削除 |
| 進捗ストリーミング | バックエンドから `analyze-progress` イベントを受信。`done/total` 形式 → % 換算してプログレスバー駆動 |
| キャンセル | `cancel_analyze` で `AtomicBool` を立て、行ループの先頭でチェック。トランザクションをロールバック |

**フロントエンド**：`src/features/analyze/`, `src/features/archive/`
**バックエンド**：`src-tauri/src/commands/archive.rs`, `src-tauri/src/commands/import.rs`

### 4.3 DB プレビュー (database)

**目的**：取り込まれた SQLite データを開発者・上級ユーザー向けに直接ブラウジングする。

| 機能 | 詳細 |
|---|---|
| サイドバー | テーブル一覧（`is_view=false`）／ビュー一覧をセクション分けして表示。日本語ラベル ⇄ 物理名トグル |
| ページネーション | 1 ページ 500 行。`currentPage * 500 + 1 ~ (currentPage+1) * 500` を表示 |
| ソート | カラムヘッダクリックで `asc → desc → 解除`（未設定時は `default_sort` にフォールバック） |
| カラム情報 | `TABLE_COMMENTS` 定数（`commands/database.rs`）で日本語ラベル＋説明文を提供 |
| 安全性 | テーブル名は `sanitize_table_name` (ASCII 英数+`_` のみ) で SQL 構築前に検証、ソートカラム名も同等の検証を経由 |
| BLOB 表示 | `<BLOB>` 文字列に置換（アイコン画像など） |
| NULL 表示 | `NULL` 文字列 |

表示可能テーブル／ビューの完全な一覧は [`database.md`](database.md) を参照。

**フロントエンド**：`src/features/database/`
**バックエンド**：`src-tauri/src/commands/database.rs`

### 4.4 設定 (settings)

| 設定 | 入力 | 保存先 |
|---|---|---|
| アーカイブ容量警告ライン | MB 単位の数値 | レジストリ `HKCU\Software\CosmoArtsStore\Polaris\CapacityThresholdBytes` |
| スタートアップ登録 | ON/OFF トグル | レジストリ `Run` キー + `HKCU\Software\CosmoArtsStore\StellaRecord\EnableStartup` |
| テーマ | Light / Dark / Midnight | `localStorage` `stella-theme` |

テーマ切替はクリック時にトランジションを一時無効化（`disable-transitions` クラス → 2 フレーム後に解除）し、テーマ間の中間色ちらつきを防ぐ。

**フロントエンド**：`src/features/settings/`
**バックエンド**：`src-tauri/src/commands/settings.rs`

---

## 5. データフロー

### 5.1 起動時インポート

```
┌──────────────┐          ┌──────────────────────────────────────┐
│ App.tsx 初期 │ on mount │ launch_startup_archive_import (IPC)  │
│  useEffect   │ ───────▶ │  ├ collect_pending_archive_sync_plans│
└──────────────┘          │  ├ sync_source_logs_into_archive_store
                          │  │   (生ログ → .tar.zst)             │
                          │  └ run_diff_import                   │
                          │     ├ collect_log_files              │
                          │     └ for each archive:              │
                          │        ├ session 既存チェック        │
                          │        ├ savepoint                   │
                          │        ├ parse_and_import_reader     │
                          │        └ commit / rollback           │
                          └───────────┬──────────────────────────┘
                                      │ emit("analyze-progress")
                                      ▼
                          ┌──────────────────────────────────────┐
                          │ useAnalyzeState                       │
                          │  listen("analyze-progress")           │
                          │   → progress bar 更新                 │
                          └──────────────────────────────────────┘
```

- バックエンドは `std::thread::spawn` でワーカースレッドを生成し IPC は即座に返す
- 進捗は `analyze-progress` イベントで `{status, progress, is_running}` を送出
- 完了通知は `analyze-finished` イベント
- UI は `isAnalyzeRunning` フラグでボタンを `disabled` にし、並行起動を防ぐ

### 5.2 復元（手動アーカイブ取り込み）

ユーザーが「ログを選択」を押下 →

1. `list_archive_files` でアーカイブ一覧取得 → `ArchiveSelectorModal` 表示
2. ユーザーが複数選択して確定 → `launch_enhanced_import(file_names)`
3. バックエンドが個別パスを検証してワーカースレッド起動
4. `run_enhanced_import_batch` が各ファイルに対し savepoint で解析・取り込み
5. キャンセル時は外側トランザクションをロールバック → DB は無変更

### 5.3 ログビューア・ストリーミング

```
ユーザー: ファイルクリック
   │
   ▼ React side:
openStreamForFile(fileKey)
   ├ stopStream()         ← 前回リスナーを解除
   ├ flushSync(empty)      ← UI を空状態にリセット
   ├ sessionId 採番        ← Date.now + Math.random
   ├ listen("log_viewer_chunk", filter by sessionId → pendingChunksRef にバッファ)
   ├ listen("log_viewer_done",  filter by sessionId → flush + 完了マーク)
   └ start*LogViewerStream(fileKey, sessionId)
                                  │ IPC
                                  ▼ Rust side:
                          spawn_compressed_log_stream
                            └ tar.zst → 1 エントリ取り出し
                               └ emit_log_viewer_chunks
                                  ├ 行を読み、レベル／カテゴリを分類
                                  ├ 500 行ごとに app.emit("log_viewer_chunk")
                                  └ EOF で app.emit("log_viewer_done")
```

ポイント：

- **500 行 × 500 ファイル分**のような大規模ログでも UI を凍結させないために、500 行単位でチャンク送信
- React 側は **100 ms タイマー**で `pendingChunksRef` をまとめてフラッシュし、再レンダリングを集約
- ファイル切替時に**残ったチャンクは sessionId 不一致でドロップ**されるため、古いファイルの行が混ざらない
- 表示は `@tanstack/react-virtual` による**仮想スクロール**で、行数に比例しないレンダリングコスト

---

## 6. IPC コマンド一覧

`src-tauri/src/lib.rs` の `invoke_handler!` で登録されたコマンド。

### archive

| コマンド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `list_archive_files` | - | `Vec<ArchiveFileItem>` | アーカイブ一覧 |
| `read_archive_log_viewer` | `file_name`, `session_id` | `LogViewerMeta` | アーカイブをストリーム再生 |
| `read_external_log_viewer` | `file_path`, `session_id` | `LogViewerMeta` | 外部ファイルをストリーム再生 |
| `pick_log_files` | - | `Vec<String>` | ネイティブダイアログでログ選択 |
| `get_storage_status` | - | `(u64, u64)` | 現在容量 / 上限 |
| `open_folder` | `path` | - | OS シェルでフォルダを開く |
| `get_deletable_source_logs` | - | `Vec<DeletableLogInfo>` | アーカイブ済み生ログ一覧 |
| `delete_source_logs` | `file_names` | `usize` | アーカイブ存在を確認の上で削除 |

### database

| コマンド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `get_db_tables` | - | `Vec<DbTableSummary>` | 表示可能テーブル／ビュー一覧 |
| `get_db_table_data` | `table_name`, `page?`, `sort_column?`, `sort_dir?` | `TableData` | 1 ページ分のテーブルデータ |

### import

| コマンド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `launch_enhanced_import` | `file_names` | `String` | 選択アーカイブの取り込み開始 |
| `launch_startup_archive_import` | - | `StartupImportSummary` | 起動時の差分取り込み |
| `cancel_analyze` | - | - | 実行中の取り込みを中断 |

### registry

| コマンド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `pick_exe_file` | - | `Option<String>` | EXE 選択ダイアログ |
| `extract_exe_display_name` | `path` | `String` | EXE の VersionInfo 表示名 |
| `register_app` | `path`, `name`, `description` | - | ランチャーに追加 |
| `unregister_app` | `path` | - | 登録解除 |
| `launch_external_app` | `app_path` | - | 外部 EXE 起動 |

### settings

| コマンド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `get_management_settings` | - | `ManagementSettings` | 現在の設定取得 |
| `save_management_settings` | `startup_enabled`, `archive_limit_mb` | - | 設定保存 |
| `read_registry_catalog` | - | `RegistryCatalog` | DB の `apps` を読み込み |

---

## 7. Tauri イベント一覧

| イベント | 送信元 | ペイロード | 用途 |
|---|---|---|---|
| `analyze-progress` | 取り込みワーカー | `{status, progress, is_running}` | 進捗バー更新 |
| `analyze-finished` | 取り込みワーカー | `()` | 完了通知（UI を非実行状態に戻す） |
| `log_viewer_chunk` | ログストリームワーカー | `LogViewerChunk` | 500 行単位のログ転送 |
| `log_viewer_done` | ログストリームワーカー | `String (sessionId)` | ストリーム完了通知 |

---

## 8. 永続化と設定

### Windows レジストリ

| キー | 値 | 説明 |
|---|---|---|
| `HKCU\Software\CosmoArtsStore\StellaRecord\InstallLocation` | string | NSIS が書き込むインストール先 |
| `HKCU\Software\CosmoArtsStore\StellaRecord\ArchivePath` | string | アーカイブストア（空ならデフォルト） |
| `HKCU\Software\CosmoArtsStore\StellaRecord\DbPath` | string | DB ファイル（空ならデフォルト） |
| `HKCU\Software\CosmoArtsStore\StellaRecord\EnableStartup` | u32 | 自動起動 ON/OFF |
| `HKCU\Software\CosmoArtsStore\StellaRecord\StartupPreferenceSet` | u32 | ユーザーが選択済みかどうか |
| `HKCU\Software\CosmoArtsStore\Polaris\CapacityThresholdBytes` | u64 | アーカイブ警告ライン（Polaris と共有） |
| `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\StellaRecord` | string | 自動起動コマンド |

### LocalStorage

| キー | 値 | 説明 |
|---|---|---|
| `stella-theme` | `"light" \| "dark" \| "midnight"` | UI テーマ |

### ファイルシステム

```
$INSTDIR/Data/
├── archive/                          .tar.zst（アンインストール時保護対象）
│   └── output_log_2025-10-21_00-59-15.txt.tar.zst
├── db/
│   └── stellarecord.db               SQLite（WAL モード）
│   └── stellarecord.db-wal           Write-Ahead Log
│   └── stellarecord.db-shm           共有メモリ
├── logs/
│   └── info-2025-11.log              月次ローテーション
└── EBWebView/                        WebView2 キャッシュ
```

---

## 9. ログ解析エンジン

`src-tauri/src/analyze/` の責務は **VRChat ログを行単位ステートマシンで読み、正規化された SQLite レコードに変換すること**。

### 検出イベント

| 行種別 | 対応テーブル | 主な情報 |
|---|---|---|
| `User Authenticated: Name (usr_xxx)` | `sessions` | アカウント |
| `[Behaviour] Entering Room: <World>` | `visits` (pending) | ワールド名候補 |
| `[Behaviour] Joining wrld_xxx:N~private(...)~region(jp)` | `visits` | インスタンス確定 |
| `[Behaviour] OnLeftRoom` | `visits` | leave_time |
| `[Behaviour] OnPlayerJoined Name (usr_xxx)` | `find_users` + `with_users` | 同席ユーザー |
| `[Behaviour] OnPlayerLeft Name (usr_xxx)` | `with_users` | leave_time |
| `[Behaviour] Initialized PlayerAPI "Name" is local` | `with_users.is_self=1` | 自分の判定 |
| `Received Notification: <...>` | `notifications` | 招待・boop・グループ通知 |
| `Get VRChat Subscription Details!` | `subscription` | VRChat+ 状態 |
| `[VRC Camera] Took screenshot to: ... NxM.png` | `screenshots` | 撮影 |
| `Found new OSC Service: ...` | `osc` | 外部 OSC ツール検出 |

正規表現は `src-tauri/src/analyze/parser.rs` で `LazyLock<Regex>` として一度だけコンパイル。不正パターンは即パニック（開発時に検知）。

### トランザクション設計

- **全アーカイブ取り込みを 1 つの外側 transaction で囲む**
- **各ファイルを savepoint** で個別にロールバック可能にする
- キャンセルや個別ファイルのエラーは savepoint rollback で部分失敗を許容
- 致命エラー／キャンセル時は外側 transaction ごとロールバックして **DB を完全に元に戻す**

### 冪等性

- `sessions` は `log_name UNIQUE` で同一ログの再取り込みを防止
- `notifications` は `notif_id UNIQUE` で重複挿入を `INSERT OR IGNORE` で吸収
- `with_users` は `UNIQUE(visit_id, vrchat_id)` で同訪問内の重複を弾く

---

## 10. 非機能要件

### パフォーマンス

| 項目 | 目標／実装 |
|---|---|
| 起動時間 | 〜2 秒（取り込み起動は別スレッドで非同期） |
| ログビューア | 10 万行超でも仮想スクロール + 100 ms バッファリングで 60fps を維持 |
| 取り込みスループット | 5000 行ごとに進捗イベント送出、SQLite は WAL モードで挿入を最適化 |
| 圧縮率 | zstd レベル 3、典型的な VRChat ログで 90% 以上の削減 |

### セキュリティ

| 項目 | 対策 |
|---|---|
| Tauri CSP | `default-src 'self'; script-src 'self'; img-src 'self' asset: https: data:; style-src 'self' 'unsafe-inline'; connect-src 'self' http://localhost:* ipc:;` |
| Tauri Capabilities | `core:default`, `shell:default`, `shell:allow-open` のみ |
| SQL インジェクション | 動的テーブル名・カラム名は ASCII 英数字+`_` のみ許可。値は全て `params!` バインド |
| パニック抑制 | clippy で `unwrap_used / expect_used / panic = deny`、`install_panic_hook` で運用ログへ退避 |
| 多重起動 | `Local\StellaRecord_SingleInstance` の Mutex |
| インストール先 | NSIS で Program Files / WINDIR への配置を拒否（LocalAppData 推奨） |

### 可用性

- ファイルパスや DB の失敗は**警告ログに退避**しつつアプリ起動は継続
- 自アプリのランチャー登録に失敗してもアプリ起動を妨げない
- パニック発生時は `install_panic_hook` がメッセージを `Data/logs/info-YYYY-MM.log` に記録
- 致命的な起動失敗は `MessageBoxW` でユーザーに通知

### 国際化

- 現バージョンは **UI 全文日本語**固定
- バックエンドのエラーメッセージも日本語
- 多言語化する場合の差し替え点は `commands/database.rs` の `TABLE_COMMENTS` と各 React コンポーネントのリテラル
