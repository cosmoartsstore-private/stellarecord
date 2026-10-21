# Architecture

実行入口、module責務、状態所有、依存、IPC/event、workerの寿命を記述する。利用者の結果は[Specification](spec.md)、物理保存は[Database Schema](database.md)を正本とする。

## Document Control

| Field | Content |
| --- | --- |
| Application | StellaRecord |
| Scope | process、entry、module責務、状態所有、依存、主要flow、IPC/event、非同期処理と終了。機能条件/schema詳細は対象外 |
| Audience | 機能を変更・調査する開発者 |
| Authority | `src/main.tsx`、`src/app/`、`src/features/`、`src/shared/`、`src-tauri/src/main.rs` / `lib.rs` / `commands/` / `analyze/` / `platform.rs` |
| Related documents | [README](../README.md)、[Specification](spec.md)、[Database Schema](database.md)、[Tech Stack](tech-stack.md)、[Operation Preview](manual.html) |
| Source review date | 2026-10-22（sourceとの静的照合日。実行試験日ではない） |

## Table of Contents

- [Process Model and Entry Points](#process-model-and-entry-points)
- [Source Responsibility Map](#source-responsibility-map)
- [Composition and State Ownership](#composition-and-state-ownership)
- [Main Data Flows](#main-data-flows)
- [Interfaces and Events](#interfaces-and-events)
- [Concurrency and Lifecycle](#concurrency-and-lifecycle)
- [External Boundaries](#external-boundaries)
- [References](#references)

## Process Model and Entry Points

```text
OS / 利用者 → src-tauri/src/main.rs（panic hook・session内Mutex）
 → lib::run → Tauri Builder（shell plugin、cancel/running state、24 handlers）
 → main window → index.html → src/main.tsx StrictMode → app/App.tsx
      React hook/service ⇄ invoke/event ⇄ Rust command
                                  ├─ import worker → analyze → SQLite
                                  ├─ viewer worker → txt / tar+zstd
                                  └─ registry / Win32 / filesystem
 外部資源: Polaris元ログ、Stella archive / DB / 設定、登録EXE
```

Tauri setupでmain windowを生成し、InstallLocation取得時はData/EBWebViewをprofileに指定する。Appが全feature hookを合成する。WebView2内部のOS process数や各IPC handlerのdispatch threadをアプリsourceから固定しない。

| Execution owner | Entry | Owned task / lifetime |
| --- | --- | --- |
| Tauri runtime | main.rs → lib.rs::run | window、plugin、IPC dispatch。process寿命 |
| React root | index.html → main.tsx → App.tsx | shellとfeature合成。mount寿命 |
| Import worker | commands/import.rsの2 start commands | 1取込ごとにthread生成。cancel Arcとrunning guardをmove |
| Viewer worker | commands/archive.rsの圧縮/plain stream起動 | fileごとにthread生成。AppHandle、局所chunk。frontend close用cancelなし |

## Source Responsibility Map

Frontendのapp/features/sharedはsrc配下、Rustのcommands/analyze等はsrc-tauri/src配下を示す。表は群単位の所有であり、全private helperの一覧ではない。

| Module | Responsibility | Entry | Owned state | Main dependencies |
| --- | --- | --- | --- | --- |
| `src/main.tsx` | React mount | `src/main.tsx` | root DOM | fonts、StrictMode→App |
| `app/App.tsx` | shell・3section・feature合成・初回取込判断 | `src/app/App.tsx` | theme/section/list-card/modal、初回判定ref | 各hook/view/service、theme、toast |
| `app/useAppModals.ts` | 選択/取込/viewer/cleanupの手順 | `src/app/useAppModals.ts` | selector/viewer/cleanup開閉・削除候補 | archive hook操作を注入、analyzeService |
| `app/CreditModal.tsx`, `ToastContainer.tsx` | credit・version・toast表示 | `src/app/CreditModal.tsx` | credit開閉/versionはview内 | Tauri app API/shell.open、shared |
| `features/analyze/models` | 進捗・容量・削除候補型 | `features/analyze/models` | 純粋型 | IPC契約 |
| `features/analyze/viewmodels` | 進捗購読・容量取得・停止要求 | `src/features/analyze/viewmodels/useAnalyzeState.ts` | running/status/progress/storage、request世代 | analyzeService |
| `features/analyze/services` | 容量・停止・cleanup IPC/進捗event | `src/features/analyze/services/analyzeService.ts` | stateなし | Rust archive/import |
| `features/analyze/views` | AnalyzeSection、PolarisCleanupModal | `features/analyze/views` | 削除候補の選択など表示局所state。業務実行はcallback | props/shared |
| `features/archive/models` | payload・日付・本文強調 | `src/features/archive/models/logFormat.tsx` | stateなし | ReactNodeを含む描画純粋関数 |
| `features/archive/viewmodels` | 一覧・外部選択・viewer受信・範囲選択 | `src/features/archive/viewmodels/useArchiveState.ts` | lines/loaded/files、stream世代/listener/timer、selection/drag | archiveService、Tauri eventを直接購読 |
| `features/archive/services` | 管理/外部viewer・batch/startup IPC | `src/features/archive/services/archiveService.ts` | stateなし | Rust archive/import |
| `features/archive/views` | ArchiveSelectorModal、LogViewerModal | `src/features/archive/views/LogViewerModal.tsx` | filter/zoom/scrollはview所有 | format/shared/TanStack Virtual |
| `features/database/models/services` | 表型・空値とcatalog/page IPC | `src/features/database/services/databaseService.ts` | stateなし | Rust database |
| `features/database/viewmodels/views` | catalog/page/sort、DatabaseSection | `src/features/database/viewmodels/useDatabaseState.ts` | table/page/sort/loading/応答世代はhook、表示局所stateはview | databaseService/shared |
| `features/registry/models/services` | app型・起動/登録/名称取得/一覧 IPC | `src/features/registry/services/registryService.ts` | stateなし | Rust registry/settings/archive |
| `features/registry/viewmodels/views` | launcher一覧・RegisterAppModal | `src/features/registry/viewmodels/useRegistryState.ts` | catalog/load世代はhook、入力/名称候補世代はmodal | registryService/shared |
| `features/settings/models` | 設定型・theme読書き | `src/features/settings/models/theme.ts` | theme永続化のみ | localStorage |
| `features/settings/viewmodels` | 管理設定下書き・初回許可 | `src/features/settings/viewmodels/useSettingsState.ts` | persisted ref/load promise/edit世代、許可/loading/saving | settingsService |
| `features/settings/services/views` | 4設定IPC・SettingsControls/Modal/Consent/DataLocationList | `src/features/settings/services/settingsService.ts` | serviceなし、viewはprops主体 | Rust settings、shared |
| `shared/components`, `shared/hooks` | Icons・dialog・toast | `src/shared/hooks/useModalDialog.ts` | dialog ref、toast queue | React/browser |
| `shared/lib`, `shared/models`, `shared/styles` | byte/storage書式・error・共通型/装飾 | `src/shared/lib/errors.ts` | errorの失敗は再帰通知しない | errorsだけlog_client_error IPC |
| `App*.css`, `assets/`, `vite-env.d.ts` | global theme素材・型 | `App*.css`, `assets/`, `vite-env.d.ts` | runtime業務stateなし | App/main |
| Rust `main.rs` / `lib.rs` | panic hook・単一起動・Builder/IPC/window | `src-tauri/src/main.rs` | managed AnalyzeCancelStatus | platform、utils、Tauri shell plugin |
| `commands/mod.rs` | 共通path/DB初期化/進捗/error IPC | `src-tauri/src/commands/mod.rs` | global cacheなし、接続は呼出し単位 | config/analyze/utils |
| `commands/import.rs` | 取込開始/停止・thread生成 | `src-tauri/src/commands/import.rs` | running guard/cancel Arcをworkerへmove | archive同期、analyze |
| `commands/archive.rs` | 圧縮同期・原本照合削除・viewer・容量・folder | `src-tauri/src/commands/archive.rs` | worker局所chunk/category state | filesystem/tar/zstd/DB/platform/utils |
| `commands/database.rs` | catalog/column allowlist・page/sort query | `src-tauri/src/commands/database.rs` | 呼出し単位接続、500行 | SQLite、static表示metadata |
| `commands/registry.rs` | 登録/解除・EXE選択/名称/起動 | `src-tauri/src/commands/registry.rs` | apps永続値 | platform、初期化済main DB |
| `commands/settings.rs` | 許可・保存先・管理設定・catalog | `src-tauri/src/commands/settings.rs` | registry/Run key、全体atomic transactionなし | config/platform、補償的復元 |
| `analyze/mod.rs` | 指紋/差分/batch/行parser・DB import | `src-tauri/src/analyze/mod.rs` | 接続/transaction/savepointと1ログparser state | db/parser/friendship、tar/zstd/hash |
| `analyze/parser.rs` | 固定regex・location/notification純粋処理 | `src-tauri/src/analyze/parser.rs` | 共通immutable pattern | LazyLock/regex |
| `analyze/friendship.rs` | 訪問内の証拠対応・友人状態推定 | `src-tauri/src/analyze/friendship.rs` | FriendshipTracker局所state | parserイベントをanalyzeから受ける |
| `analyze/db.rs` | schema/view/index・現行互換初期化 | `src-tauri/src/analyze/db.rs` | durable schema、接続設定 | SQLite |
| `config.rs` | Stella/Polaris registry設定とapps catalog | `src-tauri/src/config.rs` | durable registry/DB、プロセスglobal cacheなし | winreg/utils/SQLite |
| `platform.rs` | mutex/process/Run/icon/version/picker | `src-tauri/src/platform.rs` | process寿命mutex handle | Win32/window shell、registry |
| `utils.rs`, `models.rs` | install解決/log/error/event・IPC payload | `src-tauri/src/utils.rs` | 月次log、Serialize payload | registry/filesystem/Tauri |
| `build.rs`, `tauri.conf.json`, `capabilities/`, `windows/` | build/window権限・NSIS設置/削除 | `build.rs`, `tauri.conf.json`, `capabilities/`, `windows/` | install registryとdata保持契約。通常runtimeとは別 | Tauri生成・Windows |

### Assets, Tooling, Generated Files and Tests

| Group | Role and boundary |
| --- | --- |
| src/assets、src-tauri/icons、font packages | 製品素材。業務stateを所有しない |
| src-tauri/build.rs、tauri.conf.json、capabilities、windows | Tauri生成/権限/NSIS導入・更新・削除。runtimeとは別 |
| package/Cargo manifest/lock、toolchain、Vite/TypeScript/lint/format設定 | 開発/build/検証定義。詳細はtech-stack |
| .github/workflows/ci.yml | CI定義。通常runtimeから呼ばない |
| *.test.ts(x)、Rust *_tests.rs、src-tauri/tests/fixtures | unit/native連携検証。実行結果は公開仕様の正本ではない |
| src/app/native-ipc、src/test/native-ipc | browser/startup検証entryと補助。通常index.html→main.tsx→Appには接続しない |
| node_modules、dist、target、coverage、src-tauri/gen | 導入/生成物。production moduleと区別 |
| README/docs/AGENT | 公開説明とagent index。内部記録はworkspaceの.claude配下 |

## Composition and State Ownership

主依存はapp→feature→shared。ESLintはfeature間、feature→app、shared→feature/appを制限し、Appが機能を合成する。RegisterAppModalのservice直接利用、archive hookのevent直接購読、Creditのplugin APIがあり、一律のview→viewmodel→service一本道ではない。

Rustはimport→archive/analyze、registry→platform/DB、settings→config/platform。commandsはvalidation/委譲に加え、I/O・worker生成・保存手順も所有する。全状態を一つのglobal storeへまとめていない。

| State scope | Owner | Persistence / release |
| --- | --- | --- |
| shell・modal合成 | App/useAppModals | themeのみlocalStorage、他はメモリ |
| 容量・進捗 | useAnalyzeState | 30秒poll/event、cleanupで解除 |
| viewer/session | useArchiveState、LogViewerModal | 全受信行の配列、modal内filter/zoom、closeで購読/timer破棄 |
| DB table/page/sort | useDatabaseState | メモリ、旧request世代を除外 |
| 設定 | settings hooks/theme model | registry/localStorage。draftと保存値を区別 |
| 取込 | AnalyzeCancelStatus/worker guard/analyze | flagはメモリ、更新確定はSQLite |

### Parser State Variables

行単位ループ内で保持されるステート：

| Variable            | Type                    | Purpose                            |
| ------------------- | ----------------------- | ---------------------------------- |
| `current_ts`        | `Option<NaiveDateTime>` | 最後に観測したタイムスタンプ       |
| `current_visit_id`  | `Option<i64>`           | 現在のワールド訪問 ID              |
| `pending_room_name` | `Option<String>`        | Joining 前に観測したワールド名候補 |
| `my_user_id`        | `Option<String>`        | 自分の VRChat ID                   |
| `my_display_name`   | `Option<String>`        | 自分の表示名                       |

ほかに複数行通知の集約とFriendshipTrackerを1ログ内に持つ。保存意味は[Log Parser Reference](spec.md#log-parser-reference)を参照する。

## Main Data Flows

### Startup Import

```text
App: 設定/catalog取得 + progress購読 + 容量poll
 → get_startup_import_settings → 未回答なら保存先を表示して許可確認
 → 既回答/許可済みなら起動ごと1回、初回許可保存成功なら今回1回
 → archiveService → launch_startup_archive_import
 → Backendでもpreference_set && enabled再確認
 → begin_analyze_run → workerへcancel Arc/running guardをmove
 → Polaris原本同期 → run_diff_import
 → DB初期化 → outer transaction → 各file savepoint内で指紋/解析
 → commit/rollback → progress/finished送信 → guard Dropでrunning解除
```

見積り失敗は未処理件数0として扱う。未処理原本も既存archiveもなければfinishedを送って終了する。既存archiveがあっても原本同期失敗時は差分解析へ進まない。DB確定条件は[Import Commit Boundaries](database.md#import-commit-boundaries)へ分ける。

### Manual Import (Restore)

AnalyzeSection→useAppModals→一覧/選択→UI running表示→useArchiveState→service→launch_enhanced_import。Backendで選択名/存在検証・sort後、同じrunning guardを取得してworkerを生成する。手動は既存管理archiveを使い、Polaris同期を先行実行しない。

### Log Viewer Streaming

```text
useAppModals → useArchiveState.openStreamForFile
 → stopStream（世代増分、旧購読/timer破棄）→ 空data
 → session_id生成 → chunk/done購読をawait、各await後に世代確認
 → archiveService → read_archive_log_viewer / read_external_log_viewer
 → path/存在/圧縮entry名検証、source_nameでDBヒント収集
 → reader worker生成 + LogViewerMeta返却
 → txt / tar+zstd → 本文分類/DB marker強調 → 最大500行chunk
 → session/世代一致のみ受信 → 100ms集約 → 全行配列 → filter/virtual DOM
 → EOF/本文read失敗 → 残chunk flush → done → loaded/購読解除
```

カテゴリ地色は本文分類、キーワード強調はDB markerのみでテキストfallbackはない。外部ログもsource名でヒントを検索するが、本文指紋がDB取込済み内容と一致するかまで検証しない。

開始前のpath/存在/entry検証失敗はinvoke rejection。worker内open/decode/entry/read失敗は運用log/doneを使い、返却済みmetadataをrejectしない。本文失敗でも取得済み行を表示し得る。chunk送信失敗でworkerがreturnするとdoneまで到達しない場合がある。

### Cancellation and Shutdown

停止操作→useAnalyzeState→cancel_analyze→AtomicBool=true→指紋chunk/行先頭/file間/commit前等の確認→DB rollback→終了event送信→guard Drop。要求受付とworker完了は別で、同期処理にcancel確認はない。

viewer close/切替はfrontendの受信終了と旧応答無効化であり、Rust readerへ取消しを送らない。通常window closeはTauriへ委ね、独自CloseRequested/ExitRequested、worker join、終了時cancel送信はない。React cleanupは購読/timer/mouseupを解除するが、OS終了時に必ずunmountしworker完了を待つ保証ではない。

## Interfaces and Events

以下は24 custom commandsと4 events。CreditのgetVersion/shell.open等のplugin APIを別に扱う。Returnsは成功payloadであり、Resultのerrorはinvoke rejectionとなる。

### IPC Reference

`src-tauri/src/lib.rs` の `tauri::generate_handler!` で登録された 24 コマンド。

#### archive

| Command                     | Args                                    | Returns                 | Description                                             |
| --------------------------- | --------------------------------------- | ----------------------- | ------------------------------------------------------- |
| `list_archive_files`        | -                                       | `Vec<ArchiveFileItem>`  | アーカイブストア配下の `output_log_*.txt.tar.zst` 一覧  |
| `read_archive_log_viewer`   | `file_name: String, session_id: String` | `LogViewerMeta`         | アーカイブをストリーム再生                              |
| `read_external_log_viewer`  | `file_path: String, session_id: String` | `LogViewerMeta`         | 外部ファイルをストリーム再生（`output_log_*` 検証あり） |
| `pick_log_files`            | -                                       | `Vec<String>`           | ネイティブダイアログでログ選択                          |
| `get_storage_status`        | -                                       | `(u64, u64)`            | (現在容量, 警告ライン) のタプル                               |
| `open_folder`               | `path: &str`                            | `()`                    | OS シェルでフォルダを開く                               |
| `get_deletable_source_logs` | -                                       | `Vec<DeletableLogInfo>` | アーカイブ内容と一致した生ログ一覧                      |
| `delete_source_logs`        | `file_names: Vec<String>`               | `usize`                 | 削除した件数                                            |

#### database

| Command             | Args                                                                                         | Returns               | Description                          |
| ------------------- | -------------------------------------------------------------------------------------------- | --------------------- | ------------------------------------ |
| `get_db_tables`     | -                                                                                            | `Vec<DbTableSummary>` | 表示可能テーブル／ビュー一覧         |
| `get_db_table_data` | `table_name: &str, page: Option<u32>, sort_column: Option<String>, sort_dir: Option<String>` | `TableData`           | 1 ページ分のテーブルデータ（500 行） |

#### import

| Command                         | Args                      | Returns  | Description                                  |
| ------------------------------- | ------------------------- | -------- | -------------------------------------------- |
| `launch_enhanced_import`        | `file_names: Vec<String>` | `String` | 選択アーカイブの取り込み開始                 |
| `launch_startup_archive_import` | -                         | `()`     | 起動時の差分取り込み（進捗はイベント経由）   |
| `cancel_analyze`                | -                         | `()`     | 取消しフラグを設定。停止完了はイベントで通知 |

#### registry

| Command                    | Args                                              | Returns          | Description               |
| -------------------------- | ------------------------------------------------- | ---------------- | ------------------------- |
| `pick_exe_file`            | -                                                 | `Option<String>` | EXE 選択ダイアログ        |
| `extract_exe_display_name` | `path: String`                                    | `String`         | EXE の VersionInfo 表示名 |
| `register_app`             | `path: String, name: String, description: String` | `()`             | ランチャーに追加          |
| `unregister_app`           | `path: String`                                    | `()`             | 登録解除                  |
| `launch_external_app`      | `app_path: &str`                                  | `()`             | 外部 EXE 起動             |

#### settings

| Command                          | Args                                           | Returns                 | Description                  |
| -------------------------------- | ---------------------------------------------- | ----------------------- | ---------------------------- |
| `get_management_settings`        | -                                              | `ManagementSettings`    | 現在の設定取得               |
| `save_management_settings`       | `startup_enabled: bool, archive_limit_mb: u64` | `()`                    | 設定保存                     |
| `get_startup_import_settings`    | -                                              | `StartupImportSettings` | 起動時取込の許可・実効保存先 |
| `save_startup_import_preference` | `enabled: bool`                                | `()`                    | 起動時取込の許可を保存       |
| `read_registry_catalog`          | -                                              | `RegistryCatalog`       | DB の `apps` を読み込み      |

#### common

| Command            | Args              | Returns | Description                                        |
| ------------------ | ----------------- | ------- | -------------------------------------------------- |
| `log_client_error` | `message: String` | `()`    | フロントエンドで捕捉した詳細エラーを運用ログへ記録 |

#### Payload Types

| Type                                                                                                                                      | Definition Location                  |
| ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| `AnalyzePayload`, `DbTableSummary`, `DbColumnMeta`, `TableData`, `LogViewerMeta`, `LogViewerChunk`, `ArchiveFileItem`, `DeletableLogInfo` | `src-tauri/src/models.rs`            |
| `ManagementSettings`                                                                                                                      | `src-tauri/src/commands/mod.rs`      |
| `StartupImportSettings`                                                                                                                   | `src-tauri/src/commands/settings.rs` |
| `PolarisSetting`, `StellaRecordSetting`, `AppCard`, `RegistryCatalog`                                                                     | `src-tauri/src/config.rs`            |

---

### Event Reference

Tauri の `app.emit()` でバックエンドから送出されるイベント。フロントエンドは `@tauri-apps/api/event` の `listen()` で購読する。

| Event Name         | Payload                                                | Emitter              | Subscriber        |
| ------------------ | ------------------------------------------------------ | -------------------- | ----------------- |
| `analyze-progress` | `{status: String, progress: String, is_running: bool}` | 取り込みワーカー     | `useAnalyzeState` |
| `analyze-finished` | `()`                                                   | 取り込みワーカー     | `useAnalyzeState` |
| `log_viewer_chunk` | `LogViewerChunk`                                       | ログビューアワーカー | `useArchiveState` |
| `log_viewer_done`  | `String` (sessionId)                                   | ログビューアワーカー | `useArchiveState` |

`progress`はdone/totalまたは100%等の文字列を返す。現行AnalyzeSection.parseAnalyzeProgressは/形式だけを率へ換算し、それ以外は0として扱う。状態文とrunning flagも合わせて表示する。

### Callers and Task Lifetimes

| Interface group | Caller | Task ownership / response meaning |
| --- | --- | --- |
| archive一覧/管理viewer/外部picker/外部viewer | archiveService → useArchiveState | 一覧/選択はawait結果、viewerはmetadata後にworker/event継続 |
| 容量/削除候補/削除 | analyzeService → useAnalyzeState/useAppModals | command結果。削除はfile単位確定 |
| open_folder | registryService等 | OS open要求まで。開いたfolderの寿命は追跡しない |
| database 2 commands | databaseService → useDatabaseState | 呼出し単位接続と1page、旧応答を世代除外 |
| import 2 start commands | archiveService → App/useArchiveState | worker開始受付。完了はprogress/finished |
| cancel_analyze | analyzeService → useAnalyzeState | flag設定受付。停止完了ではない |
| registry 5 commands | registryService → App/RegisterAppModal | picker/metadata/DB write/spawnまで。子process終了を追跡しない |
| settings/catalog 5 commands | settingsService/registryService | registry/DBの結果。管理設定は補償的復元 |
| log_client_error | shared/lib/errors.ts | 改行を正規化し先頭4000文字を運用logへ |

LogViewerMetaはsession_id/archive_name/source_name。LogViewerChunkはsession_idとtimestamps/levels/categories/raw_lines/highlightsの列配列。levelsは0 plain/1 info/2 warning/3 error/4 debug、categoriesは0 plain/1 world/2 notification/3 player_join/4 player_ready/5 player_left/6 debug-system。TableDataは表metadata、columns、文字列rows、total_rowsを持つ。完全な定義はPayload Typesのsourceを正本とする。

取込購読はhookのmount寿命でrun IDなし。viewerはsessionごとに購読し、session_id/generationで旧event/metadata/errorを除外する。listenが遅れて解決した場合も解除callbackを登録・実行する。

## Concurrency and Lifecycle

### Write Exclusion

AnalyzeCancelStatus.runningのcompare_exchangeは同一processの取込workerを同時1本にする。UI runningは表示と操作抑止でありBackend guardの代替ではない。apps write、schema初期化、cleanup、viewerを含む全writer/全I/Oの共通lockではない。

SQLiteは用途ごとの接続でWALを使う。未確定取込と確定済み読取りを分離するが、全writer/schema変更を直列化しない。

### Stale Responses

| Owner | Invalidated boundary |
| --- | --- |
| useAnalyzeState | 容量request世代/unmount |
| useRegistryState | catalog load/reload世代 |
| useDatabaseState | table/page/sort request世代、失敗時は確定tableへ戻す |
| useSettingsState | load/persisted ref/draft edit世代、遅い取得で新編集を戻さない |
| useStartupImportSettings | isMountedで旧mount応答を除外 |
| RegisterAppModal | EXE表示名候補世代と手入力/再選択 |
| useArchiveState | stream generation/session、各await/event境界 |

### Cancellation Latency and Process-level Exclusion

取消しは協調確認で停止時間上限はない。I/O待ち・同期・1行処理を即時中断しない。進捗は5000行ごとだが取消しは行先頭にも確認する。running guardはworker終了時に解除する。

Local\StellaRecord_SingleInstanceは同じWindows session内のMutex。既存時exit(0)、作成失敗時warnして続行する。全OS sessionや外部DB writerを排除する仕組みではない。

### Performance Characteristics

| Metric | Value | Meaning |
| --- | --- | --- |
| zstd level | 3 | 圧縮率/所要時間の保証ではない |
| viewer chunk | 500行 | 送信batch。全文メモリ不保持ではない |
| UI flush | 100ms | React更新の集約timer |
| virtual overscan | 10 | DOM化範囲。全受信行は配列保持 |
| DB page | 500行 | LIMIT/OFFSET取得単位 |

## External Boundaries

- WebView→Rustは登録commandとCSP/Capabilitiesが境界。現Capabilitiesはcore:default/shell:default/shell:allow-open。CSPはscript-src self、connect-src self/localhost/ipcを許可する。
- DB table/columnは表示metadata allowlist、値はbind。path/形式等の機能条件は[External Effects and Validation](spec.md#external-effects-and-validation)を参照する。
- Polarisはregistry/file契約で連携し、直接IPC/linked libraryではない。任意EXE起動と姉妹app専用保存契約を区別する。
- 容量測定はStella archive、閾値はPolaris CapacityThresholdBytes。Polaris ArchivePathをload/saveするconfigがあっても原本resolverはInstallLocation/Data/archiveを使う。
- installerの起動中処理はCheckIfAppIsRunningへ委ね、hookから強制終了しない。[Installation Lifecycle](tech-stack.md#installation-lifecycle)を参照する。

## References

- [Specification](spec.md): 利用者の前提、判断、成功/失敗/取消し後の結果。
- [Database Schema](database.md): schema、identity、transaction、保存先と寿命。
- [Tech Stack](tech-stack.md): 依存採用、command、CI、配布。
