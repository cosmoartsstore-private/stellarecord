# Tech Stack and Architecture Decisions

> StellaRecord で採用した技術の詳細リファレンスと、主要な技術選定の意思決定記録 (ADR: Architecture Decision Record)。

## Table of Contents

- [Tech Stack Reference](#tech-stack-reference)
  - [Frontend](#frontend)
  - [Backend](#backend)
  - [Build and Distribution](#build-and-distribution)
  - [Quality and Tooling](#quality-and-tooling)
- [Architecture Decision Records](#architecture-decision-records)
  - [ADR-001 Application Framework: Tauri v2](#adr-001-application-framework-tauri-v2)
  - [ADR-002 State Management: React Hooks](#adr-002-state-management-react-hooks)
  - [ADR-003 Virtual Scrolling: @tanstack/react-virtual](#adr-003-virtual-scrolling-tanstackreact-virtual)
  - [ADR-004 Styling: CSS Modules](#adr-004-styling-css-modules)
  - [ADR-005 Database: SQLite via rusqlite (bundled)](#adr-005-database-sqlite-via-rusqlite-bundled)
  - [ADR-006 Compression: tar + zstd](#adr-006-compression-tar--zstd)
  - [ADR-007 Log Parser: Hand-written Regex State Machine](#adr-007-log-parser-hand-written-regex-state-machine)
  - [ADR-008 Rust Lints: Deny panic-equivalent paths](#adr-008-rust-lints-deny-panic-equivalent-paths)
  - [ADR-009 Module Boundaries: Forbid feature-to-feature imports](#adr-009-module-boundaries-forbid-feature-to-feature-imports)
  - [ADR-010 Installer: NSIS via Tauri Bundler](#adr-010-installer-nsis-via-tauri-bundler)

---

## Tech Stack Reference

### Frontend

| Layer | Technology | Version | License |
| ----- | ---------- | ------- | ------- |
| Language | [TypeScript](https://www.typescriptlang.org/) | 5.9 | Apache-2.0 |
| UI Framework | [React](https://react.dev/) | 19.2 | MIT |
| Build Tool | [Vite](https://vitejs.dev/) | 7.3 | MIT |
| Vite React Plugin | [@vitejs/plugin-react](https://github.com/vitejs/vite-plugin-react) | 5.1 | MIT |
| Virtual Scroll | [@tanstack/react-virtual](https://tanstack.com/virtual) | 3.13 | MIT |
| Tauri SDK | [@tauri-apps/api](https://tauri.app/) | 2.10 | Apache-2.0 / MIT |
| Tauri Shell Plugin | [@tauri-apps/plugin-shell](https://tauri.app/plugin/shell/) | 2.3.5 | Apache-2.0 / MIT |
| Font (UI) | [@fontsource-variable/m-plus-1](https://fontsource.org/) | 5.2 | OFL |
| Font (Mono) | [@fontsource/jetbrains-mono](https://fontsource.org/) | 5.2 | OFL |
| Styling | CSS Modules (built-in to Vite) | - | - |

### Backend

| Layer | Technology | Version | License |
| ----- | ---------- | ------- | ------- |
| Language | [Rust](https://www.rust-lang.org/) | Edition 2021 | Apache-2.0 / MIT |
| Application Framework | [tauri](https://crates.io/crates/tauri) | 2.2.4 | Apache-2.0 / MIT |
| Tauri Build | [tauri-build](https://crates.io/crates/tauri-build) | 2.0.5 | Apache-2.0 / MIT |
| Tauri Shell Plugin | [tauri-plugin-shell](https://crates.io/crates/tauri-plugin-shell) | 2.3.5 | Apache-2.0 / MIT |
| Tauri FS Plugin | [tauri-plugin-fs](https://crates.io/crates/tauri-plugin-fs) | 2.2.0 | Apache-2.0 / MIT |
| Database | [rusqlite](https://crates.io/crates/rusqlite) (`bundled` feature) | 0.38 | MIT |
| Date/Time | [chrono](https://crates.io/crates/chrono) | 0.4 | Apache-2.0 / MIT |
| Regex | [regex](https://crates.io/crates/regex) | 1.x | Apache-2.0 / MIT |
| Compression | [zstd](https://crates.io/crates/zstd) | 0.13 | MIT |
| Archive | [tar](https://crates.io/crates/tar) | 0.4 | Apache-2.0 / MIT |
| Win32 API | [windows](https://crates.io/crates/windows) | 0.58 | Apache-2.0 / MIT |
| Registry I/O | [winreg](https://crates.io/crates/winreg) | 0.52 | MIT |
| Shell Integration | [opener](https://crates.io/crates/opener) | 0.8 | Apache-2.0 / MIT |
| Image Encoding | [image](https://crates.io/crates/image) (PNG only) | 0.25 | Apache-2.0 / MIT |
| Base64 | [base64](https://crates.io/crates/base64) | 0.22 | Apache-2.0 / MIT |
| Serialization | [serde](https://crates.io/crates/serde) (with `derive`) | 1.0 | Apache-2.0 / MIT |

### Build and Distribution

| Layer | Technology | Configuration |
| ----- | ---------- | ------------- |
| Bundler | Tauri Bundler (NSIS target) | `src-tauri/tauri.conf.json` |
| Installer Script | NSIS | `src-tauri/windows/installer.nsi`, `hooks.nsi` |
| Install Mode | currentUser | `%LOCALAPPDATA%\Programs\StellaRecord` |
| Languages | Japanese | - |
| Release Profile | LTO + strip + 1 codegen-unit | `Cargo.toml [profile.release]` |

### Quality and Tooling

| Layer | Technology | Version |
| ----- | ---------- | ------- |
| TS Linter | [ESLint](https://eslint.org/) (flat config) | 9.39 |
| TS Type-Aware Linter | [typescript-eslint](https://typescript-eslint.io/) | 8.48 |
| React Linter | [eslint-plugin-react](https://github.com/jsx-eslint/eslint-plugin-react) | 7.37 |
| React Hooks Linter | [eslint-plugin-react-hooks](https://www.npmjs.com/package/eslint-plugin-react-hooks) | 7.0 |
| A11y Linter | [eslint-plugin-jsx-a11y](https://www.npmjs.com/package/eslint-plugin-jsx-a11y) | 6.10 |
| Code Quality Linter | [eslint-plugin-unicorn](https://github.com/sindresorhus/eslint-plugin-unicorn) | 61.0 |
| CSS Linter | [Stylelint](https://stylelint.io/) (`stylelint-config-standard`) | 16.25 |
| Formatter | [Prettier](https://prettier.io/) | 3.6 |
| Rust Linter | clippy (workspace lints) | bundled |

---

## Architecture Decision Records

各意思決定は以下のテンプレートで記述する。

```
- Status:    Accepted | Superseded | Deprecated
- Date:      決定日
- Context:   何を解決しようとしているか
- Decision:  何を採用したか
- Rationale: なぜそれを採用したか
- Alternatives Considered: 検討した他の選択肢と却下理由
- Consequences: 採用後に発生する影響（良い面・悪い面）
```

### ADR-001 Application Framework: Tauri v2

- **Status**: Accepted
- **Date**: 2024-12 (StellaRecord プロジェクト発足時)

**Context**

VRChat ユーザー向けの Windows デスクトップアプリケーションを開発する必要があった。要件は以下の通り。

- リッチな UI（テーマ切替、仮想スクロール、モーダル）
- ローカルファイル/レジストリへのアクセス
- 配布バイナリのサイズ最小化
- メモリ・CPU 消費の抑制
- 長期メンテナンス可能なエコシステム

**Decision**

Tauri v2 を採用する。フロントエンドは WebView2 上の React、バックエンドは Rust で実装する。

**Rationale**

- バイナリサイズが Electron 比 1/10 以下（約 12 MB）
- WebView2 が Windows 10 1809 以降に標準同梱されているためランタイム配布不要
- バックエンドが Rust であることで、メモリ安全性・型安全性が言語レベルで保証される
- Tauri Capabilities により、フロントエンドから呼び出せる OS API を明示的に許可する設計
- Tauri v2 で公式に Mobile プラットフォームが対象となり、長期サポートへの安心感

**Alternatives Considered**

| Option | Rejected Reason |
| ------ | --------------- |
| Electron | バイナリ約 150 MB、メモリ約 200 MB、Node.js ランタイム同梱必須 |
| WPF (.NET) | TypeScript / React のフロントエンドエコシステムを活用できない |
| Flutter Desktop | Win32 API 連携が Platform Channel 経由で煩雑、Dart の人材確保 |
| ネイティブ Win32 (C++) | UI 開発コストが高い、テーマ切替・仮想スクロールの自前実装が必要 |

**Consequences**

- (+) ユーザーは 12 MB のセットアップ EXE をダウンロード・インストールするだけ
- (+) Rust 側で Win32 API を直接呼び出せる
- (+) セキュリティ既定値が堅牢（CSP・Capabilities が必須）
- (−) WebView2 Runtime が前提（Windows 10 1809 以降は同梱、それ以前は別途インストール必要）
- (−) ネイティブ通知やシステムトレイは未実装（Tauri 拡張で対応可能）

---

### ADR-002 State Management: React Hooks

- **Status**: Accepted
- **Date**: 2024-12

**Context**

フロントエンドは 5 セクション（Launcher / Analyze / Database / Settings / Archive）+ モーダル群で構成される。状態は機能ごとに独立しており、グローバル共有は最小限。

**Decision**

React 標準の Hooks（`useState`, `useReducer`, `useCallback`, `useRef`, `useEffect`, `useMemo`）のみで状態管理を実装する。Redux / Zustand / Jotai 等の状態管理ライブラリは採用しない。

**Rationale**

- アプリ規模が小さい（コンポーネント約 40 個）ため、グローバル状態管理ライブラリは過剰
- MVVM 風の `viewmodels/use*State.ts` カスタムフックパターンで状態を集約しており、Redux 等の追加抽象化を入れると複雑性が増すだけ
- 依存ライブラリゼロでバンドルサイズを最小化できる
- React 19 の最新機能（`flushSync`, `useTransition`）を直接活用できる

**Alternatives Considered**

| Option | Rejected Reason |
| ------ | --------------- |
| Redux Toolkit | 規模に対してボイラープレート過多 |
| Zustand | 採用しても問題ないが、Hooks のみで成立しているため追加メリットなし |
| Jotai / Recoil | アトミック設計が活きるほど状態が細分化されていない |
| MobX | reactive プログラミング学習コスト |

**Consequences**

- (+) 依存ライブラリゼロ、バンドルサイズ最小
- (+) フック単位で個別にテスト可能
- (−) アプリ規模が現状の 10 倍を超える場合は再評価が必要
- (−) Tauri イベント (`listen()`) の cleanup を `useEffect` で手書きする必要がある

---

### ADR-003 Virtual Scrolling: @tanstack/react-virtual

- **Status**: Accepted
- **Date**: 2024-12

**Context**

ログビューア機能で 10 万行超のテキスト表示が必要。全行を DOM 化すると React のリコンサイル処理がフリーズする。

**Decision**

[@tanstack/react-virtual](https://tanstack.com/virtual) を採用する。

**Rationale**

- ヘッドレス設計（CSS に介入してこない）のため、テーマ切替・独自スタイルと干渉しない
- React 19 の concurrent features と互換性がある
- TanStack 製で長期メンテナンスへの安心感

**Alternatives Considered**

| Option | Rejected Reason |
| ------ | --------------- |
| react-window | 動的サイズ対応が HOC 必要、TypeScript 型情報が後付け |
| react-virtuoso | バンドルサイズ約 30 KB と大きい、水平スクロール非対応 |
| 自前実装 | 開発・保守コスト過大 |

**Consequences**

- (+) 10 万行ログでも DOM 数は常に約 30 行（overscan 10 含む）
- (+) ズーム時に `virtualizer.measure()` で再計算可能
- (−) 動的高さ行は未使用（ログビューア用途では全行同じ高さで十分）

---

### ADR-004 Styling: CSS Modules

- **Status**: Accepted
- **Date**: 2024-12

**Context**

3 テーマ（Light / Dark / Midnight）の切替が必要。コンポーネント別のスタイルスコープが欲しい。CSP は可能な限り厳格化したい。

**Decision**

CSS Modules + CSS Variables によるテーマ切替を採用する。`<html>` 要素のクラス切替（`light-theme` / `dark-theme` / `midnight-theme`）で `:root` の CSS 変数を一括上書きする。

**Rationale**

- CSP の `unsafe-inline` の用途を最小化できる（CSS-in-JS は `unsafe-inline` 必須）
- テーマ切替時はクラス 1 つの書き換えで全配色が変わる
- Vite が CSS Modules を hash 化し、本番ビルドでの重複削減
- ランタイムオーバーヘッドゼロ（styled-components のような実行時パーサ不要）

**Alternatives Considered**

| Option | Rejected Reason |
| ------ | --------------- |
| styled-components | CSP `unsafe-inline` 必須 |
| Emotion | 同上 |
| Tailwind CSS | `dark:` variant では 3 値テーマ（Midnight）を扱いにくい |
| Vanilla CSS | スコープ衝突リスク |

**Consequences**

- (+) Stylelint で CSS の品質保証が容易
- (+) テーマ切替が CSS 変数の差し替えのみで完結
- (−) 動的スタイルは `style={{}}` のため `style-src 'unsafe-inline'` は最小限残る
- (−) デザイントークンの一元管理が `App.{theme}.css` 4 ファイルに分散

---

### ADR-005 Database: SQLite via rusqlite (bundled)

- **Status**: Accepted
- **Date**: 2024-12

**Context**

VRChat ログ（数十 MB × 数百ファイル）を構造化保管し、横断検索可能にする必要がある。ローカル完結で動作させたい。

**Decision**

[rusqlite](https://crates.io/crates/rusqlite) の `bundled` feature で SQLite を静的リンクする。WAL モードと外部キー制約を有効化する。

**Rationale**

- SQL の表現力で 9 テーブル + 3 ビュー + 8 インデックスの関係を自然に表現できる
- `bundled` feature により OS の SQLite ライブラリに依存しない（環境差を排除）
- WAL モードで取り込み中（書き込み）と閲覧（読み取り）の並行動作が可能
- DB プレビュー機能で `SELECT * FROM <table>` をそのまま UI に表示できる
- savepoint と transaction で部分失敗を許容する設計が可能

**Alternatives Considered**

| Option | Rejected Reason |
| ------ | --------------- |
| IndexedDB | WebView 内のみで完結、Rust バックエンドから操作できない |
| sled (Rust 製 KVS) | SQL 不可、ビュー・JOIN 不可 |
| ファイル + JSON | スキーマ進化・検索効率が劣る |
| 外部 DB (PostgreSQL 等) | サーバ起動が必要、ローカルアプリの要件に反する |

**Consequences**

- (+) 単一ファイル `stellarecord.db` で完結、バックアップは file copy のみ
- (+) `params!` マクロでパラメータバインドが容易、SQL インジェクションを物理的に排除
- (+) 標準ツール（`sqlite3` CLI, DB Browser）で直接読み出し可能
- (−) 同時書き込みはアプリレベルで排他制御が必要 (`isAnalyzeRunning`)
- (−) スキーママイグレーションは手動コード（refinery 等のツール不使用）

---

### ADR-006 Compression: tar + zstd

- **Status**: Accepted
- **Date**: 2024-12

**Context**

VRChat の生ログ（数十〜数百 MB）を恒久保管する。圧縮率と速度を両立し、標準的なツールで展開可能なフォーマットを採用する。

**Decision**

[tar](https://crates.io/crates/tar) で 1 ログ 1 アーカイブを作成し、[zstd](https://crates.io/crates/zstd) level 3 で圧縮する。出力は `*.tar.zst`。

**Rationale**

- zstd level 3 は VRChat ログのテキストデータで圧縮率約 90%、速度は gzip の 3〜5 倍
- tar でファイル名情報がアーカイブ内に保持されるため、後続パースでメタデータが不要
- `.tar.zst` は 7-Zip / WinRAR / Linux 標準ツールで展開可能
- Rust の `tar` crate / `zstd` crate は枯れていて安定

**Alternatives Considered**

| Option | Rejected Reason |
| ------ | --------------- |
| zip | 圧縮率約 80%、tar+zstd より 10pt 程度劣る |
| gzip | 単一ファイルのみ、ファイル名保持不可 |
| 7z (LZMA2) | 圧縮率約 95% だが速度が遅く、取り込み時間が数倍 |
| 無圧縮 | ストレージ容量を圧迫 |

**Consequences**

- (+) 数百 MB のログが数十 MB に縮む
- (+) アーカイブ閲覧時は zstd ストリーム解凍で即座にビューア起動
- (−) Windows エクスプローラ標準で `.tar.zst` を開けない（アプリ内ビューアでカバー）
- (−) zstd level 3 は速度寄りの設定。最高圧縮率 (level 19) は不採用

---

### ADR-007 Log Parser: Hand-written Regex State Machine

- **Status**: Accepted
- **Date**: 2024-12

**Context**

VRChat ログは非構造化テキスト。10 種類のイベント（Joining, OnPlayerJoined, Notification 等）を検出し、時系列依存（直前のワールド名を保持する等）を解決する必要がある。

**Decision**

`LazyLock<Regex>` で 23 個の正規表現を一度だけコンパイルし、行単位ループ内のステートマシン（5 つの `Option` 変数）で時系列依存を解決する。

**Rationale**

- VRChat のログ形式は安定しており、正規表現で十分追随可能
- `LazyLock` でコンパイルコストは起動時 1 回のみ
- 不正パターンは起動時に panic するため、開発時に即検知可能
- 過剰な抽象化（パーサコンビネータ、Aggregate Root）を避けて手続き的に書く

**Alternatives Considered**

| Option | Rejected Reason |
| ------ | --------------- |
| nom (パーサコンビネータ) | コードが冗長になる、デバッグ困難 |
| BNF / lex+yacc | 文法定義の保守コスト過大 |
| LLM ベースのパース | 確率的で再現性なし、レイテンシ過大 |

**Consequences**

- (+) 取り込み速度約 100k 行/秒 を達成
- (+) 新イベント追加は正規表現 1 個と if 分岐 1 個を追加するだけ
- (−) VRChat 側でログ形式が変更されたら正規表現の手動修正が必要
- (−) 非 UTF-8 バイトを含む行は skip される

---

### ADR-008 Rust Lints: Deny panic-equivalent paths

- **Status**: Accepted
- **Date**: 2024-12

**Context**

デスクトップアプリで Rust 側がクラッシュすると「UI フリーズ + DB 中途半端な状態 + ユーザーが何も保存できない」最悪のシナリオになる。

**Decision**

Cargo workspace lints で `unwrap_used`, `expect_used`, `panic` を `deny` に設定する。

```toml
[workspace.lints.clippy]
unwrap_used  = "deny"
expect_used  = "deny"
panic        = "deny"
todo         = "warn"
dbg_macro    = "warn"
print_stdout = "warn"
```

**Rationale**

- コンパイル時に `Result` の伝播を強制できる
- 「忘れた頃のクラッシュ」原因を排除できる
- コードレビュー時の議論を削減できる

**Alternatives Considered**

| Option | Rejected Reason |
| ------ | --------------- |
| デフォルトのみ | unwrap が混入しやすい |
| `clippy::all = warn` のみ | 警告は無視されがち |

**Consequences**

- (+) Rust コード 4,825 行で `unwrap()` ゼロ、`expect()` ゼロ
- (+) エラーは全て `Result` で UI まで伝播し、トースト通知される
- (−) `parser.rs::compile_regex` のみ `#[allow(clippy::panic)]` を明示（固定パターン破損の早期検知のため意図的）
- (−) プロトタイピング時は一時的に lint を緩める運用が必要

---

### ADR-009 Module Boundaries: Forbid feature-to-feature imports

- **Status**: Accepted
- **Date**: 2024-12

**Context**

feature モジュールが肥大化するとファイル間依存がスパゲッティ化する。最初から境界を機械的に強制したい。

**Decision**

ESLint の `no-restricted-imports` ルールで feature 間の相互 import を禁止する。共通処理は `shared/` に上げる以外の選択肢を消す。

```js
'no-restricted-imports': ['error', {
  patterns: [
    { group: ['**/features/analyze/**'], message: 'feature 間の直接参照禁止 → shared/ へ' },
    { group: ['**/features/archive/**'], message: '同上' },
    { group: ['**/features/database/**'], message: '同上' },
    { group: ['**/features/registry/**'], message: '同上' },
    { group: ['**/features/settings/**'], message: '同上' },
  ]
}]
```

**Rationale**

- 各 feature を独立してテスト・リファクタ可能にする
- 「ちょっと別 feature の関数を使いたい」誘惑を機械的に防止する

**Alternatives Considered**

| Option | Rejected Reason |
| ------ | --------------- |
| 規約のみ（リント無し） | 守られない |
| Nx 等のモノレポツール | 規模に対して過剰 |

**Consequences**

- (+) feature が独立性を保てる
- (+) リファクタリング時の影響範囲が明確
- (−) 共有が必要になった場合、毎回 `shared/` への切り出しと命名判断が発生

---

### ADR-010 Installer: NSIS via Tauri Bundler

- **Status**: Accepted
- **Date**: 2024-12

**Context**

Windows 向けの配布形式が必要。MSI / NSIS / MSIX / portable EXE の選択肢がある。

**Decision**

Tauri Bundler の NSIS ターゲットを採用し、`installer.nsi` / `hooks.nsi` をカスタムテンプレートとして使用する。

**Rationale**

- Tauri Bundler が公式サポート（`tauri.conf.json` で `"targets": ["nsis"]` 指定のみ）
- カスタムスクリプトで独自処理（タスクキル、データ保護）が記述可能
- `installMode: "currentUser"` で管理者権限不要、配布障壁を最小化
- NSIS スクリプトで Program Files / WINDIR への配置を明示的に拒否可能

**Alternatives Considered**

| Option | Rejected Reason |
| ------ | --------------- |
| MSI (WiX) | Tauri 標準サポートなし、カスタマイズコスト高 |
| MSIX | Microsoft Store 配布前提、現状はサイドロード配布 |
| Portable EXE | レジストリ書き込み・自動起動登録ができない |

**Consequences**

- (+) 約 12 MB のセットアップ EXE 1 つで配布完結
- (+) アンインストール時に `Data/archive/` `Data/db/` を保護対象として除外可能
- (+) 日本語インストーラ UI
- (−) 自動更新の仕組みは別途必要（v1.0 では未実装）
- (−) コード署名は別途運用が必要（v1.0 では未実装）

---

## Rejected Technologies

主要な検討の中で意図的に採用しなかった技術と却下理由の一覧。

| Technology | Reason for Rejection |
| ---------- | -------------------- |
| Electron | バイナリサイズ約 10 倍、メモリ約 4 倍 |
| Redux / Zustand / Jotai | アプリ規模で過剰、Hooks のみで成立 |
| Tailwind CSS | 3 値テーマ（Midnight）と相性が悪い |
| TanStack Query | サーバ通信ゼロ、ローカル DB にキャッシュ層不要 |
| ORM (Diesel / SeaORM) | スキーマが小規模で生 SQL が読みやすい |
| MSI / MSIX インストーラ | Tauri 標準サポートなし、または用途違い |
| Tauri Updater | v1.0 ではスコープ外、インストーラ再実行で運用 |
| Sentry / Telemetry | ローカル完結ポリシーに反する |
| tracing / log crate | 月次ファイル append で十分 |
| i18next | 日本語固定、要件発生時に導入 |
| Storybook | コンポーネント数 40 程度で過剰 |
| GraphQL | フロント↔バック間は Tauri invoke で完結 |
