# STELLA RECORD

VRChat のログを「取り込み → アーカイブ → 構造化 DB に蓄積 → 閲覧」まで一気通貫で扱う、Windows 向けのデスクトップアプリケーション。

CosmoArtsStore の VRChat エコシステム製品の 1 つで、姉妹アプリ Polaris が確保した生ログを `.tar.zst` で圧縮保管し、SQLite に正規化して蓄積する。ワールド訪問・同席ユーザー・通知・スクリーンショット・OSC 連携などのイベントを横断的に検索／集計できる。

| | |
|---|---|
| プラットフォーム | Windows 10 / 11 |
| ライセンス | Proprietary — CosmoArtsStore |
| バージョン | 1.0.0 |

---

## 主要機能

| セクション | 概要 |
|---|---|
| **ランチャー** | 登録済みアプリ（StellaRecord 本体／外部 EXE）をアイコン付きで一覧・起動 |
| **解析** | ストレージメーター、アーカイブ取り込み、ログビューア、元ログ削除モーダル |
| **DB プレビュー** | SQLite テーブル／ビューをページネーション・ソート付きで閲覧 |
| **設定** | OS スタートアップ登録、アーカイブ容量警告ライン、テーマ切替 (Light / Dark / Midnight) |

機能の詳細仕様は [`docs/spec.md`](docs/spec.md) を参照。

---

## 技術スタック

| レイヤ | 採用技術 |
|---|---|
| フロントエンド | React 19, TypeScript 5.9, Vite 7, @tanstack/react-virtual |
| バックエンド | Rust (Edition 2021), Tauri v2 |
| データベース | SQLite (rusqlite, bundled, WAL モード) |
| 圧縮 | tar + zstd |
| スタイル | CSS Modules（Light / Dark / Midnight の 3 テーマ） |
| フォント | M PLUS 1 (UI), JetBrains Mono (ログ表示) |
| インストーラ | NSIS (Tauri Bundler) |

詳細は [`docs/tech-stack.md`](docs/tech-stack.md) を参照。

---

## ドキュメント

| ファイル | 内容 |
|---|---|
| [`docs/spec.md`](docs/spec.md) | 機能仕様書（各セクションの詳細、画面フロー、IPC 一覧） |
| [`docs/database.md`](docs/database.md) | DB 定義書（ER 図 + 全テーブル／ビューのカラム定義） |
| [`docs/tech-stack.md`](docs/tech-stack.md) | 技術スタック参考資料（採用技術と選定理由） |

---

## セットアップ

### 必要環境

- [Node.js](https://nodejs.org/) 18 以上
- [Rust](https://rustup.rs/)（`rust-toolchain.toml` でバージョン固定）
- [Tauri CLI](https://tauri.app/)（`npm install` で同時にインストールされる）
- Windows SDK（Rust ビルドツールチェーンに含まれる `windows` crate が要求）

### 開発起動

```bash
npm install
npm run tauri dev
```

### 本番ビルド（NSIS インストーラ生成）

```bash
npm run tauri build
```

出力は `src-tauri/target/release/bundle/nsis/` 配下。

### Lint / Format

```bash
npm run lint        # ESLint (typescript-eslint strict + a11y)
npm run format      # Prettier
npm run stylelint   # Stylelint
```

Rust 側は `cargo clippy --workspace` で `unwrap_used / expect_used / panic = deny` の厳格ルールが適用される。

---

## データレイアウト（インストール後）

```
$INSTDIR/
  └── Data/
      ├── archive/      # .tar.zst ログアーカイブ（アンインストール時も保護）
      ├── db/            # stellarecord.db (SQLite, WAL モード)
      ├── logs/          # アプリ運用ログ (info-YYYY-MM.log)
      └── EBWebView/     # WebView2 キャッシュ
```

---

## 設定の永続化先

| 項目 | 保存先 |
|---|---|
| アプリ設定（StellaRecord） | レジストリ `HKCU\Software\CosmoArtsStore\StellaRecord` |
| アーカイブ容量上限（Polaris と共有） | レジストリ `HKCU\Software\CosmoArtsStore\Polaris` |
| スタートアップ登録 | レジストリ `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |
| テーマ設定 | `localStorage` |

---

## アーキテクチャ概要

```
┌─────────────────────────────┐         ┌─────────────────────────────┐
│  React Frontend (src/)      │  IPC    │  Rust Backend (src-tauri/)  │
│  ─────────────────────────  │ ◀────▶  │  ─────────────────────────  │
│  app/        ルーティング    │ invoke  │  commands/   IPC ハンドラ   │
│  features/   機能モジュール  │ /event  │  analyze/    パーサ + DB    │
│    ├ analyze                │         │  config.rs   レジストリ I/O │
│    ├ archive                │         │  platform.rs Win32 連携     │
│    ├ database               │         │  utils.rs    ロガー         │
│    ├ registry               │         └──────┬──────────────────────┘
│    └ settings               │                │
│  shared/     共通基盤        │                ▼
└─────────────────────────────┘         ┌──────────────────────────────┐
                                        │  SQLite (Data/db/             │
                                        │            stellarecord.db)   │
                                        │  tar.zst (Data/archive/)      │
                                        └──────────────────────────────┘
```

詳細フローは [`docs/spec.md`](docs/spec.md) の「アーキテクチャ」「データフロー」セクションを参照。

---

## ライセンス

Proprietary — CosmoArtsStore. 本リポジトリは社内開発・配布用途のみを想定する。
