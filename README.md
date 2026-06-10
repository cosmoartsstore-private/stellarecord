# StellaRecord

[![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D6)](https://www.microsoft.com/windows)
[![Tauri](https://img.shields.io/badge/Tauri-2.2-24C8DB)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-19-61DAFB)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-2021-DEA584)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Proprietary-lightgrey)](#license)

VRChat のゲームログを圧縮アーカイブと SQLite に正規化して保管・閲覧する Windows デスクトップアプリケーション。

---

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Tech Stack](#tech-stack)
- [Architecture](#architecture)
- [Requirements](#requirements)
- [Installation](#installation)
- [Build from Source](#build-from-source)
- [Project Structure](#project-structure)
- [Data and Privacy](#data-and-privacy)
- [Security](#security)
- [Documentation](#documentation)
- [Acknowledgements](#acknowledgements)
- [License](#license)

---

## Overview

StellaRecord は VRChat が出力するゲームログ (`output_log_*.txt`) を恒久保管し、構造化データに変換して閲覧するための Windows デスクトップアプリケーションである。

VRChat のログファイルは古いものから順に削除されるため、過去の訪問履歴・同席ユーザー・通知などの情報を後から参照できなくなる。本アプリは生ログを `tar + zstd` で圧縮保管しつつ、行単位パーサで内容を SQLite に正規化することで、過去のログを検索可能なデータとして長期保存する。

WebView2 (Edge) + Rust の Tauri v2 アーキテクチャで実装され、外部サーバとの通信は行わない。アンインストール時は再生成できない圧縮済みログアーカイブだけを保護対象として残す。

---

## Features

- 生ログの `.tar.zst` 圧縮アーカイブ化（zstd level 3）
- 行単位パーサによる VRChat ログの SQLite 構造化保管（9 テーブル + 3 ビュー、検出イベント 10 種）
- 圧縮アーカイブを展開せずストリーム閲覧する仮想スクロール対応のログビューア
- カテゴリ／レベルフィルタおよび DB キーワードハイライト
- アーカイブ容量警告ラインの設定とストレージメーター
- 登録 EXE のランチャー機能（Windows VersionInfo からの表示名抽出と高解像度アイコン取得）
- DB プレビュー（テーブル／ビューのページネーション・ソート閲覧）
- Light / Dark / Midnight の 3 テーマ切替
- Windows 起動時の自動起動オプション
- 取り込み処理のキャンセル機能（savepoint による部分ロールバック）

---

## Tech Stack

### Frontend

| Layer | Technology | Version |
| ----- | ---------- | ------- |
| Language | TypeScript | 5.9 |
| UI Framework | React | 19.2 |
| Build Tool | Vite | 7.3 |
| Virtual Scroll | @tanstack/react-virtual | 3.13 |
| Styling | CSS Modules | - |
| Tauri SDK | @tauri-apps/api | 2.10 |

### Backend

| Layer | Technology | Version |
| ----- | ---------- | ------- |
| Language | Rust | Edition 2021 |
| Application Framework | Tauri | 2.2.4 |
| Database | rusqlite (bundled SQLite) | 0.38 |
| Compression | zstd | 0.13 |
| Archive Format | tar | 0.4 |
| Win32 API | windows-rs | 0.58 |
| Registry I/O | winreg | 0.52 |
| Image Processing | image (PNG only) | 0.25 |

### Distribution

| Layer | Technology |
| ----- | ---------- |
| Installer | NSIS (via Tauri Bundler) |
| Install mode | currentUser |
| Code Signing | （未実装） |

技術選定の詳細と意思決定記録は [docs/tech-stack.md](docs/tech-stack.md) を参照。

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  Windows 10 / 11 (x64)                      │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              StellaRecord.exe (Tauri v2)            │    │
│  │                                                     │    │
│  │   WebView2 (Chromium)             Rust Backend      │    │
│  │   ─────────────────────────       ───────────────   │    │
│  │   React 19 + TypeScript      ◀──▶ Tauri 2.2         │    │
│  │   Vite 7                  IPC     rusqlite 0.38     │    │
│  │   @tanstack/react-virtual         tar + zstd        │    │
│  │   CSS Modules                     windows-rs 0.58   │    │
│  │                                                     │    │
│  └────────────────────────┬────────────────────────────┘    │
│                           │                                 │
│       ┌───────────────────┴────────────────────┐            │
│       ▼                                        ▼            │
│   ┌──────────────────────┐       ┌──────────────────────┐   │
│   │ SQLite (WAL mode)    │       │ Compressed Archives  │   │
│   │ Data/db/             │       │ Data/archive/        │   │
│   │   stellarecord.db    │       │   *.tar.zst          │   │
│   └──────────────────────┘       └──────────────────────┘   │
│                                                             │
│   Windows Registry: HKCU\Software\CosmoArtsStore\...        │
└─────────────────────────────────────────────────────────────┘
```

詳細なモジュール構成・データフロー・並行性モデルは [docs/spec.md](docs/spec.md) を参照。

---

## Requirements

### Runtime

- Windows 10 (1809 以降) または Windows 11 (x64)
- Microsoft Edge WebView2 Runtime（Windows 10 1809 以降は OS 標準同梱）
- 約 50 MB のディスク空き容量（ログアーカイブストアは別途）

### Build

- [Node.js](https://nodejs.org/) 20.19 以上、または 22.12 以上（Vite 7 の要求バージョン）
- [Rust](https://rustup.rs/) toolchain（stable、edition 2021）
- Windows SDK（`windows` crate のビルドに必要）

---

## Installation

### From Installer

1. [Releases](https://github.com/cosmoartsstore-private/stellarecord/releases) ページから最新の `StellaRecord_Setup.exe` をダウンロード
2. インストーラを実行
3. 既定のインストール先は `%LOCALAPPDATA%\Programs\StellaRecord`

管理者権限は不要。Program Files / Windows ディレクトリ配下へのインストールはインストーラが拒否する。

### Uninstallation

Windows の「アプリと機能」から `StellaRecord` をアンインストールする。アプリ本体と DB は削除されるが、`Data/archive/` ディレクトリは保護対象として残される。完全に削除する場合は手動で `Data/archive/` ディレクトリを削除する。

---

## Build from Source

```bash
# 依存関係のインストール
npm install

# 開発ビルド（Vite dev server + Tauri dev）
npm run tauri dev

# 本番ビルド（NSIS インストーラを生成）
npm run tauri build
```

ビルド成果物は `src-tauri/target/release/bundle/nsis/` に出力される。

### Lint and Format

```bash
npm run lint                              # ESLint (typescript-eslint strict)
npm run format                            # Prettier
npm run stylelint                         # Stylelint
cargo clippy --workspace --all-targets    # Rust clippy（unwrap/expect/panic = deny）
```

---

## Project Structure

```
.
├── src/                        React frontend (TypeScript)
│   ├── app/                    アプリケーションルート、ルーティング、モーダル統括
│   ├── features/               機能モジュール
│   │   ├── analyze/            解析セクション
│   │   ├── archive/            アーカイブ取り込み・ログビューア
│   │   ├── database/           DB プレビュー
│   │   ├── registry/           ランチャー
│   │   └── settings/           設定
│   └── shared/                 共通コンポーネント・ユーティリティ
├── src-tauri/                  Rust backend
│   ├── src/
│   │   ├── lib.rs              Tauri 起動と IPC ハンドラ登録
│   │   ├── commands/           Tauri IPC コマンドハンドラ
│   │   ├── analyze/            ログ解析パイプライン
│   │   ├── config.rs           Windows レジストリ I/O
│   │   ├── platform.rs         Win32 API 連携
│   │   └── utils.rs            ロガー・エラー整形
│   ├── capabilities/           Tauri capability 定義
│   └── windows/                NSIS インストーラスクリプト
├── docs/                       技術ドキュメント
└── package.json
```

各 feature は `views/` `viewmodels/` `services/` `models/` に分割した MVVM 風構成を採用している。feature 間の相互参照は ESLint の `no-restricted-imports` で禁止されている。

---

## Data and Privacy

本アプリはローカル完結で動作する。以下のデータをローカル保存する。

| Data | Location | Purpose |
| ---- | -------- | ------- |
| 圧縮ログアーカイブ (`*.tar.zst`) | `%LOCALAPPDATA%\Programs\StellaRecord\Data\archive\` | VRChat 生ログの恒久保管 |
| SQLite データベース | `%LOCALAPPDATA%\Programs\StellaRecord\Data\db\stellarecord.db` | 解析済みログデータと登録アプリ情報 |
| アプリ運用ログ | `%LOCALAPPDATA%\Programs\StellaRecord\Data\logs\info-YYYY-MM.log` | 障害調査用ログ（月次ローテーション） |
| ユーザー設定 | Windows Registry `HKCU\Software\CosmoArtsStore\StellaRecord` | アプリ設定（容量上限、自動起動等） |
| UI テーマ | LocalStorage `stella-record-theme` | テーマ選択（Light/Dark/Midnight） |

**外部通信**: 本アプリは外部サーバとの通信を行わない。テレメトリ送信、クラッシュレポート送信、自動アップデートチェックは未実装。

**データの可搬性**: `Data/` ディレクトリ全体をコピーすることで別 PC への移行が可能。SQLite データベースは `sqlite3` CLI など標準ツールで直接読み出すことができる。

---

## Security

### Application

- **Tauri Capabilities**: 許可しているのは `core:default`, `shell:default`, `shell:allow-open` のみ。ファイルシステム操作は Rust 側で実装し、フロントエンドには直接公開していない。
- **Content Security Policy**: `default-src 'self'; script-src 'self'; img-src 'self' asset: https: data:; style-src 'self' 'unsafe-inline'; connect-src 'self' http://localhost:* ipc:;`
- **SQL Injection 対策**: 動的テーブル名・カラム名は ASCII 英数字と `_` のみを許可するバリデーションを通過した値のみ SQL に補間。すべての値は `params!` でバインドする。
- **多重起動防止**: Windows カーネル名前付き Mutex (`Local\StellaRecord_SingleInstance`) で単一インスタンスを保証。
- **クラッシュ抑制**: Rust リントで `unwrap_used`, `expect_used`, `panic` を `deny` に設定。コンパイル時にクラッシュ経路を排除している。

### Installation

- **管理者権限不要**: `installMode: currentUser` で `%LOCALAPPDATA%` 配下にインストール。
- **インストール先制限**: NSIS インストーラスクリプトが Program Files / `%WINDIR%` 配下への配置を拒否する。
- **コード署名**: 現バージョンでは未実装。SmartScreen 警告が表示される可能性がある。

### Known Risks

- 本アプリは現在コード署名されていない。Windows SmartScreen による起動時警告が表示される。
- 取り込み処理中に外部から DB ファイルを直接編集した場合の動作は保証されない。

脆弱性報告は GitHub Issues ではなく CosmoArtsStore へ直接連絡すること。

---

## Documentation

| Document | Description |
| -------- | ----------- |
| [docs/spec.md](docs/spec.md) | 機能仕様書（アーキテクチャ、モジュール、IPC リファレンス、データフロー） |
| [docs/database.md](docs/database.md) | データベース定義書（ER 図、スキーマ、インデックス） |
| [docs/tech-stack.md](docs/tech-stack.md) | 技術スタック詳細と意思決定記録（ADR） |

---

## Acknowledgements

本アプリは以下の OSS を利用している。各ライセンス条項は配布物に同梱される NOTICE ファイルを参照。

主要な依存関係：[Tauri](https://tauri.app/), [React](https://react.dev/), [Vite](https://vitejs.dev/), [TanStack Virtual](https://tanstack.com/virtual), [rusqlite](https://github.com/rusqlite/rusqlite), [zstd](https://github.com/gyscos/zstd-rs), [tar-rs](https://github.com/alexcrichton/tar-rs), [windows-rs](https://github.com/microsoft/windows-rs)

SVG アイコン：[Material Design Icons (Pictogrammers)](https://pictogrammers.com/library/mdi/) — Apache License 2.0

---

## License

Proprietary — Copyright (c) CosmoArtsStore. All rights reserved.

本ソフトウェアの再配布・改変・リバースエンジニアリングは許可されていない。
