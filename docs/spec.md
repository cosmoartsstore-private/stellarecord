# Specification

> StellaRecord v1.0.0 の機能仕様書。アーキテクチャ・モジュール構成・IPC API・データフロー・並行性モデル・性能特性をリファレンス形式で記述する。

## Table of Contents

- [Overview](#overview)
- [Glossary](#glossary)
- [Architecture](#architecture)
- [Module Organization](#module-organization)
- [Feature Specifications](#feature-specifications)
  - [Launcher (Registry)](#launcher-registry)
  - [Analyze](#analyze)
  - [Database Preview](#database-preview)
  - [Settings](#settings)
- [IPC Reference](#ipc-reference)
- [Event Reference](#event-reference)
- [Data Flow](#data-flow)
- [State Management](#state-management)
- [Concurrency Model](#concurrency-model)
- [Performance Characteristics](#performance-characteristics)
- [Security Model](#security-model)
- [Persistence](#persistence)
- [Log Parser Reference](#log-parser-reference)
- [Known Limitations](#known-limitations)

---

## Overview

StellaRecord は VRChat のゲームログを長期保存と検索可能な構造化データに変換する Windows デスクトップアプリケーションである。

### Goals

- VRChat の生ログを失わずに恒久保管する
- 過去のワールド訪問・同席ユーザー・通知・スクリーンショットを後から検索可能にする
- 圧縮アーカイブを展開せず即座に閲覧する
- ローカル完結で動作し、外部サーバ依存を持たない

### Non-Goals

- VRChat クライアントの改造・パッチ適用
- リアルタイムでの VRChat ネットワーク通信の傍受
- マルチユーザー・サーバ運用
- iOS / Android / macOS / Linux 対応

---

## Glossary

| 用語 | 定義 |
| ---- | ---- |
| **生ログ** | VRChat が出力する `output_log_*.txt` ファイル |
| **アーカイブ** | 生ログを `tar + zstd` で圧縮した `.tar.zst` ファイル |
| **アーカイブストア** | アーカイブを格納するディレクトリ (`Data/archive/`) |
| **メイン DB** | 解析済みデータを格納する SQLite (`Data/db/stellarecord.db`) |
| **セッション** | 1 つのログファイル＝1 セッション |
| **訪問 (visit)** | ワールドインスタンスへの 1 回の入退室 |
| **取り込み (import)** | アーカイブを解析して DB に書き込む処理 |
| **Polaris** | 姉妹アプリ。生ログのバックアップ／同期を担当 |

---

## Architecture

### Layered View

```
┌──────────────────────────────────────────────────────────────┐
│ Presentation Layer (React + TypeScript)                      │
│  - Views: 表示専用コンポーネント                              │
│  - ViewModels: 状態管理フック (use*State.ts)                  │
│  - Services: Tauri invoke ラッパー                            │
├──────────────────────────────────────────────────────────────┤
│ IPC Boundary (Tauri 2.2)                                     │
│  - 21 commands (invoke)                                      │
│  - 4 event channels (listen)                                 │
├──────────────────────────────────────────────────────────────┤
│ Application Layer (Rust)                                     │
│  - commands/: IPC ハンドラ（バリデーション → 委譲）          │
│  - analyze/: ログ解析パイプライン                            │
│  - config.rs: レジストリ I/O                                 │
│  - platform.rs: Win32 API ラッパー                           │
├──────────────────────────────────────────────────────────────┤
│ Storage Layer                                                │
│  - SQLite (rusqlite, WAL mode)                               │
│  - Filesystem (.tar.zst archives)                            │
│  - Windows Registry                                          │
└──────────────────────────────────────────────────────────────┘
```

### Process Model

StellaRecord は 1 プロセス・複数スレッドで動作する。

| Thread | Owner | Lifetime |
| ------ | ----- | -------- |
| Main thread | Tauri runtime, IPC handler | プロセス全寿命 |
| Import worker | `analyze::run_diff_import` / `run_enhanced_import_batch` | 取り込み 1 回ごと |
| Log viewer worker | `spawn_compressed_log_stream` / `spawn_plain_log_stream` | ファイル 1 つごと |

ワーカースレッドは `std::thread::spawn` で生成され、終了時に自動回収される。スレッド間通信は `Arc<AtomicBool>`（キャンセルフラグ）と Tauri イベント送出経由で行う。

---

## Module Organization

### Frontend (`src/`)

```
src/
├── app/                    アプリケーションルート
│   ├── App.tsx             ルートコンポーネント、ルーティング、モーダル統括
│   ├── useAppModals.ts     モーダル状態管理
│   ├── ToastContainer.tsx  グローバルトースト
│   ├── CreditModal.tsx     クレジット表示
│   └── section.ts          セクション定義
├── features/               機能モジュール（feature 間の相互参照禁止）
│   ├── analyze/            解析セクション
│   ├── archive/            アーカイブ取り込み・ログビューア
│   ├── database/           DB プレビュー
│   ├── registry/           ランチャー
│   └── settings/           設定
└── shared/                 共通基盤
    ├── components/Icons.tsx
    ├── hooks/useToasts.ts
    ├── lib/storageFormat.ts
    ├── models/types.ts
    └── styles/shared.module.css
```

各 feature 内部の構成:

```
features/<name>/
├── models/         型定義・純粋関数
├── viewmodels/     状態管理フック (use*State.ts)
├── views/          表示コンポーネント
└── services/       Tauri IPC ラッパー
```

### Backend (`src-tauri/src/`)

```
src-tauri/src/
├── main.rs                    エントリポイント
├── lib.rs                     Tauri 起動と IPC ハンドラ登録
├── commands/                  Tauri IPC コマンド
│   ├── mod.rs                 共通ヘルパー、設定型
│   ├── archive.rs             アーカイブ・ログビューア
│   ├── database.rs            DB プレビュー
│   ├── import.rs              取り込み制御
│   ├── registry.rs            登録アプリ CRUD
│   └── settings.rs            管理設定 CRUD
├── analyze/                   ログ解析パイプライン
│   ├── mod.rs                 行単位ステートマシン
│   ├── parser.rs              正規表現定義
│   └── db.rs                  スキーマ定義 + マイグレーション
├── config.rs                  Windows レジストリ I/O
├── platform.rs                Win32 API（Mutex, アイコン, ダイアログ）
├── utils.rs                   ロガー、エラー整形、イベント送信
└── models.rs                  IPC ペイロード型
```

---

## Feature Specifications

### Launcher (Registry)

**Purpose**: 登録された外部 EXE を一覧表示し、起動・フォルダオープン・登録解除を行う。

#### Components

- `RegistrySection.tsx` — メインビュー（リスト／カードの表示モード切替）
- `RegisterAppModal.tsx` — 新規登録モーダル
- `RegistryIcons.tsx` — フォールバックアイコン
- `useRegistryState.ts` — 状態管理フック
- `registryService.ts` — IPC ラッパー

#### Behavior

- 起動時に `read_registry_catalog` で `apps` テーブルを読み込み一覧表示する
- アイコンは PNG バイナリ (BLOB) として DB に保管され、IPC 境界で Base64 化して `<img src="data:image/png;base64,...">` でレンダリングする
- 表示モード（リスト／カード）は React state で管理（永続化なし）

#### Commands Used

`pick_exe_file`, `extract_exe_display_name`, `register_app`, `unregister_app`, `launch_external_app`, `open_folder`, `read_registry_catalog`

---

### Analyze

**Purpose**: アーカイブストアの容量管理、ログ取り込み、ログ閲覧、元ログ削除を統括する解析ダッシュボード。

#### Components

- `AnalyzeSection.tsx` — メインダッシュボード
- `PolarisCleanupModal.tsx` — 元ログ削除モーダル
- `ArchiveSelectorModal.tsx` — アーカイブ選択モーダル
- `LogViewerModal.tsx` — フルスクリーンログビューア
- `useAnalyzeState.ts`, `useArchiveState.ts`, `useArchiveSelection.ts` — 状態管理
- `analyzeService.ts`, `archiveService.ts` — IPC ラッパー

#### Behavior

##### Storage Meter

`get_storage_status` で現在のアーカイブストアサイズと閾値を取得し、進捗バーで表示する。10 GB を超える場合は GB 表示、それ以下は MB 表示に自動切替する。

##### Manual Import (Restore)

1. ユーザーが「ログを選択」を押下 → `list_archive_files` でアーカイブ一覧を取得
2. `ArchiveSelectorModal` で複数選択
3. `launch_enhanced_import(file_names)` でバックグラウンドスレッドを開始
4. バックエンドから `analyze-progress` イベントを受信 → 進捗バーを更新
5. 完了時に `analyze-finished` イベントを受信 → UI を非実行状態に戻す

##### Log Viewer

詳細は [Data Flow](#log-viewer-streaming) を参照。

##### Cleanup

`get_deletable_source_logs` でアーカイブ済み生ログの一覧を取得し、`PolarisCleanupModal` で選択削除する。アーカイブが存在しない生ログは保護対象として表示されない。

#### Commands Used

`get_storage_status`, `list_archive_files`, `read_archive_log_viewer`, `read_external_log_viewer`, `pick_log_files`, `launch_enhanced_import`, `launch_startup_archive_import`, `cancel_analyze`, `get_deletable_source_logs`, `delete_source_logs`, `open_folder`

---

### Database Preview

**Purpose**: 取り込まれた SQLite データを読み取り専用でブラウジングする開発者・上級ユーザー向け機能。

#### Components

- `DatabaseSection.tsx` — メインビュー
- `useDatabaseState.ts` — 状態管理
- `databaseService.ts` — IPC ラッパー

#### Behavior

- サイドバーに表示可能テーブル／ビューを一覧表示。`is_view` フラグで「テーブル」「ビュー」セクションに分類
- 物理名（英語）／論理名（日本語）の表示切替トグルを提供
- ページネーション: 1 ページ 500 行固定
- ソート: カラムヘッダクリックで `asc → desc → 解除`。未指定時は `TABLE_COMMENTS` の `default_sort` を使用
- BLOB は `<BLOB>` 文字列に置換。NULL は `NULL` 文字列で表示

#### Validation

- テーブル名は `sanitize_table_name` で ASCII 英数字と `_` のみを許可
- ソートカラム名は同等のバリデーションを `get_db_table_data` 内で実施
- ソート方向は `"asc"` リテラルとそれ以外（`DESC`）の二択

#### Commands Used

`get_db_tables`, `get_db_table_data`

---

### Settings

**Purpose**: アプリ全体の管理設定（容量上限、自動起動、テーマ）を編集する。設定 UI は Analyze セクション内に配置されている。

#### Components

- `SettingsControls.tsx` — 設定入力フォーム
- `useSettingsState.ts` — 状態管理
- `settingsService.ts` — IPC ラッパー
- `models/theme.ts` — テーマ定義と LocalStorage I/O

#### Behavior

| Setting | Source | Persistence |
| ------- | ------ | ----------- |
| アーカイブ容量警告ライン (MB) | `get_management_settings` | レジストリ `Polaris\CapacityThresholdBytes` |
| 自動起動 | `get_management_settings` | レジストリ `Run` キー + `StellaRecord\EnableStartup` |
| 自動起動の初回プロンプト判定 | `get_management_settings` (`startup_preference_set`) | レジストリ `StellaRecord\StartupPreferenceSet` |
| テーマ | `localStorage` | `localStorage["stella-record-theme"]` |

`save_management_settings(startup_enabled, archive_limit_mb)` で容量と自動起動を一括保存する。容量は最低 1 MB に正規化される。`startup_preference_set` は読み取り専用で、ユーザーが一度でも自動起動を明示的に保存したかを示す（未保存ならスプラッシュで初回プロンプトを表示する）。

#### Commands Used

`get_management_settings`, `save_management_settings`

---

## IPC Reference

`src-tauri/src/lib.rs` の `tauri::generate_handler!` で登録された 21 コマンド。

### archive

| Command | Args | Returns | Description |
| ------- | ---- | ------- | ----------- |
| `list_archive_files` | - | `Vec<ArchiveFileItem>` | アーカイブストア配下の `.tar.zst` 一覧 |
| `read_archive_log_viewer` | `file_name: String, session_id: String` | `LogViewerMeta` | アーカイブをストリーム再生 |
| `read_external_log_viewer` | `file_path: String, session_id: String` | `LogViewerMeta` | 外部ファイルをストリーム再生（`output_log_*` 検証あり） |
| `pick_log_files` | - | `Vec<String>` | ネイティブダイアログでログ選択 |
| `get_storage_status` | - | `(u64, u64)` | (現在容量, 上限) のタプル |
| `open_folder` | `path: &str` | `()` | OS シェルでフォルダを開く |
| `get_deletable_source_logs` | - | `Vec<DeletableLogInfo>` | アーカイブ済み生ログ一覧 |
| `delete_source_logs` | `file_names: Vec<String>` | `usize` | 削除した件数 |

### database

| Command | Args | Returns | Description |
| ------- | ---- | ------- | ----------- |
| `get_db_tables` | - | `Vec<DbTableSummary>` | 表示可能テーブル／ビュー一覧 |
| `get_db_table_data` | `table_name: &str, page: Option<u32>, sort_column: Option<String>, sort_dir: Option<String>` | `TableData` | 1 ページ分のテーブルデータ（500 行） |

### import

| Command | Args | Returns | Description |
| ------- | ---- | ------- | ----------- |
| `launch_enhanced_import` | `file_names: Vec<String>` | `String` | 選択アーカイブの取り込み開始 |
| `launch_startup_archive_import` | - | `()` | 起動時の差分取り込み（進捗はイベント経由） |
| `cancel_analyze` | - | `()` | 実行中の取り込みを中断 |

### registry

| Command | Args | Returns | Description |
| ------- | ---- | ------- | ----------- |
| `pick_exe_file` | - | `Option<String>` | EXE 選択ダイアログ |
| `extract_exe_display_name` | `path: String` | `String` | EXE の VersionInfo 表示名 |
| `register_app` | `path: String, name: String, description: String` | `()` | ランチャーに追加 |
| `unregister_app` | `path: String` | `()` | 登録解除 |
| `launch_external_app` | `app_path: &str` | `()` | 外部 EXE 起動 |

### settings

| Command | Args | Returns | Description |
| ------- | ---- | ------- | ----------- |
| `get_management_settings` | - | `ManagementSettings` | 現在の設定取得 |
| `save_management_settings` | `startup_enabled: bool, archive_limit_mb: u64` | `()` | 設定保存 |
| `read_registry_catalog` | - | `RegistryCatalog` | DB の `apps` を読み込み |

### Payload Types

| Type | Definition Location |
| ---- | ------------------- |
| `AnalyzePayload`, `DbTableSummary`, `DbColumnMeta`, `TableData`, `LogViewerMeta`, `LogViewerChunk`, `ArchiveFileItem`, `DeletableLogInfo` | `src-tauri/src/models.rs` |
| `ManagementSettings` | `src-tauri/src/commands/mod.rs` |
| `PolarisSetting`, `StellaRecordSetting`, `AppCard`, `RegistryCatalog` | `src-tauri/src/config.rs` |

---

## Event Reference

Tauri の `app.emit()` でバックエンドから送出されるイベント。フロントエンドは `@tauri-apps/api/event` の `listen()` で購読する。

| Event Name | Payload | Emitter | Subscriber |
| ---------- | ------- | ------- | ---------- |
| `analyze-progress` | `{status: String, progress: String, is_running: bool}` | 取り込みワーカー | `useAnalyzeState` |
| `analyze-finished` | `()` | 取り込みワーカー | `useAnalyzeState` |
| `log_viewer_chunk` | `LogViewerChunk` | ログビューアワーカー | `useArchiveState` |
| `log_viewer_done` | `String` (sessionId) | ログビューアワーカー | `useArchiveState` |

`progress` フィールドは `"done/total"` 形式（例: `"12/40"`）または `"100%"` の混在。フロント側で `/` の有無を判定して % に換算する。

---

## Data Flow

### Startup Import

アプリ起動時に `App.tsx` の `useEffect` が `launch_startup_archive_import` を呼び出す。

```
React (App.tsx)
  └─ useEffect on mount
       └─ invoke("launch_startup_archive_import")
            │
            ▼ (Rust, std::thread::spawn)
       Worker thread:
         1. collect_pending_archive_sync_plans
              → 生ログ vs アーカイブの差分を計算
         2. sync_source_logs_into_archive_store
              → 未アーカイブの生ログを .tar.zst に圧縮
         3. analyze::run_diff_import
              ├─ Connection::open + init_main_db (WAL + foreign_keys)
              ├─ collect_log_files (アーカイブ一覧)
              ├─ outer transaction.begin
              └─ for each archive:
                   ├─ session 既存チェック (SELECT EXISTS)
                   ├─ savepoint.begin
                   ├─ parse_and_import_reader
                   ├─ savepoint.commit (or rollback on error)
                   └─ emit("analyze-progress")
              outer transaction.commit
              emit("analyze-finished")
            │
            ▼
       React (useAnalyzeState):
         listen("analyze-progress") → progress bar 更新
         listen("analyze-finished") → isAnalyzeRunning = false
```

### Manual Import (Restore)

```
User: 「ログを選択」クリック
  └─ list_archive_files → ArchiveSelectorModal
       └─ User: 複数選択 + 確定
            └─ launch_enhanced_import(file_names)
                 │
                 ▼ (Rust)
            Worker thread:
              analyze::run_enhanced_import_batch
                同上の savepoint パターンで取り込み
                emit("analyze-progress") を逐次送出
                emit("analyze-finished")
```

### Log Viewer Streaming

```
User: ファイル選択
  ↓
React (useArchiveState.openStreamForFile):
  1. stopStream()                            前回リスナー解除
  2. clearTimeout(flushTimer)
  3. flushSync(setLogViewerData(empty))      UI を空状態にリセット
  4. sessionId = `${Date.now()}-${random}`   セッション ID 採番
  5. listen("log_viewer_chunk", handler)     新リスナー登録
  6. listen("log_viewer_done", handler)
  7. invoke("read_archive_log_viewer", {file_name, sessionId})
       │
       ▼ (Rust, std::thread::spawn)
  Worker thread (spawn_compressed_log_stream):
    1. fs::File::open + zstd::Decoder + tar::Archive
    2. tar entries.next() → 単一エントリを取得
    3. emit_log_viewer_chunks(BufReader, sessionId, ...)
         for line in reader.lines():
           - レベル分類 (plain/info/warning/error/debug)
           - カテゴリ分類 (世界/通知/入退室/...)
           - DB キーワードマーカー解決
           - 500 行ごとに app.emit("log_viewer_chunk", chunk)
         on EOF:
           app.emit("log_viewer_done", sessionId)
       │
       ▼
React (handler):
  - if (payload.session_id !== currentSessionId) return  ← 古いセッション破棄
  - pendingChunksRef.current.push(payload)
  - if (!flushTimer) setTimeout(flushChunks, 100)
       │
       ▼ 100ms 経過
  flushChunks:
    setLogViewerData(prev => chunks.reduce(appendChunk, prev))
       │
       ▼
React (LogViewerModal):
  @tanstack/react-virtual で表示行のみ DOM 化
```

### Cancellation

```
User: 「停止」ボタンクリック
  └─ invoke("cancel_analyze")
       └─ AnalyzeCancelStatus.0.store(true, Ordering::SeqCst)
            │
            ▼
       Worker thread (parse_and_import_reader):
         for line in reader.lines():
           if cancel_status.load(SeqCst):
             return Err(analyze_cancel_sqlite_err())
                ↓
              run_diff_import がキャッチ:
                rollback_savepoint
                rollback_outer_transaction
                emit("analyze-progress", "キャンセルしました")
                emit("analyze-finished")
```

---

## State Management

フロントエンドの状態管理は React Hooks のみで構成される。状態管理ライブラリ（Redux 等）は使用しない。

### State Hierarchy

| Scope | Owner | Hook |
| ----- | ----- | ---- |
| Application root | `App.tsx` | `useState` (theme, activeSection, modal flags) |
| Per-feature | `features/<name>/viewmodels/use*State.ts` | カスタムフック |
| Modal coordination | `useAppModals.ts` | カスタムフック |
| Toast queue | `useToasts.ts` (shared) | カスタムフック |

### Persistence

| State | Persistence |
| ----- | ----------- |
| Theme | LocalStorage `stella-record-theme` |
| Active section | メモリのみ（再起動でリセット） |
| Display mode (list/card) | メモリのみ |
| Settings (容量上限、自動起動) | Windows Registry |
| Database content | SQLite |

---

## Concurrency Model

### Shared State

| Resource | Synchronization |
| -------- | --------------- |
| Cancel flag | `Arc<AtomicBool>` (Tauri State) |
| SQLite database | WAL モード + UI レベルの排他制御 (`isAnalyzeRunning`) |
| Tauri AppHandle | `Clone` で各ワーカーに配布 |

### Write Exclusion

書き込みは常に 1 ワーカーのみ。UI が `isAnalyzeRunning` フラグで取り込みボタンを `disabled` にし、複数ワーカーの同時起動を防止する。

### Read Concurrency

WAL モードにより、書き込み中でも読み取り（ログビューア、DB プレビュー）が並行動作可能。`SQLITE_BUSY` は発生しない設計。

### Cancellation Latency

キャンセルは協調的にチェックされる。`parse_and_import_reader` の行ループ先頭で `cancel_status.load()` を毎回確認し、典型的には 50ms 以内（5000 行 ÷ 約 100k 行/秒）に停止する。

### Process-level Exclusion

複数プロセスからの DB 競合を防ぐため、Windows カーネル名前付き Mutex (`Local\StellaRecord_SingleInstance`) で多重起動を阻止する。`CreateMutexW` の戻り値が `ERROR_ALREADY_EXISTS` を返した場合、`std::process::exit(0)` で即終了する。

---

## Performance Characteristics

| Metric | Value | Notes |
| ------ | ----- | ----- |
| Compression ratio | 約 90% | zstd level 3、典型的な VRChat ログ |
| Parse throughput | ~100k 行/秒 | ローカル開発環境（Ryzen 7, NVMe SSD） |
| Cancellation latency | ~50ms | 5000 行ごとのチェックポイント |
| Log viewer first chunk | < 100ms | sessionId 採番から最初の `log_viewer_chunk` 到達まで |
| Log viewer scroll | 60 fps | 仮想スクロール、overscan 10 |
| IPC chunk size | 500 行 | `emit_log_viewer_chunks::CHUNK_SIZE` |
| UI flush interval | 100ms | `useArchiveState` の `flushTimer` |
| DB page size | 500 行 | `get_db_table_data::PAGE_SIZE` |

数値はローカル測定の概算。本番環境では構成により変動する。

---

## Security Model

### Threat Model

本アプリの脅威モデルは「シングルユーザーのデスクトップアプリケーション」を前提とする。マルチテナント・ネットワーク経由の攻撃は対象外。

### Mitigations

| Threat | Mitigation |
| ------ | ---------- |
| WebView 経由の任意 OS API 実行 | Tauri Capabilities を `core:default`, `shell:default`, `shell:allow-open` に限定 |
| リモートスクリプトロード | CSP `script-src 'self'`、`connect-src` を `'self' http://localhost:* ipc:` に制限 |
| SQL インジェクション | テーブル名・カラム名は ASCII 英数字+`_` のみを許可、値は `params!` でバインド |
| パストラバーサル（外部ログ閲覧） | `matches_external_log_format` で `output_log_*.txt` / `*.tar.zst` に制限 |
| 多重起動による DB 破壊 | Windows カーネル名前付き Mutex で防止 |
| 予期せぬ panic | clippy で `unwrap_used / expect_used / panic = deny`、`install_panic_hook` で運用ログへ退避 |
| インストール先の権限問題 | NSIS で Program Files / WINDIR を拒否、`installMode: currentUser` |

### Out-of-Scope

- コード署名（未実装）
- 自動アップデート（未実装、インストーラ再実行で対応）
- 暗号化された DB（SQLCipher 未採用、ローカル FS 権限に依存）

---

## Persistence

### Filesystem Layout

```
%LOCALAPPDATA%\Programs\StellaRecord\
├── StellaRecord.exe
├── ...
└── Data/
    ├── archive/                       .tar.zst 圧縮アーカイブ
    │   └── output_log_YYYY-MM-DD_HH-MM-SS.txt.tar.zst
    ├── db/
    │   ├── stellarecord.db            SQLite メイン DB
    │   ├── stellarecord.db-wal        Write-Ahead Log
    │   └── stellarecord.db-shm        共有メモリ
    ├── logs/
    │   └── info-YYYY-MM.log           月次運用ログ
    └── EBWebView/                     WebView2 キャッシュ
```

### Windows Registry

| Key | Value Name | Type | Description |
| --- | ---------- | ---- | ----------- |
| `HKCU\Software\CosmoArtsStore\StellaRecord` | `InstallLocation` | REG_SZ | NSIS が書き込むインストール先 |
| 同上 | `ArchivePath` | REG_SZ | アーカイブストア（空ならデフォルト） |
| 同上 | `DbPath` | REG_SZ | DB ファイル（空ならデフォルト） |
| 同上 | `EnableStartup` | REG_DWORD | 自動起動 ON/OFF |
| 同上 | `StartupPreferenceSet` | REG_DWORD | ユーザーが選択済みかどうか |
| `HKCU\Software\CosmoArtsStore\Polaris` | `CapacityThresholdBytes` | REG_QWORD | アーカイブ警告ライン |
| `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | `StellaRecord` | REG_SZ | 自動起動コマンド |

### LocalStorage

| Key | Value | Description |
| --- | ----- | ----------- |
| `stella-record-theme` | `"light" \| "dark" \| "midnight"` | UI テーマ |

---

## Log Parser Reference

`src-tauri/src/analyze/parser.rs` で定義された 16 個の正規表現を `parse_and_import_reader` の行単位ループ内で順次マッチさせる。

### Detected Events

| Event | Source Pattern | Target Table |
| ----- | -------------- | ------------ |
| Authentication | `User Authenticated: <name> (<usr_id>)` | `sessions` |
| Entering Room | `[Behaviour] Entering Room: <world>` | `visits` (pending) |
| Joining | `[Behaviour] Joining wrld_xxx:N~<access>~region(<r>)` | `visits` |
| OnLeftRoom | `[Behaviour] OnLeftRoom` | `visits` (leave_time) |
| Player Joined | `[Behaviour] OnPlayerJoined <name> (<usr_id>)` | `find_users` + `with_users` |
| Player Left | `[Behaviour] OnPlayerLeft <name> (<usr_id>)` | `with_users` (leave_time) |
| Local Player | `[Behaviour] Initialized PlayerAPI "<name>" is local` | `with_users.is_self=1` |
| Notification | `Received Notification: <Notification ...>` | `notifications` |
| Subscription | `Get VRChat Subscription Details!` | `subscription` |
| Screenshot | `[VRC Camera] Took screenshot to: ...` | `screenshots` |
| OSC Service | `Found new OSC Service: <name> at <ip>:<port>` | `osc` |

### State Variables

行単位ループ内で保持されるステート：

| Variable | Type | Purpose |
| -------- | ---- | ------- |
| `current_ts` | `Option<NaiveDateTime>` | 最後に観測したタイムスタンプ |
| `current_visit_id` | `Option<i64>` | 現在のワールド訪問 ID |
| `pending_room_name` | `Option<String>` | Joining 前に観測したワールド名候補 |
| `my_user_id` | `Option<String>` | 自分の VRChat ID |
| `my_display_name` | `Option<String>` | 自分の表示名 |

### Idempotency

| Table | Idempotency Key | Conflict Resolution |
| ----- | --------------- | ------------------- |
| `sessions` | `log_name UNIQUE` | `INSERT OR IGNORE` |
| `notifications` | `notif_id UNIQUE` | `INSERT OR IGNORE` |
| `with_users` | `UNIQUE(visit_id, vrchat_id)` | `INSERT OR IGNORE` |
| `find_users` | `vrchat_id PRIMARY KEY` | `ON CONFLICT DO UPDATE SET account_name = excluded.account_name` |

---

## Known Limitations

| Limitation | Impact | Mitigation |
| ---------- | ------ | ---------- |
| ログビューアが非 UTF-8 行で打ち切られる | バイト破損行以降が UI に表示されない | 取り込み側 (`continue` パターン) と同じ実装に統一する予定 |
| `read_archive_log_viewer` がパストラバーサルを許容 | 理論上はアーカイブストア外のファイルを読める（CSP で実用的脅威は低） | `output_log_*.tar.zst` 形式の検証を追加予定 |
| LogViewerModal に Esc キー／背景クリックで閉じる機能なし | UX 一貫性 | `useEffect` で `keydown` リスナー追加予定 |
| `apps` 移行時の `INSERT OR IGNORE` がサイレントに重複ドロップ | 旧スキーマで path 重複があった場合のみ | 件数ログ追加予定 |
| 多言語化未対応 | 日本語固定 | i18n リソース外出しが必要 |
| Windows のみ対応 | macOS / Linux で動作不可 | プラットフォーム抽象化は `#[cfg(windows)]` で隔離済み |
| コード署名なし | SmartScreen 警告 | 商用配布時に対応予定 |

過去の調査で問題なしと判定された項目はリポジトリ管理外の内部メモを参照。
