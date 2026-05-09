# STELLA RECORD

VRChat のログデータを管理・分析するデスクトップアプリケーション。ログの自動取り込み、圧縮アーカイブ、SQLite によるデータ閲覧、外部アプリ連携を提供します。

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Frontend | React 19, TypeScript 5.9, Vite 7 |
| Backend | Rust (Edition 2021), Tauri v2 |
| Database | SQLite (rusqlite) |
| Compression | tar + zstd |
| Styling | CSS Modules (Light / Dark / Midnight テーマ) |
| Fonts | M PLUS 1 (UI), JetBrains Mono (ログ表示) |

## Features

- **Analyze** — ログの取り込み状況モニタリング、アーカイブサイズ制限、スタートアップ設定
- **Archive** — tar.zst 形式での圧縮保存、ストレージクォータ管理
- **Database** — SQLite テーブルのページネーション付きブラウジング
- **Registry (Launcher)** — 登録済みアプリの起動とフォルダオープン
- **Settings** — OS スタートアップ登録、テーマ切替

## Project Structure

```
src/                    # React フロントエンド
  ├── app/              #   アプリルート・ルーティング
  ├── features/         #   機能単位モジュール (analyze, archive, database, registry, settings)
  └── shared/           #   共通コンポーネント・ユーティリティ
src-tauri/              # Rust バックエンド
  ├── src/commands/     #   Tauri コマンドハンドラ (archive, database, import, polaris, settings)
  ├── src/analyze/      #   ログ解析・DB 書き込みロジック
  └── windows/          #   NSIS インストーラスクリプト
```

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/)
- [Tauri CLI](https://tauri.app/) (`npm install -g @tauri-apps/cli`)

### Setup

```bash
npm install
npm run tauri dev
```

### Build

```bash
npm run tauri build
```

### Lint / Format

```bash
npm run lint        # ESLint
npm run format      # Prettier
npm run stylelint   # Stylelint
```

## Data Layout (Installed)

```
$INSTDIR/
  └── Data/
      ├── archive/      # .tar.zst ログアーカイブ (アンインストール時も保護)
      ├── db/            # stellarecord.db
      ├── logs/          # アプリログ
      └── EBWebView/     # WebView2 キャッシュ
```

## Settings Storage

アプリ設定は Windows レジストリに保存されます。

```
HKCU\Software\CosmoArtsStore\StellaRecord
```

## License

Proprietary — CosmoArtsStore
