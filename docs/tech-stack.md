# STELLA RECORD 技術スタック参考資料

> 本書は STELLA RECORD v1.0 で採用している全技術と、その**選定理由・運用上の留意点**をまとめたもの。
> 新規参画時の前提知識として、また同種プロダクトの技術選定リファレンスとして利用する。

---

## 目次

1. [全体像](#1-全体像)
2. [フロントエンド](#2-フロントエンド)
3. [バックエンド](#3-バックエンド)
4. [データ永続化](#4-データ永続化)
5. [ビルド・配布](#5-ビルド配布)
6. [品質・開発支援](#6-品質開発支援)
7. [採用基準と選定理由](#7-採用基準と選定理由)

---

## 1. 全体像

```
┌────────────────────────────────────────────────────────────┐
│                        Windows 10/11                        │
│                                                            │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              StellaRecord.exe (Tauri v2)              │  │
│  │                                                       │  │
│  │  ┌────────────────────────┐ ┌─────────────────────┐  │  │
│  │  │   WebView2 (Edge)       │ │   Rust Backend       │  │  │
│  │  │   ─────────────────     │ │   ─────────────       │  │  │
│  │  │   React 19 + TS 5.9    │ │   tauri 2.2          │  │  │
│  │  │   Vite 7                │◀┼─▶ rusqlite 0.38       │  │  │
│  │  │   @tanstack/virtual    │ │   tar + zstd          │  │  │
│  │  │   CSS Modules          │ │   windows-rs 0.58    │  │  │
│  │  └────────────────────────┘ └─────────────────────┘  │  │
│  │                                       │              │  │
│  └───────────────────────────────────────┼──────────────┘  │
│                                          ▼                  │
│   ┌─────────────────────────────────────────────────────┐  │
│   │ Filesystem: $INSTDIR/Data/                           │  │
│   │  ├ archive/   .tar.zst                              │  │
│   │  ├ db/        stellarecord.db (SQLite WAL)          │  │
│   │  └ logs/      info-YYYY-MM.log                       │  │
│   └─────────────────────────────────────────────────────┘  │
│                                                            │
│   Windows Registry: HKCU\Software\CosmoArtsStore\...        │
└────────────────────────────────────────────────────────────┘
```

---

## 2. フロントエンド

### 言語・ランタイム

| 技術 | バージョン | 役割 |
|---|---|---|
| TypeScript | 5.9 | 型安全な JavaScript。`tsconfig.app.json` で `strict`/`noUnusedLocals`/`noUnusedParameters` を有効化 |
| Node.js | 18+ | ビルド時のみ。実行時には不要 |

### UI ライブラリ

| 技術 | バージョン | 役割 |
|---|---|---|
| **React** | 19.2 | UI フレームワーク。`useState` / `useEffect` / `useCallback` / `useRef` / `useMemo` のみ使用（状態管理ライブラリは不採用） |
| `react-dom` | 19.2 | `flushSync` を活用してログビューア開始時の DOM 同期更新を保証 |

### 仮想スクロール

| 技術 | バージョン | 役割 |
|---|---|---|
| **@tanstack/react-virtual** | 3.13 | ログビューア（最大数十万行）の仮想スクロール。`overscan: 10` でスクロール時のちらつきを抑制 |

採用理由：

- ヘッドレスで CSS に干渉しない
- React 19 と TypeScript 完全対応
- 動的サイズ・水平スクロール対応

### ビルドツール

| 技術 | バージョン | 役割 |
|---|---|---|
| **Vite** | 7.3 | 開発サーバ／本番ビルド。Tauri との統合は `tauri.conf.json` の `beforeDevCommand` / `beforeBuildCommand` で連動 |
| `@vitejs/plugin-react` | 5.1 | React Fast Refresh |

### スタイリング

| 技術 | 役割 |
|---|---|
| **CSS Modules** | コンポーネントローカルスコープ。`*.module.css` 形式 |
| **CSS Variables** | テーマ切替（`light` / `dark` / `midnight` の 3 種類）を `<html class="dark-theme">` のクラス切替で実現 |

採用理由：

- 外部 CSS-in-JS（styled-components, Emotion）を入れず、CSP `style-src 'self' 'unsafe-inline'` の `unsafe-inline` を必要最小限に
- 軽量で TypeScript の型情報も最小化

### フォント

| 技術 | 役割 |
|---|---|
| **@fontsource-variable/m-plus-1** | UI 全般。M PLUS 1（可変フォント）で日本語・英数字とも均整 |
| **@fontsource/jetbrains-mono** | ログ表示専用の等幅フォント |

ローカル同梱（`@fontsource` パッケージ）でオフライン環境でも UI が崩れない。

### Tauri 連携

| 技術 | バージョン | 役割 |
|---|---|---|
| `@tauri-apps/api` | 2.10 | `invoke()` / `listen()` を呼ぶ TypeScript SDK |
| `@tauri-apps/plugin-shell` | 2.3 | OS シェルでフォルダを開く |

---

## 3. バックエンド

### 言語・ランタイム

| 技術 | バージョン | 役割 |
|---|---|---|
| **Rust** | Edition 2021 (`rust-toolchain.toml` で固定) | バックエンド全般 |

### Tauri フレームワーク

| 技術 | バージョン | 役割 |
|---|---|---|
| **tauri** | 2.2.4 | フロントエンドとの IPC、WebView2 ホスティング、Tauri Builder |
| `tauri-build` | 2.0.5 | コンパイル時のコマンド／アイコン埋め込み |
| `tauri-plugin-shell` | 2.3.5 | `shell:allow-open` 経由でフォルダオープン |
| `tauri-plugin-fs` | 2.2.0 | ファイル I/O プラグイン（型定義主体） |

採用理由：

- Electron 比でバイナリサイズ 1/10、メモリ使用量大幅削減
- フロント／バックを明確に分離（プロセス境界）し、ネイティブクラッシュが UI を巻き込まない
- セキュリティ既定値が堅牢（CSP・capabilities 必須）

### SQLite クライアント

| 技術 | バージョン | 役割 |
|---|---|---|
| **rusqlite** | 0.38 | SQLite C ラッパー。`bundled` feature で libsqlite3 を静的リンク |
| `chrono` | 0.4 | タイムスタンプパース（`NaiveDateTime::parse_from_str`） |

採用理由：

- bundled で OS の SQLite に依存しない（Windows 10/11 で完全動作）
- `params!` マクロでパラメータバインドが容易、SQL インジェクションを防御
- savepoint / transaction の生 API が使える

### 圧縮

| 技術 | バージョン | 役割 |
|---|---|---|
| **zstd** | 0.13 | ログの可逆圧縮。レベル 3（標準）で速度と圧縮率のバランス |
| **tar** | 0.4 | tar アーカイブ。1 アーカイブ＝1 ログファイルで元ファイル名を保持 |

採用理由：

- `.tar.zst` は 7-Zip / WinRAR / Linux 標準ツールで開けるオープン形式
- 単一 ZIP より高速・高圧縮率
- ファイル名情報がアーカイブ内に残るためインポート時にメタデータを別管理する必要がない

### 正規表現

| 技術 | バージョン | 役割 |
|---|---|---|
| **regex** | 1 | VRChat ログ行のパターンマッチング。`LazyLock<Regex>` で一度だけコンパイル |

### Windows ネイティブ連携

| 技術 | バージョン | 役割 |
|---|---|---|
| **windows** (crate) | 0.58 | Win32 API 直接呼び出し |
| **winreg** | 0.52 | Windows レジストリ I/O |
| **opener** | 0.8 | OS シェルでフォルダ／URL を開く |
| **image** | 0.25 | アイコン抽出時の BGRA→RGBA→PNG 変換 |
| **base64** | 0.22 | アイコン BLOB を Base64 文字列化して IPC 転送 |

Win32 API の利用箇所：

| API | 用途 |
|---|---|
| `CreateMutexW` | 単一インスタンスガード |
| `SHGetImageList(SHIL_JUMBO)` | 256×256 アイコン取得 |
| `ExtractIconExW` | 32×32 アイコンフォールバック |
| `GetFileVersionInfoW` / `VerQueryValueW` | EXE の `FileDescription` 取得 |
| `IFileOpenDialog` | ネイティブファイル／フォルダ選択ダイアログ |
| `MessageBoxW` | 起動失敗時のエラー表示 |
| `CreateProcess` 相当（`Command::spawn` + `CREATE_NO_WINDOW`） | 外部 EXE 起動 |

### シリアライズ

| 技術 | バージョン | 役割 |
|---|---|---|
| **serde** | 1.0 | `derive(Serialize, Deserialize)` で IPC ペイロード型を生成 |

---

## 4. データ永続化

| ストア | 場所 | 用途 |
|---|---|---|
| SQLite (WAL) | `$INSTDIR/Data/db/stellarecord.db` | 解析済みログデータ／登録アプリ |
| `.tar.zst` | `$INSTDIR/Data/archive/` | VRChat 生ログの恒久保管 |
| 月次ログ | `$INSTDIR/Data/logs/info-YYYY-MM.log` | アプリ運用ログ |
| Windows レジストリ | `HKCU\Software\CosmoArtsStore\{StellaRecord,Polaris}` | アプリ設定 |
| LocalStorage | WebView2 ストレージ | UI テーマのみ |

SQLite を WAL モードで運用する理由：

- 書き込み中でも読み取りが可能（ログビューアでの並行参照）
- ロールバックジャーナルより高速
- アプリ終了時に自動 checkpoint

---

## 5. ビルド・配布

### NSIS インストーラ

| 技術 | 役割 |
|---|---|
| **NSIS** | Tauri Bundler 経由でインストーラ EXE を生成 |
| カスタムテンプレート | `src-tauri/windows/installer.nsi` |
| フック | `src-tauri/windows/hooks.nsi` でインストール／アンインストール前に `taskkill` |

### インストール仕様

| 項目 | 内容 |
|---|---|
| `installMode` | `currentUser`（管理者権限不要） |
| 言語 | 日本語のみ |
| 配置先 | 既定で `%LOCALAPPDATA%\Programs\StellaRecord`。Program Files / WINDIR への配置は拒否 |
| アンインストール時保護 | `Data/archive/` と `Data/db/` は削除されない |

### Release プロファイル

```toml
[profile.release]
lto = true            # Link-Time Optimization で実行ファイルサイズと速度を最適化
codegen-units = 1     # 並列度を犠牲にして最終バイナリ品質を最大化
strip = true          # デバッグシンボル削除
```

典型的なリリースビルド成果物サイズ：12 MB 程度（NSIS 包装後）。

---

## 6. 品質・開発支援

### TypeScript リント

| 技術 | バージョン | 役割 |
|---|---|---|
| **ESLint** | 9.39 | flat config 採用 |
| `typescript-eslint` | 8.48 | `strictTypeChecked` + `stylisticTypeChecked` |
| `eslint-plugin-react` | 7.37 | React 専用ルール |
| `eslint-plugin-react-hooks` | 7.0 | `exhaustive-deps` 等 |
| `eslint-plugin-jsx-a11y` | 6.10 | アクセシビリティ |
| `eslint-plugin-unicorn` | 61.0 | コード品質向上ルール |

特筆設定：

- `no-restricted-imports` で **feature 間の相互参照を禁止**し、共通処理は必ず `shared/` に置く方針を強制
- `react/jsx-no-bind` は緩めてあるが、ハンドラの命名規約は `handle*` 統一

### CSS リント

| 技術 | バージョン | 役割 |
|---|---|---|
| **Stylelint** | 16.25 | `stylelint-config-standard` |

### フォーマッタ

| 技術 | バージョン | 役割 |
|---|---|---|
| **Prettier** | 3.6 | TypeScript / JSON / CSS の整形 |

### Rust リント

`Cargo.toml` の workspace lints：

```toml
[workspace.lints.clippy]
all          = { level = "warn",  priority = -1 }
pedantic     = { level = "warn",  priority = -1 }
unwrap_used  = "deny"   # ← panic 経路を許さない
expect_used  = "deny"   # ←
panic        = "deny"   # ←
todo         = "warn"
dbg_macro    = "warn"
print_stdout = "warn"
too_many_lines       = "warn"
module_name_repetitions = "allow"
must_use_candidate   = "allow"
```

`unwrap_used` / `expect_used` / `panic` を `deny` にしているのが本プロジェクトの特徴。`Result` の伝播を強制し、運用中のクラッシュリスクを最小化する。

---

## 7. 採用基準と選定理由

### 大方針

1. **デスクトップアプリらしい応答性**：Electron 系より軽量な Tauri を採用
2. **長期メンテナンス性**：依存ライブラリは可能な限り**型付き・公式メンテ・最新メジャー**
3. **データ保護**：ログは消えない／DB は壊れにくい設計（WAL、savepoint、トランザクション境界）
4. **セキュリティ既定堅牢**：Tauri Capabilities / CSP / clippy strict / NSIS 配置制限

### 個別の選定理由

| 領域 | 選定 | 理由 |
|---|---|---|
| デスクトップフレームワーク | **Tauri v2**（vs Electron） | バイナリサイズ・メモリ・起動速度で優位。Rust バックエンドが堅牢 |
| 状態管理 | **React Hooks のみ** | アプリ規模で Redux/Zustand は過剰。`useState` + Context 不採用で props drill のみで成立 |
| 仮想スクロール | **@tanstack/react-virtual**（vs react-window） | ヘッドレス・水平スクロール対応・型情報が最新 |
| スタイリング | **CSS Modules**（vs Tailwind / styled-components） | CSP `unsafe-inline` を最小化、テーマ切替を CSS Variables で素直に表現 |
| DB | **SQLite**（vs IndexedDB / 外部 DB） | デスクトップアプリのローカル単一ファイル DB に最適、SQL の表現力 |
| 圧縮 | **tar + zstd**（vs ZIP / gzip） | 圧縮率と速度のバランス、ファイル名メタを残しつつクロスプラットフォーム |
| 解析エンジン | **行単位ステートマシン**（vs グラマ／AST） | VRChat ログは非構造化テキストで、正規表現＋状態保持で十分かつ高速 |
| 多重起動防止 | **CreateMutexW**（vs ファイルロック） | Windows カーネルがプロセス終了時に自動解放、安全で簡潔 |
| アイコン抽出 | **SHIL_JUMBO + ExtractIconExW**（vs 自前 PE パース） | OS のキャッシュ済みアイコンを利用、解像度フォールバックも容易 |

### 意図的に採用しなかった技術

| 技術 | 理由 |
|---|---|
| Electron | 重量級、バイナリ巨大、Node.js ランタイム同梱が必要 |
| Redux / Zustand | アプリ規模で不要、React Hooks のみで十分 |
| Tailwind CSS | テーマ切替（3 種）の表現には CSS Variables の方が直接的 |
| TanStack Query | データはローカル DB で取得頻度が低く、サーバキャッシュ層は不要 |
| ORM（Diesel / SeaORM） | スキーマがシンプルで `rusqlite` の生 SQL で十分、ビルド時間も短い |
| Sentry / Telemetry | ローカルアプリかつプロプライエタリ、テレメトリ送信は不採用 |
| ロギングフレームワーク (tracing / log) | 単純な月次ファイル append で十分、依存を最小化 |
| 自動更新 | NSIS インストーラの手動再実行で対応、Tauri Updater は v1.0 では不採用 |
