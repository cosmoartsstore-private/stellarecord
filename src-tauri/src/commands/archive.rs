//! アーカイブ圧縮、同期計画、ログビューアストリーミング、ファイル管理。
//!
//! `.tar.zst` のライフサイクルを管理する: Polaris ソースディレクトリからの
//! VRChat 生ログ圧縮、アーカイブの新規作成・安全な置き換え判断、
//! ログビューア UI へのアーカイブ内容ストリーミング、アーカイブ済みソースログの
//! クリーンアップ操作（一覧表示/削除）。

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use tauri::AppHandle;
use tauri::Emitter;

use crate::analyze;
use crate::config;
use crate::models::{ArchiveFileItem, DeletableLogInfo, LogViewerChunk, LogViewerMeta};
use crate::platform;
use crate::utils;

use super::{get_archive_store_dir, get_db_path, get_source_log_dir};

/// 外部フォルダ閲覧で受け入れるログ拡張子。
const EXTERNAL_LOG_EXTENSIONS: &[&str] = &[".txt", ".tar.zst"];

/// ライブ `VRChat` ファイルに必要な共有フラグでソースログを1件開く。
///
/// 元ログは変更してはならないため、`StellaRecord` は許可的な共有で読み取りのみ行い、
/// ソースファイルのリネームや削除は一切行わない。
fn open_source_log_for_read(path: &Path) -> Result<fs::File, String> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::{
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };

        fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0)
            .open(path)
            .map_err(|err| utils::command_open_err(path, err))
    }

    #[cfg(not(windows))]
    {
        fs::File::open(path).map_err(|err| utils::command_open_err(path, err))
    }
}

/// `VRChat` 生ログ1件を単一エントリの `.tar.zst` アーカイブに圧縮する。
///
/// tar でラップすることで元のファイル名が保持され、圧縮後も後続の
/// インポートやログビューアフローがソース名を使い続けられる。
fn compress_single_file(src: &Path, dst: &Path) -> Result<(), String> {
    let output = fs::File::create(dst).map_err(|err| utils::command_create_err(dst, err))?;
    let encoder = zstd::stream::Encoder::new(output, 3)
        .map_err(|err| utils::command_err("zstd エンコーダーを初期化できませんでした", err))?
        .auto_finish();
    let mut tar = tar::Builder::new(encoder);

    let file_name = src
        .file_name()
        .ok_or_else(|| format!("ファイル名を解決できませんでした [{}]", src.display()))?;
    let mut input = open_source_log_for_read(src)?;
    tar.append_file(file_name, &mut input)
        .map_err(|err| utils::command_err("tar アーカイブへ追加できませんでした", err))?;
    tar.finish()
        .map_err(|err| utils::command_err("tar アーカイブを確定できませんでした", err))?;
    Ok(())
}

/// `VRChat` の命名規則に合致するソースログファイルを収集する。
///
/// # 引数
/// * `source_dir` - Polaris と共有する不変の `VRChat` ログディレクトリ。
///
/// # 戻り値
/// ファイルシステム列挙順でソートされたソース `.txt` ログパス。
fn collect_source_logs(source_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries =
        fs::read_dir(source_dir).map_err(|err| utils::command_read_err(source_dir, err))?;
    let mut paths = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                utils::log_warn(&format!(
                    "Polaris 元ログ内の項目を読み取れませんでした: {err}"
                ));
                continue;
            }
        };

        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let is_txt_log = Path::new(name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"));
        if path.is_file() && name.starts_with("output_log_") && is_txt_log {
            paths.push(path);
        }
    }

    paths.sort();
    Ok(paths)
}

/// ソースログ1件を管理アーカイブストアにどう反映すべきかを記述する。
pub(crate) struct ArchiveSyncPlan {
    source_path: PathBuf,
    archive_path: PathBuf,
    mode: ArchiveSyncMode,
}

/// 初回アーカイブ作成と安全なアーカイブ置き換えを区別する。
#[derive(Clone, Copy)]
pub(crate) enum ArchiveSyncMode {
    Create,
    Replace,
}

/// バッファを可能な限り満たす。EOF に到達した場合は読めた分だけ返す。
fn fill_buf(reader: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut pos = 0;
    while pos < buf.len() {
        match reader.read(&mut buf[pos..])? {
            0 => break,
            n => pos += n,
        }
    }
    Ok(pos)
}

/// 2つのリーダーを先頭からチャンク単位で比較する。
///
/// `reader_a` を EOF まで読み、同量のバイトを `reader_b` から読んで一致を確認する。
/// `reader_a` の全バイトが `reader_b` の先頭と一致すれば `Ok(true)` を返す。
fn streaming_prefix_equal(
    reader_a: &mut impl Read,
    reader_b: &mut impl Read,
) -> std::io::Result<bool> {
    const CHUNK: usize = 64 * 1024;
    let mut buf_a = vec![0u8; CHUNK];
    let mut buf_b = vec![0u8; CHUNK];
    loop {
        let na = fill_buf(reader_a, &mut buf_a)?;
        if na == 0 {
            return Ok(true);
        }
        let nb = fill_buf(reader_b, &mut buf_b[..na])?;
        if na != nb || buf_a[..na] != buf_b[..nb] {
            return Ok(false);
        }
    }
}

/// ソースログ1件に新規または更新 `.tar.zst` アーカイブが必要か計画する。
///
/// 既存アーカイブはソースログがアーカイブ済みバイト列を厳密に拡張している
/// 場合のみ置き換える。履歴が乖離している場合は警告をログに記録しスキップする。
fn build_archive_sync_plan(
    source_path: &Path,
    archive_store_dir: &Path,
) -> Result<Option<ArchiveSyncPlan>, String> {
    let file_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "ファイル名を解決できませんでした [{}]",
                source_path.display()
            )
        })?;
    let archive_path = archive_store_dir.join(format!("{file_name}.tar.zst"));
    if !archive_path.exists() {
        return Ok(Some(ArchiveSyncPlan {
            source_path: source_path.to_path_buf(),
            archive_path,
            mode: ArchiveSyncMode::Create,
        }));
    }

    // アーカイブエントリのサイズを tar ヘッダーから取得し、ソースファイルサイズと比較する。
    // 全内容をメモリに読み込まず、ストリーミングでバイト比較を行う。
    let archive_file =
        fs::File::open(&archive_path).map_err(|err| utils::command_open_err(&archive_path, err))?;
    let decoder = zstd::stream::Decoder::new(archive_file)
        .map_err(|err| utils::command_err("zstd デコーダーを初期化できませんでした", err))?;
    let mut archive = tar::Archive::new(decoder);
    let mut entries = archive
        .entries()
        .map_err(|err| utils::command_err("zst エントリ一覧を取得できませんでした", err))?;
    let Some(entry_result) = entries.next() else {
        return Err(format!(
            "アーカイブ内にログファイルがありません: {}",
            archive_path.display()
        ));
    };
    let mut entry =
        entry_result.map_err(|err| utils::command_err("zst エントリを読み取れませんでした", err))?;
    let archived_size = entry
        .header()
        .size()
        .map_err(|err| utils::command_err("アーカイブエントリのサイズを取得できませんでした", err))?;

    let mut source_file = open_source_log_for_read(source_path)?;
    let source_size = source_file
        .metadata()
        .map_err(|err| utils::command_read_err(source_path, err))?
        .len();

    if source_size < archived_size {
        utils::log_warn(&format!(
            "Data の zst を更新しません。元ログが既存アーカイブより小さいためです [{file_name}]"
        ));
        return Ok(None);
    }

    if source_size == archived_size {
        let equal = streaming_prefix_equal(&mut entry, &mut source_file)
            .map_err(|err| utils::command_read_err(source_path, err))?;
        if !equal {
            utils::log_warn(&format!(
                "Data の zst を更新しません。同サイズですが内容が異なります [{file_name}]"
            ));
        }
        return Ok(None);
    }

    // source_size > archived_size: ソースがアーカイブ済み内容を厳密に拡張しているか確認
    let extends = streaming_prefix_equal(&mut entry, &mut source_file)
        .map_err(|err| utils::command_read_err(source_path, err))?;
    if !extends {
        utils::log_warn(&format!(
            "Data の zst を更新しません。元ログが既存アーカイブを素直に拡張していません [{file_name}]"
        ));
        return Ok(None);
    }

    Ok(Some(ArchiveSyncPlan {
        source_path: source_path.to_path_buf(),
        archive_path,
        mode: ArchiveSyncMode::Replace,
    }))
}

/// `Data` でアーカイブまたは更新が必要なソースログを収集する。
///
/// # 引数
/// * `source_dir` - Polaris と共有する不変の `VRChat` ログディレクトリ。
/// * `archive_store_dir` - `StellaRecord` が管理する `.tar.zst` ファイルの格納ディレクトリ。
///
/// # 戻り値
/// 新規または安全に置き換え可能なログのアーカイブ同期計画（順序付き）。
pub(crate) fn collect_pending_archive_sync_plans(
    source_dir: &Path,
    archive_store_dir: &Path,
) -> Result<Vec<ArchiveSyncPlan>, String> {
    let mut plans = Vec::new();
    for source_path in collect_source_logs(source_dir)? {
        if let Some(plan) = build_archive_sync_plan(&source_path, archive_store_dir)? {
            plans.push(plan);
        }
    }
    Ok(plans)
}

/// ファイルまたはディレクトリツリーの合計サイズを再帰的に計算する。
///
/// ストレージパネルは単純な合計サイズを必要とするため、アーカイブ
/// ディレクトリを走査し、読み取り不可の子は警告付きで許容する。
fn collect_directory_size(path: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];

    while let Some(current) = stack.pop() {
        let metadata = fs::metadata(&current).map_err(|err| {
            utils::command_err(
                &format!("メタデータを取得できませんでした [{}]", current.display()),
                err,
            )
        })?;

        if metadata.is_file() {
            total += metadata.len();
            continue;
        }

        let entries =
            fs::read_dir(&current).map_err(|err| utils::command_read_err(&current, err))?;

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    utils::log_warn(&format!("ストレージ項目を読み取れませんでした: {err}"));
                    continue;
                }
            };

            stack.push(entry.path());
        }
    }

    Ok(total)
}

/// 一時ファイルとバックアップ復元パスを使用して対象ファイルを置き換える。
///
/// 長い生ログからアーカイブを再圧縮する際に使用し、最終リネームが
/// 失敗しても既存アーカイブが失われないようにする。
fn replace_file_atomically(temp_path: &Path, target_path: &Path) -> Result<(), String> {
    let backup_path = target_path.with_extension("bak");
    if backup_path.exists() {
        fs::remove_file(&backup_path)
            .map_err(|err| utils::command_remove_err(&backup_path, err))?;
    }

    fs::rename(target_path, &backup_path).map_err(|err| {
        utils::command_err(
            &format!(
                "バックアップへ退避できませんでした [{}]",
                target_path.display()
            ),
            err,
        )
    })?;

    if let Err(err) = fs::rename(temp_path, target_path) {
        if let Err(restore_err) = fs::rename(&backup_path, target_path) {
            utils::log_warn(&format!(
                "バックアップの復元に失敗しました [{} -> {}]: {}",
                backup_path.display(),
                target_path.display(),
                restore_err
            ));
        }
        return Err(utils::command_err(
            &format!("置き換えに失敗しました [{}]", target_path.display()),
            err,
        ));
    }

    if let Err(err) = fs::remove_file(&backup_path) {
        utils::log_warn(&format!(
            "バックアップの削除に失敗しましたが、アーカイブの置き換えは成功しています [{}]: {}",
            backup_path.display(),
            err
        ));
    }
    Ok(())
}

/// ソースログを管理された `Data` アーカイブストアに同期する。
///
/// 元ログは削除もリネームもしない。`StellaRecord` は自身の `.tar.zst`
/// コピーの作成またはアトミック置き換えのみ行う。
pub(crate) fn sync_source_logs_into_archive_store(
    source_dir: &Path,
    archive_store_dir: &Path,
) -> Result<usize, String> {
    fs::create_dir_all(archive_store_dir)
        .map_err(|err| utils::command_create_err(archive_store_dir, err))?;

    let plans = collect_pending_archive_sync_plans(source_dir, archive_store_dir)?;
    for plan in &plans {
        match plan.mode {
            ArchiveSyncMode::Create => {
                compress_single_file(&plan.source_path, &plan.archive_path)?;
            }
            ArchiveSyncMode::Replace => {
                let temp_path = plan.archive_path.with_extension("tmp");
                compress_single_file(&plan.source_path, &temp_path)?;
                replace_file_atomically(&temp_path, &plan.archive_path)?;
            }
        }
    }

    Ok(plans.len())
}

/// ビューアの重要度レベルをチャンク転送用のコンパクトな `u8` にエンコードする。
///
/// フロントエンドがこれを CSS クラスにデコードするため、数値マッピングは
/// TypeScript のログビューア定数と同期を保つ必要がある。
fn encode_log_level_u8(level: &str) -> u8 {
    match level {
        "info" => 1,
        "warning" => 2,
        "error" => 3,
        "debug" => 4,
        _ => 0,
    }
}

/// ビューアのカテゴリタグをチャンク転送用のコンパクトな `u8` にエンコードする。
///
/// カテゴリはログビューア UI のフィルタサイドバーと対応する。
/// DB に登録される情報を中心に番号を振り、`debug-system` のみ複数行
/// デバッグブロックの範囲タグとして残している。
fn encode_log_category_u8(category: &str) -> u8 {
    match category {
        "world" => 1,
        "notification" => 2,
        "player_join" => 3,
        "player_ready" => 4,
        "player_left" => 5,
        "video" => 6,
        "debug-system" => 7,
        _ => 0,
    }
}

/// `.tar.zst` アーカイブの最初の tar エントリのソースファイル名だけを取得する。
///
/// アーカイブ内容は展開せず、ヘッダーのみ読み取る。
fn read_archive_source_name(archive_path: &Path) -> Result<String, String> {
    let file =
        fs::File::open(archive_path).map_err(|err| utils::command_open_err(archive_path, err))?;
    let decoder = zstd::stream::Decoder::new(file)
        .map_err(|err| utils::command_err("zstd デコーダーを初期化できませんでした", err))?;
    let mut archive = tar::Archive::new(decoder);
    let mut entries = archive
        .entries()
        .map_err(|err| utils::command_err("zst エントリ一覧を取得できませんでした", err))?;
    let Some(entry) = entries.next() else {
        return Err(format!(
            "アーカイブ内にログファイルがありません: {}",
            archive_path.display()
        ));
    };
    let entry =
        entry.map_err(|err| utils::command_err("zst エントリを読み取れませんでした", err))?;
    let entry_path = entry
        .path()
        .map_err(|err| utils::command_err("zst エントリパスを解決できませんでした", err))?;
    entry_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "アーカイブ内のファイル名を解決できませんでした".to_string())
        .map(str::to_string)
}

/// ログビューア行分類用に DB 内容から導出されたマーカーテキスト。
#[derive(Clone)]
struct DbKeywordMarker {
    category: String,
    text: String,
}

/// 生ログ行を粗いビューア重要度レベルに分類する。
fn classify_log_level(line: &str) -> String {
    if line.contains("[UserInfoLogger] Environment Info:") {
        return "debug".to_string();
    }
    if line.contains("[UserInfoLogger] User Settings Info:") {
        return "debug".to_string();
    }
    if line.contains("Microphones installed (") {
        return "debug".to_string();
    }
    if line.contains(" Error ") || line.contains("Error      -") {
        return "error".to_string();
    }
    if line.contains(" Warning ") || line.contains("Warning    -") {
        return "warning".to_string();
    }
    if line.contains(" Debug ") || line.contains("Debug      -") {
        return "debug".to_string();
    }

    "plain".to_string()
}

/// 正規化されたデータベース行からタイムスタンプ→カテゴリのマッピングを収集する。
///
/// ビューアカテゴリごとに1つの SQL ブロックを意図的に維持し、
/// 格納データとハイライト対象ログ行の関係を監査しやすくする。
fn collect_db_log_categories(
    conn: &rusqlite::Connection,
    source_name: &str,
) -> Result<HashMap<String, Vec<String>>, String> {
    let mut categories: HashMap<String, Vec<String>> = HashMap::new();

    // DB に格納済みのカテゴリだけを収集する。動画 URL は現在 DB に持たないため
    // `video` カテゴリは emit 側で正規表現マーカーから合成する。
    let sql = "
        SELECT join_time, 'world'
          FROM visits
         WHERE session_id IN (SELECT id FROM sessions WHERE log_name = ?1)
        UNION ALL
        SELECT leave_time, 'world'
          FROM visits
         WHERE leave_time IS NOT NULL
           AND session_id IN (SELECT id FROM sessions WHERE log_name = ?1)
        UNION ALL
        SELECT wu.join_time, 'player_join'
          FROM with_users wu
          JOIN visits v ON v.id = wu.visit_id
          JOIN sessions s ON s.id = v.session_id
         WHERE s.log_name = ?1 AND wu.is_self = 0
        UNION ALL
        SELECT wu.leave_time, 'player_left'
          FROM with_users wu
          JOIN visits v ON v.id = wu.visit_id
          JOIN sessions s ON s.id = v.session_id
         WHERE s.log_name = ?1 AND wu.is_self = 0 AND wu.leave_time IS NOT NULL
        UNION ALL
        SELECT received_at, 'notification'
          FROM notifications
         WHERE session_id IN (SELECT id FROM sessions WHERE log_name = ?1)
    ";

    let mut stmt = conn
        .prepare(sql)
        .map_err(|err| utils::command_err("ログビューア用カテゴリクエリを準備できませんでした", err))?;

    let rows = stmt
        .query_map([source_name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| utils::command_err("ログビューア用カテゴリクエリを実行できませんでした", err))?;

    for row in rows {
        match row {
            Ok((timestamp, category)) => {
                categories.entry(timestamp).or_default().push(category);
            }
            Err(err) => utils::log_warn(&format!(
                "ログビューア行をデコードできませんでした: {err}"
            )),
        }
    }

    Ok(categories)
}

/// キーワードベースの行マッチング用に DB 行から表示名と URL 断片を収集する。
///
/// マーカーは最長優先でソートし、行に複数の候補文字列が含まれる場合
/// （例: ワールド名の部分文字列であるプレイヤー名）に最も具体的なマッチが
/// 優先されるようにする。
fn collect_db_keyword_markers(
    conn: &rusqlite::Connection,
    source_name: &str,
) -> Result<Vec<DbKeywordMarker>, String> {
    let mut markers = Vec::new();

    let sql = "
        SELECT world_name, 'world'
          FROM visits
         WHERE session_id IN (SELECT id FROM sessions WHERE log_name = ?1)
           AND world_name IS NOT NULL AND trim(world_name) <> ''
        UNION ALL
        SELECT fu.account_name, 'player_join'
          FROM with_users wu
          JOIN find_users fu ON fu.vrchat_id = wu.vrchat_id
          JOIN visits v ON v.id = wu.visit_id
          JOIN sessions s ON s.id = v.session_id
         WHERE s.log_name = ?1
           AND fu.account_name IS NOT NULL AND trim(fu.account_name) <> ''
        UNION ALL
        SELECT fu.account_name, 'player_left'
          FROM with_users wu
          JOIN find_users fu ON fu.vrchat_id = wu.vrchat_id
          JOIN visits v ON v.id = wu.visit_id
          JOIN sessions s ON s.id = v.session_id
         WHERE s.log_name = ?1 AND wu.leave_time IS NOT NULL
           AND fu.account_name IS NOT NULL AND trim(fu.account_name) <> ''
        UNION ALL
        SELECT message, 'notification'
          FROM notifications
         WHERE session_id IN (SELECT id FROM sessions WHERE log_name = ?1)
           AND message IS NOT NULL AND trim(message) <> ''
        UNION ALL
        SELECT sender_name, 'notification'
          FROM notifications
         WHERE session_id IN (SELECT id FROM sessions WHERE log_name = ?1)
           AND sender_name IS NOT NULL AND trim(sender_name) <> ''
    ";

    let mut stmt = conn
        .prepare(sql)
        .map_err(|err| utils::command_err("キーワード抽出クエリを準備できませんでした", err))?;

    let rows = stmt
        .query_map([source_name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| utils::command_err("キーワード抽出クエリを実行できませんでした", err))?;

    for row in rows {
        match row {
            Ok((text, category)) => {
                let trimmed = text.trim();
                if trimmed.len() >= 2 {
                    markers.push(DbKeywordMarker {
                        category,
                        text: trimmed.to_string(),
                    });
                }
            }
            Err(err) => utils::log_warn(&format!(
                "ログビューア用キーワードをデコードできませんでした: {err}"
            )),
        }
    }

    // 最長優先ソートにより、行スキャン時に最も具体的なキーワードが優先される。
    markers.sort_by(|left, right| right.text.len().cmp(&left.text.len()));
    markers.dedup_by(|left, right| left.category == right.category && left.text == right.text);
    Ok(markers)
}

/// 生行とタイムスタンプから DB 登録済みカテゴリを解決する。
///
/// DB タイムスタンプが権威で、同一秒に複数カテゴリがある場合は行内容で
/// タイブレークする。`player_ready` は DB に直接列を持たないが、訪問への
/// 参加完了として `player_join` の派生扱いにする。
fn resolve_db_category(
    line: &str,
    timestamp: &str,
    db_categories: &HashMap<String, Vec<String>>,
) -> Option<String> {
    let matched = db_categories.get(timestamp)?;

    let preferred = if analyze::RE_PLAYER_JOIN_COMPLETE.is_match(line) {
        // OnPlayerJoinComplete は join_time と同一秒に出力されることが多いので
        // player_join のヒットに便乗して player_ready を別カテゴリとして付与する。
        if matched.iter().any(|category| category == "player_join") {
            return Some("player_ready".to_string());
        }
        None
    } else if analyze::RE_PLAYER_JOIN.is_match(line) {
        Some("player_join")
    } else if analyze::RE_PLAYER_LEFT.is_match(line) {
        Some("player_left")
    } else if analyze::RE_NOTIFICATION.is_match(line) {
        Some("notification")
    } else if line.contains("[Behaviour] Entering Room:") || line.contains("[Behaviour] OnLeftRoom")
    {
        Some("world")
    } else {
        None
    };

    if let Some(expected) = preferred {
        if matched.iter().any(|category| category == expected) {
            return Some(expected.to_string());
        }
    }

    matched.first().cloned()
}

/// 生ログ行に含まれる最初のキーワードマーカーを検索する。
fn resolve_db_keyword_marker<'a>(
    line: &str,
    db_keyword_markers: &'a [DbKeywordMarker],
) -> Option<&'a DbKeywordMarker> {
    db_keyword_markers
        .iter()
        .find(|marker| line.contains(marker.text.as_str()))
}

/// 複数行にまたがる DB 関連ブロックを継続中であることを表す。
///
/// VRChat ログでは大半の行が独立したタイムスタンプを持つが、一部のレコードは
/// 複数行に渡る (UserInfo デバッグブロックや、稀に改行を含む通知ペイロード)。
/// 範囲全体に同じカテゴリを付与してフィルタチップで一括選択できるようにする。
#[derive(Clone, Copy)]
enum RangeBlockKind {
    /// `[UserInfoLogger]` 系の 4 スペースインデント継続ブロック。
    DebugSystem,
    /// `Received Notification: <Notification ...>` が改行を含むケース。
    /// 行内に閉じ `>` が出現するまで継続する。
    Notification,
}

/// 進行中の範囲ブロックの状態。
struct RangeBlock {
    kind: RangeBlockKind,
    category: &'static str,
}

/// 行が新しい範囲ブロックを開始するなら、その種類とカテゴリを返す。
fn detect_range_block_start(line: &str) -> Option<RangeBlock> {
    if line.contains("[UserInfoLogger] Environment Info:")
        || line.contains("[UserInfoLogger] User Settings Info:")
        || line.contains("Microphones installed (")
    {
        return Some(RangeBlock {
            kind: RangeBlockKind::DebugSystem,
            category: "debug-system",
        });
    }
    if line.contains("Received Notification: <Notification") && !line.contains('>') {
        return Some(RangeBlock {
            kind: RangeBlockKind::Notification,
            category: "notification",
        });
    }
    None
}

/// アーカイブログテキストと DB ヒントからカテゴリ付きログビューア行を構築する。
///
/// 生ログ内容と正規化 DB データをマージし、TypeScript でログ全体を
/// 再パースせずに UI が重要な行をハイライトできるようにする。
///
/// ハイライトは DB に登録されたキーワードにマッチした場合のみ付与し、
/// 正規表現のみで検出される断片はハイライトしない。複数行にわたる DB
/// レコード（UserInfo ブロック、複数行通知、動画ロードシーケンス）は
/// 範囲全体に同一カテゴリを付与し、フィルタチップで一括選択できるようにする。
fn emit_log_viewer_chunks(
    reader: impl BufRead,
    session_id: String,
    db_categories: Option<HashMap<String, Vec<String>>>,
    db_keyword_markers: Option<Vec<DbKeywordMarker>>,
    app: AppHandle,
) {
    // 1イベントあたり500行で IPC オーバーヘッドを抑えつつ UI への更新を途絶えさせない。
    const CHUNK_SIZE: usize = 500;

    let mut timestamps: Vec<String> = Vec::with_capacity(CHUNK_SIZE);
    let mut levels: Vec<u8> = Vec::with_capacity(CHUNK_SIZE);
    let mut categories: Vec<u8> = Vec::with_capacity(CHUNK_SIZE);
    let mut raw_lines: Vec<String> = Vec::with_capacity(CHUNK_SIZE);
    let mut highlights: Vec<Option<String>> = Vec::with_capacity(CHUNK_SIZE);

    let flush = |ts: &mut Vec<String>,
                 lv: &mut Vec<u8>,
                 cat: &mut Vec<u8>,
                 rl: &mut Vec<String>,
                 hl: &mut Vec<Option<String>>,
                 sid: &str,
                 app: &AppHandle| -> bool {
        app.emit(
            "log_viewer_chunk",
            &LogViewerChunk {
                session_id: sid.to_string(),
                timestamps: ts.drain(..).collect(),
                levels: lv.drain(..).collect(),
                categories: cat.drain(..).collect(),
                raw_lines: rl.drain(..).collect(),
                highlights: hl.drain(..).collect(),
            },
        )
        .is_ok()
    };

    let mut active_range: Option<RangeBlock> = None;
    for line in reader.lines().map_while(Result::ok) {
        // 既存の範囲ブロックの終了判定。終了行は範囲外扱いで、終了が成立した時点で
        // 範囲を解除してから当該行を分類する（インデント解除した行は範囲に含めない）。
        if let Some(block) = active_range.as_ref() {
            let should_end = match block.kind {
                RangeBlockKind::DebugSystem => !line.starts_with("    "),
                // Notification は閉じ `>` を含む行を範囲の最終行として含めるため
                // ここでは終了しない（後段で `>` 検出後に解除する）。
                RangeBlockKind::Notification => false,
            };
            if should_end {
                active_range = None;
            }
        }

        // 範囲未開始のときのみ新規範囲ブロックの開始を検出する。
        if active_range.is_none() {
            active_range = detect_range_block_start(&line);
        }

        let timestamp = analyze::RE_TIME
            .captures(&line)
            .and_then(|caps| caps.get(1))
            .and_then(|m| {
                NaiveDateTime::parse_from_str(m.as_str(), "%Y.%m.%d %H:%M:%S")
                    .ok()
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            })
            .unwrap_or_default();

        // 範囲ブロック内では debug-system 由来なら debug 固定。
        // Notification 範囲はタイムスタンプ付き行の本来のレベルを尊重する。
        let level = if matches!(
            active_range.as_ref().map(|b| b.kind),
            Some(RangeBlockKind::DebugSystem)
        ) {
            "debug".to_string()
        } else {
            classify_log_level(&line)
        };

        // DB キーワードマーカーを引いてハイライトとカテゴリを決定する。
        let keyword_marker = if timestamp.is_empty() {
            None
        } else {
            db_keyword_markers
                .as_deref()
                .and_then(|markers| resolve_db_keyword_marker(&line, markers))
        };

        // カテゴリ優先順位:
        //   1. 範囲ブロックが進行中ならその範囲カテゴリ（複数行を同じカテゴリに統一）
        //   2. DB キーワードマーカーのカテゴリ
        //   3. DB タイムスタンプヒントのカテゴリ
        //   4. 上記いずれも該当しなければ "plain"
        let category = active_range
            .as_ref()
            .map(|block| block.category.to_string())
            .or_else(|| keyword_marker.map(|marker| marker.category.clone()))
            .or_else(|| {
                db_categories
                    .as_ref()
                    .and_then(|cat_map| resolve_db_category(&line, &timestamp, cat_map))
            })
            .unwrap_or_else(|| "plain".to_string());

        // ハイライトは DB マーカーがマッチした場合のみ。正規表現フォールバックは持たない。
        let highlight_text = keyword_marker.map(|marker| marker.text.clone());

        // Notification 範囲は閉じ `>` を含む行を最終行として範囲を解除する。
        if matches!(
            active_range.as_ref().map(|b| b.kind),
            Some(RangeBlockKind::Notification)
        ) && line.contains('>')
        {
            active_range = None;
        }

        timestamps.push(timestamp);
        levels.push(encode_log_level_u8(&level));
        categories.push(encode_log_category_u8(&category));
        raw_lines.push(line);
        highlights.push(highlight_text);

        if raw_lines.len() >= CHUNK_SIZE {
            if !flush(
                &mut timestamps,
                &mut levels,
                &mut categories,
                &mut raw_lines,
                &mut highlights,
                &session_id,
                &app,
            ) {
                return;
            }
        }
    }

    if !raw_lines.is_empty() {
        flush(
            &mut timestamps,
            &mut levels,
            &mut categories,
            &mut raw_lines,
            &mut highlights,
            &session_id,
            &app,
        );
    }

    app.emit("log_viewer_done", &session_id).ok();
}

/// インポート可能なアーカイブ済み `.tar.zst` ファイルを一覧表示する。
///
/// # エラー
/// アーカイブディレクトリを読み取れない場合にエラーを返す。
#[tauri::command]
pub fn list_archive_files() -> Result<Vec<ArchiveFileItem>, String> {
    let zst_dir = get_archive_store_dir()?;
    let mut files = Vec::new();

    if !zst_dir.exists() {
        return Ok(files);
    }

    let entries = fs::read_dir(&zst_dir).map_err(|err| utils::command_read_err(&zst_dir, err))?;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                utils::log_warn(&format!("archive 内の項目を読み取れませんでした: {err}"));
                continue;
            }
        };

        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            if path.is_file() && name.ends_with(".tar.zst") {
                let size_bytes = match entry.metadata() {
                    Ok(metadata) => metadata.len(),
                    Err(err) => {
                        utils::log_warn(&format!(
                            "archive メタデータを読み取れませんでした [{}]: {}",
                            path.display(),
                            err
                        ));
                        0
                    }
                };
                files.push(ArchiveFileItem {
                    name: name.to_string(),
                    size_bytes,
                });
            }
        }
    }

    files.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(files)
}

/// ソースログを管理された `Data` ディレクトリにコピー・圧縮する。
///
/// # エラー
/// ソースログディレクトリが存在しない、または同期に失敗した場合にエラーを返す。
#[tauri::command]
pub fn compress_logs() -> Result<String, String> {
    let source_dir = get_source_log_dir()?;
    let archive_store_dir = get_archive_store_dir()?;
    let count = sync_source_logs_into_archive_store(&source_dir, &archive_store_dir)?;

    Ok(format!(
        "完了しました。{count}件のログを Data に圧縮同期しました。"
    ))
}

/// ソース名で DB の正規化済みヒント（カテゴリ・キーワード）を取得する。
///
/// DB が存在しない・該当セッションが未取り込みなど、ヒントが得られないケースは
/// 警告ログを出して `(None, None)` を返し、ビューアは正規表現ベースの分類に
/// フォールバックする。
#[allow(clippy::type_complexity)]
fn build_db_hints(
    source_name: &str,
) -> (
    Option<HashMap<String, Vec<String>>>,
    Option<Vec<DbKeywordMarker>>,
) {
    match get_db_path() {
        Ok(db_path) if db_path.exists() => match rusqlite::Connection::open(&db_path) {
            Ok(conn) => {
                let categories = match collect_db_log_categories(&conn, source_name) {
                    Ok(c) => Some(c),
                    Err(err) => {
                        utils::log_warn(&format!(
                            "ログビューア用カテゴリを読み込めませんでした: {err}"
                        ));
                        None
                    }
                };
                let keyword_markers = match collect_db_keyword_markers(&conn, source_name) {
                    Ok(m) => Some(m),
                    Err(err) => {
                        utils::log_warn(&format!(
                            "ログビューア用キーワードを読み込めませんでした: {err}"
                        ));
                        None
                    }
                };
                (categories, keyword_markers)
            }
            Err(err) => {
                utils::log_warn(&format!(
                    "ログビューア用 DB を開けませんでした [{}]: {}",
                    db_path.display(),
                    err
                ));
                (None, None)
            }
        },
        Ok(_) => (None, None),
        Err(err) => {
            utils::log_warn(&format!(
                "ログビューア用 DB パスを解決できませんでした: {err}"
            ));
            (None, None)
        }
    }
}

/// 圧縮済み `.tar.zst` ログを別スレッドでストリーミングしてビューアイベントを送出する。
fn spawn_compressed_log_stream(
    archive_path: PathBuf,
    session_id: String,
    db_categories: Option<HashMap<String, Vec<String>>>,
    db_keyword_markers: Option<Vec<DbKeywordMarker>>,
    app: AppHandle,
) {
    std::thread::spawn(move || {
        let app_fallback = app.clone();
        let sid_fallback = session_id.clone();
        let result: Result<(), String> = (|| {
            let file = fs::File::open(&archive_path)
                .map_err(|err| utils::command_open_err(&archive_path, err))?;
            let decoder = zstd::stream::Decoder::new(file)
                .map_err(|err| utils::command_err("zstd デコーダーを初期化できませんでした", err))?;
            let mut archive = tar::Archive::new(decoder);
            let mut entries = archive
                .entries()
                .map_err(|err| utils::command_err("zst エントリ一覧を取得できませんでした", err))?;
            let Some(entry_result) = entries.next() else {
                return Err(format!(
                    "アーカイブ内にログファイルがありません: {}",
                    archive_path.display()
                ));
            };
            let mut entry = entry_result
                .map_err(|err| utils::command_err("zst エントリを読み取れませんでした", err))?;
            emit_log_viewer_chunks(
                BufReader::new(&mut entry),
                session_id,
                db_categories,
                db_keyword_markers,
                app,
            );
            Ok(())
        })();
        if let Err(err) = result {
            utils::log_warn(&format!("ログビューアストリーム失敗: {err}"));
            app_fallback.emit("log_viewer_done", &sid_fallback).ok();
        }
    });
}

/// プレーンテキスト `.txt` ログを別スレッドでストリーミングしてビューアイベントを送出する。
fn spawn_plain_log_stream(
    log_path: PathBuf,
    session_id: String,
    db_categories: Option<HashMap<String, Vec<String>>>,
    db_keyword_markers: Option<Vec<DbKeywordMarker>>,
    app: AppHandle,
) {
    std::thread::spawn(move || {
        let app_fallback = app.clone();
        let sid_fallback = session_id.clone();
        let result: Result<(), String> = (|| {
            let file = open_source_log_for_read(&log_path)?;
            emit_log_viewer_chunks(
                BufReader::new(file),
                session_id,
                db_categories,
                db_keyword_markers,
                app,
            );
            Ok(())
        })();
        if let Err(err) = result {
            utils::log_warn(&format!("ログビューアストリーム失敗: {err}"));
            app_fallback.emit("log_viewer_done", &sid_fallback).ok();
        }
    });
}

/// アーカイブファイル1件のストリーミングログビューアセッションを開く。
///
/// メタデータを即座に返し、バックグラウンドスレッドから `log_viewer_chunk` /
/// `log_viewer_done` イベントを送出して UI が行を漸進的に描画できるようにする。
///
/// # エラー
/// アーカイブファイルが見つからない、またはデコードできない場合にエラーを返す。
#[tauri::command]
pub fn read_archive_log_viewer(
    file_name: String,
    session_id: String,
    app: AppHandle,
) -> Result<LogViewerMeta, String> {
    let archive_path = get_archive_store_dir()?.join(&file_name);
    if !archive_path.exists() {
        return Err(format!("ファイルが見つかりません: {file_name}"));
    }

    let source_name = read_archive_source_name(&archive_path)?;
    let (db_categories, db_keyword_markers) = build_db_hints(&source_name);

    // セッション ID により、前回のストリームがまだ飛行中にユーザーが別ファイルに
    // 切り替えた場合、フロントエンドが古いチャンクイベントを破棄できる。
    spawn_compressed_log_stream(
        archive_path,
        session_id.clone(),
        db_categories,
        db_keyword_markers,
        app,
    );

    Ok(LogViewerMeta {
        session_id,
        archive_name: file_name,
        source_name,
    })
}

/// `Data` で管理アーカイブがまだ必要なソースログの件数を数える。
///
/// # エラー
/// ソースログまたは `Data` ディレクトリパスを解決できない場合にエラーを返す。
#[tauri::command]
pub fn get_pending_archive_log_count() -> Result<usize, String> {
    let source_dir = get_source_log_dir()?;
    let archive_store_dir = get_archive_store_dir()?;
    Ok(collect_pending_archive_sync_plans(&source_dir, &archive_store_dir)?.len())
}

/// 現在のアーカイブディレクトリサイズと設定済み上限値を算出する。
///
/// # エラー
/// アーカイブディレクトリパスを解決またはスキャンできない場合にエラーを返す。
#[tauri::command]
pub fn get_storage_status() -> Result<(u64, u64), String> {
    let archive_dir = get_archive_store_dir()?;
    let setting = config::load_polaris_setting();

    let total_size = if archive_dir.exists() {
        collect_directory_size(&archive_dir)?
    } else {
        0
    };

    Ok((total_size, setting.capacity_threshold_bytes))
}

/// 指定されたフォルダパスを OS シェルで開く。
///
/// # エラー
/// フォルダを開けない場合にエラーを返す。
#[tauri::command]
pub fn open_folder(path: &str) -> Result<(), String> {
    opener::open(path).map_err(|err| utils::command_err("フォルダを開けませんでした", err))
}

/// `.tar.zst` アーカイブが確認済みで安全に削除可能なソースログファイルを一覧表示する。
///
/// # エラー
/// ソースログディレクトリまたはアーカイブディレクトリを読み取れない場合にエラーを返す。
#[tauri::command]
pub fn get_deletable_source_logs() -> Result<Vec<DeletableLogInfo>, String> {
    let source_dir = get_source_log_dir()?;
    let archive_dir = get_archive_store_dir()?;

    let entries = fs::read_dir(&source_dir)
        .map_err(|err| format!("ソースログディレクトリの読み取りに失敗しました: {err}"))?;

    let mut results: Vec<DeletableLogInfo> = entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if !path.is_file() {
                return None;
            }
            let name = path.file_name()?.to_str()?.to_string();
            if !name.starts_with("output_log_") || !name.ends_with(".txt") {
                return None;
            }
            let archive_path = archive_dir.join(format!("{name}.tar.zst"));
            if !archive_path.exists() {
                return None;
            }
            let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
            Some(DeletableLogInfo { file_name: name, size_bytes })
        })
        .collect();

    results.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(results)
}

/// 外部フォルダのファイル名がログビューア対応フォーマットに合致するか判定する。
///
/// VRChat の `output_log_*` 命名を満たし、生ログ `.txt` または `StellaRecord` の
/// `.tar.zst` アーカイブ拡張子を持つ場合のみ受け入れる。
fn matches_external_log_format(name: &str) -> bool {
    if !name.starts_with("output_log_") {
        return false;
    }
    EXTERNAL_LOG_EXTENSIONS
        .iter()
        .any(|ext| name.ends_with(*ext))
}

/// ネイティブダイアログでユーザーにフォルダを選択させる。
///
/// # 戻り値
/// 選択されたフォルダの絶対パス、またはキャンセル時は `None`。
///
/// # エラー
/// ダイアログの初期化または表示に失敗した場合にエラーを返す。
#[tauri::command]
pub fn pick_log_folder() -> Result<Option<String>, String> {
    platform::pick_folder_dialog()
}

/// 指定フォルダ内のログビューア対応ファイル（`output_log_*.txt` / `*.tar.zst`）を一覧表示する。
///
/// # エラー
/// フォルダが存在しない、または読み取れない場合にエラーを返す。
#[tauri::command]
pub fn list_external_log_files(folder_path: String) -> Result<Vec<ArchiveFileItem>, String> {
    let dir = PathBuf::from(&folder_path);
    if !dir.is_dir() {
        return Err(format!("フォルダが見つかりません: {folder_path}"));
    }

    let entries = fs::read_dir(&dir).map_err(|err| utils::command_read_err(&dir, err))?;
    let mut files = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                utils::log_warn(&format!("外部フォルダ項目を読み取れませんでした: {err}"));
                continue;
            }
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !matches_external_log_format(name) {
            continue;
        }

        let size_bytes = match entry.metadata() {
            Ok(metadata) => metadata.len(),
            Err(err) => {
                utils::log_warn(&format!(
                    "外部ログのメタデータを読み取れませんでした [{}]: {}",
                    path.display(),
                    err
                ));
                0
            }
        };

        files.push(ArchiveFileItem {
            name: name.to_string(),
            size_bytes,
        });
    }

    files.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(files)
}

/// 外部フォルダのログファイル1件のストリーミングログビューアセッションを開く。
///
/// 既定アーカイブストアではなく任意のフォルダから `output_log_*.txt` または
/// `*.tar.zst` を直接読み込む。`.txt` はそのまま、`.tar.zst` は zstd を解凍して
/// 内部の最初のエントリを使用する。
///
/// # エラー
/// フォルダ・ファイル名が不正、ファイルが見つからない、または開けない場合にエラーを返す。
#[tauri::command]
pub fn read_external_log_viewer(
    folder_path: String,
    file_name: String,
    session_id: String,
    app: AppHandle,
) -> Result<LogViewerMeta, String> {
    if !matches_external_log_format(&file_name) {
        return Err(format!("対応していないログ形式です: {file_name}"));
    }

    let dir = PathBuf::from(&folder_path);
    if !dir.is_dir() {
        return Err(format!("フォルダが見つかりません: {folder_path}"));
    }

    // `file_name` がディレクトリ区切りを含む場合、ユーザー選択フォルダ外を指し得るので拒否する。
    if file_name.contains('/') || file_name.contains('\\') {
        return Err("ファイル名に区切り文字を含めることはできません。".to_string());
    }

    let log_path = dir.join(&file_name);
    if !log_path.is_file() {
        return Err(format!("ファイルが見つかりません: {file_name}"));
    }

    let is_compressed = file_name.ends_with(".tar.zst");
    let source_name = if is_compressed {
        read_archive_source_name(&log_path)?
    } else {
        file_name.clone()
    };

    let (db_categories, db_keyword_markers) = build_db_hints(&source_name);

    if is_compressed {
        spawn_compressed_log_stream(
            log_path,
            session_id.clone(),
            db_categories,
            db_keyword_markers,
            app,
        );
    } else {
        spawn_plain_log_stream(
            log_path,
            session_id.clone(),
            db_categories,
            db_keyword_markers,
            app,
        );
    }

    Ok(LogViewerMeta {
        session_id,
        archive_name: file_name,
        source_name,
    })
}

/// 各ファイルの `.tar.zst` アーカイブ存在を確認した上で指定ソースログを削除する。
///
/// # エラー
/// ファイル名が不正、アーカイブが存在しない、または削除に失敗した場合にエラーを返す。
#[tauri::command]
pub fn delete_source_logs(file_names: Vec<String>) -> Result<usize, String> {
    let source_dir = get_source_log_dir()?;
    let archive_dir = get_archive_store_dir()?;

    let mut deleted_count: usize = 0;

    for file_name in &file_names {
        if !file_name.starts_with("output_log_") || !file_name.ends_with(".txt") {
            return Err(format!("不正なファイル名です: {file_name}"));
        }
        let archive_path = archive_dir.join(format!("{file_name}.tar.zst"));
        if !archive_path.exists() {
            return Err(format!(
                "アーカイブが見つかりません。削除を中止しました: {file_name}.tar.zst"
            ));
        }
        let source_path = source_dir.join(file_name);
        if source_path.exists() {
            fs::remove_file(&source_path)
                .map_err(|err| format!("{file_name} の削除に失敗しました: {err}"))?;
            deleted_count += 1;
        }
    }

    Ok(deleted_count)
}
