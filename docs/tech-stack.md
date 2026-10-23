# Tech Stack and Development

依存の宣言/解決値、採用理由、開発command、CIと配布の契約を記述する。module/flowの正本はarchitecture、保存寿命はdatabaseに分ける。

## Document Control

| Field              | Content                                                                                                                                                                             |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Application        | StellaRecord                                                                                                                                                                        |
| Scope              | 依存/採用理由/toolchain/build/検証定義/packaging/導入・更新・削除。機能条件/module/schemaの詳細は対象外                                                                             |
| Audience           | 開発環境、依存、検証、配布を管理する人                                                                                                                                              |
| Authority          | package.json/package-lock.json、Cargo.toml/Cargo.lock、src-tauri/Cargo.toml、rust-toolchain.toml、src-tauri/tauri.conf.json/windows、.github/workflows/ci.yml、Vite/lint/format設定 |
| Related documents  | [README](../README.md)、[Specification](spec.md)、[Architecture](architecture.md)、[Database Schema](database.md)、[Operation Preview](manual.html)                                 |
| Source review date | 2026-10-23（sourceとの静的照合日。実行試験日ではない）                                                                                                                              |

## Table of Contents

- [Stack Reference](#stack-reference)
- [Runtime Decisions](#runtime-decisions)
- [Build and Packaging Contract](#build-and-packaging-contract)
- [Verification Commands and CI](#verification-commands-and-ci)
- [Installation Lifecycle](#installation-lifecycle)
- [Current Distribution Constraints](#current-distribution-constraints)
- [References](#references)

## Stack Reference

Declared versionはmanifest、Resolved versionはlockfile。導入済み環境の観測値ではない。Rustで同名推移依存が複数ある場合は直接依存の系列を示し、全推移依存はlockfileを正本とする。

### Frontend Runtime

| Layer    | Dependency                      | Declared version | Resolved version | Authority                        | Purpose          |
| -------- | ------------------------------- | ---------------- | ---------------- | -------------------------------- | ---------------- |
| Frontend | `@fontsource-variable/m-plus-1` | `^5.2.9`         | `5.2.9`          | package.json / package-lock.json | 日本語font同梱   |
| Frontend | `@fontsource/jetbrains-mono`    | `^5.2.8`         | `5.2.8`          | package.json / package-lock.json | 等幅font同梱     |
| Frontend | `@tanstack/react-virtual`       | `^3.13.24`       | `3.13.24`        | package.json / package-lock.json | viewer仮想scroll |
| Frontend | `@tauri-apps/plugin-shell`      | `^2.3.5`         | `2.3.5`          | package.json / package-lock.json | 明示URL open     |
| Frontend | `react`                         | `^19.2.0`        | `19.2.4`         | package.json / package-lock.json | 画面/Hook        |
| Frontend | `react-dom`                     | `^19.2.0`        | `19.2.4`         | package.json / package-lock.json | DOM/flushSync    |

### Backend

| Layer   | Dependency           | Declared version | Resolved version | Authority                   | Purpose          |
| ------- | -------------------- | ---------------- | ---------------- | --------------------------- | ---------------- |
| Backend | `tauri`              | `2.10`           | `2.10.3`         | Cargo manifest / Cargo.lock | WebView/IPC      |
| Backend | `tauri-build`        | `2.5`            | `2.5.6`          | Cargo manifest / Cargo.lock | build生成        |
| Backend | `tauri-plugin-shell` | `2.3.5`          | `2.3.5`          | Cargo manifest / Cargo.lock | 明示open         |
| Backend | `rusqlite`           | `0.38`           | `0.38.0`         | Cargo manifest / Cargo.lock | bundled SQLite   |
| Backend | `chrono`             | `0.4`            | `0.4.44`         | Cargo manifest / Cargo.lock | 日時             |
| Backend | `regex`              | `1`              | `1.12.3`         | Cargo manifest / Cargo.lock | parser pattern   |
| Backend | `tar`                | `0.4`            | `0.4.45`         | Cargo manifest / Cargo.lock | 1ログ1entry      |
| Backend | `zstd`               | `0.13`           | `0.13.3`         | Cargo manifest / Cargo.lock | 圧縮             |
| Backend | `sha2`               | `0.10`           | `0.10.9`         | Cargo manifest / Cargo.lock | 本文指紋         |
| Backend | `tempfile`           | `3`              | `3.27.0`         | Cargo manifest / Cargo.lock | 一時archive/検証 |
| Backend | `windows`            | `0.58`           | `0.58.0`         | Cargo manifest / Cargo.lock | Win32 API        |
| Backend | `winreg`             | `0.52`           | `0.52.0`         | Cargo manifest / Cargo.lock | registry         |
| Backend | `opener`             | `0.8`            | `0.8.4`          | Cargo manifest / Cargo.lock | folder open      |
| Backend | `image`              | `0.25`           | `0.25.10`        | Cargo manifest / Cargo.lock | PNG icon         |
| Backend | `base64`             | `0.22`           | `0.22.1`         | Cargo manifest / Cargo.lock | icon payload     |
| Backend | `serde`              | `1.0`            | `1.0.228`        | Cargo manifest / Cargo.lock | serialize        |

rusqliteはbundled、imageはdefault features無効/PNGのみ。tempfileは検証だけでなくarchive生成に使う。Rust editionは2021、release profileはLTO/strip/codegen-units=1（workspace Cargo.toml）。

### Build and Verification Dependencies

| Layer                | Dependency                  | Declared version | Resolved version | Authority                        | Purpose              |
| -------------------- | --------------------------- | ---------------- | ---------------- | -------------------------------- | -------------------- |
| Build / verification | `@eslint/js`                | `^9.39.1`        | `9.39.3`         | package.json / package-lock.json | lint基本規則         |
| Build / verification | `@tauri-apps/api`           | `^2.10.1`        | `2.10.1`         | package.json / package-lock.json | invoke/event/app API |
| Build / verification | `@tauri-apps/cli`           | `^2.10.0`        | `2.10.0`         | package.json / package-lock.json | Tauri開発/配布build  |
| Build / verification | `@testing-library/react`    | `^16.3.2`        | `16.3.2`         | package.json / package-lock.json | UI検証               |
| Build / verification | `@types/node`               | `^24.10.1`       | `24.10.13`       | package.json / package-lock.json | Node型               |
| Build / verification | `@types/react`              | `^19.2.7`        | `19.2.14`        | package.json / package-lock.json | React型              |
| Build / verification | `@types/react-dom`          | `^19.2.3`        | `19.2.3`         | package.json / package-lock.json | React DOM型          |
| Build / verification | `@vitejs/plugin-react`      | `^5.1.1`         | `5.1.4`          | package.json / package-lock.json | React変換            |
| Build / verification | `@vitest/coverage-v8`       | `^4.1.7`         | `4.1.7`          | package.json / package-lock.json | coverage出力         |
| Build / verification | `esbuild`                   | `0.28.1`         | `0.28.1`         | package.json / package-lock.json | native probeのbundle |
| Build / verification | `eslint`                    | `^9.39.1`        | `9.39.3`         | package.json / package-lock.json | JS/TS lint           |
| Build / verification | `eslint-plugin-jsx-a11y`    | `^6.10.2`        | `6.10.2`         | package.json / package-lock.json | JSX規則              |
| Build / verification | `eslint-plugin-react`       | `^7.37.5`        | `7.37.5`         | package.json / package-lock.json | React規則            |
| Build / verification | `eslint-plugin-react-hooks` | `^7.0.1`         | `7.0.1`          | package.json / package-lock.json | Hook規則             |
| Build / verification | `eslint-plugin-unicorn`     | `^61.0.2`        | `61.0.2`         | package.json / package-lock.json | JS/TS規則            |
| Build / verification | `globals`                   | `^16.5.0`        | `16.5.0`         | package.json / package-lock.json | lint用global         |
| Build / verification | `jsdom`                     | `^29.1.1`        | `29.1.1`         | package.json / package-lock.json | DOM検証環境          |
| Build / verification | `prettier`                  | `^3.6.2`         | `3.8.1`          | package.json / package-lock.json | format               |
| Build / verification | `stylelint`                 | `^16.25.0`       | `16.26.1`        | package.json / package-lock.json | CSS lint             |
| Build / verification | `stylelint-config-standard` | `^38.0.0`        | `38.0.0`         | package.json / package-lock.json | CSS規則              |
| Build / verification | `typescript`                | `~5.9.3`         | `5.9.3`          | package.json / package-lock.json | 型確認               |
| Build / verification | `typescript-eslint`         | `^8.48.0`        | `8.56.1`         | package.json / package-lock.json | TS lint              |
| Build / verification | `vite`                      | `^7.3.1`         | `7.3.6`          | package.json / package-lock.json | 開発server/bundle    |
| Build / verification | `vitest`                    | `^4.1.7`         | `4.1.7`          | package.json / package-lock.json | Frontend test        |

### Toolchain and Runtime Requirements

| Layer    | Dependency       | Declared version          | Resolved version                  | Authority                  | Purpose                                    |
| -------- | ---------------- | ------------------------- | --------------------------------- | -------------------------- | ------------------------------------------ |
| Compiler | Rust             | 1.93.0                    | toolchain指定値。導入環境は別確認 | rust-toolchain.toml        | rustfmt/clippy/x86_64-pc-windows-msvc      |
| Platform | Windows / MSVC   | Windows 10/11 x64         | SDK/MSVC版は固定しない            | target設定/Windows API使用 | runtime/Rust build                         |
| WebView  | WebView2 Runtime | installerが導入有無を確認 | installed版はlockfileで固定しない | NSIS WebView2 section      | 画面runtime                                |
| Node     | Vite要求         | ^20.19.0 または >=22.12.0 | installed版はlockfileで固定しない | package-lockのvite engines | Frontend開発/build                         |
| Node     | 開発・CI共通     | 24.x                      | CIはsetup-nodeで24を選択          | .github/workflows/ci.yml   | native probeのTypeScript直接実行を含む検証 |

ViteのNode要件は検証全体の条件ではない。jsdom 29.1.1は`^20.19.0 || ^22.13.0 || >=24.0.0`、Vitest 4.1.7は`^20.0.0 || ^22.0.0 || >=24.0.0`を宣言する。Node22.12/23をViteだけから全検証対応と扱わない。

Rust native browser/startup検証は`node src/test/native-ipc/build-probe.ts`を直接実行するため、開発・CIはNode.js 24を使用する。build-probeがimportするesbuildを直接のdevDependencyとして宣言し、Rust test前にもrepository rootでnpm依存を導入する。TypeScriptの型除去による直接実行は[Node.js公式資料](https://nodejs.org/docs/latest-v24.x/api/typescript.html)を参照する。専用npm scriptはない。

## Runtime Decisions

現在の採用理由と制約を示す。実行主体/状態/flowは[Architecture](architecture.md)、transaction/保持/保全は[Database Schema](database.md)を正本とし、以下の要約から性能/通知成功を保証しない。

### ADR-001 Application Framework: Tauri v2

**Status**: Accepted

**Decision**: React/TypeScriptの画面をWebView2で動かし、Rustからfilesystem・SQLite・registry・Win32 APIへ接続する。

**Rationale**: `invoke` とイベントで画面と処理を分離し、CSP/CapabilitiesでWebViewから使う機能を宣言できる。

**Consequences**: WebView2 Runtimeが必要である。Tauriを使うこと自体は、メモリ使用量、配布サイズ、全OS動作、全エラーの通知を保証しない。

### ADR-002 State Management: React Hooks

**Status**: Accepted

**Decision**: `viewmodels/use*State` で状態を扱い、アプリルートで必要な状態と操作を組み合わせる。Redux / Zustand等は導入しない。

**Rationale**: ランチャー、解析、DBと、そのmodal・設定の責務を機能ごとに保つ。

**Consequences**: Tauriイベントのlistener解除や非同期状態の更新をhook内で扱う。ログビューアのリセットには `flushSync` を使う。

### ADR-003 Virtual Scrolling: @tanstack/react-virtual

**Status**: Accepted

**Decision**: ログビューアで表示範囲付近だけをDOM化し、`overscan: 10` を設定する。

**Rationale**: ログ全行を画面要素に変換する負荷を避け、既存のスタイルと表示倍率を維持する。

**Consequences**: 行高を倍率に合わせて再計測する。表示されるDOM数やfpsはウィンドウ・環境によって変わる。

### ADR-004 Styling: CSS Modules

**Status**: Accepted

**Decision**: CSS Modules、CSS Variables、`light-theme` / `dark-theme` を採用する。ダークは黒基調とする。

**Rationale**: 機能ごとのスタイルを分け、テーマ切替を共通変数へ反映する。

**Consequences**: 動的なinline styleも使用するため、現行CSPの `style-src` は `'unsafe-inline'` を許可する。テーマはlocalStorageに保存する。

### ADR-005 Database: SQLite via rusqlite (bundled)

**Status**: Accepted

**Decision**: bundled SQLite、WAL、外部キーを有効にし、10テーブル、3ビュー、11明示indexを持つ。

**Rationale**: ログの関連データをSQLで扱い、savepointと外側transactionで取込の確定範囲を制御する。

**Consequences**: 差分取込のファイル失敗は成功分を残せるが、手動バッチのファイル失敗と両方式の取消しは今回のDB更新全体を巻き戻す。WALでもあらゆる `SQLITE_BUSY` は防げない。確定境界・保全は [Write Transactions and Invariants](database.md#write-transactions-and-invariants) と [Backup and Restore](database.md#backup-and-restore) を参照する。

### ADR-006 Compression: tar + zstd

**Status**: Accepted

**Decision**: 1ログ1tar entryをzstdレベル3で圧縮し、`output_log_*.txt.tar.zst` として保存する。

**Rationale**: entryに元ファイル名を保持し、ビューアとparserがストリームで読む。

**Consequences**: 開始時点の元ログサイズを一時ファイルへ圧縮し、tar/zstdのfinishとsync_all後に配置する。置換時は既存本文のprefixが生成済み本文に保持されることを再確認して、同一directoryのrenameで置き換える。複数ファイル同期全体を原子的に更新する処理はない。圧縮率と処理時間はログ内容による。

### ADR-007 Log Parser: Hand-written Regex State Machine

**Status**: Accepted

**Decision**: 固定正規表現を `LazyLock` の初回利用時に構築し、行単位の状態で訪問・ユーザー・通知等を保存する。`friendship.rs` が訪問内のフレンド状態推定を扱う。

**Rationale**: 直前のワールド名やユーザー対応など、行をまたぐ情報を保持する。

**Consequences**: VRChatの形式変更にはpatternと状態処理の見直しが必要である。非UTF-8バイトはU+FFFDへ置換する。フレンド証拠が一意に対応しない場合は不明を保存し、parser版を使って必要なログを再解析する。固定patternのコンパイル失敗には明示的なpanicを許可する。

### ADR-008 Rust Lints: Deny panic-equivalent paths

**Status**: Accepted

**Decision**: `unwrap_used` / `expect_used` / `panic` をdenyとし、clippyで確認する。局所的なallowを含め、実際の設定とコードを正本とする。

**Rationale**: 通常の失敗はResultで扱い、処理の確定範囲を明確にする。

**Consequences**: lintだけでは全panicの不存在を保証しない。本文ストリームの途中read失敗は運用ログと取得済み行・doneイベントで処理し、返却済みinvokeのエラーやtoastへは伝播しない。イベント送信とロガー自身の失敗も別の扱いである。

### ADR-009 Module Boundaries: Forbid feature-to-feature imports

**Status**: Accepted

**Decision**: `eslint.config.js` が feature間参照、feature→app、shared→feature/appを制限する。appから必要なfeatureを組み合わせる。

**Rationale**: featureを独立して追跡し、依存方向を保つ。

**Consequences**: 再利用する共通処理はsharedで扱う。単独の処理を、参照規則だけを理由に不要な抽象化へ分割しない。

### ADR-010 Installer: NSIS via Tauri Bundler

**Status**: Accepted

**Decision**: NSISのcurrent-user install、日本語UI、独自template/hookを採用する。Program FilesとWindows directoryを配置先として拒否する。

**Rationale**: registryの配置情報、通常削除時のデータ保持、更新時の分岐を配布設定にまとめる。

**Consequences**: `hooks.nsi` からアプリを強制終了せず、templateの `CheckIfAppIsRunning` へ委ねる。通常削除の固定Data対象と外部overrideは [Deletion and Retention](database.md#deletion-and-retention) を参照する。コード署名とTauri Updaterは現行設定にない。

---

### Rejected Technologies

現行未採用の技術と採用済み構成の関係であり、比較試験/過去の採否議事録ではない。

| Technology               | Reason for Rejection                                      |
| ------------------------ | --------------------------------------------------------- |
| Electron                 | 現行のTauri/WebView2/Rust構成で画面とOS連携を実装している |
| Redux / Zustand / Jotai  | 状態管理はReact Hooksで構成している                       |
| Tailwind CSS / CSS-in-JS | CSS ModulesとCSS Variablesでライト・ダークを実装している  |
| TanStack Query / GraphQL | 画面とRustはTauri invoke・イベントで通信する              |
| ORM / 外部DB server      | ローカルSQLiteと明示SQLでschema・transactionを扱う        |
| MSI / MSIX               | 現行の配布targetはNSISである                              |
| Tauri Updater            | 更新はインストーラーの再実行で扱う                        |
| Sentry / Telemetry       | 通常動作は外部送信を行わず、月次のローカルログを保存する  |
| tracing / log crate      | RustのWARN/ERRORを月次ファイルへappendする                |
| i18next                  | 製品UIは日本語を使用する                                  |

## Build and Packaging Contract

repository rootで実行する。Windows MSVC開発環境、固定Rust toolchain、必要なNode/npmを用意する。READMEのnpm ci --ignore-scriptsとFrontend CIのnpm ciはinstall scripts実行条件が異なる。

| Purpose           | Command                 | Configuration / artifact                                       |
| ----------------- | ----------------------- | -------------------------------------------------------------- |
| lockfile導入      | npm ci --ignore-scripts | package-lock。CI Frontendはnpm ci                              |
| Frontend開発      | npm run dev             | Vite localhost:5173                                            |
| デスクトップ開発  | npm run tauri dev       | beforeDevCommandでFrontend起動、devUrl接続                     |
| Frontend本番build | npm run build           | tsc -b && vite build → dist/                                   |
| 配布build         | npm run tauri build     | beforeBuildCommandでFrontend build、frontendDist=../dist、NSIS |
| 配布成果物        | 上記配布build           | workspaceのtarget/release/bundle/nsis/                         |

Tauri configはproductName StellaRecord、identifier com.cosmoartsstore.stellarecord、version1.0.0、nsis target、resources空、製品iconを指定。Cargoのbin名はStellarecord、package名はstella_record_ui、version1.0.0。package.jsonも1.0.0。

fontと画像/iconを同梱する。previewはGoogle Fontsを読む別資料。windowはconfigの静的windows配列でなくlib.rs setupで生成する。

## Verification Commands and CI

| Purpose                  | Command                          | Configuration                                                | Scope                                                          | CI gate                                          |
| ------------------------ | -------------------------------- | ------------------------------------------------------------ | -------------------------------------------------------------- | ------------------------------------------------ |
| Frontend test            | npm run test                     | Vitest                                                       | TS/TSX test                                                    | CIはcoverage variant                             |
| watch                    | npm run test:watch               | Vitest                                                       | 対話watch                                                      | なし                                             |
| coverage                 | npm run test:coverage            | vite.config.ts                                               | src/**/\*.{ts,tsx}、型宣言/src/test/**除外                     | Frontend必須、数値閾値なし                       |
| Rust test                | npm run test:rust                | cargo test --locked --workspace --all-targets                | unit/native連携                                                | CIはsrc-tauriでcargo test --locked --all-targets |
| Prettier                 | npm run format:check             | Prettier設定                                                 | format                                                         | Frontend必須                                     |
| CSS lint                 | npm run stylelint                | stylelint設定                                                | src/\*_/_.css                                                  | Frontend必須                                     |
| TS/JS lint               | npm run lint                     | eslint.config.js                                             | warnings 0、module参照規則等                                   | Frontend必須                                     |
| Rust format              | npm run fmt:check                | cargo fmt --manifest-path src-tauri/Cargo.toml --all --check | Rust format                                                    | continue-on-error: true、参考check               |
| Rust lint                | npm run clippy                   | --locked --all-targets -- -D warnings                        | workspace lint/局所allow                                       | Rust必須                                         |
| Frontend build/typecheck | npm run build                    | TypeScript/Vite                                              | 型確認とbundle                                                 | Frontend必須                                     |
| local統合                | npm run verify                   | package.json                                                 | format→stylelint→lint→coverage→Frontend build→clippy→Rust test | 別jobのCIと同一ではない                          |
| 配布/実操作              | npm run tauri build / 明示実操作 | NSIS/Windows                                                 | installer/署名/導入/更新/削除                                  | 現CI対象外                                       |

npm run formatとstylelint:fixは書換え用で検査commandではない。verifyにrustfmt/installer生成は含まない。coverage下限は現設定に存在しない。src/app/native-ipcはsrc/test/\*\*除外に含まれない。coverage値/成功件数を全機能確認完了と扱わない。

### Test Layout

Frontendはapp/feature/shared近傍の*.test.ts(x)。Rustは各moduleのcfg(test)から隣接*\_tests.rsをincludeし、commands_tests.rsがnative command/IPC/startup等を含む。tests/fixtures/native_launcher.rsはfixtureで、tests直下の独立targetではない。native-ipc補助は通常製品entryへ接続しない。

### CI Definition and Boundaries

master push/master宛PRで起動、同一refの新pushで実行中jobを取消す。Rustはwindows-latest/src-tauri、Frontendはubuntu-latestで別jobとし、両方でNode.js 24とnpm依存を準備する。

- Rust: checkout→setup-node24→repository rootでnpm ci→rustup show→cargo cache→format（参考）→clippy→cargo test。
- Frontend: checkout→setup-node24→npm ci→Prettier→Stylelint→ESLint→coverage→build。
- NSIS生成/署名/installer実操作、coverage数値閾値、coverage artifact uploadは定義されていない。

Rust jobのrun既定directoryはsrc-tauriだが、npm ciのstepだけはrepository rootを明示する。cargo cacheとnpm cacheは別に扱い、別job間のnode_modules共有を前提にしない。CI定義の準備と実際のリモートjobの成功確認は区別する。

実行日時、成功件数、coverage、exit等は内部の実行記録を正本とし、静的照合日を実行完了と扱わない。

## Installation Lifecycle

| Stage         | Process / placement / registry                                                                                         | Data and failure behavior                                                                        |
| ------------- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| 通常導入      | currentUser/日本語、既定LOCALAPPDATA/CosmoArtsStore/StellaRecord。禁止directory検証、CheckIfAppIsRunning、WebView2確認 | 本体/shortcut/uninstaller/アプリ一覧登録。既存の保存先と設定を保持し、不足するDB参照先だけ初期化 |
| /UPDATE       | 既存本体の通常削除をskip。利用者データと設定を保持                                                                     | InstallLocationは現配置先へ更新。保存済みDB/archive参照先は保持し、有効な自動起動を登録          |
| 再実行UI      | 同版再導入、upgrade/downgradeで既存本体削除/継続を選ぶ                                                                 | 既存uninstallerへ必ず/UPDATEを渡し、データ・設定を保持する                                       |
| 通常uninstall | 実行中確認、本体/同梱file/対象shortcut/アプリ一覧登録/Runを削除                                                        | Data全体、bundle data、Stella/Polaris設定key、外部DbPath/ArchivePathを保持                       |
| 失敗/再試行   | 必須file/registry/shortcut更新・削除失敗で中断                                                                         | 全rollbackではない。必須削除完了前の失敗では再試行用uninstaller/後段登録を保持                   |

hooks.nsiから強制終了せずtemplateの起動中確認へ委ねる。通常app終了時のworker joinとは別。InstallLocationは本体配置先として更新するが、保存済みDbPathは上書きしない。DbPath未設定時は以前のInstallLocationがあればその既定DB、なければ今回の配置先の既定DBを初期値にする。配置先変更時にArchivePathが未設定なら、以前のInstallLocation/Data/archiveを参照先として保存する。既存データを自動で移動しない。

通常uninstallではDB・archive・運用ログ・WebView・bundle dataとStella設定keyを残し、Windowsのアプリ一覧登録とRunだけを解除する。再インストール時はEnableStartupが有効なら現配置先をRunへ登録する。保存物ごとの範囲は[Deletion and Retention](database.md#deletion-and-retention)を参照する。

## Current Distribution Constraints

コード署名とTauri Updaterは現bundle設定にない。NSISの署名placeholderだけで署名済みと扱わない。Windows警告、実配布物署名、WebView2導入、導入/更新/削除の実操作結果は静的設定と別に確認する。

成果物pathは公開済み配布URLやrelease成功の証拠ではない。未確認の将来署名予定を確定方針として記載しない。

## References

- [README](../README.md): 導入と開発開始。
- [Specification](spec.md): 操作条件と結果。
- [Architecture](architecture.md): module/状態/IPC/event/worker。
- [Database Schema](database.md): schema/保存先/確定境界/保持/削除/保全。
