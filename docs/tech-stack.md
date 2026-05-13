# 技術スタック — 選定理由と意思決定の記録

> 本書はポートフォリオ用に、STELLA RECORD で採用した技術一つひとつについて**「なぜそれを選んだか」「他に何を検討したか」「却下した理由は何か」**を記録したもの。
> ADR (Architecture Decision Record) 風に、各意思決定を **Context → Alternatives → Decision → Consequences** の構造で記述している。

---

## 目次

1. [全体俯瞰](#1-全体俯瞰)
2. [意思決定記録](#2-意思決定記録)
   - [ADR-001: デスクトップフレームワーク — Tauri v2](#adr-001-デスクトップフレームワーク--tauri-v2)
   - [ADR-002: 状態管理 — React Hooks のみ](#adr-002-状態管理--react-hooks-のみ)
   - [ADR-003: 仮想スクロール — @tanstack/react-virtual](#adr-003-仮想スクロール--tanstackreact-virtual)
   - [ADR-004: スタイリング — CSS Modules](#adr-004-スタイリング--css-modules)
   - [ADR-005: データベース — SQLite (rusqlite bundled)](#adr-005-データベース--sqlite-rusqlite-bundled)
   - [ADR-006: 圧縮 — tar + zstd](#adr-006-圧縮--tar--zstd)
   - [ADR-007: ログ解析 — 手書きパーサ (正規表現 + ステートマシン)](#adr-007-ログ解析--手書きパーサ-正規表現--ステートマシン)
   - [ADR-008: Rust リント — panic 全面禁止](#adr-008-rust-リント--panic-全面禁止)
   - [ADR-009: アーキテクチャ境界 — feature 間相互参照禁止](#adr-009-アーキテクチャ境界--feature-間相互参照禁止)
   - [ADR-010: インストーラ — NSIS (Tauri Bundler)](#adr-010-インストーラ--nsis-tauri-bundler)
3. [採用技術一覧](#3-採用技術一覧)
4. [意図的に採用しなかった技術](#4-意図的に採用しなかった技術)

---

## 1. 全体俯瞰

```
┌────────────────────────────────────────────────────────────┐
│                  Windows 10 / 11 (64bit)                   │
│                                                            │
│  ┌────────────────────────────────────────────────────┐    │
│  │           StellaRecord.exe (Tauri v2)              │    │
│  │                                                    │    │
│  │  ┌────────────────────┐  ┌──────────────────────┐  │    │
│  │  │ WebView2 (Edge)     │  │ Rust Backend         │  │    │
│  │  │ ──────────────────  │  │ ──────────────────── │  │    │
│  │  │ React 19            │  │ tauri 2.2            │  │    │
│  │  │ TypeScript 5.9      │  │ rusqlite 0.38        │  │    │
│  │  │ Vite 7              │◀─┼─▶ tar 0.4 + zstd 0.13 │  │    │
│  │  │ @tanstack/virtual   │  │ windows-rs 0.58     │  │    │
│  │  │ CSS Modules         │  │ winreg 0.52          │  │    │
│  │  │                     │  │ image 0.25           │  │    │
│  │  └────────────────────┘  └──────────────────────┘    │
│  │                                    │                  │
│  └────────────────────────────────────┼──────────────────┘
│                                       ▼                    │
│   ┌─────────────────────────────────────────────────────┐  │
│   │ Filesystem: $INSTDIR/Data/                           │  │
│   │ Registry:   HKCU\Software\CosmoArtsStore\...         │  │
│   └─────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
```

### 設計原則

| 原則 | 意味 |
|---|---|
| **ローカル完結** | サーバ依存ゼロ、オフライン動作前提 |
| **軽量起動** | バイナリサイズ・起動時間で Electron 比 1/5 を目標 |
| **データ無損失** | ユーザーデータは絶対に失わない（複数の保護策） |
| **堅牢性 > 機能数** | クラッシュしないことを最優先、機能は段階的に追加 |
| **依存最小化** | npm 直接依存 6 個、Rust 直接依存 12 個に抑制 |

---

## 2. 意思決定記録

### ADR-001: デスクトップフレームワーク — Tauri v2

**Context**: VRChat ユーザー向け Windows デスクトップアプリ。Web UI でリッチな表現をしたいが、ローカルファイル/レジストリアクセスが必須。

**Alternatives**:

| 候補 | バイナリサイズ | メモリ消費 | OS API アクセス | TypeScript 親和性 |
|---|---|---|---|---|
| **Tauri v2** | ~12 MB | 〜50 MB | Rust ネイティブ | 完璧 |
| Electron | ~150 MB | 〜200 MB | Node.js 経由 | 完璧 |
| WPF + .NET | ~30 MB（.NET 込み） | 〜80 MB | C# ネイティブ | 不可（XAML/C#） |
| Flutter | ~50 MB | 〜100 MB | Platform Channel | 不可（Dart） |
| ネイティブ Win32 (C++) | ~5 MB | ~30 MB | 直接 | 不可 |

**Decision**: **Tauri v2** を採用。

**理由**:

- **バイナリサイズ**: 12MB は Electron の 1/10 以下。ダウンロード障壁が低い
- **WebView2 を活用**: Windows 10/11 に標準搭載なのでランタイム配布不要
- **Rust の堅牢性**: バックエンドが Rust なのでメモリ安全性・型安全性が標準で確保される
- **セキュリティ既定値**: CSP・Capabilities が必須化されていて、初期からセキュア設定
- **エコシステム成熟度 (2024-2025)**: v2 で Mobile 対応もあり、長期メンテナンスへの安心感

**Consequences**:

- ✅ ユーザーは 12MB の EXE をインストールするだけ
- ✅ Rust 側で OS API を直叩きできる（Win32 アイコン抽出、レジストリ、CreateMutex 等）
- ⚠️ WebView2 Runtime が前提（Win10 1809 以降は標準同梱、それより古い OS は別途インストールが必要）
- ⚠️ ネイティブ通知やシステムトレイは未使用だが、Tauri 拡張で対応可能

---

### ADR-002: 状態管理 — React Hooks のみ

**Context**: 機能は 5 セクション（Launcher / Analyze / Database / Settings / Archive）+ モーダル群。状態は機能ごとに独立で、グローバル状態は最小限。

**Alternatives**:

| 候補 | 学習コスト | ボイラープレート | デバッグ容易性 |
|---|---|---|---|
| **React Hooks のみ** | 低 | 最小 | React DevTools |
| Redux Toolkit | 中 | 中 | 強力な Time Travel |
| Zustand | 低 | 最小 | シンプル |
| Jotai / Recoil | 低 | 最小 | アトミック設計 |
| MobX | 中 | 低 | reaction tracking |

**Decision**: **`useState` / `useReducer` / `useCallback` / `useRef` のみ**で実装。

**理由**:

- **アプリ規模が小さい**: 全コンポーネント約 40 個、state を共有する範囲は親子間で完結
- **MVVM 構造**: `viewmodels/use*State.ts` フックに状態管理を集約しており、Redux 等の追加抽象化を入れると過剰
- **テスト不要レベルの単純さ**: 各 viewmodel フックは React Testing Library で容易にテスト可能（現状は手動 QA のみ）

**Consequences**:

- ✅ 依存ライブラリゼロ、バンドルサイズ最小
- ✅ React 19 の最新機能（`useTransition`, `flushSync` 等）を直接活用
- ⚠️ アプリ規模が 10 倍になったら状態管理ライブラリ導入を再検討
- ⚠️ Tauri イベント (`listen()`) のクリーンアップを `useEffect` で手書きする必要がある

---

### ADR-003: 仮想スクロール — @tanstack/react-virtual

**Context**: ログビューアで 10 万行超のテキスト表示が必要。普通に全行 DOM 化すると React がフリーズする。

**Alternatives**:

| 候補 | バンドルサイズ | 動的サイズ対応 | 水平スクロール | TypeScript |
|---|---|---|---|---|
| **@tanstack/react-virtual** | ~5 KB | ✅ | ✅ | ファーストクラス |
| react-window | ~6 KB | △ (HOC 必要) | ✅ | 後付け |
| react-virtuoso | ~30 KB | ✅ | ❌ | ファーストクラス |
| 自前実装 | 0 | 自由 | 自由 | 完全制御 |

**Decision**: **@tanstack/react-virtual** を採用。

**理由**:

- **ヘッドレス設計**: CSS に介入してこないため、テーマ切替や独自スタイルと干渉しない
- **API のシンプルさ**: `useVirtualizer` フック 1 つで完結
- **React 19 完全対応**: 最新 React の concurrent features と相性が良い
- **メンテナンス**: TanStack 製で長期サポートへの安心感

**Consequences**:

- ✅ 10 万行のログでもスクロール時の DOM 数は常に ~30 行（overscan 10 含む）
- ✅ ズーム（行高変更）時に `virtualizer.measure()` で再計算可能
- ⚠️ 動的高さの行は使っていない（全行同じ高さ）— ログビューア用途では十分

---

### ADR-004: スタイリング — CSS Modules

**Context**: 3 テーマ（Light / Dark / Midnight）を切り替え可能にしたい。コンポーネント別のスタイルスコープも欲しい。

**Alternatives**:

| 候補 | スコープ | テーマ切替 | CSP `unsafe-inline` |
|---|---|---|---|
| **CSS Modules** | ファイル単位 | CSS Variables で可能 | 不要 |
| styled-components | コンポーネント単位 | ThemeProvider | 必須 |
| Emotion | コンポーネント単位 | ThemeProvider | 必須 |
| Tailwind CSS | ユーティリティ | dark: variant | 不要 |
| Vanilla CSS | グローバル | 手動 | 不要 |

**Decision**: **CSS Modules + CSS Variables によるテーマ切替**。

**理由**:

- **CSP 強化**: `unsafe-inline` の use case を最小化したい（理想は撤廃だが、React の `style={{ ... }}` で部分的に必要）
- **テーマ切替の自然さ**: `<html class="dark-theme">` のクラス切替で `:root` の CSS 変数を一括上書きできる
- **ビルド時最適化**: Vite が CSS Modules を hash 化し、production ビルドで重複削減
- **ランタイムオーバーヘッドゼロ**: styled-components のような実行時パーサ不要

**Consequences**:

- ✅ テーマ切替時はクラス 1 つ書き換えで全配色が変わる（`requestAnimationFrame` で transition を一時無効化してちらつき防止）
- ✅ Stylelint で CSS の品質保証が容易
- ⚠️ 動的スタイル（state による色変更等）は `className` の組み合わせか `style={{}}` で対応
- ⚠️ デザイントークンの一元管理は `App.{theme}.css` 4 ファイルに分散 — 将来 design-system 化するなら整理が必要

---

### ADR-005: データベース — SQLite (rusqlite bundled)

**Context**: VRChat ログ（数十 MB × 数百ファイル）を構造化保管し、ワールド訪問・同席ユーザー・通知などを横断検索したい。ローカル完結。

**Alternatives**:

| 候補 | ファイル形式 | SQL | 並行性 | バンドル容易性 |
|---|---|---|---|---|
| **SQLite** (rusqlite bundled) | 単一ファイル | ✅ | WAL モード | bundled feature で静的リンク |
| IndexedDB | ブラウザストレージ | ❌ | 単一 origin | WebView 内のみ |
| sled (Rust 製 KVS) | バイナリ | ❌ | MVCC | crate 直接 |
| ファイル + JSON | ファイル群 | ❌ | 排他制御自作 | ファイル I/O のみ |
| 外部 DB (PostgreSQL 等) | サーバ | ✅ | 強力 | サーバ起動が必要 |

**Decision**: **rusqlite (bundled feature)** を採用。

**理由**:

- **SQL の表現力**: 9 テーブル + 3 ビュー + 8 インデックスでの関係性表現が自然
- **bundled で完全静的リンク**: OS の SQLite ライブラリに依存しない（Windows 10/11 で確実動作）
- **WAL モード**: 取り込み中（書き込み）とログビューア（読み取り）の並行動作が可能
- **DB プレビュー機能の容易さ**: SQL `SELECT * FROM <table>` でそのまま UI に表示
- **ロールバック設計**: savepoint と transaction で部分失敗を許容できる

**Consequences**:

- ✅ 単一ファイル `stellarecord.db` で完結、バックアップは file copy のみ
- ✅ `params!` マクロでパラメータバインド、SQL インジェクションを物理的に排除
- ⚠️ 同時書き込みはアプリレベルで排他制御（`isAnalyzeRunning`）
- ⚠️ schema migration は手動コード（refinery 等のツール不使用、規模が小さいため）

---

### ADR-006: 圧縮 — tar + zstd

**Context**: VRChat の生ログ（数十〜数百 MB）を恒久保管したい。圧縮率と速度の両立、かつ標準的なツールで展開可能なフォーマット。

**Alternatives**:

| 候補 | 圧縮率 | 速度 | 互換性 | ファイル名保持 |
|---|---|---|---|---|
| **tar + zstd** | ~90% | 高速 | Linux 標準、7-Zip 対応 | ✅ tar header |
| zip | ~80% | 中 | ほぼ全 OS 標準 | ✅ |
| gzip | ~85% | 中 | ほぼ全 OS 標準 | ❌ (単一ファイルのみ) |
| 7z (LZMA2) | ~95% | 遅 | 7-Zip 専用 | ✅ |
| 無圧縮 | 0% | 即時 | - | - |

**Decision**: **tar + zstd (level 3)** を採用。

**理由**:

- **zstd の圧縮率/速度比**: VRChat ログのテキストデータで圧縮率 ~90%、速度は gzip より 3-5 倍速い
- **tar でファイル名保持**: アーカイブ内に元ファイル名（`output_log_2025-10-21_00-59-15.txt`）が残るため、後続パースでメタデータが不要
- **7-Zip / WinRAR で展開可能**: ユーザーが手動で中身を確認できる安心感
- **Rust 純正実装**: `tar` crate + `zstd` crate が枯れていて安定

**Consequences**:

- ✅ 数百 MB のログが数十 MB に縮む（ディスク容量問題を解消）
- ✅ アーカイブ閲覧時は zstd ストリーム解凍で即座にビューア起動
- ⚠️ zstd level 3 は速度寄り。最高圧縮率を求めるなら level 19 だが、取り込み時間が数倍になるので採用しない
- ⚠️ Windows エクスプローラは標準で .tar.zst を開けない（7-Zip 等が必要）— アプリ内ビューアでカバー

---

### ADR-007: ログ解析 — 手書きパーサ (正規表現 + ステートマシン)

**Context**: VRChat ログは非構造化テキスト。10 種類のイベント（Join, OnPlayerJoined, Notification 等）を検出し、時系列依存（直前のワールド名を覚えておく等）も解決する必要がある。

**Alternatives**:

| 候補 | 学習コスト | 速度 | 保守性 |
|---|---|---|---|
| **正規表現 + 状態変数** | 低 | 高 | 良 |
| パーサコンビネータ (nom) | 中 | 高 | コードが冗長になりがち |
| BNF / lex+yacc | 高 | 最高 | 文法定義の保守コスト |
| LLM ベースのパース | - | 低 | 確率的で再現性なし |

**Decision**: **正規表現を 1 度だけコンパイルし、ループ内ステートマシンで処理**。

**コア設計**:

```rust
static RE_TIME: LazyLock<Regex> = LazyLock::new(|| compile_regex(...));
static RE_JOINING: LazyLock<Regex> = LazyLock::new(|| compile_regex(...));
// ... 23 個

fn parse_and_import_reader(...) -> Result<()> {
    let mut current_ts: Option<NaiveDateTime> = None;
    let mut pending_room_name: Option<String> = None;
    let mut current_visit_id: Option<i64> = None;

    for line_result in reader.lines() {
        let Ok(line) = line_result else { continue };
        if let Some(caps) = RE_TIME.captures(&line) { current_ts = ...; }
        if let Some(caps) = RE_ENTERING.captures(&line) { pending_room_name = ...; }
        if let Some(caps) = RE_JOINING.captures(&line) {
            if let Some(room) = &pending_room_name {
                // INSERT INTO visits
            }
        }
        // ... 10 イベント
    }
}
```

**理由**:

- **VRChat ログ形式は安定**: パターンが頻繁に変わらないため、正規表現で十分追随できる
- **LazyLock で初期化 1 回**: アプリ起動時に全パターンをコンパイル、ループ内コストは matching のみ
- **コンパイル時 panic で早期検知**: 不正パターンは起動時にプロセス停止 → 開発者が即気付ける
- **ステート変数 5 つで完結**: 過剰な抽象化（Aggregate Root 等）を避けて手続き的に書く判断

**Consequences**:

- ✅ 取り込み速度 ~100k 行/秒 を達成
- ✅ 新イベント追加は正規表現 1 つと if 分岐 1 つを足すだけ
- ⚠️ VRChat 側でログ形式が変更されたら正規表現の手動修正が必要
- ⚠️ 非 UTF-8 バイトを含む行は skip される（mod プラグインの混入対応）

---

### ADR-008: Rust リント — panic 全面禁止

**Context**: デスクトップアプリで Rust 側がクラッシュすると「UI が固まる + DB が中途半端な状態で残る + ユーザーが何も保存できない」最悪のシナリオになる。

**Alternatives**:

| 候補 | 厳格度 | 開発体験 |
|---|---|---|
| デフォルトのみ | 緩 | unwrap 多用しがち |
| `clippy::all = warn` | 中 | 警告のみ、無視できる |
| **`panic = deny` + `unwrap_used = deny`** | 厳 | コンパイルエラーで強制 |

**Decision**: Cargo.toml workspace lints で **`unwrap_used / expect_used / panic = deny`** を設定。

```toml
[workspace.lints.clippy]
unwrap_used  = "deny"
expect_used  = "deny"
panic        = "deny"
todo         = "warn"
dbg_macro    = "warn"
print_stdout = "warn"
```

**理由**:

- **Result の伝播を強制**: コンパイル時に `?` 演算子か明示的なエラーハンドリングを要求
- **「忘れた頃のクラッシュ」を排除**: unwrap は開発時には便利だが、本番では予期しないクラッシュ原因
- **コードレビューが楽**: 「ここ unwrap してるけど大丈夫？」の議論が起きない

**Consequences**:

- ✅ Rust コード 4,825 行で `unwrap()` ゼロ、`expect()` ゼロ、`panic!()` 1 箇所のみ（正規表現コンパイル失敗時の意図的な panic）
- ✅ エラーはすべて `Result` で UI まで伝播し、トースト通知される
- ⚠️ 例外: `parser.rs::compile_regex` のみ `#[allow(clippy::panic)]` を明示
- ⚠️ プロトタイピング時の素早い書き捨てができない（開発時のみ allow で外す運用）

---

### ADR-009: アーキテクチャ境界 — feature 間相互参照禁止

**Context**: feature が肥大化するとファイル間依存がスパゲッティ化する。最初から境界を強制したい。

**Decision**: ESLint `no-restricted-imports` で **feature 間の相互 import を禁止**。共通処理は `shared/` に上げる以外の選択肢を消す。

```js
// eslint.config.js
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

**Consequences**:

- ✅ 各 feature が独立してテスト/リファクタ可能
- ✅ 「ちょっと別 feature の関数使いたい」誘惑を機械的に防止
- ⚠️ 共有が必要になったら必ず `shared/` への切り出しを要求 → 命名と配置に毎回判断が必要

---

### ADR-010: インストーラ — NSIS (Tauri Bundler)

**Context**: Windows 向けの配布形式が必要。MSI / NSIS / MSIX / portable EXE の選択肢。

**Decision**: **NSIS (Tauri Bundler のテンプレートをカスタム)**

**理由**:

- **Tauri Bundler のサポート**: `tauri.conf.json` で `"targets": ["nsis"]` 指定だけで生成
- **カスタムスクリプト**: `installer.nsi` / `hooks.nsi` で独自のインストール前後処理（タスクキル、データ保護）を記述可能
- **管理者権限不要**: `installMode: "currentUser"` で配布障壁を最小化
- **Program Files 配置の拒否**: NSIS スクリプトで明示的に拒否（書き込み権限問題を回避）

**Consequences**:

- ✅ ~12 MB のセットアップ EXE 1 つで配布完結
- ✅ アンインストール時に `Data/archive/` `Data/db/` を保護対象として除外可能
- ✅ 日本語インストーラ UI（`languages: ["Japanese"]`）
- ⚠️ MSIX に比べて自動更新の仕組みは別途必要（v1 では未実装）

---

## 3. 採用技術一覧

### フロントエンド

| 領域 | 技術 | バージョン | 役割 |
|---|---|---|---|
| 言語 | TypeScript | 5.9 | 型安全 (`strict: true`, `noUnusedLocals`, `noUnusedParameters`) |
| UI | React | 19.2 | Hooks ベースの状態管理 |
| ビルド | Vite | 7.3 | dev サーバ + 本番バンドル |
| 仮想スクロール | @tanstack/react-virtual | 3.13 | 10 万行対応のログビューア |
| Tauri SDK | @tauri-apps/api | 2.10 | invoke / listen |
| Shell プラグイン | @tauri-apps/plugin-shell | 2.3.5 | OS シェル統合 |
| フォント | @fontsource-variable/m-plus-1 | 5.2 | UI 用日本語可変フォント |
| フォント | @fontsource/jetbrains-mono | 5.2 | ログ表示用等幅フォント |

### バックエンド

| 領域 | 技術 | バージョン | 役割 |
|---|---|---|---|
| 言語 | Rust | Edition 2021 | バックエンド全般 |
| フレームワーク | tauri | 2.2.4 | IPC / WebView ホスティング |
| | tauri-build | 2.0.5 | コンパイル時設定 |
| | tauri-plugin-shell | 2.3.5 | フォルダオープン |
| | tauri-plugin-fs | 2.2.0 | ファイル I/O 権限 |
| DB | rusqlite | 0.38 (bundled) | SQLite クライアント |
| 時刻 | chrono | 0.4 | DATETIME パース |
| 正規表現 | regex | 1.x | ログ解析 |
| 圧縮 | zstd | 0.13 | 高速圧縮 |
| アーカイブ | tar | 0.4 | ファイル名保持 |
| Windows API | windows | 0.58 | Win32 直接呼び出し |
| Windows レジストリ | winreg | 0.52 | 設定永続化 |
| シェル統合 | opener | 0.8 | OS シェルでフォルダ開く |
| 画像処理 | image | 0.25 (PNG only) | アイコン PNG エンコード |
| Base64 | base64 | 0.22 | IPC でのアイコン転送 |
| シリアライズ | serde | 1.0 (derive) | IPC ペイロード型生成 |

### 品質支援

| 領域 | 技術 | バージョン | 役割 |
|---|---|---|---|
| TS リント | ESLint | 9.39 (flat config) | コード品質 |
| | typescript-eslint | 8.48 | strict + stylistic type checking |
| | eslint-plugin-react | 7.37 | React 専用ルール |
| | eslint-plugin-react-hooks | 7.0 | exhaustive-deps |
| | eslint-plugin-jsx-a11y | 6.10 | アクセシビリティ |
| | eslint-plugin-unicorn | 61.0 | 一般品質ルール |
| CSS リント | Stylelint | 16.25 | standard config |
| フォーマッタ | Prettier | 3.6 | コード整形 |
| Rust リント | clippy (built-in) | - | `unwrap = deny` 等 |

### 配布

| 領域 | 技術 | 役割 |
|---|---|---|
| インストーラ | NSIS | カレントユーザーインストール |
| 配布形式 | `.exe` セットアップ | 単一ファイル配布 |
| バイナリ最適化 | `lto = true`, `strip = true`, `codegen-units = 1` | サイズと速度の最大化 |

---

## 4. 意図的に採用しなかった技術

| 不採用 | 検討時の判断 |
|---|---|
| **Electron** | Tauri と比較してバイナリサイズ 10 倍、メモリ 4 倍。Node.js ランタイム同梱必須 |
| **Redux / Zustand / Jotai** | アプリ規模でグローバル状態管理は過剰、React Hooks のみで十分 |
| **Tailwind CSS** | 3 テーマ切替は CSS Variables の方が直接的、Tailwind の `dark:` variant では Midnight 等の 3 値目を扱いにくい |
| **TanStack Query** | サーバ通信ゼロ、ローカル DB に取得頻度の低いキャッシュ層は不要 |
| **ORM (Diesel / SeaORM)** | スキーマが小規模で生 SQL が読みやすい、ビルド時間も短縮 |
| **MSI / MSIX インストーラ** | NSIS の方が Tauri 標準サポートで導入が早い、自動更新は将来課題 |
| **自動更新 (Tauri Updater)** | v1 ではスコープ外。インストーラ再実行で運用 |
| **Sentry / Telemetry** | ローカル完結ポリシーに反する、テレメトリ送信は採用しない |
| **ロギングフレームワーク (tracing / log)** | 単純な月次ファイル append で十分、依存最小化 |
| **i18n フレームワーク (i18next 等)** | 日本語固定、多言語化要件が出てから導入 |
| **Storybook** | コンポーネント数 40 程度で過剰、手動 QA で対応 |
| **GraphQL** | フロント↔バック間の IPC は Tauri invoke で完結、スキーマ層を別途設ける価値なし |
| **マイクロサービス / 別プロセス分割** | パーソナルアプリで集約 DB の方が JOIN コスト低い |
