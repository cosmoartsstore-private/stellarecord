# StellaRecord

StellaRecord は、Polaris が保存した VRChat ログを `.tar.zst` と SQLite に保存し、ログ本文と解析済みデータを閲覧する Windows 向けデスクトップアプリケーションです。

通常の処理で外部 API、認証、テレメトリ送信は行いません。利用者が登録した実行ファイル、フォルダ、クレジットのリンクを明示操作で開きます。

## Features

- **Archive import** — Polaris の `Data/archive` にあるログを、1ログ1ファイルの `.tar.zst` へ zstd レベル3で圧縮します。起動時取込は初回確認で許可した場合だけ実行し、設定から変更できます。
- **Log analysis** — 認証、ワールド訪問、同席ユーザー、通知、撮影、OSC、VRChat+ を10テーブルと3ビューへ保存します。本文の SHA-256・長さと解析版で取込済み判定を行います。
- **Friend status** — 旧形式のアバター取得ログを同一訪問のユーザーへ一意に対応できた場合だけ、フレンド状態を保存します。判定不能・矛盾・対応する情報がない場合は不明として扱います。
- **Log viewer** — 管理アーカイブと利用者が選んだログをストリームで読み、仮想スクロール、カテゴリ、ログレベル、DB由来のキーワード強調で閲覧します。
- **Storage and cleanup** — 容量警告ラインを設定し、圧縮保存した本文と一致する Polaris の元ログだけを、選択・確認後に削除できます。
- **Launcher** — 登録した EXE の表示名・アイコンを取得し、起動、フォルダ表示、登録解除を行います。
- **Database preview** — テーブル・ビューを読取り専用で500行ずつ表示し、列で並べ替えます。
- **Settings** — Light / Dark / Midnight、Windowsログイン時の自動起動、起動時取込の許可、容量警告ラインを保存します。
- **Cancellation** — 取込へ停止を要求できます。要求受理と停止完了は別で、解析の取消しは今回の DB 更新をロールバックします。

## Architecture

```text
Polaris/Data/archive/output_log_*.txt
└─ StellaRecord.exe                  Tauri v2
   ├─ WebView2                       React / TypeScript
   │  └─ features                    views / viewmodels / services / models
   ├─ Rust                           IPC、圧縮、解析、Windows連携
   └─ Data
      ├─ archive/*.tar.zst            圧縮ログ
      ├─ db/stellarecord.db           解析結果・登録アプリ
      ├─ logs/info-YYYY-MM.log        運用ログ
      └─ EBWebView                    WebViewのローカルデータ
```

画面は `src/app/App.tsx`、IPC登録と起動は `src-tauri/src/lib.rs`、schemaは `src-tauri/src/analyze/db.rs` を正本とします。module責務、状態所有、IPC/eventとworkerの流れは [Architecture](docs/architecture.md) を参照してください。

## Tech Stack

| Layer        | Technology                                            |
| ------------ | ----------------------------------------------------- |
| Frontend     | React `19.2.4`、TypeScript `5.9.3`、Vite `7.3.6`      |
| UI           | CSS Modules、`@tanstack/react-virtual 3.13.24`        |
| Runtime      | Tauri `2.10.3`、Rust edition 2021、toolchain `1.93.0` |
| Database     | `rusqlite 0.38.0`、bundled SQLite、WAL                |
| Archive      | `tar 0.4.45`、`zstd 0.13.3`、`sha2`                   |
| Windows      | `windows 0.58.0`、`winreg 0.52.0`                     |
| Distribution | Tauri CLI、NSIS、current-user install                 |

版番号は現在の lockfile の解決値です。宣言値、ビルド設定、採用理由は [Tech Stack](docs/tech-stack.md) を参照してください。

## Requirements

### Runtime

- Windows 10 / 11、x64
- Microsoft Edge WebView2 Runtime。未導入の場合はインストーラーが導入処理を行います。
- Polaris のインストール情報とログコピー。起動時の同期と元ログの削除に使用します。
- アーカイブと DB を保存できる空き容量と書込み権限

### Development

- Node.js と npm。Viteの要求は `^20.19.0 || >=22.12.0` ですが、jsdomとRust native probeを含む検証には別の版・実行条件があります。[開発前提](docs/tech-stack.md#toolchain-and-runtime-requirements)を参照してください。
- `rust-toolchain.toml` に記載する Rust `1.93.0`、rustfmt、clippy
- `x86_64-pc-windows-msvc` target をビルドできる MSVC 開発環境

## Installation

NSIS インストーラーを実行します。既定の配置は `%LOCALAPPDATA%\CosmoArtsStore\StellaRecord` です。current-user install とし、Program Files と Windows directory 配下は選択できません。

更新はインストーラーを再実行します。`/UPDATE` は通常データ削除を通りませんが、`DbPath` はインストール先の既定DBへ再設定されます。外部DBのfile保持と参照先設定の保持は別です。通常終了時に取込の自動取消し・完了待ちを行う独自処理はありません。

通常のアンインストールではアプリ本体、インストール先の `Data/db`・`Data/logs`・`Data/EBWebView`、bundle data、関連registry登録を削除し、`Data/archive` を残します。外部の `ArchivePath` / `DbPath` を探索して削除しませんが、保存先を記録したStellaRecord registryは削除します。登録app情報と設定を残す場合は事前に保全してください。[保存寿命](docs/database.md#deletion-and-retention)と[導入・更新・削除](docs/tech-stack.md#installation-lifecycle)に詳細があります。

## Build and Test

コマンドは repository root で実行します。

```powershell
# lockfileどおりに依存関係を導入
npm ci --ignore-scripts

# 開発起動
npm run tauri dev

# Frontend本番ビルド
npm run build

# NSISインストーラー生成
npm run tauri build

# Frontend・Rustの検証
npm run test:coverage
npm run test:rust
npm run verify
```

既定のインストーラー出力先は `target/release/bundle/nsis/` です。`npm run verify` は Prettier、Stylelint、ESLint、Frontend coverage、Frontend build、clippy、Rust test を順に実行します。rustfmt の確認は `npm run fmt:check` で別途実行します。

CIはRustをWindows、FrontendをUbuntuで検証する定義です。Rust formatは参考check、配布buildは対象外で、Rust native probeに必要なnpm依存準備は現jobに含まれません。実gateと前提の差は [Verification Commands and CI](docs/tech-stack.md#verification-commands-and-ci)を参照してください。成功件数やcoverage値、静的文書の照合日を全機能の確認完了とは扱いません。

## Data and Privacy

| Data             | Location                                                                | Purpose                                      |
| ---------------- | ----------------------------------------------------------------------- | -------------------------------------------- |
| Source logs      | Polaris の `<install>\Data\archive`                                     | 圧縮・解析元。自動同期では削除しない         |
| Archives         | StellaRecord の `<install>\Data\archive\*.tar.zst`                      | ログ本文の保管                               |
| SQLite           | `<install>\Data\db\stellarecord.db`                                     | 解析情報と `apps`                            |
| Application logs | `<install>\Data\logs\info-YYYY-MM.log`                                  | 障害調査用の WARN / ERROR                    |
| Registry         | `HKCU\Software\CosmoArtsStore\StellaRecord`、`Polaris`、Windows Run key | 保存先、自動起動、起動時取込、容量警告ライン |
| Theme            | localStorage `stella-record-theme`                                      | Light / Dark / Midnight                      |
| WebView data     | `<install>\Data\EBWebView`                                              | WebView2のローカルデータ                     |

archive と DB の明示保存先は registry の `ArchivePath` / `DbPath` を優先し、未指定時は `InstallLocation` 配下を使います。現在の exe の位置から保存先を推測しません。元ログはPolarisが保存した入力、archiveは本文の保存物、解析行は再生成できるデータ、`apps`は利用者が付与した情報です。元ログ削除は明示確認したPolaris側fileだけを変更し、登録解除はEXE本体を削除しません。[Storage and Ownership](docs/database.md#storage-and-ownership)に詳細があります。

専用の全データバックアップ・復元 UI はありません。手動移行ではアプリを終了し、実効保存先の archive と DB directory をコピーします。registry と localStorage はこのコピーに含まれません。通常アンインストール後に残るアーカイブから解析 DB を再構築できますが、ランチャー登録はログから再生成できません。

ログビューアの本文読取りが途中で失敗した場合は、取得済みの行と完了イベントを返して停止します。画面上の完了だけではログ末尾まで読めたことを保証しません。取消し・部分失敗・元ログ削除の条件は [Specification](docs/spec.md) を参照してください。

## Project Structure

```text
.
├─ src/
│  ├─ app/                    画面ルート・初回確認・modal統括
│  ├─ features/               analyze / archive / database / registry / settings
│  └─ shared/                 共通UI・hook・型・書式
├─ src-tauri/
│  ├─ src/lib.rs              Tauri起動・IPC登録
│  ├─ src/commands/           IPC・filesystem・設定・ランチャー
│  ├─ src/analyze/            parser・friendship・schema・取込
│  └─ windows/                NSIS template・hook
├─ Cargo.toml / Cargo.lock   Rust workspaceと依存
├─ package.json              Frontend・Tauri・検証command
├─ docs/                     機能・構造・保存・開発配布・操作体験サンプル
└─ AGENT.md                  project-specific routing
```

## Documentation

| Document | Audience and scope |
| --- | --- |
| [Specification](docs/spec.md) | 利用者・仕様担当。操作条件、判断、成功/取消し/失敗後の結果 |
| [Architecture](docs/architecture.md) | 保守担当。entry、module責務、状態所有、IPC/event、workerと終了 |
| [Database Schema](docs/database.md) | データ担当・保全する人。全schema、archive/設定の保存先・transaction・保持/削除 |
| [Tech Stack](docs/tech-stack.md) | 開発・配布担当。依存、採用理由、command、CI、導入/更新/削除 |
| [Operation Preview](docs/manual.html) | 利用検討者。固定データによる限定体験。実ファイル・registry・DBへ接続しない |

利用開始はREADME→必要なspec→操作補助資料、保守参加はREADME→architecture→対象spec→database→tech-stackの順に読みます。backup/削除は[保存寿命](docs/database.md#deletion-and-retention)へ直接進めます。

## License

Proprietary — Copyright (c) CosmoArtsStore. All rights reserved.

主要な OSS は Tauri、React、Vite、TanStack Virtual、rusqlite、tar、zstd、windows-rs です。依存関係の権利情報は各 package の LICENSE を参照してください。SVG アイコンは Material Design Icons（Pictogrammers、Apache License 2.0）を使用します。
