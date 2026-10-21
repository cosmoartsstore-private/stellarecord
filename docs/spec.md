# Specification

操作条件、判断、保存結果と、取消し・部分失敗後の扱いを記述する。module/IPC/eventの正本は[Architecture](architecture.md)、物理形式と確定手順は[Database Schema](database.md)に分ける。

## Document Control

| Field | Content |
| --- | --- |
| Application | StellaRecord |
| Scope | 起動/終了、機能、workflow、成功/部分成功/取消し/失敗、外部効果と現在の制約。内部構造/schema/開発配布の詳細は対象外 |
| Audience | 利用者、仕様を判断する人、機能を変更する人 |
| Authority | `src/app/App.tsx` / `useAppModals.ts`、`src/features/`、`src-tauri/src/commands/` / `analyze/` |
| Related documents | [README](../README.md)、[Architecture](architecture.md)、[Database Schema](database.md)、[Tech Stack](tech-stack.md)、[Operation Preview](manual.html) |
| Source review date | 2026-10-22（sourceとの静的照合日。実行試験日ではない） |

## Table of Contents

- [Scope](#scope)
- [Terms](#terms)
- [Startup and Shutdown](#startup-and-shutdown)
- [Feature Specifications](#feature-specifications)
- [Workflows and State Transitions](#workflows-and-state-transitions)
- [Concurrency and Failure Handling](#concurrency-and-failure-handling)
- [External Effects and Validation](#external-effects-and-validation)
- [Current Constraints](#current-constraints)
- [References](#references)

## Scope

### Overview

Polarisが保存したVRChatログを圧縮保管し、archive本文と解析済み情報を閲覧するWindows向けアプリケーションである。解析情報は本文から再構築できるが、ランチャー登録・設定は同じ復元対象ではない。

### Goals

- 生ログ本文をarchiveへ保持する。
- 過去の訪問・同席ユーザー・通知・撮影等を構造化し、DB previewで確認する。
- 圧縮archiveを別fileへ展開せずストリーム閲覧する。
- 通常処理をローカルで行い、外部API/認証/テレメトリに依存しない。

### Non-Goals

VRChat改造、リアルタイム通信傍受、マルチユーザー/サーバ運用、Windows以外のOS対応は対象外。DB previewは任意SQL/編集、全データbackup、DB cleanup/resetの画面ではない。

## Terms

| 用語                  | 定義                                                        |
| --------------------- | ----------------------------------------------------------- |
| **生ログ**            | VRChat が出力する `output_log_*.txt` ファイル               |
| **アーカイブ**        | 生ログを `tar + zstd` で圧縮した `.tar.zst` ファイル        |
| **アーカイブストア**  | アーカイブを格納するディレクトリ (`Data/archive/`)          |
| **メイン DB**         | 解析済みデータを格納する SQLite (`Data/db/stellarecord.db`) |
| **セッション**        | 1 つのログファイル＝1 セッション                            |
| **訪問 (visit)**      | ワールドインスタンスへの 1 回の入退室                       |
| **取り込み (import)** | アーカイブを解析して DB に書き込む処理                      |
| **Polaris**           | 姉妹アプリ。生ログのバックアップ／同期を担当                |

## Startup and Shutdown

初回は実効archive/DB保存先を表示し、起動時取込を確認する。許可/不許可の保存成功で回答済みとなり、許可した場合だけ今回の取込を開始する。未回答の既定は許可なし。取得/保存失敗はerrorを表示し、許可保存に失敗したまま開始しない。

回答済みかつ許可済みなら起動ごとに1回同期・差分解析する。設定画面での許可変更は次回起動から適用する。Windowsサインイン時にStellaRecordを起動する設定とは独立している。

通常window closeはTauri runtimeへ委ねる。終了時の自動cancel/worker完了待ちを行う独自処理はない。完了してから終了する場合は進捗/状態で確認する。途中jobを再開するcheckpointはなく、再実行時は本文指紋でskip/再構築を判断する。

## Feature Specifications

### Launcher (Registry)

#### ランチャー一覧・再読込・list/card

| Item | Contract |
| --- | --- |
| Purpose and Entry | ランチャー一覧・再読込・list/card |
| Preconditions and Input | `apps` を実効DBから取得 |
| Decision and Processing | 名前順、icon BLOB→Base64。遅い旧取得は世代で破棄 |
| Success and Output | 一覧・表示モードはメモリ。取得失敗では空catalogになり得る |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | reloadで再取得。catalog読込みではDB/schemaを作成しない |
| Source and Constraints | `src-tauri/src/config.rs:179`, `:192`; `src/features/registry/viewmodels/useRegistryState.ts:10` |

#### EXE参照・表示名補完

| Item | Contract |
| --- | --- |
| Purpose and Entry | EXE参照・表示名補完 |
| Preconditions and Input | native picker。選択取消しはNone |
| Decision and Processing | FileDescription→ProductName→stem。手入力・再選択を古い応答で戻さない |
| Success and Output | 登録前のpath/name/descriptionはmodal state |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | picker失敗はUIで無通知、表示名取得失敗はstemへ。modal取消しは未保存 |
| Source and Constraints | `src-tauri/src/commands/registry.rs:22`, `:31`; `src/features/registry/views/RegisterAppModal.tsx:25` |

#### アプリ登録

| Item | Contract |
| --- | --- |
| Purpose and Entry | アプリ登録 |
| Preconditions and Input | pathのis_file、trim後name必須、description任意 |
| Decision and Processing | icon取得は任意。DB/schema初期化後INSERT、path UNIQUE |
| Success and Output | `apps` にname/description/path/icon/registered_at |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | 重複pathは専用エラー、失敗時modalを維持。同名・別pathは許容 |
| Source and Constraints | `src-tauri/src/commands/registry.rs:45`, `:67`; `src/app/App.tsx:97` |

#### 外部EXE起動・フォルダ表示

| Item | Contract |
| --- | --- |
| Purpose and Entry | 外部EXE起動・フォルダ表示 |
| Preconditions and Input | 登録pathを明示操作 |
| Decision and Processing | `Command::new(path).spawn()`、Windows CREATE_NO_WINDOW。フォルダはopener |
| Success and Output | 対象アプリの起動・OSフォルダ表示。Stellaの登録は変更しない |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | spawn成功を起動成功と通知。子アプリ終了・健康状態は追跡しない。再操作可 |
| Source and Constraints | `src-tauri/src/platform.rs:83`; `src-tauri/src/commands/archive.rs:966`; `src/app/App.tsx:75` |

#### 登録解除

| Item | Contract |
| --- | --- |
| Purpose and Entry | 登録解除 |
| Preconditions and Input | path一致の登録 |
| Decision and Processing | DELETE apps、0件は該当なしエラー、再読込 |
| Success and Output | 登録だけ削除。対象EXE・対象アプリのデータは削除しない |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | UI操作から直ちに実行。解除後の取消し・復元APIはない。再登録可 |
| Source and Constraints | `src-tauri/src/commands/registry.rs:83`, `:92`; `src/app/App.tsx:110` |

### Analyze

#### 起動時取込の初回確認・設定

| Item | Contract |
| --- | --- |
| Purpose and Entry | 起動時取込の初回確認・設定 |
| Preconditions and Input | 実効archive/DB保存先を取得して表示 |
| Decision and Processing | 未選択なら確認。許可の保存成功後だけ今回実行。既回答かつ許可済みなら起動ごと1回 |
| Success and Output | StartupImportEnabled→StartupImportPreferenceSet。設定画面変更は次回起動から |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | 不許可も回答済み保存。保存失敗は開始しない。通常のWindows自動起動とは別 |
| Source and Constraints | `src/app/App.tsx:142`, `:154`, `:191`; `src-tauri/src/config.rs:141`, `:157` |

#### Polaris原本→archive同期

| Item | Contract |
| --- | --- |
| Purpose and Entry | Polaris原本→archive同期 |
| Preconditions and Input | Polaris InstallLocation/Data/archive、Stellaの実効archive |
| Decision and Processing | 初回圧縮、厳密な追記だけ置換。同サイズ異内容・縮小・非prefixは旧archive保持 |
| Success and Output | 1ログ1entry、zstd level3、tmp生成→finish/sync_all→配置、置換時.bak |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | 同期はcancelを確認しない。複数件の前半成功は残る。エラー時DB解析へ進まない |
| Source and Constraints | `src-tauri/src/commands/mod.rs:65`; `src-tauri/src/commands/archive.rs:178`, `:314`, `:339`, `:367`; `src-tauri/src/commands/import.rs:110` |

#### 起動時差分解析

| Item | Contract |
| --- | --- |
| Purpose and Entry | 起動時差分解析 |
| Preconditions and Input | Backendでも許可2条件を再確認。管理archive全体を走査 |
| Decision and Processing | SHA-256/本文長/parser版一致はskip、変更はsession関連再構築 |
| Success and Output | outer transaction＋file savepoint。通常ファイル失敗だけは成功分を最後にcommit |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | cancelは今回DB全rollback。失敗ファイルは次回再試行対象。archive同期済み分は残る |
| Source and Constraints | `src-tauri/src/commands/import.rs:110`; `src-tauri/src/analyze/mod.rs:442` |

#### 手動バッチ取込（UI「復元」）

| Item | Contract |
| --- | --- |
| Purpose and Entry | 手動バッチ取込（UI「復元」） |
| Preconditions and Input | 管理archive一覧から複数選択。単一名・命名規則・存在を事前検証 |
| Decision and Processing | ソート後worker。内容同一ならskip、変更は再構築。外部pickerはこの入力経路ではない |
| Success and Output | 全選択成功時だけ今回DB更新をcommit |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | 1ファイル失敗・cancelの双方で全選択rollback。選択modal取消しは開始しない。再選択再実行 |
| Source and Constraints | `src/app/useAppModals.ts:53`; `src-tauri/src/commands/import.rs:68`; `src-tauri/src/analyze/mod.rs:855` |

#### 取込進捗・停止

| Item | Contract |
| --- | --- |
| Purpose and Entry | 取込進捗・停止 |
| Preconditions and Input | Backend running compare_exchangeで同時worker1本 |
| Decision and Processing | analyze-progress/analyze-finishedを購読。停止はAtomicBoolへの要求 |
| Success and Output | 進捗・running状態はメモリ。終了時guardがrunning解除 |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | 停止要求受理≠停止完了。解析は行/指紋chunk/commit前に確認、停止時間上限なし |
| Source and Constraints | `src-tauri/src/commands/import.rs:23`, `:52`, `:167`; `src-tauri/src/analyze/mod.rs:209`, `:538`; `src/features/analyze/viewmodels/useAnalyzeState.ts:76` |

#### 容量メーター・警告ライン

| Item | Contract |
| --- | --- |
| Purpose and Entry | 容量メーター・警告ライン |
| Preconditions and Input | 実効archive tree全体、Polaris閾値 |
| Decision and Processing | 30秒・手動・解析終了時に再計算、表示率を100%までclamp |
| Success and Output | 閾値のみregistry。容量値は表示state |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | 警告は容量制限や自動削除ではない。取得失敗は直前値保持。旧応答は世代で破棄 |
| Source and Constraints | `src-tauri/src/commands/archive.rs:280`, `:952`; `src/features/analyze/viewmodels/useAnalyzeState.ts:23`; `src/features/analyze/views/AnalyzeSection.tsx:30` |

#### Polaris元ログ削除

| Item | Contract |
| --- | --- |
| Purpose and Entry | Polaris元ログ削除 |
| Preconditions and Input | 名前・size・本文がarchiveと完全一致した候補だけ。初期は全候補選択 |
| Decision and Processing | modalで選択・確定後、削除直前にも一致を再確認して逐次remove_file |
| Success and Output | Polaris原本だけを削除。archive/DBは保持 |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | modal取消し可。実行後undoなし。途中失敗では既削除分は戻らず、次回一覧から再確認 |
| Source and Constraints | `src-tauri/src/commands/archive.rs:227`, `:975`, `:1070`; `src/features/analyze/views/PolarisCleanupModal.tsx:17`; `src/app/useAppModals.ts:123` |

#### 管理ログ閲覧・外部ログ選択

| Item | Contract |
| --- | --- |
| Purpose and Entry | 管理ログ閲覧・外部ログ選択 |
| Preconditions and Input | 管理はoutput_log_*.txt.tar.zst。外部はoutput_log_*＋.txt/.tar.zst、is_file |
| Decision and Processing | 圧縮は先頭entry、plainは直接read。カテゴリは本文分類、強調はDB markerだけ |
| Success and Output | 500行/event、100ms集約、全文配列をメモリ保持して仮想DOM表示 |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | 外部picker取消しは選択維持。file切替/closeはlistener解除と旧世代破棄。Rust読取りworker停止IPCはない |
| Source and Constraints | `src-tauri/src/commands/archive.rs:692`, `:931`, `:1015`, `:1040`; `src/features/archive/viewmodels/useArchiveState.ts:64` |

#### ビューアfilter・zoom・切替

| Item | Contract |
| --- | --- |
| Purpose and Entry | ビューアfilter・zoom・切替 |
| Preconditions and Input | world/notification/player/warning/error/debugチップ |
| Decision and Processing | levelまたはcategory一致で表示行を絞る。Ctrl+wheel 0.5～2.5倍、overscan10 |
| Success and Output | filter/zoomはmodal内state。file切替はscrollだけreset |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | file切替でfilter維持、close/reopenでstate再作成。原文・DBへの書込みなし |
| Source and Constraints | `src/features/archive/views/LogViewerModal.tsx:15`, `:63`, `:92`, `:105` |

### Database Preview

#### DB一覧・ページ・sort・物理名切替

| Item | Contract |
| --- | --- |
| Purpose and Entry | DB一覧・ページ・sort・物理名切替 |
| Preconditions and Input | 実効DB内のTABLE_COMMENTS登録済みobjectのみ |
| Decision and Processing | 9可視table＋3view。内部imported_logs非表示。500行、asc→desc→解除、既定sort |
| Success and Output | NULL/Blobは表示文字列、切替状態はメモリ。任意SQL/編集機能なし |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | load失敗toast。世代で旧応答を破棄し、行取得失敗は前の確定tableへ戻す |
| Source and Constraints | `src-tauri/src/commands/database.rs:155`, `:248`, `:289`, `:342`; `src/features/database/viewmodels/useDatabaseState.ts:18`; `src/features/database/views/DatabaseSection.tsx:23` |

#### DB cleanup/reset/backup

| Item | Contract |
| --- | --- |
| Purpose and Entry | DB cleanup/reset/backup |
| Preconditions and Input | 現行UI・24 IPCに入口なし |
| Decision and Processing | DB削除・VACUUM・全データbackup・restore APIなし |
| Success and Output | archive再解析は解析DBの再構築でありapps/settings復元ではない |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | 外部手動保全の注意はstorage文書へ。存在しない操作手順を共通章の必須機能にしない |
| Source and Constraints | `src-tauri/src/lib.rs:41`; `src/features/database/views/DatabaseSection.tsx:23`; `docs/database.md:628`, `:664` |

### Settings

#### Windowsログイン時自動起動・容量設定

| Item | Contract |
| --- | --- |
| Purpose and Entry | Windowsログイン時自動起動・容量設定 |
| Preconditions and Input | 閾値1～10,485,760 MiB相当の整数、enable bool |
| Decision and Processing | Polaris設定→Run→Stella設定の順。UI toggleは即時保存、閾値は保存操作 |
| Success and Output | Run/Stella EnableStartup・StartupPreferenceSet/Polaris CapacityThresholdBytes |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | 途中失敗で開始前値の復元を試みる。復元失敗を連結通知。全registry書込みの原子性なし |
| Source and Constraints | `src-tauri/src/commands/settings.rs:67`, `:75`, `:117`; `src-tauri/src/platform.rs:97`; `src/features/settings/viewmodels/useSettingsState.ts:103` |

#### theme・設定保存先表示・credit

| Item | Contract |
| --- | --- |
| Purpose and Entry | theme・設定保存先表示・credit |
| Preconditions and Input | light/dark/midnight。保存先表示はStartupImportSettings |
| Decision and Processing | theme変更即反映。archive/DB保存先の変更UIはない。creditから明示URL open |
| Success and Output | themeだけlocalStorage。credit開閉/版表示はメモリ |
| Persistence | 保存先・更新単位・保持/削除は[Database Schema](database.md)を正本とし、上記出力と区別する |
| Cancellation and Failure | theme保存に独自のResult/復元処理なし。credit外部リンクは通常業務のAPI通信と別 |
| Source and Constraints | `src/features/settings/models/theme.ts:1`; `src/features/settings/views/DataLocationList.tsx:1`; `src/app/CreditModal.tsx:39`; `src/app/App.tsx:314` |


### Operation Details

- 設定はglobal navの「設定」からSettingsModalを開く。Analyze内には配置しない。テーマはselectで即時反映、自動起動toggleは即時保存、警告ラインは保存操作、取込許可は次回起動から適用する。保存済み変更をmodal closeで取り消す機能ではない。
- 容量表示は設定警告ラインが10 GiB以上ならGB、それ未満ならMB。90%超で警告色、bar幅を0〜100%へ制限する。警告は自動削除/保存拒否/空き容量予約ではない。
- 手動archive選択は全選択/解除、個別toggle、Shift範囲追加、Ctrl/Meta個別toggle、drag選択/解除、keyboard toggleを扱う。
- viewerのカテゴリ地色は本文分類、キーワード強調はDB markerのみ。DB未取込でも本文カテゴリ/levelは表示し、テキストだけから強調を推測しない。source名からヒントを探し、同名外部本文の指紋一致まで検証する契約ではない。
- viewer file切替はfilter/zoomを保ってscrollを戻し、close/reopenではmodal stateを作り直す。全受信行を配列保持するため仮想DOMを全文メモリ不保持と扱わない。
- DB previewはBLOBを`<BLOB>`、NULLを`NULL`として表示し、table/viewを区別する。読取り専用画面はfilesystem無変更/read-only接続の保証ではない。
- 登録解除はEXE/対象appデータを削除しない。fallback iconはRegistrySection内にある。Creditのversion表示と明示リンクopenはメモリ状態で、通常業務の外部API通信とは別の効果である。

### Log Parser Reference

認証、訪問、ユーザー、通知、撮影、OSC、VRChat+を行単位で解析する。OSC検出保存は接続/送受信ではなく、撮影eventの保存は画像本体の移動/保管ではない。

### Detected Events

以下の11項目を検出する。`Entering Room` は `Joining` で訪問を確定するまでの補助状態として保持する。

| Event          | Source Pattern                                        | Target Table                |
| -------------- | ----------------------------------------------------- | --------------------------- |
| Authentication | `User Authenticated: <name> (<usr_id>)`               | `sessions`                  |
| Entering Room  | `[Behaviour] Entering Room: <world>`                  | `visits` (pending)          |
| Joining        | `[Behaviour] Joining wrld_xxx:N~<access>~region(<r>)` | `visits`                    |
| OnLeftRoom     | `[Behaviour] OnLeftRoom`                              | `visits` (leave_time)       |
| Player Joined  | `[Behaviour] OnPlayerJoined <name> (<usr_id>)`        | `find_users` + `with_users` |
| Player Left    | `[Behaviour] OnPlayerLeft <name> (<usr_id>)`          | `with_users` (leave_time)   |
| Local Player   | `[Behaviour] Initialized PlayerAPI "<name>" is local` | `with_users.is_self=1`      |
| Notification   | `Received Notification: <Notification ...>`           | `notifications`             |
| Subscription   | `Get VRChat Subscription Details!`                    | `subscription`              |
| Screenshot     | `[VRC Camera] Took screenshot to: ...`                | `screenshots`               |
| OSC Service    | `Found new OSC Service: <name> at <ip>:<port>`        | `osc`                       |

#### Friend Status

`friendship.rs` は同一訪問内の表示名・アバター切替・旧形式の `Friend:0` / `Friend:1` と unpack 行を対応付ける。3行以内の download / unpack が1ユーザー・1アバターへ一意に対応する場合だけ `with_users.friend_status` を0または1に更新する。情報不足・複数候補・矛盾・新形式で証拠がない場合は2（不明）を保ち、自分自身も不明とする。外部フレンド一覧 API を参照しない。

時刻のない行は直前時刻を引き継ぎ、EOFでは未退室visit/userを最終観測時刻で閉じる。複数行通知は終端まで集約し、未閉鎖のままEOFに達した通知は保存しない。無効UTF-8 bytesはU+FFFDへ置換して解析し、archiveは元本文bytesを保持する。

## Workflows and State Transitions

### Import Outcomes

開始受付、file進捗、最終確定は別段階である。fileの完了進捗はouter commit前に送るため、その時点で永続化確定とは限らない。

| Mode and result | Database result | Retained effects and retry |
| --- | --- | --- |
| 手動全成功 | 全選択更新をcommit | 元archive保持 |
| 手動の通常file失敗 | 今回の全選択更新をrollback | 既存archiveを再選択し実行 |
| 起動時差分の通常file失敗 | 失敗fileを戻し、成功分を最後にcommit | 失敗fileは次回起動時または手動で再試行 |
| いずれかの取消し | 今回DB全体をrollback | 先行同期archiveは残る。終了確認後に再実行 |
| 同期失敗 | DB解析を開始しない | 先行成功archiveは残る。原本/保存先を確認して再試行 |
| outer DB/commit失敗 | 成功として扱わない | errorを確認し再実行。別資源までrollbackしない |

schema初期化/互換処理はouter transaction開始前で、cancelのrollback対象外。手動経路は既存管理archiveを使いPolaris同期を行わない。同期済み本文・解析DB・apps/設定の「復元」を区別する。[Write Transactions and Invariants](database.md#write-transactions-and-invariants)に物理確定手順を記す。

起動時同期の見積り失敗は0件扱い。既存archiveがあれば同期を試みるが、原本directory不在等で同期失敗すると既存archiveの差分解析にも進まない。

### Cancellation

「停止」は要求受付でありworker完了ではない。終了event/非実行状態まで新しい取込を開始しない。圧縮同期/I/O待ちを即時中断せず、停止時間上限はない。

元ログ削除は確認modalまで取消し可能で、削除開始後の一括rollback/undoはない。viewer closeは受信終了/旧応答破棄で、取込cancelとは別である。

## Concurrency and Failure Handling

- Backendは同一processの取込workerを同時1本にする。全DB writer、原本削除、viewerの全操作排他ではない。
- 容量/catalog/DB page/設定draft/EXE名候補/viewerの旧応答は所有者ごとの境界で除外する。[Stale Responses](architecture.md#stale-responses)を参照する。
- すべてのDB競合/fileアクセス/event失敗を自動再試行で回復する契約はない。通知と実効保存先を確認し、対象操作をやり直す。
- viewer doneは全文成功、file進捗はcommit、spawn受付は子app健康状態を意味しない。worker内open/decode/entry/read失敗はlog/done経路を持ち、返却済みmetadataのrejectionにはしない。chunk送信失敗ではdoneに到達しない場合がある。

## External Effects and Validation

| Boundary | Current rule and effect |
| --- | --- |
| 管理archive | 単一filename、output_log_*.txt.tar.zst。path traversalを許可しない |
| 外部viewer | 明示選択した既存file、output_log_*と.txt/.tar.zstを検証。取込入力とは別経路 |
| DB table/column | ASCII英数字/_と表示metadata allowlistを検証。page500行、sortのasc以外はDESC |
| app登録 | 既存file、trim後非空name、path UNIQUE。起動/folderは明示操作 |
| 警告ライン | 1〜10,485,760 MB整数、1 MB=1024² bytes。Polaris共有値も変更 |
| registry | 管理設定は順次保存と補償的復元でatomicではない。[Settings Writes](database.md#settings-writes) |
| 通信/権限 | 通常API/認証/テレメトリなし。明示URL/EXE/folderを開く。installerのWebView2導入とpreviewのGoogle Fontsは別 |

## Current Constraints

### Known Limitations

| Limitation | Impact | Current handling |
| --- | --- | --- |
| VRChat形式依存 | 未対応/変更形式は解析情報が欠け得る | 本文と解析を区別し、parser版更新後に再構築 |
| 生instance_typeラベル | hidden等の内部名を保存/表示 | 人向けmappingはconsumer責務 |
| 非UTF-8置換 | 該当fieldを完全復元できない | 元bytesはarchiveに保持 |
| Windowsのみ/日本語UI | 他OS/多言語を提供しない | 対応範囲拡張の保証なし |
| viewer途中失敗/大容量 | 部分行表示、全行メモリ保持 | doneを全文成功とせずlog確認/再取得 |
| 設定保存/復元失敗 | 一部値だけ更新され得る | 連結errorを確認し再取得 |
| 非暗号化DB | local filesystem権限に依存 | SQLCipherなし、backup先も利用者管理 |
| 署名/Updaterなし | Windows警告/手動更新 | [Current Distribution Constraints](tech-stack.md#current-distribution-constraints) |
| 復元対象の差 | archiveだけではapps/設定/themeを戻せない | [Backup and Restore](database.md#backup-and-restore)の範囲を分けて保全 |

## References

- [Architecture](architecture.md): entry/module/状態所有、24 IPC・4 event、worker/終了。
- [Database Schema](database.md): 全schema、保存先、identity、transaction、保持/削除/保全。
- [Tech Stack](tech-stack.md): 依存、command、CI、導入/更新/削除。
- [Operation Preview](manual.html): 非接続sample、全機能/全状態の再現ではない。
