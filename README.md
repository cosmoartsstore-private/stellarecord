# STELLA RECORD

> **VRChat のログ、消える前に守る。**
> 古いログを `.tar.zst` で圧縮保管し、ワールド訪問・通知・スクリーンショットを SQLite に整理する Windows デスクトップアプリ。

<!-- TODO: docs/images/hero.png にメインビジュアル/ロゴを配置 -->
![STELLA RECORD](docs/images/hero.png)

---

## こんな悩みを解決します

- 📦 **「3 ヶ月前のあのワールド、どこだったっけ…」**
  → VRChat のログは古いものから順に消えていきます。STELLA RECORD は自動で圧縮保管します。
- 🔎 **「あの人と初めて会ったの、いつだっけ？」**
  → ワールド訪問と同席ユーザーがすべて SQLite に記録されます。
- 📸 **「あのスクショ、どのワールドで撮ったか分からない…」**
  → スクリーンショットも訪問先と紐付けて保管。
- 💾 **「ログがディスクを圧迫してきた」**
  → zstd 圧縮で生ログから約 90% 削減。容量警告ラインの設定も可能。

---

## できること

### 🗄 ログを圧縮して恒久保管

VRChat の生ログ (`output_log_*.txt`) を `.tar.zst` 形式で自動アーカイブ。アンインストールしてもアーカイブと DB は保護されます。

<!-- TODO: docs/images/analyze.png に解析画面 (Light + Dark) を配置 -->
![解析画面](docs/images/analyze.png)

### 📊 ログを構造化データに変換

ログを行単位で解析し、SQLite に整理。ワールド訪問、同席ユーザー、通知、スクリーンショット、OSC 連携などを横断的に検索できます。

<!-- TODO: docs/images/database.png に DB プレビュー画面を配置 -->
![DB プレビュー](docs/images/database.png)

### 📖 圧縮済みログをそのまま閲覧

`.tar.zst` を展開せずに直接ビューア表示。10 万行超のログでも仮想スクロールでサクサク動きます。レベル（エラー／警告／デバッグ）やカテゴリ（ワールド／通知／入退室）でフィルタ可能。

<!-- TODO: docs/images/logviewer.png にログビューア画面を配置 -->
![ログビューア](docs/images/logviewer.png)

### 🚀 連携アプリのランチャー

VRChat 関連の自作ツール（OyasumiVR など）を STELLA RECORD から直接起動。アイコンは EXE から自動抽出します。

<!-- TODO: docs/images/launcher.png にランチャー画面 (リスト / カード両方) を配置 -->
![ランチャー](docs/images/launcher.png)

### 🎨 3 つのテーマ

Light / Dark / Midnight の 3 テーマを切替可能。長時間の作業も目に優しく。

<!-- TODO: docs/images/themes.png に 3テーマ並びショットを配置 -->
![テーマ](docs/images/themes.png)

---

## 動作環境

- **OS**: Windows 10 / 11（64bit）
- **ディスク**: 50 MB 以上（アーカイブストアは別途、デフォルト上限 300 MB）
- **VRChat**: ログ出力フォルダがデフォルトの場所にあること

---

## インストール

1. [Releases ページ](https://github.com/cosmoartsstore-private/stellarecord/releases) から最新の `StellaRecord_Setup.exe` をダウンロード <!-- TODO: 配布開始後にリンク有効化 -->
2. インストーラを実行（管理者権限は不要）
3. インストール先は既定で `%LOCALAPPDATA%\Programs\StellaRecord`
4. インストール完了後、自動でアプリが起動します

> ⚠️ **Program Files 配下にはインストールできません。** ユーザーが書き込み可能な領域（既定値）を選択してください。

### アンインストール時

- アプリ本体は完全に削除されます
- `Data/archive/` のアーカイブと `Data/db/stellarecord.db` は**保護され削除されません**
- 完全に削除したい場合は手動で `%LOCALAPPDATA%\Programs\StellaRecord\Data\` を削除してください

---

## 使い方

### 初回起動

1. アプリ起動時に**自動で起動時取り込み**が走ります（既存ログをアーカイブ＋DB に反映）
2. 進捗バーが表示され、完了するとランチャー画面が開きます

### 古いログを再取り込み

「解析」タブ →「復元」ボタン → アーカイブを選択して「取込開始」

### ログを閲覧

「解析」タブ →「ログビューア」ボタン → 閲覧したいアーカイブを選択

- 左サイドバーでファイル切替
- 上部チップでカテゴリフィルタ（ワールド／通知／入退室／警告／エラー／デバッグ）
- `Ctrl + マウスホイール` でズーム

### DB を覗く

「DB」タブで全テーブル／ビューをページネーション付きで閲覧。ヘッダクリックでソート可能。

### 設定

「解析」タブ →「ストレージ管理」セクション

- **アーカイブ容量警告ライン**：MB 単位で設定（既定 300 MB）
- **自動起動**：Windows 起動時に STELLA RECORD を自動起動するか
- **テーマ**：右上のテーマアイコンで Light → Dark → Midnight を切替

### 連携アプリの登録

「ランチャー」タブ →「登録」ボタン → 任意の EXE を選択

- アイコンは自動抽出
- 表示名は EXE の VersionInfo から自動取得（編集可能）
- 「起動」「フォルダを開く」「登録解除」が可能

---

## FAQ

<details>
<summary><strong>Q. VRChat 公式に怒られませんか？</strong></summary>

STELLA RECORD は VRChat の**公式に出力されるログファイルを読むだけ**で、ゲームクライアントの改造やネットワーク通信の傍受は一切行いません。VRChat の利用規約に違反する動作はありません。
</details>

<details>
<summary><strong>Q. ログが取り込まれません</strong></summary>

VRChat のログ出力先が既定 (`%USERPROFILE%\AppData\LocalLow\VRChat\VRChat\`) になっているかご確認ください。また姉妹アプリ Polaris がインストールされている場合、Polaris の `archive` フォルダが参照先になります。
</details>

<details>
<summary><strong>Q. アーカイブが警告ラインを超えました</strong></summary>

容量警告は**通知のみ**で、自動削除はしません。容量を減らしたい場合は「元ログの削除」モーダルから古いアーカイブを選択して削除してください（アーカイブされていない生ログは保護されます）。
</details>

<details>
<summary><strong>Q. 多重起動できません</strong></summary>

仕様です。STELLA RECORD は単一インスタンスで動作します（DB の整合性を保つため）。
</details>

<details>
<summary><strong>Q. データを別 PC に移行したい</strong></summary>

`%LOCALAPPDATA%\Programs\StellaRecord\Data\` フォルダごとコピーすれば、移行先で同じデータベースとアーカイブが利用できます。
</details>

---

## トラブルシューティング

### 起動しない

1. `%LOCALAPPDATA%\Programs\StellaRecord\Data\logs\info-YYYY-MM.log` を確認
2. Microsoft Edge WebView2 Runtime がインストールされているか確認
3. [Issues](https://github.com/cosmoartsstore-private/stellarecord/issues) でログを添えて報告 <!-- TODO: Issues 受付フロー確定後にリンク有効化 -->

### ログビューアが途中で止まる

非 UTF-8 バイトを含むログ行で停止する既知問題があります。ログを開き直すか、別のアーカイブをお試しください。

### アイコンが取得できない

EXE が破損している、または保護フォルダ配下にある可能性があります。別のパスに EXE をコピーしてから登録してください。

---

## クレジット

- 開発: **ぷらねっと** ([@cosmoartsstore](https://github.com/cosmoartsstore-private)) <!-- TODO: 表記揺れ確認、SNS リンクなど -->
- アイコン素材: アプリ内クレジットモーダルを参照
- 姉妹アプリ: [Polaris](https://github.com/cosmoartsstore-private/polaris) — 生ログのバックアップ／同期 <!-- TODO: Polaris 公開リポジトリリンク確定後 -->

---

## 開発者向け情報

ビルド方法・アーキテクチャ・技術選定の詳細は [`docs/`](docs/) を参照してください。

| ドキュメント | 内容 |
|---|---|
| [`docs/spec.md`](docs/spec.md) | 機能仕様書（設計判断・データフロー） |
| [`docs/database.md`](docs/database.md) | DB 定義書（ER 図 + スキーマ） |
| [`docs/tech-stack.md`](docs/tech-stack.md) | 技術スタックと選定理由 |

---

## ライセンス

Proprietary — CosmoArtsStore. 本ソフトウェアの再配布・改変は許可されていません。
