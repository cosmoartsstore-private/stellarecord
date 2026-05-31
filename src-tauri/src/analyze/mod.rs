//! `VRChat` ログ解析パイプライン。
//!
//! アーカイブコマンドで生成された `.tar.zst` 圧縮ログを読み取り、VRChat ログの
//! 内容を行単位で解析し、正規化データをメイン `SQLite` データベース
//! (`stellarecord.db`) に書き込む。
//!
//! ファイル単位の savepoint を持つトランザクション内で書き込むため、
//! キャンセルや単一ファイルの失敗で部分データが残ることはない。

mod db;
mod parser;

pub use db::init_main_db;
pub use parser::{
    parse_access_type, parse_location, is_collectible_notification, ParsedLocation,
    RE_ENTERING, RE_IS_LOCAL, RE_JOINING, RE_LEFT_ROOM, RE_NOTIFICATION,
    RE_NOTIFICATION_WORLD_ID, RE_NOTIFICATION_WORLD_NAME, RE_OSC_FOUND,
    RE_PLAYER_JOIN, RE_PLAYER_JOIN_COMPLETE, RE_PLAYER_LEFT, RE_SCREENSHOT,
    RE_SUBSCRIPTION_STATUS, RE_TIME, RE_USER_AUTH,
};

use chrono::NaiveDateTime;
use rusqlite::{params, Connection, Result, Savepoint, Transaction};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// インポートが意図的にキャンセルされた際にユーザーに返すメッセージ。
///
/// パーサー、コマンド、UI で同一テキストを再利用し、キャンセルを
/// 汎用的なエラーではなく一貫した制御フローとして扱う。
pub const ANALYZE_CANCELED_MESSAGE: &str = "解析を中断しました。変更はロールバックされました。";

/// コンテキストラベルと元エラーから一貫した解析エラー文字列を生成する。
///
/// ユーザー向けエラーメッセージの可読性を保ちつつ、ログやコマンド応答用に
/// 元の原因テキストも保持する。
fn analyze_err<E: std::fmt::Display>(context: &str, err: E) -> String {
    format!("{context}: {err}")
}

/// 解析エラーをパーサーコードパス用の `rusqlite::Error` に変換する。
///
/// パーサーは INSERT とパースが交互に行われるため主に `SQLite` 結果を返す。
/// SQL 以外のエラーは `SQLite` 形式のエラーにラップする。
fn analyze_sqlite_err<E: std::fmt::Display>(context: &str, err: E) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(analyze_err(context, err))
}

/// 正規の `SQLite` 形式キャンセルエラーを生成する。
///
/// 専用メッセージを返すことで外層がユーザーキャンセルとパースエラーを
/// 区別し、インポート全体を確実にロールバックできる。
fn analyze_cancel_sqlite_err() -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(ANALYZE_CANCELED_MESSAGE.to_string())
}

/// 共有キャンセルフラグが設定されている場合に現在の操作を停止する。
///
/// # 引数
/// * `cancel_status` - キャンセルコマンドで更新される共有フラグ。
///
/// # 戻り値
/// 処理続行可能時は `Ok(())`、中断すべき場合は正規キャンセルメッセージ。
fn ensure_not_canceled(cancel_status: &AtomicBool) -> std::result::Result<(), String> {
    if cancel_status.load(Ordering::SeqCst) {
        Err(ANALYZE_CANCELED_MESSAGE.to_string())
    } else {
        Ok(())
    }
}

/// メインデータベースの外側トランザクションをロールバックする。
fn rollback_outer_transaction(
    main_tx: Transaction<'_>,
    context: &str,
) -> std::result::Result<(), String> {
    main_tx.rollback().map_err(|err| {
        analyze_err(
            &format!("{context}: メイン DB をロールバックできませんでした"),
            err,
        )
    })?;
    Ok(())
}

/// メインデータベースのファイル単位 savepoint をロールバックする。
///
/// 個別ファイルの解析失敗時に、そのファイルの変更のみを巻き戻す。
/// 外側トランザクションは維持されるため、他ファイルの処理を続行できる。
fn rollback_savepoint(
    mut main_sp: Savepoint<'_>,
    context: &str,
) -> std::result::Result<(), String> {
    main_sp.rollback().map_err(|err| {
        analyze_err(
            &format!("{context}: メイン DB savepoint をロールバックできませんでした"),
            err,
        )
    })?;
    Ok(())
}

/// 整数の進捗を「完了数/合計数」形式の文字列にフォーマットする。
fn format_progress_fraction(completed: usize, total: usize) -> String {
    format!("{completed}/{total}")
}


/// インポート命名規則に合致する圧縮アーカイブログを一覧取得する。
///
/// # 引数
/// * `archive_store_dir` - `StellaRecord` が管理する `.tar.zst` ログの格納ディレクトリ。
///
/// # 戻り値
/// ソート済みのアーカイブパス。不正なエントリは警告付きでスキップし、
/// インポートキュー全体をブロックしない。
fn collect_log_files(archive_store_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !archive_store_dir.exists() {
        return files;
    }
    let entries = match fs::read_dir(archive_store_dir) {
        Ok(entries) => entries,
        Err(err) => {
            crate::utils::log_warn(&format!(
                "Data ディレクトリを読み取れませんでした [{}]: {}",
                archive_store_dir.display(),
                err
            ));
            return files;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                crate::utils::log_warn(&format!("Data エントリを読み取れませんでした: {err}"));
                continue;
            }
        };
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("output_log_") && name.ends_with(".txt.tar.zst") {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    files
}

/// アーカイブパスからデータベースに格納する論理的なソース名を導出する。
///
/// # 戻り値
/// `.tar.zst` サフィックスを除いたアーカイブファイル名。パスに UTF-8 ファイル名が
/// 含まれない場合は `None`。
fn source_name_for_archive(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    Some(file_name.trim_end_matches(".tar.zst").to_string())
}

/// スクリーンショット撮影イベントをメインデータベースに挿入する。
fn insert_screenshot(
    tx: &Connection,
    visit_id: Option<i64>,
    file_path: &str,
    resolution_width: Option<i64>,
    resolution_height: Option<i64>,
    timestamp: &str,
) -> Result<()> {
    tx.execute(
        "INSERT INTO screenshots (visit_id, file_path, resolution_width, resolution_height, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![visit_id, file_path, resolution_width, resolution_height, timestamp],
    )?;
    Ok(())
}

/// OSC/OSCQuery サービスイベントをメインデータベースに挿入する。
fn insert_osc_event(
    tx: &Connection,
    session_id: i64,
    event_type: &str,
    service_name: Option<&str>,
    service_type: Option<&str>,
    ip_address: Option<&str>,
    port: Option<i64>,
    timestamp: &str,
) -> Result<()> {
    tx.execute(
        "INSERT INTO osc
         (session_id, event_type, service_name, service_type, ip_address, port, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![session_id, event_type, service_name, service_type, ip_address, port, timestamp],
    )?;
    Ok(())
}


/// `VRChat`+ サブスクリプション状態のスナップショットをメインデータベースに挿入する。
///
/// セッションごとにスナップショットは1件のみのため `INSERT OR IGNORE` を使用。
fn insert_subscription_status(
    tx: &Connection,
    session_id: i64,
    is_active: bool,
    subscription_id: Option<&str>,
    description: Option<&str>,
    checked_at: &str,
) -> Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO subscription
         (session_id, is_active, subscription_id, description, checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![session_id, is_active, subscription_id, description, checked_at],
    )?;
    Ok(())
}

/// 通知行 (1 行または複数行集約後の文字列) を `notifications` テーブルに保存する。
///
/// `Received Notification: <... message: "..." >` のうち、収集対象タイプ
/// (`is_collectible_notification` 参照) のみ INSERT する。`worldId`/`worldName`
/// のペイロードがある場合はターゲット情報も併せて保存する。
fn persist_notification(
    main_tx: &Connection,
    session_id: i64,
    ts_str: &str,
    notification_text: &str,
) -> Result<()> {
    let Some(caps) = RE_NOTIFICATION.captures(notification_text) else {
        return Ok(());
    };
    let Some(type_match) = caps.get(3) else {
        return Ok(());
    };
    let notif_type = type_match.as_str().trim().to_string();
    if !is_collectible_notification(&notif_type) {
        return Ok(());
    }

    let sender_name = caps
        .get(1)
        .map(|m| m.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let sender_user_id = caps
        .get(2)
        .map(|m| m.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let notif_id = caps.get(4).map(|m| m.as_str().to_string());
    let Some(created_match) = caps.get(5) else {
        return Ok(());
    };
    let created_at_raw = created_match.as_str().trim();
    let message = caps.get(6).map(|m| m.as_str().to_string());
    let created_at = NaiveDateTime::parse_from_str(created_at_raw, "%m/%d/%Y %H:%M:%S UTC")
        .ok()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());

    let location = RE_NOTIFICATION_WORLD_ID
        .captures(notification_text)
        .and_then(|captures| captures.get(1))
        .map(|m| parse_location(m.as_str()))
        .unwrap_or_default();
    let target_world_name = RE_NOTIFICATION_WORLD_NAME
        .captures(notification_text)
        .and_then(|captures| captures.get(1))
        .map(|m| m.as_str().to_string());

    main_tx.execute(
        "INSERT OR IGNORE INTO notifications
         (session_id, notif_id, notif_type, sender_user_id, sender_name, message, created_at, received_at,
          target_world_name, target_instance_id, target_instance_type, target_owner, target_region)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            session_id,
            notif_id,
            notif_type,
            sender_user_id,
            sender_name,
            message,
            created_at,
            ts_str,
            target_world_name.as_deref(),
            location.instance_id.as_deref(),
            location.access_type.as_deref(),
            location.instance_owner.as_deref(),
            location.region.as_deref()
        ],
    )?;
    Ok(())
}

/// 管理された `Data` ディレクトリ内のログを差分インポートする。
///
/// 外側トランザクション内でファイルごとに savepoint を切り、個々のファイル失敗で
/// バッチ全体が巻き戻ることを防ぐ。キャンセル時は外側トランザクションごと
/// ロールバックし、部分データが残らないことを保証する。
///
/// # Errors
/// データベースを開けない、または初期化できない場合にエラーを返す。
#[allow(clippy::too_many_lines)]
pub fn run_diff_import<F>(
    main_db_path: &Path,
    archive_store_dir: &Path,
    cancel_status: &AtomicBool,
    mut progress_callback: F,
) -> Result<(), String>
where
    F: FnMut(String, String),
{
    let mut main_conn = Connection::open(main_db_path).map_err(|e| {
        analyze_err(
            &format!("メイン DB を開けませんでした [{}]", main_db_path.display()),
            e,
        )
    })?;

    init_main_db(&main_conn).map_err(|e| analyze_err("メイン DB を初期化できませんでした", e))?;

    let log_files = collect_log_files(archive_store_dir);
    if log_files.is_empty() {
        progress_callback("処理対象ログなし".to_string(), "0/0".to_string());
        return Ok(());
    }

    progress_callback(
        format!("{}件のログを処理します", log_files.len()),
        format!("0/{}", log_files.len()),
    );

    let mut main_tx = main_conn
        .transaction()
        .map_err(|e| analyze_err("メイン DB トランザクションを開始できませんでした", e))?;

    let total = log_files.len();
    for (idx, log_path) in log_files.iter().enumerate() {
        if let Err(err) = ensure_not_canceled(cancel_status) {
            rollback_outer_transaction(main_tx, "解析中断時")?;
            return Err(err);
        }

        let Some(filename) = source_name_for_archive(log_path) else {
            continue;
        };

        let already_processed: bool = main_tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE log_name = ?1)",
                params![filename],
                |row| row.get(0),
            )
            .map_err(|e| analyze_err("セッション存在確認に失敗しました", e))?;

        if already_processed {
            continue;
        }

        progress_callback(
            format!("処理中: {filename}"),
            format_progress_fraction(idx, total),
        );

        let main_sp = main_tx
            .savepoint()
            .map_err(|e| analyze_err("メイン DB savepoint を開始できませんでした", e))?;

        match parse_and_import(
            &main_sp,
            log_path,
            &filename,
            cancel_status,
            &mut progress_callback,
        ) {
            Ok(()) => {
                main_sp
                    .commit()
                    .map_err(|e| analyze_err("メイン DB savepoint を確定できませんでした", e))?;
                progress_callback(
                    format!("取り込み完了: {filename}"),
                    format_progress_fraction(idx + 1, total),
                );
            }
            Err(err) if err.to_string() == ANALYZE_CANCELED_MESSAGE => {
                rollback_savepoint(main_sp, "解析中断時")?;
                rollback_outer_transaction(main_tx, "解析中断時")?;
                return Err(ANALYZE_CANCELED_MESSAGE.to_string());
            }
            Err(err) => {
                rollback_savepoint(main_sp, "ファイル単位ロールバック時")?;
                crate::utils::log_err(&format!("[StellaRecord] エラー ({filename}): {err}"));
            }
        }
    }

    if let Err(err) = ensure_not_canceled(cancel_status) {
        rollback_outer_transaction(main_tx, "解析中断時")?;
        return Err(err);
    }

    main_tx
        .commit()
        .map_err(|e| analyze_err("メイン DB 反映を確定できませんでした", e))?;

    progress_callback("処理完了".to_string(), format_progress_fraction(total, total));
    Ok(())
}

/// ログファイル（プレーンテキストまたは `.tar.zst` アーカイブ）を開き、
/// 行単位パーサーに委譲する。
///
/// `.tar.zst` が推奨アーカイブ形式: Zstandard は高速展開を提供し、tar は
/// ログを単一ファイルにまとめてディレクトリ管理を簡素化する。
/// 非圧縮 `VRChat` ログの手動インポート用にプレーンテキストのフォールバックもある。
fn parse_and_import<F>(
    main_conn: &Connection,
    log_path: &Path,
    filename: &str,
    cancel_status: &AtomicBool,
    progress_callback: &mut F,
) -> Result<()>
where
    F: FnMut(String, String),
{
    if cancel_status.load(Ordering::SeqCst) {
        return Err(analyze_cancel_sqlite_err());
    }

    if log_path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.ends_with(".tar.zst"))
    {
        let file = fs::File::open(log_path)
            .map_err(|err| analyze_sqlite_err("圧縮ログを開けませんでした", err))?;
        let decoder = zstd::stream::Decoder::new(file).map_err(|err| {
            analyze_sqlite_err("圧縮ログの zstd デコーダーを初期化できませんでした", err)
        })?;
        let mut archive = tar::Archive::new(decoder);

        let mut entries = archive.entries().map_err(|err| {
            analyze_sqlite_err("圧縮ログのエントリ一覧を取得できませんでした", err)
        })?;
        let Some(entry) = entries.next() else {
            return Err(rusqlite::Error::InvalidParameterName(
                "圧縮ログに解析対象がありませんでした".to_string(),
            ));
        };
        let mut entry =
            entry.map_err(|err| analyze_sqlite_err("圧縮ログのエントリを読めませんでした", err))?;
        return parse_and_import_reader(
            main_conn,
            BufReader::new(&mut entry),
            filename,
            cancel_status,
            progress_callback,
        );
    }

    // FILE_SHARE_READ により、VRChat が書き込み中のログでも読み取り可能にする。
    #[cfg(windows)]
    let file = {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;
        fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0)
            .open(log_path)
            .map_err(|err| analyze_sqlite_err("ログファイルを開けませんでした", err))?
    };
    #[cfg(not(windows))]
    let file = fs::File::open(log_path)
        .map_err(|err| analyze_sqlite_err("ログファイルを開けませんでした", err))?;

    parse_and_import_reader(
        main_conn,
        BufReader::new(file),
        filename,
        cancel_status,
        progress_callback,
    )
}

/// ログストリーム1件を解析し、正規化データをメインデータベースに書き込む。
///
/// 行単位のステートマシンを意図的に1か所にまとめている。パーサーは
/// 時系列の副作用に依存しており、フローを過度に分割すると
/// セッション・訪問・プレイヤー・通知の状態更新の整合性検証が困難になる。
#[allow(clippy::too_many_lines)]
fn parse_and_import_reader<R, F>(
    main_tx: &Connection,
    reader: BufReader<R>,
    filename: &str,
    cancel_status: &AtomicBool,
    progress_callback: &mut F,
) -> Result<()>
where
    R: std::io::Read,
    F: FnMut(String, String),
{
    let mut start_time: Option<String> = None;
    let mut end_time: Option<String> = None;
    let mut my_user_id: Option<String> = None;
    let mut my_display_name: Option<String> = None;
    let mut current_ts: Option<NaiveDateTime> = None;
    let mut current_visit_id: Option<i64> = None;
    let mut pending_room_name: Option<String> = None;
    // `Received Notification: <... message: "..." >` が改行を含むケース用の蓄積バッファ。
    // 開始行で `">` で閉じていない場合のみ Some になり、`">` を含む行で確定する。
    // EOF 時に未閉鎖のまま残った内容は破棄する。
    let mut pending_notification: Option<String> = None;

    main_tx.execute(
        "INSERT OR IGNORE INTO sessions (start_time, end_time, account_id, account_name, log_name)
         VALUES ('', NULL, NULL, NULL, ?1)",
        params![filename],
    )?;
    let session_id: i64 = main_tx.query_row(
        "SELECT id FROM sessions WHERE log_name = ?1",
        params![filename],
        |row| row.get(0),
    )?;

    progress_callback("パース開始".to_string(), String::new());

    let mut line_count = 0;
    for line_result in reader.lines() {
        if cancel_status.load(Ordering::SeqCst) {
            return Err(analyze_cancel_sqlite_err());
        }

        let Ok(line) = line_result else { continue };
        line_count += 1;

        if line_count % 5000 == 0 {
            progress_callback(format!("パース中... {line_count} 行"), String::new());
        }

        // --- 複数行通知の継続行 ---
        // バッファ蓄積中は他のパース処理を一切走らせない (本文中に偶発的に他パターン文字列が
        // 含まれてもイベント誤検出しないため)。`">` で閉じた時点で確定処理に回す。
        if let Some(buf) = pending_notification.as_mut() {
            buf.push('\n');
            buf.push_str(&line);
            if line.trim_end().ends_with("\">") {
                let merged = pending_notification.take().unwrap_or_default();
                let ts_str_for_notif = current_ts
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_default();
                persist_notification(main_tx, session_id, &ts_str_for_notif, &merged)?;
            }
            continue;
        }

        // --- タイムスタンプ抽出 ---
        if let Some(caps) = RE_TIME.captures(&line) {
            let Some(match_ts) = caps.get(1) else {
                continue;
            };
            let ts_str = match_ts.as_str();
            if let Ok(dt) = NaiveDateTime::parse_from_str(ts_str, "%Y.%m.%d %H:%M:%S") {
                current_ts = Some(dt);
                let formatted = dt.format("%Y-%m-%d %H:%M:%S").to_string();
                if start_time.is_none() {
                    start_time = Some(formatted.clone());
                }
                end_time = Some(formatted);
            } else {
                crate::utils::log_warn(&format!("タイムスタンプのパースをスキップしました [{filename}]: {ts_str}"));
            }
        }
        let ts_str = current_ts
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();

        // --- セッション識別（認証済みユーザー） ---
        if let Some(caps) = RE_USER_AUTH.captures(&line) {
            if my_display_name.is_none() {
                if let (Some(name_match), Some(user_match)) = (caps.get(1), caps.get(2)) {
                    my_display_name = Some(name_match.as_str().to_string());
                    my_user_id = Some(user_match.as_str().to_string());
                }
            }
            continue;
        }

        // --- ワールド訪問ライフサイクル（入室・参加・退室） ---
        if let Some(caps) = RE_ENTERING.captures(&line) {
            if let Some(visit_id) = current_visit_id {
                main_tx.execute(
                    "UPDATE visits SET leave_time = ?1 WHERE id = ?2 AND leave_time IS NULL",
                    params![ts_str, visit_id],
                )?;
                main_tx.execute(
                    "UPDATE with_users SET leave_time = ?1 WHERE visit_id = ?2 AND leave_time IS NULL",
                    params![ts_str, visit_id],
                )?;
            }
            if let Some(room_match) = caps.get(1) {
                pending_room_name = Some(room_match.as_str().to_string());
            }
            current_visit_id = None;
            continue;
        }

        if let Some(caps) = RE_JOINING.captures(&line) {
            if let Some(room_name) = pending_room_name.as_ref() {
                let Some(location_match) = caps.get(1) else {
                    continue;
                };
                let location = parse_location(location_match.as_str());
                let instance_id = location.instance_id.clone().unwrap_or_default();

                main_tx.execute(
                    "INSERT INTO visits
                     (session_id, world_name, instance_id, instance_type, region, join_time)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        session_id,
                        room_name,
                        instance_id,
                        location.access_type,
                        location.region,
                        ts_str
                    ],
                )?;
                current_visit_id = Some(main_tx.last_insert_rowid());
                pending_room_name = None;
            }
            continue;
        }

        if RE_LEFT_ROOM.is_match(&line) {
            if let Some(visit_id) = current_visit_id {
                main_tx.execute(
                    "UPDATE visits SET leave_time = ?1 WHERE id = ?2 AND leave_time IS NULL",
                    params![ts_str, visit_id],
                )?;
                main_tx.execute(
                    "UPDATE with_users SET leave_time = ?1 WHERE visit_id = ?2 AND leave_time IS NULL",
                    params![ts_str, visit_id],
                )?;
                current_visit_id = None;
                pending_room_name = None;
            }
            continue;
        }

        // --- プレイヤー参加/退出の追跡 ---
        if let Some(caps) = RE_PLAYER_JOIN.captures(&line) {
            let (Some(display_match), Some(user_match)) = (caps.get(1), caps.get(2)) else {
                continue;
            };
            let display_name = display_match.as_str().to_string();
            let user_id = user_match.as_str().to_string();

            main_tx.execute(
                "INSERT INTO find_users (vrchat_id, account_name) VALUES (?1, ?2)
                 ON CONFLICT(vrchat_id) DO UPDATE SET account_name = excluded.account_name",
                params![user_id, display_name],
            )?;

            if let Some(visit_id) = current_visit_id {
                main_tx.execute(
                    "INSERT OR IGNORE INTO with_users (visit_id, vrchat_id, is_self, join_time)
                     VALUES (?1, ?2, 0, ?3)",
                    params![visit_id, user_id, ts_str],
                )?;
            }
            continue;
        }

        if let Some(caps) = RE_PLAYER_LEFT.captures(&line) {
            let Some(user_match) = caps.get(2) else {
                continue;
            };
            let user_id = user_match.as_str().to_string();
            if let Some(visit_id) = current_visit_id {
                main_tx.execute(
                    "UPDATE with_users SET leave_time = ?1
                     WHERE visit_id = ?2 AND vrchat_id = ?3 AND leave_time IS NULL",
                    params![ts_str, visit_id, user_id],
                )?;
            }
            continue;
        }

        // --- ローカルプレイヤー識別 ---
        if let Some(caps) = RE_IS_LOCAL.captures(&line) {
            let (Some(display_match), Some(locality_match)) = (caps.get(1), caps.get(2)) else {
                continue;
            };
            let display_name = display_match.as_str();
            let locality = locality_match.as_str();
            if locality == "local" {
                if my_display_name.is_none() || my_display_name.as_deref() == Some("[LocalPlayer]")
                {
                    my_display_name = Some(display_name.to_string());
                }
                if let Some(visit_id) = current_visit_id {
                    main_tx.execute(
                        "UPDATE with_users SET is_self = 1
                         WHERE visit_id = ?1
                           AND vrchat_id IN (SELECT vrchat_id FROM find_users WHERE account_name = ?2)",
                        params![visit_id, display_name],
                    )?;
                }
            }
            continue;
        }

        // --- スクリーンショット撮影イベント ---
        if let Some(caps) = RE_SCREENSHOT.captures(&line) {
            if let Some(path_match) = caps.get(1) {
                let width = caps
                    .get(2)
                    .and_then(|m| m.as_str().parse::<i64>().ok());
                let height = caps
                    .get(3)
                    .and_then(|m| m.as_str().parse::<i64>().ok());
                insert_screenshot(
                    main_tx,
                    current_visit_id,
                    path_match.as_str(),
                    width,
                    height,
                    &ts_str,
                )?;
            }
            continue;
        }

        // --- OSC サービス検出（外部ツールのみ） ---
        if let Some(caps) = RE_OSC_FOUND.captures(&line) {
            let service_name = caps.get(1).map(|m| m.as_str());
            let ip_address = caps.get(2).map(|m| m.as_str());
            let port = caps
                .get(3)
                .and_then(|m| m.as_str().parse::<i64>().ok());
            insert_osc_event(
                main_tx,
                session_id,
                "found",
                service_name,
                None,
                ip_address,
                port,
                &ts_str,
            )?;
            continue;
        }


        // --- 通知（招待、フレンドリクエスト、boop、グループ） ---
        // 1行で完結する通知 (行末が `">`) はその場で確定処理する。
        // 行末が `">` で閉じていない場合は複数行通知 (例: 改行を含むグループ告知本文) の
        // 開始行と見なし、`pending_notification` に積んで継続行をループ先頭で集約する。
        if line.contains("Received Notification: <") {
            if line.trim_end().ends_with("\">") {
                persist_notification(main_tx, session_id, &ts_str, &line)?;
            } else {
                pending_notification = Some(line.clone());
            }
            continue;
        }

        // --- サブスクリプション状態 ---
        if let Some(caps) = RE_SUBSCRIPTION_STATUS.captures(&line) {
            let subscription_id = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            let active = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
            let desc = caps.get(3).map(|m| m.as_str()).unwrap_or_default();
            let is_active = active.eq_ignore_ascii_case("true");
            let sub_id_opt = if subscription_id.is_empty() || subscription_id == "None" {
                None
            } else {
                Some(subscription_id)
            };
            let desc_opt = if desc.is_empty() { None } else { Some(desc) };
            insert_subscription_status(
                main_tx,
                session_id,
                is_active,
                sub_id_opt,
                desc_opt,
                &ts_str,
            )?;
        }
    }

    progress_callback("コミット中".to_string(), String::new());

    if let Some(visit_id) = current_visit_id {
        let last_ts = current_ts
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
        main_tx.execute(
            "UPDATE visits SET leave_time = ?1 WHERE id = ?2 AND leave_time IS NULL",
            params![last_ts, visit_id],
        )?;
        main_tx.execute(
            "UPDATE with_users SET leave_time = ?1 WHERE visit_id = ?2 AND leave_time IS NULL",
            params![last_ts, visit_id],
        )?;
    }

    main_tx.execute(
        "UPDATE sessions
         SET start_time = ?1, end_time = ?2, account_id = ?3, account_name = ?4
         WHERE log_name = ?5",
        params![
            start_time.unwrap_or_default(),
            end_time,
            my_user_id,
            my_display_name,
            filename
        ],
    )?;

    Ok(())
}

/// 選択された複数のアーカイブまたはテキストログをメインデータベースにインポートする。
///
/// `run_diff_import` と異なり、任意パスのファイルを対象とするため
/// ファイル単位の失敗はバッチ全体をロールバックする（ユーザーが明示的に
/// 選択したファイルが壊れている場合、部分取り込みより失敗通知が適切なため）。
///
/// 各ファイルを savepoint でラップし、不正なログをスキップ可能にしつつ
/// キャンセル要求時にはコミット前にバッチ全体をロールバックする。
///
/// # Errors
/// DB セットアップ失敗、バッチ全体のロールバック必要、またはコミット前の
/// キャンセル要求時にエラーを返す。
pub fn run_enhanced_import_batch<F>(
    main_db_path: &Path,
    target_paths: &[PathBuf],
    cancel_status: &AtomicBool,
    mut progress_callback: F,
) -> Result<(), String>
where
    F: FnMut(String, String),
{
    let mut main_conn = Connection::open(main_db_path).map_err(|e| {
        analyze_err(
            &format!("メイン DB を開けませんでした [{}]", main_db_path.display()),
            e,
        )
    })?;

    init_main_db(&main_conn).map_err(|e| analyze_err("メイン DB を初期化できませんでした", e))?;

    if target_paths.is_empty() {
        progress_callback("処理対象ログなし".to_string(), "0/0".to_string());
        return Ok(());
    }

    let mut main_tx = main_conn
        .transaction()
        .map_err(|e| analyze_err("メイン DB トランザクションを開始できませんでした", e))?;

    let total = target_paths.len();
    for (index, target_path) in target_paths.iter().enumerate() {
        if let Err(err) = ensure_not_canceled(cancel_status) {
            rollback_outer_transaction(main_tx, "解析中断時")?;
            return Err(err);
        }

        let filename = source_name_for_archive(target_path)
            .ok_or_else(|| "対象ファイル名を解決できませんでした".to_string())?;

        let already_processed: bool = main_tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE log_name = ?1)",
                params![filename],
                |row| row.get(0),
            )
            .map_err(|e| analyze_err("セッション存在確認に失敗しました", e))?;

        if already_processed {
            progress_callback(
                format!("スキップ（DB登録済み）: {filename}"),
                format_progress_fraction(index + 1, total),
            );
            continue;
        }

        progress_callback(
            format!("処理中: {filename}"),
            format_progress_fraction(index + 1, total),
        );

        let main_sp = main_tx
            .savepoint()
            .map_err(|e| analyze_err("メイン DB savepoint を開始できませんでした", e))?;

        match parse_and_import(
            &main_sp,
            target_path,
            &filename,
            cancel_status,
            &mut progress_callback,
        ) {
            Ok(()) => {
                main_sp
                    .commit()
                    .map_err(|e| analyze_err("メイン DB savepoint を確定できませんでした", e))?;
                progress_callback(
                    format!("取り込み完了: {filename}"),
                    format_progress_fraction(index + 1, total),
                );
            }
            Err(err) if err.to_string() == ANALYZE_CANCELED_MESSAGE => {
                rollback_savepoint(main_sp, "解析中断時")?;
                rollback_outer_transaction(main_tx, "解析中断時")?;
                return Err(ANALYZE_CANCELED_MESSAGE.to_string());
            }
            Err(err) => {
                rollback_savepoint(main_sp, "対象ログロールバック時")?;
                rollback_outer_transaction(main_tx, "解析エラー時")?;
                return Err(analyze_err("対象ログを取り込めませんでした", err));
            }
        }
    }

    if let Err(err) = ensure_not_canceled(cancel_status) {
        rollback_outer_transaction(main_tx, "解析中断時")?;
        return Err(err);
    }

    main_tx
        .commit()
        .map_err(|e| analyze_err("メイン DB 反映を確定できませんでした", e))?;

    progress_callback("完了".to_string(), format!("{total}/{total}"));
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::init_main_db(&conn).unwrap();
        conn
    }

    fn make_reader(content: &str) -> BufReader<Cursor<Vec<u8>>> {
        BufReader::new(Cursor::new(content.as_bytes().to_vec()))
    }

    #[test]
    fn source_name_strips_tar_zst() {
        let path = Path::new("/data/output_log_2025-04-30.txt.tar.zst");
        assert_eq!(
            source_name_for_archive(path),
            Some("output_log_2025-04-30.txt".to_string())
        );
    }

    #[test]
    fn source_name_plain_txt() {
        let path = Path::new("/data/output_log_2025-04-30.txt");
        assert_eq!(
            source_name_for_archive(path),
            Some("output_log_2025-04-30.txt".to_string())
        );
    }

    #[test]
    fn ensure_not_canceled_ok() {
        let flag = AtomicBool::new(false);
        assert!(ensure_not_canceled(&flag).is_ok());
    }

    #[test]
    fn ensure_not_canceled_err() {
        let flag = AtomicBool::new(true);
        let err = ensure_not_canceled(&flag).unwrap_err();
        assert_eq!(err, ANALYZE_CANCELED_MESSAGE);
    }

    #[test]
    fn format_progress_fraction_zero() {
        assert_eq!(format_progress_fraction(0, 10), "0/10");
    }

    #[test]
    fn format_progress_fraction_done() {
        assert_eq!(format_progress_fraction(5, 5), "5/5");
    }

    #[test]
    fn import_session_with_auth_and_visit() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = "\
2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:01:00 Log        -  User Authenticated: TestUser (usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee)
2025.04.30 20:02:00 Log        -  [Behaviour] Entering Room: My Cool World
2025.04.30 20:02:01 Log        -  [Behaviour] Joining wrld_abc123:99999~private(usr_owner-1234)~region(jp)
2025.04.30 20:03:00 Log        -  [Behaviour] OnPlayerJoined FriendUser (usr_1111-2222-3333-4444-555555555555)
2025.04.30 20:04:00 Log        -  [Behaviour] Initialized PlayerAPI \"TestUser\" is local
2025.04.30 20:10:00 Log        -  [Behaviour] OnPlayerLeft FriendUser (usr_1111-2222-3333-4444-555555555555)
2025.04.30 20:15:00 Log        -  [Behaviour] OnLeftRoom
";
        let reader = make_reader(log);
        let mut progress_calls = Vec::new();
        parse_and_import_reader(
            &conn,
            reader,
            "test_log.txt",
            &cancel,
            &mut |status: String, progress: String| {
                progress_calls.push((status, progress));
            },
        )
        .unwrap();

        let (account_id, account_name): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT account_id, account_name FROM sessions WHERE log_name = 'test_log.txt'",
                [],
                |row| Ok((row.get(0).ok(), row.get(1).ok())),
            )
            .unwrap();
        assert_eq!(account_id.as_deref(), Some("usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee"));
        assert_eq!(account_name.as_deref(), Some("TestUser"));

        let world_name: String = conn
            .query_row("SELECT world_name FROM visits LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(world_name, "My Cool World");

        let leave_time: Option<String> = conn
            .query_row("SELECT leave_time FROM visits LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(leave_time.is_some());

        let player_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM find_users", [], |row| row.get(0))
            .unwrap();
        assert!(player_count >= 1);

        let with_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM with_users", [], |row| row.get(0))
            .unwrap();
        assert!(with_count >= 1);
    }

    #[test]
    fn import_screenshot_event() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = "\
2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:05:00 Log        -  [VRC Camera] Took screenshot to: C:\\Users\\test\\VRChat_2025-04-30_20-05-00.000_3840x2160.png
";
        let reader = make_reader(log);
        parse_and_import_reader(
            &conn,
            reader,
            "screenshot_test.txt",
            &cancel,
            &mut |_: String, _: String| {},
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM screenshots", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let (width, height): (Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT resolution_width, resolution_height FROM screenshots LIMIT 1",
                [],
                |row| Ok((row.get(0).ok(), row.get(1).ok())),
            )
            .unwrap();
        assert_eq!(width, Some(3840));
        assert_eq!(height, Some(2160));
    }

    #[test]
    fn import_osc_event() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = "\
2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:01:00 Log        -  Found new OSC Service: OyasumiVR at 127.0.0.1:61080
";
        let reader = make_reader(log);
        parse_and_import_reader(
            &conn,
            reader,
            "osc_test.txt",
            &cancel,
            &mut |_: String, _: String| {},
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM osc", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let service_name: String = conn
            .query_row("SELECT service_name FROM osc LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(service_name, "OyasumiVR");
    }

    #[test]
    fn import_subscription_status() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = "\
2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:01:00 Log        -  Get VRChat Subscription Details! Subscription Id:sub_12345 active:True desc:VRChat Plus
";
        let reader = make_reader(log);
        parse_and_import_reader(
            &conn,
            reader,
            "sub_test.txt",
            &cancel,
            &mut |_: String, _: String| {},
        )
        .unwrap();

        let (is_active, desc): (bool, Option<String>) = conn
            .query_row(
                "SELECT is_active, description FROM subscription LIMIT 1",
                [],
                |row| Ok((row.get(0).unwrap(), row.get(1).ok())),
            )
            .unwrap();
        assert!(is_active);
        assert_eq!(desc.as_deref(), Some("VRChat Plus"));
    }

    #[test]
    fn import_cancellation_rolls_back() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(true);
        let log = "2025.04.30 20:00:00 Log        -  VRChat starting\n";
        let reader = make_reader(log);
        let result = parse_and_import_reader(
            &conn,
            reader,
            "cancel_test.txt",
            &cancel,
            &mut |_: String, _: String| {},
        );
        assert!(result.is_err());
    }

    #[test]
    fn import_single_line_notification() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = r#"2025.04.30 20:00:00 Log        -  VRChat starting
2025.04.30 20:01:00 Log        -  Received Notification: <Notification from username:SomeUser, sender user id:usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee to usr_self of type: invite, id: not_1234-5678-abcd-ef01-234567890abc, created at: 04/30/2025 11:01:00 UTC, expires: 04/30/2025 12:01:00 UTC, message: "Come join me!">
"#;
        let reader = make_reader(log);
        parse_and_import_reader(
            &conn,
            reader,
            "notif_test.txt",
            &cancel,
            &mut |_: String, _: String| {},
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notifications", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let notif_type: String = conn
            .query_row(
                "SELECT notif_type FROM notifications LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(notif_type, "invite");
    }

    #[test]
    fn import_empty_log_creates_session() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let reader = make_reader("");
        parse_and_import_reader(
            &conn,
            reader,
            "empty_test.txt",
            &cancel,
            &mut |_: String, _: String| {},
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn import_multiple_visits_closes_previous() {
        let conn = setup_test_db();
        let cancel = AtomicBool::new(false);
        let log = "\
2025.04.30 20:00:00 Log        -  [Behaviour] Entering Room: World A
2025.04.30 20:00:01 Log        -  [Behaviour] Joining wrld_aaa:111~public~region(jp)
2025.04.30 20:10:00 Log        -  [Behaviour] Entering Room: World B
2025.04.30 20:10:01 Log        -  [Behaviour] Joining wrld_bbb:222~public~region(us)
2025.04.30 20:20:00 Log        -  [Behaviour] OnLeftRoom
";
        let reader = make_reader(log);
        parse_and_import_reader(
            &conn,
            reader,
            "multi_visit.txt",
            &cancel,
            &mut |_: String, _: String| {},
        )
        .unwrap();

        let visit_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM visits", [], |row| row.get(0))
            .unwrap();
        assert_eq!(visit_count, 2);

        let first_leave: Option<String> = conn
            .query_row(
                "SELECT leave_time FROM visits ORDER BY id LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(first_leave.is_some());
    }
}
