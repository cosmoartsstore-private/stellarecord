//! `VRChat` ログ解析パイプライン。
//!
//! アーカイブコマンドで生成された `.tar.zst` 圧縮ログを読み取り、VRChat ログの
//! 内容を行単位で解析し、正規化データをメイン `SQLite` データベース
//! (`stellarecord.db`) に書き込む。
//!
//! ファイル単位の savepoint を持つtransaction内で書き込むため、
//! キャンセルや単一ファイルの失敗で部分データが残ることはない。

mod db;
mod friendship;
mod parser;

pub use db::init_main_db;
pub use parser::{
    is_collectible_notification, parse_access_type, parse_location, ParsedLocation, RE_AVATAR_DOWNLOAD_FRIEND, RE_AVATAR_SWITCH, RE_AVATAR_UNPACK, RE_ENTERING, RE_IS_LOCAL, RE_JOINING, RE_LEFT_ROOM, RE_LOG_HEADER, RE_LOG_RICH_TEXT,
    RE_NOTIFICATION, RE_NOTIFICATION_WORLD_ID, RE_NOTIFICATION_WORLD_NAME, RE_OSC_FOUND, RE_PLAYER_JOIN, RE_PLAYER_JOIN_COMPLETE, RE_PLAYER_LEFT, RE_SCREENSHOT, RE_SUBSCRIPTION_STATUS, RE_TIME, RE_USER_AUTH, RE_VIDEO_URL,
};

use chrono::NaiveDateTime;
use rusqlite::{params, Connection, Result, Savepoint, Transaction};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::utils::decode_log_line_lossy;
use friendship::{FriendStatus, FriendshipTracker, UNKNOWN_FRIEND_STATUS};

/// 取り込み結果へ影響するログ解析仕様のバージョン。
const CURRENT_PARSER_VERSION: i64 = 3;

/// インポートが意図的にキャンセルされた際にユーザーに返すメッセージ。
///
/// パーサー、コマンド、UI で同一テキストを再利用し、キャンセルを
/// 汎用的なエラーではなく一貫した制御結果として扱う。
pub const ANALYZE_CANCELED_MESSAGE: &str = message!("Analyze.constants.rollbackCancelled");

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

/// SQLiteの表示文は接頭辞を加えるため、中断は元のエラー値で識別する。
fn is_analyze_canceled_sqlite_err(err: &rusqlite::Error) -> bool {
    matches!(err, rusqlite::Error::InvalidParameterName(message) if message == ANALYZE_CANCELED_MESSAGE)
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

/// メインデータベースの外側transactionをロールバックする。
fn rollback_outer_transaction(main_tx: Transaction<'_>, context: &str) -> std::result::Result<(), String> {
    main_tx.rollback().map_err(|err| analyze_err(&format!(message!("Analyze.rollbackOuterTransaction.databaseRollbackFailed"), context = context), err))?;
    Ok(())
}

/// メインデータベースのファイル単位 savepoint をロールバックする。
///
/// 個別ファイルの解析失敗時に、そのファイルの変更のみを巻き戻す。
/// 外側transactionは維持されるため、他ファイルの処理を続行できる。
fn rollback_savepoint(mut main_sp: Savepoint<'_>, context: &str) -> std::result::Result<(), String> {
    main_sp.rollback().map_err(|err| analyze_err(&format!(message!("Analyze.rollbackSavepoint.databaseSavepointRollbackFailed"), context = context), err))?;
    Ok(())
}

/// 整数の進捗を「完了数/合計数」形式の文字列にフォーマットする。
fn format_progress_fraction(completed: usize, total: usize) -> String {
    format!("{completed}/{total}")
}

/// インポート命名規則に合致する圧縮アーカイブログを一覧取得する。
///
/// # 引数
/// * `archive_store_dir` - `StellaRecord` が管理する `.tar.zst` ログの格納directory。
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
            crate::utils::log_warn(&format!(message!("Analyze.collectLogFiles.directoryReadFailed"), archive_store_dir.display(), err));
            return files;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                crate::utils::log_warn(&format!(message!("Analyze.collectLogFiles.entryReadFailed"), err = err));
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

/// ログ本文の内容同一性を判定する指紋。
///
/// 圧縮ファイル自体ではなく展開後の本文を対象にするため、tar のファイル時刻などが
/// 変化しても同じログを再取り込みしない。SHA-256 と長さを併用し、単純なチェック
/// サムの衝突やファイル時刻への依存を避ける。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LogFingerprint {
    sha256: [u8; 32],
    content_size: i64,
}

/// 管理対象ログの差分インポート結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffImportSummary {
    /// 処理対象として検出したログの総数。
    pub total_count: usize,
    /// ファイル単位でロールバックし、取り込めなかったログ名。
    pub failed_filenames: Vec<String>,
}

/// ログファイルを開き、プレーン本文のリーダーをコールバックへ渡す。
///
/// `.tar.zst` は先頭の単一ログエントリを展開し、プレーンログは共有読み取りで開く。
/// 指紋計算とパーサーで同じ入力解釈を共有する。
fn with_log_content_reader<T, F>(log_path: &Path, callback: F) -> Result<T>
where
    F: FnOnce(&mut dyn Read) -> Result<T>,
{
    if log_path.file_name().and_then(|value| value.to_str()).is_some_and(|value| value.ends_with(".tar.zst")) {
        let file = fs::File::open(log_path).map_err(|err| analyze_sqlite_err(message!("Analyze.withLogContentReader.logOpenFailed"), err))?;
        let decoder = zstd::stream::Decoder::new(file).map_err(|err| analyze_sqlite_err(message!("Analyze.withLogContentReader.logZstdFailed"), err))?;
        let mut archive = tar::Archive::new(decoder);
        let mut entries = archive.entries().map_err(|err| analyze_sqlite_err(message!("Analyze.withLogContentReader.logEntryEntryListReadFailed"), err))?;
        let Some(entry) = entries.next() else {
            return Err(rusqlite::Error::InvalidParameterName(message!("Analyze.withLogContentReader.logMissing").to_string()));
        };
        let entry = entry.map_err(|err| analyze_sqlite_err(message!("Analyze.withLogContentReader.logEntryReadFailed"), err))?;
        if !entry.header().entry_type().is_file() {
            return Err(rusqlite::Error::InvalidParameterName(message!("Analyze.withLogContentReader.logMissing").to_string()));
        }
        let declared_size = entry.header().size().map_err(|err| analyze_sqlite_err(message!("Analyze.withLogContentReader.logEntryReadFailed"), err))?;
        // tar の本文リーダーは宣言サイズより早い EOF を返すことがあるため、消費量も検証する。
        let mut content = entry.take(declared_size);
        let result = callback(&mut content)?;
        if content.limit() != 0 {
            return Err(rusqlite::Error::InvalidParameterName(message!("Analyze.withLogContentReader.logIncomplete").to_string()));
        }
        return Ok(result);
    }

    // FILE_SHARE_READ により、VRChat が書き込み中のログでも読み取り可能にする。
    #[cfg(windows)]
    let mut file = {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;
        fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ.0).open(log_path).map_err(|err| analyze_sqlite_err(message!("Analyze.withLogContentReader.logFileOpenFailed"), err))?
    };
    #[cfg(not(windows))]
    let mut file = fs::File::open(log_path).map_err(|err| analyze_sqlite_err(message!("Analyze.withLogContentReader.logFileOpenFailed"), err))?;

    callback(&mut file)
}

/// 展開後ログ本文をストリーミングし、取り込み判定用の指紋を計算する。
fn fingerprint_log_content(log_path: &Path, cancel_status: &AtomicBool) -> Result<LogFingerprint> {
    with_log_content_reader(log_path, |reader| {
        let mut hasher = Sha256::new();
        let mut content_size = 0_u64;
        let mut buffer = vec![0_u8; 64 * 1024];

        loop {
            if cancel_status.load(Ordering::SeqCst) {
                return Err(analyze_cancel_sqlite_err());
            }
            let bytes_read = reader.read(&mut buffer).map_err(|err| analyze_sqlite_err(message!("Analyze.fingerprintLogContent.logReadFailed"), err))?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
            content_size = content_size.checked_add(bytes_read as u64).ok_or_else(|| rusqlite::Error::InvalidParameterName(message!("Analyze.fingerprintLogContent.logLimitExceeded").to_string()))?;
        }

        let content_size = i64::try_from(content_size).map_err(|_| rusqlite::Error::InvalidParameterName(message!("Analyze.fingerprintLogContent.logLimitExceeded").to_string()))?;
        Ok(LogFingerprint { sha256: hasher.finalize().into(), content_size })
    })
}

/// DB に記録された指紋と現在のログ本文が一致するか確認する。
///
/// 旧版 DB には指紋行が存在しないため、その場合は `false` として一度だけ安全に
/// 再構築し、現在内容に対応する指紋を登録する。
fn imported_content_is_current(conn: &Connection, filename: &str, fingerprint: &LogFingerprint) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM imported_logs AS imported
            JOIN sessions AS session ON session.log_name = imported.log_name
            WHERE imported.log_name = ?1
              AND imported.content_sha256 = ?2
              AND imported.content_size = ?3
              AND imported.parser_version = ?4
         )",
        params![filename, &fingerprint.sha256[..], fingerprint.content_size, CURRENT_PARSER_VERSION],
        |row| row.get(0),
    )
}

/// 同名ログから生成されたセッション所有データを削除する。
///
/// この関数は必ずファイル単位 savepoint 内で呼び、後続の再解析または指紋登録が
/// 失敗した場合に旧データ一式を復元できるようにする。`find_users` は複数セッション
/// で共有するユーザーマスターのため削除しない。
fn delete_imported_session(conn: &Connection, filename: &str) -> Result<()> {
    conn.execute("DELETE FROM imported_logs WHERE log_name = ?1", [filename])?;
    conn.execute(
        "DELETE FROM screenshots
         WHERE session_id IN (
                   SELECT id FROM sessions WHERE log_name = ?1
               )
            OR visit_id IN (
                   SELECT visit.id
                   FROM visits AS visit
                   JOIN sessions AS session ON session.id = visit.session_id
                   WHERE session.log_name = ?1
               )",
        [filename],
    )?;
    conn.execute(
        "DELETE FROM with_users
         WHERE visit_id IN (
             SELECT visit.id
             FROM visits AS visit
             JOIN sessions AS session ON session.id = visit.session_id
             WHERE session.log_name = ?1
         )",
        [filename],
    )?;
    conn.execute(
        "DELETE FROM visits
         WHERE session_id IN (SELECT id FROM sessions WHERE log_name = ?1)",
        [filename],
    )?;
    conn.execute(
        "DELETE FROM notifications
         WHERE session_id IN (SELECT id FROM sessions WHERE log_name = ?1)",
        [filename],
    )?;
    conn.execute(
        "DELETE FROM osc
         WHERE session_id IN (SELECT id FROM sessions WHERE log_name = ?1)",
        [filename],
    )?;
    conn.execute(
        "DELETE FROM subscription
         WHERE session_id IN (SELECT id FROM sessions WHERE log_name = ?1)",
        [filename],
    )?;
    conn.execute("DELETE FROM sessions WHERE log_name = ?1", [filename])?;
    Ok(())
}

/// 正常に再構築できたログ本文の指紋をセッションへ紐付けて保存する。
fn record_imported_content(conn: &Connection, filename: &str, fingerprint: &LogFingerprint) -> Result<()> {
    conn.execute(
        "INSERT INTO imported_logs (log_name, content_sha256, content_size, parser_version)
         VALUES (?1, ?2, ?3, ?4)",
        params![filename, &fingerprint.sha256[..], fingerprint.content_size, CURRENT_PARSER_VERSION],
    )?;
    Ok(())
}

/// ファイル単位 savepoint 内での取り込み結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportOutcome {
    Unchanged,
    Imported,
}

/// 1件のログを内容ベースでスキップまたは再構築する。
///
/// 呼び出し元が渡す接続はファイル単位 savepoint であり、旧セッション削除、
/// 再解析、指紋登録のいずれかが失敗すれば一式をロールバックする。
fn import_log_in_savepoint<F>(conn: &Connection, log_path: &Path, filename: &str, cancel_status: &AtomicBool, progress_callback: &mut F) -> Result<ImportOutcome>
where
    F: FnMut(String, String),
{
    let fingerprint = fingerprint_log_content(log_path, cancel_status)?;
    if imported_content_is_current(conn, filename, &fingerprint)? {
        return Ok(ImportOutcome::Unchanged);
    }

    delete_imported_session(conn, filename)?;
    parse_and_import(conn, log_path, filename, cancel_status, progress_callback)?;
    record_imported_content(conn, filename, &fingerprint)?;
    Ok(ImportOutcome::Imported)
}

/// スクリーンショット撮影イベントをメインデータベースに挿入する。
fn insert_screenshot(tx: &Connection, visit_id: Option<i64>, file_path: &str, resolution_width: Option<i64>, resolution_height: Option<i64>, timestamp: &str, session_id: i64) -> Result<()> {
    tx.execute(
        "INSERT INTO screenshots
         (visit_id, file_path, resolution_width, resolution_height, timestamp, session_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![visit_id, file_path, resolution_width, resolution_height, timestamp, session_id],
    )?;
    Ok(())
}

/// OSC/OSCQuery サービスイベントをメインデータベースに挿入する。
fn insert_osc_event(tx: &Connection, session_id: i64, event_type: &str, service_name: Option<&str>, service_type: Option<&str>, ip_address: Option<&str>, port: Option<i64>, timestamp: &str) -> Result<()> {
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
fn insert_subscription_status(tx: &Connection, session_id: i64, is_active: bool, subscription_id: Option<&str>, description: Option<&str>, checked_at: &str) -> Result<()> {
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
fn persist_notification(main_tx: &Connection, session_id: i64, ts_str: &str, notification_text: &str) -> Result<()> {
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

    let sender_name = caps.get(1).map(|m| m.as_str()).filter(|s| !s.is_empty()).map(String::from);
    let sender_user_id = caps.get(2).map(|m| m.as_str()).filter(|s| !s.is_empty()).map(String::from);
    let notif_id = caps.get(4).map(|m| m.as_str().to_string());
    let Some(created_match) = caps.get(5) else {
        return Ok(());
    };
    let created_at_raw = created_match.as_str().trim();
    let message = caps.get(6).map(|m| m.as_str().to_string());
    let created_at = NaiveDateTime::parse_from_str(created_at_raw, "%m/%d/%Y %H:%M:%S UTC").ok().map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());

    let location = RE_NOTIFICATION_WORLD_ID.captures(notification_text).and_then(|captures| captures.get(1)).map(|m| parse_location(m.as_str())).unwrap_or_default();
    let target_world_name = RE_NOTIFICATION_WORLD_NAME.captures(notification_text).and_then(|captures| captures.get(1)).map(|m| m.as_str().to_string());

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

/// 管理された `Data` directory内のログを差分インポートする。
///
/// 外側transaction内でファイルごとに savepoint を切り、個々のファイル失敗で
/// バッチ全体が巻き戻ることを防ぐ。キャンセル時は外側transactionごと
/// ロールバックし、部分データが残らないことを保証する。
///
/// # Errors
/// データベースを開けない、初期化できない、キャンセルされた、またはバッチ全体の
/// 続行・確定ができない場合にエラーを返す。ファイル単位の失敗は、正常に取り込めた
/// ファイルを確定したうえで `DiffImportSummary` に格納する。
#[allow(clippy::too_many_lines)]
pub fn run_diff_import<F>(main_db_path: &Path, archive_store_dir: &Path, cancel_status: &AtomicBool, mut progress_callback: F) -> Result<DiffImportSummary, String>
where
    F: FnMut(String, String),
{
    let mut main_conn = Connection::open(main_db_path).map_err(|e| analyze_err(&format!(message!("Analyze.runDiffImport.databaseOpenFailed"), main_db_path.display()), e))?;

    init_main_db(&main_conn).map_err(|e| analyze_err(message!("Analyze.runDiffImport.databaseFailed"), e))?;

    let log_files = collect_log_files(archive_store_dir);
    if log_files.is_empty() {
        progress_callback(message!("Analyze.runDiffImport.logText").to_string(), "0/0".to_string());
        return Ok(DiffImportSummary { total_count: 0, failed_filenames: Vec::new() });
    }

    progress_callback(format!(message!("Analyze.runDiffImport.logCount"), log_files.len()), format!("0/{}", log_files.len()));

    let mut main_tx = main_conn.transaction().map_err(|e| analyze_err(message!("Analyze.runDiffImport.databaseTransactionBeginFailed"), e))?;

    let total = log_files.len();
    let mut failed_filenames = Vec::new();
    for (idx, log_path) in log_files.iter().enumerate() {
        if let Err(err) = ensure_not_canceled(cancel_status) {
            rollback_outer_transaction(main_tx, message!("Analyze.runDiffImport.cancelled"))?;
            return Err(err);
        }

        let Some(filename) = source_name_for_archive(log_path) else {
            continue;
        };

        progress_callback(format!(message!("Analyze.runDiffImport.text"), filename = filename), format_progress_fraction(idx, total));

        let main_sp = main_tx.savepoint().map_err(|e| analyze_err(message!("Analyze.runDiffImport.databaseSavepointBeginFailed"), e))?;

        match import_log_in_savepoint(&main_sp, log_path, &filename, cancel_status, &mut progress_callback) {
            Ok(outcome) => {
                main_sp.commit().map_err(|e| analyze_err(message!("Analyze.runDiffImport.databaseSavepointCommitFailed"), e))?;
                let status = match outcome {
                    ImportOutcome::Unchanged => format!(message!("Analyze.runDiffImport.unchangedLogSkipped"), filename = filename),
                    ImportOutcome::Imported => format!(message!("Analyze.runDiffImport.completed"), filename = filename),
                };
                progress_callback(status, format_progress_fraction(idx + 1, total));
            }
            Err(err) if is_analyze_canceled_sqlite_err(&err) => {
                rollback_savepoint(main_sp, message!("Analyze.runDiffImport.cancelled"))?;
                rollback_outer_transaction(main_tx, message!("Analyze.runDiffImport.cancelled"))?;
                return Err(ANALYZE_CANCELED_MESSAGE.to_string());
            }
            Err(err) => {
                rollback_savepoint(main_sp, message!("Analyze.runDiffImport.fileRollbackText"))?;
                crate::utils::log_err(&format!(message!("Analyze.runDiffImport.logImportError"), err = err, filename = filename));
                failed_filenames.push(filename);
            }
        }
    }

    if let Err(err) = ensure_not_canceled(cancel_status) {
        rollback_outer_transaction(main_tx, message!("Analyze.runDiffImport.cancelled"))?;
        return Err(err);
    }

    main_tx.commit().map_err(|e| analyze_err(message!("Analyze.runDiffImport.databaseCommitFailed"), e))?;

    if !failed_filenames.is_empty() {
        let failed_count = failed_filenames.len();
        progress_callback(format!(message!("Analyze.runDiffImport.saveFailed"), failed_count = failed_count), format_progress_fraction(total - failed_count, total));
        return Ok(DiffImportSummary { total_count: total, failed_filenames });
    }

    progress_callback(message!("Analyze.runDiffImport.processingCompleted").to_string(), format_progress_fraction(total, total));
    Ok(DiffImportSummary { total_count: total, failed_filenames })
}

/// ログファイル（プレーンテキストまたは `.tar.zst` アーカイブ）を開き、
/// 行単位パーサーに委譲する。
///
/// `.tar.zst` が推奨アーカイブ形式: Zstandard は高速展開を提供し、tar は
/// ログを単一ファイルにまとめてdirectory管理を簡素化する。
/// 非圧縮 `VRChat` ログの手動インポート用にプレーンテキストのフォールバックもある。
fn parse_and_import<F>(main_conn: &Connection, log_path: &Path, filename: &str, cancel_status: &AtomicBool, progress_callback: &mut F) -> Result<()>
where
    F: FnMut(String, String),
{
    if cancel_status.load(Ordering::SeqCst) {
        return Err(analyze_cancel_sqlite_err());
    }

    with_log_content_reader(log_path, |reader| parse_and_import_reader(main_conn, BufReader::new(reader), filename, cancel_status, progress_callback))
}

/// 訪問内の参加者だけからローカルユーザーを確定し、同名の別ユーザーを一括指定しない。
/// ローカル初期化がある場合はその表示名を優先し、同名候補が複数なら認証済み ID で判別する。
fn identify_visit_local_player(conn: &Connection, visit_id: i64, local_name: Option<&str>, authenticated_account: Option<(&str, &str)>) -> Result<Option<(String, String)>> {
    let authenticated_id = authenticated_account.map(|(id, _)| id);
    let mut statement = conn.prepare(
        "SELECT w.vrchat_id, u.account_name FROM with_users w
         JOIN find_users u ON u.vrchat_id = w.vrchat_id
         WHERE w.visit_id = ?1
           AND ((?2 IS NOT NULL AND u.account_name = ?2)
                OR (?2 IS NULL AND w.vrchat_id = ?3))",
    )?;
    let candidates = statement.query_map(params![visit_id, local_name, authenticated_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?.collect::<Result<Vec<_>>>()?;
    let selected = if let Some(account) = candidates.iter().find(|(id, _)| Some(id.as_str()) == authenticated_id) {
        Some(account.clone())
    } else if local_name.is_some() && local_name == authenticated_account.map(|(_, name)| name) {
        // 同名参加者が先に到着しても、認証済みユーザーの参加を待つ。
        None
    } else if candidates.len() == 1 {
        candidates.into_iter().next()
    } else {
        None
    };
    conn.execute(
        "UPDATE with_users
         SET is_self = CASE WHEN vrchat_id = ?2 THEN 1 ELSE 0 END,
             friend_status = CASE WHEN vrchat_id = ?2 THEN ?3 ELSE friend_status END
         WHERE visit_id = ?1",
        params![visit_id, selected.as_ref().map(|(id, _)| id.as_str()), UNKNOWN_FRIEND_STATUS],
    )?;
    Ok(selected)
}

/// ログストリーム1件を解析し、正規化データをメインデータベースに書き込む。
///
/// 行単位のステートマシンを意図的に1か所にまとめている。パーサーは
/// 時系列の副作用に依存しており、処理を過度に分割すると
/// セッション・訪問・プレイヤー・通知の状態更新の整合性検証が困難になる。
#[allow(clippy::too_many_lines)]
fn parse_and_import_reader<R, F>(main_tx: &Connection, mut reader: BufReader<R>, filename: &str, cancel_status: &AtomicBool, progress_callback: &mut F) -> Result<()>
where
    R: std::io::Read,
    F: FnMut(String, String),
{
    let mut start_time: Option<String> = None;
    let mut end_time: Option<String> = None;
    let mut my_user_id: Option<String> = None;
    let mut authenticated_account: Option<(String, String)> = None;
    let mut my_display_name: Option<String> = None;
    let mut log_accounts = BTreeMap::new();
    let mut visit_local_name: Option<String> = None;
    let mut current_ts: Option<NaiveDateTime> = None;
    let mut current_visit_id: Option<i64> = None;
    let mut pending_room_name: Option<String> = None;
    // `Received Notification: <... message: "..." >` が改行を含むケース用の蓄積バッファ。
    // 開始行で `">` で閉じていない場合のみ Some になり、`">` を含む行で確定する。
    // EOF 時に未閉鎖のまま残った内容は破棄する。
    let mut pending_notification: Option<String> = None;
    let mut friendship_tracker = FriendshipTracker::default();

    main_tx.execute(
        "INSERT OR IGNORE INTO sessions (start_time, end_time, account_id, account_name, log_name)
         VALUES ('', NULL, NULL, NULL, ?1)",
        params![filename],
    )?;
    let session_id: i64 = main_tx.query_row("SELECT id FROM sessions WHERE log_name = ?1", params![filename], |row| row.get(0))?;

    progress_callback(message!("Analyze.parseAndImportReader.started").to_string(), String::new());

    let mut line_count = 0;
    let mut invalid_utf8_line_count = 0;
    let mut line_buffer = Vec::new();
    loop {
        if cancel_status.load(Ordering::SeqCst) {
            return Err(analyze_cancel_sqlite_err());
        }

        line_buffer.clear();
        let bytes_read = reader.read_until(b'\n', &mut line_buffer).map_err(|err| analyze_sqlite_err(message!("Analyze.parseAndImportReader.logReadFailed"), err))?;
        if bytes_read == 0 {
            break;
        }
        if std::str::from_utf8(&line_buffer).is_err() {
            invalid_utf8_line_count += 1;
        }
        let line = decode_log_line_lossy(&line_buffer);
        line_count += 1;

        if line_count % 5000 == 0 {
            progress_callback(format!(message!("Analyze.parseAndImportReader.text"), line_count = line_count), String::new());
        }

        // --- 複数行通知の継続行 ---
        // バッファ蓄積中は他のパース処理を一切走らせない (本文中に偶発的に他パターン文字列が
        // 含まれてもイベント誤検出しないため)。`">` で閉じた時点で確定処理に回す。
        if let Some(buf) = pending_notification.as_mut() {
            buf.push('\n');
            buf.push_str(&line);
            if line.trim_end().ends_with("\">") {
                let merged = pending_notification.take().unwrap_or_default();
                let ts_str_for_notif = current_ts.map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default();
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
                crate::utils::log_warn(&format!(message!("Analyze.parseAndImportReader.timestampText"), filename = filename, ts_str = ts_str));
            }
        }
        let ts_str = current_ts.map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default();

        // --- 旧形式アバター取得ログによるフレンド状態の推定 ---
        // `Friend` 値だけではユーザーを識別できないため、同一訪問内でアバター名、
        // 展開順、入室中の表示名とユーザー ID がすべて一意な場合だけ保存する。
        if current_visit_id.is_some() {
            if let Some(caps) = RE_AVATAR_SWITCH.captures(&line) {
                if let (Some(display_match), Some(avatar_match)) = (caps.get(1), caps.get(2)) {
                    friendship_tracker.on_avatar_switch(display_match.as_str(), avatar_match.as_str());
                }
            } else if line.contains("[AssetBundleDownloadManager] Download for avatar (") {
                let status = RE_AVATAR_DOWNLOAD_FRIEND.captures(&line).and_then(|caps| caps.get(1)).and_then(|value| FriendStatus::from_log_value(value.as_str()));
                friendship_tracker.on_avatar_download(status, line_count);
            } else if let Some(caps) = RE_AVATAR_UNPACK.captures(&line) {
                if let Some(payload) = caps.get(1) {
                    if let (Some(visit_id), Some((user_id, friend_status))) = (current_visit_id, friendship_tracker.on_avatar_unpack(payload.as_str(), line_count)) {
                        main_tx.execute(
                            "UPDATE with_users
                             SET friend_status = ?1
                             WHERE visit_id = ?2 AND vrchat_id = ?3 AND is_self = 0",
                            params![friend_status, visit_id, user_id],
                        )?;
                    }
                }
            }
        }

        // --- セッション識別（認証済みユーザー） ---
        if let Some(caps) = RE_USER_AUTH.captures(&line) {
            if let (Some(name_match), Some(user_match)) = (caps.get(1), caps.get(2)) {
                let user_id = user_match.as_str();
                if my_user_id.as_deref().is_some_and(|previous| previous != user_id) {
                    // 新アカウントのイベントを、退室出力のない旧訪問へ紐付けない。
                    if let Some(visit_id) = current_visit_id.take() {
                        main_tx.execute("UPDATE visits SET leave_time = ?1 WHERE id = ?2 AND leave_time IS NULL", params![ts_str, visit_id])?;
                        main_tx.execute("UPDATE with_users SET leave_time = ?1 WHERE visit_id = ?2 AND leave_time IS NULL", params![ts_str, visit_id])?;
                    }
                    pending_room_name = None;
                    visit_local_name = None;
                    friendship_tracker.reset_visit();
                }
                my_display_name = Some(name_match.as_str().to_string());
                my_user_id = Some(user_id.to_string());
                authenticated_account = Some((user_id.to_string(), name_match.as_str().to_string()));
                log_accounts.insert(user_id.to_string(), name_match.as_str().to_string());
                if let Some(visit_id) = current_visit_id {
                    identify_visit_local_player(main_tx, visit_id, visit_local_name.as_deref(), authenticated_account.as_ref().map(|(id, name)| (id.as_str(), name.as_str())))?;
                }
            }
            continue;
        }

        // --- ワールド訪問ライフサイクル（入室・参加・退室） ---
        if let Some(caps) = RE_ENTERING.captures(&line) {
            visit_local_name = None;
            friendship_tracker.reset_visit();
            if let Some(visit_id) = current_visit_id {
                main_tx.execute("UPDATE visits SET leave_time = ?1 WHERE id = ?2 AND leave_time IS NULL", params![ts_str, visit_id])?;
                main_tx.execute("UPDATE with_users SET leave_time = ?1 WHERE visit_id = ?2 AND leave_time IS NULL", params![ts_str, visit_id])?;
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
                    params![session_id, room_name, instance_id, location.access_type, location.region, ts_str],
                )?;
                friendship_tracker.reset_visit();
                current_visit_id = Some(main_tx.last_insert_rowid());
                pending_room_name = None;
            }
            continue;
        }

        if RE_LEFT_ROOM.is_match(&line) {
            visit_local_name = None;
            if let Some(visit_id) = current_visit_id {
                main_tx.execute("UPDATE visits SET leave_time = ?1 WHERE id = ?2 AND leave_time IS NULL", params![ts_str, visit_id])?;
                main_tx.execute("UPDATE with_users SET leave_time = ?1 WHERE visit_id = ?2 AND leave_time IS NULL", params![ts_str, visit_id])?;
                current_visit_id = None;
                pending_room_name = None;
            }
            friendship_tracker.reset_visit();
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
                friendship_tracker.on_player_join(&display_name, &user_id);
                main_tx.execute(
                    "INSERT OR IGNORE INTO with_users (visit_id, vrchat_id, is_self, join_time)
                     VALUES (?1, ?2, 0, ?3)",
                    params![visit_id, user_id, ts_str],
                )?;
                if my_user_id.as_deref() == Some(&user_id) || visit_local_name.as_deref() == Some(&display_name) {
                    let identity = if visit_local_name.is_some() { authenticated_account.as_ref().map(|(id, name)| (id.as_str(), name.as_str())) } else { my_user_id.as_deref().zip(my_display_name.as_deref()) };
                    if let Some((local_id, local_name)) = identify_visit_local_player(main_tx, visit_id, visit_local_name.as_deref(), identity)? {
                        if visit_local_name.is_some() && authenticated_account.as_ref().map(|(id, _)| id.as_str()) != Some(local_id.as_str()) {
                            authenticated_account = None;
                        }
                        my_user_id = Some(local_id);
                        my_display_name = Some(local_name);
                    } else if visit_local_name.is_some() && authenticated_account.is_none() {
                        my_user_id = None;
                    }
                }
            }
            continue;
        }

        if let Some(caps) = RE_PLAYER_LEFT.captures(&line) {
            let (Some(display_match), Some(user_match)) = (caps.get(1), caps.get(2)) else {
                continue;
            };
            let display_name = display_match.as_str().to_string();
            let user_id = user_match.as_str().to_string();
            if let Some(visit_id) = current_visit_id {
                main_tx.execute(
                    "UPDATE with_users SET leave_time = ?1
                     WHERE visit_id = ?2 AND vrchat_id = ?3 AND leave_time IS NULL",
                    params![ts_str, visit_id, user_id],
                )?;
                friendship_tracker.on_player_left(&display_name, &user_id);
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
                my_display_name = Some(display_name.to_string());
                visit_local_name = Some(display_name.to_string());
                if let Some(visit_id) = current_visit_id {
                    if let Some((local_id, _)) = identify_visit_local_player(main_tx, visit_id, Some(display_name), authenticated_account.as_ref().map(|(id, name)| (id.as_str(), name.as_str())))? {
                        if authenticated_account.as_ref().map(|(id, _)| id.as_str()) != Some(local_id.as_str()) {
                            authenticated_account = None;
                        }
                        my_user_id = Some(local_id);
                    } else if authenticated_account.is_none() {
                        my_user_id = None;
                    }
                }
            }
            continue;
        }

        // --- スクリーンショット撮影イベント ---
        if let Some(caps) = RE_SCREENSHOT.captures(&line) {
            if let Some(path_match) = caps.get(1) {
                let width = caps.get(2).and_then(|m| m.as_str().parse::<i64>().ok());
                let height = caps.get(3).and_then(|m| m.as_str().parse::<i64>().ok());
                insert_screenshot(main_tx, current_visit_id, path_match.as_str(), width, height, &ts_str, session_id)?;
            }
            continue;
        }

        // --- OSC サービス検出（外部ツールのみ） ---
        if let Some(caps) = RE_OSC_FOUND.captures(&line) {
            let service_name = caps.get(1).map(|m| m.as_str());
            let ip_address = caps.get(2).map(|m| m.as_str());
            let port = caps.get(3).and_then(|m| m.as_str().parse::<i64>().ok());
            insert_osc_event(main_tx, session_id, "found", service_name, None, ip_address, port, &ts_str)?;
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
            let sub_id_opt = if subscription_id.is_empty() || subscription_id == "None" { None } else { Some(subscription_id) };
            let desc_opt = if desc.is_empty() { None } else { Some(desc) };
            insert_subscription_status(main_tx, session_id, is_active, sub_id_opt, desc_opt, &ts_str)?;
        }
    }
    if invalid_utf8_line_count > 0 {
        crate::utils::log_warn(&format!(message!("Analyze.parseAndImportReader.invalidUtf8Replaced"), filename = filename, invalid_utf8_line_count = invalid_utf8_line_count));
    }

    progress_callback(message!("Analyze.parseAndImportReader.committing").to_string(), String::new());

    if let Some(visit_id) = current_visit_id {
        let last_ts = current_ts.map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default();
        main_tx.execute("UPDATE visits SET leave_time = ?1 WHERE id = ?2 AND leave_time IS NULL", params![last_ts, visit_id])?;
        main_tx.execute("UPDATE with_users SET leave_time = ?1 WHERE visit_id = ?2 AND leave_time IS NULL", params![last_ts, visit_id])?;
    }

    // 途中の仮判定を残さず、各訪問で最終確定したローカル ID を集計する。
    let mut local_players = main_tx.prepare(
        "SELECT DISTINCT w.vrchat_id, u.account_name FROM with_users w
         JOIN visits v ON v.id = w.visit_id
         JOIN find_users u ON u.vrchat_id = w.vrchat_id
         WHERE v.session_id = ?1 AND w.is_self = 1",
    )?;
    for account in local_players.query_map([session_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))? {
        let (user_id, display_name) = account?;
        log_accounts.insert(user_id, display_name);
    }
    // sessions は1ログ1行。複数アカウントを単一の所有者へまとめず、各訪問の is_self を正とする。
    let (account_id, account_name) = if log_accounts.len() > 1 {
        (None, None)
    } else if let Some((user_id, display_name)) = log_accounts.into_iter().next() {
        (Some(user_id), Some(display_name))
    } else {
        (None, my_display_name)
    };
    main_tx.execute(
        "UPDATE sessions
         SET start_time = ?1, end_time = ?2, account_id = ?3, account_name = ?4
         WHERE log_name = ?5",
        params![start_time.unwrap_or_default(), end_time, account_id, account_name, filename],
    )?;

    Ok(())
}

/// 選択された複数のアーカイブまたはテキストログをメインデータベースにインポートする。
///
/// `run_diff_import` と異なり、任意パスのファイルを対象とするため
/// ファイル単位の失敗はバッチ全体をロールバックする（ユーザーが明示的に
/// 選択したファイルが壊れている場合、部分取り込みより失敗通知が適切なため）。
///
/// 各ファイルを savepoint でラップし、不正なログまたはキャンセル要求を検出した場合は
/// コミット前にバッチ全体をロールバックする。
///
/// # Errors
/// DB セットアップ失敗、バッチ全体のロールバック必要、またはコミット前の
/// キャンセル要求時にエラーを返す。
pub fn run_enhanced_import_batch<F>(main_db_path: &Path, target_paths: &[PathBuf], cancel_status: &AtomicBool, mut progress_callback: F) -> Result<(), String>
where
    F: FnMut(String, String),
{
    let mut main_conn = Connection::open(main_db_path).map_err(|e| analyze_err(&format!(message!("Analyze.runEnhancedImportBatch.databaseOpenFailed"), main_db_path.display()), e))?;

    init_main_db(&main_conn).map_err(|e| analyze_err(message!("Analyze.runEnhancedImportBatch.databaseFailed"), e))?;

    if target_paths.is_empty() {
        progress_callback(message!("Analyze.runEnhancedImportBatch.logText").to_string(), "0/0".to_string());
        return Ok(());
    }

    let mut main_tx = main_conn.transaction().map_err(|e| analyze_err(message!("Analyze.runEnhancedImportBatch.databaseTransactionBeginFailed"), e))?;

    let total = target_paths.len();
    for (index, target_path) in target_paths.iter().enumerate() {
        if let Err(err) = ensure_not_canceled(cancel_status) {
            rollback_outer_transaction(main_tx, message!("Analyze.runEnhancedImportBatch.cancelled"))?;
            return Err(err);
        }

        let filename = source_name_for_archive(target_path).ok_or_else(|| message!("Analyze.runEnhancedImportBatch.fileFailed").to_string())?;

        progress_callback(format!(message!("Analyze.runEnhancedImportBatch.text"), filename = filename), format_progress_fraction(index + 1, total));

        let main_sp = main_tx.savepoint().map_err(|e| analyze_err(message!("Analyze.runEnhancedImportBatch.databaseSavepointBeginFailed"), e))?;

        match import_log_in_savepoint(&main_sp, target_path, &filename, cancel_status, &mut progress_callback) {
            Ok(outcome) => {
                main_sp.commit().map_err(|e| analyze_err(message!("Analyze.runEnhancedImportBatch.databaseSavepointCommitFailed"), e))?;
                let status = match outcome {
                    ImportOutcome::Unchanged => format!(message!("Analyze.runEnhancedImportBatch.unchangedLogSkipped"), filename = filename),
                    ImportOutcome::Imported => format!(message!("Analyze.runEnhancedImportBatch.completed"), filename = filename),
                };
                progress_callback(status, format_progress_fraction(index + 1, total));
            }
            Err(err) if is_analyze_canceled_sqlite_err(&err) => {
                rollback_savepoint(main_sp, message!("Analyze.runEnhancedImportBatch.cancelled"))?;
                rollback_outer_transaction(main_tx, message!("Analyze.runEnhancedImportBatch.cancelled"))?;
                return Err(ANALYZE_CANCELED_MESSAGE.to_string());
            }
            Err(err) => {
                rollback_savepoint(main_sp, message!("Analyze.runEnhancedImportBatch.logRollbackText"))?;
                rollback_outer_transaction(main_tx, message!("Analyze.runEnhancedImportBatch.parseErrorContext"))?;
                return Err(analyze_err(message!("Analyze.runEnhancedImportBatch.logImportFailed"), err));
            }
        }
    }

    if let Err(err) = ensure_not_canceled(cancel_status) {
        rollback_outer_transaction(main_tx, message!("Analyze.runEnhancedImportBatch.cancelled"))?;
        return Err(err);
    }

    main_tx.commit().map_err(|e| analyze_err(message!("Analyze.runEnhancedImportBatch.databaseCommitFailed"), e))?;

    progress_callback(message!("Analyze.runEnhancedImportBatch.completedStatus").to_string(), format!("{total}/{total}"));
    Ok(())
}

#[cfg(test)]
include!("analyze_tests.rs");
