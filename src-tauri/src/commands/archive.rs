//! アーカイブ圧縮、同期計画、ログビューアストリーミング、ファイル管理。
//!
//! `.tar.zst` のライフサイクルを管理する: 設定された読込元directoryからの
//! `VRChat` 生ログ圧縮、アーカイブの新規作成・安全な置き換え判断、
//! ログビューア UI へのアーカイブ内容ストリーミング、アーカイブ済みソースログの
//! クリーンアップ操作（一覧表示/削除）。

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use tauri::AppHandle;
use tauri::Emitter;

use crate::analyze;
use crate::config;
use crate::models::{ArchiveFileItem, DeletableLogInfo, LogViewerChunk, LogViewerHighlight, LogViewerMeta, SourceLogCandidates};
use crate::platform;
use crate::utils;

use super::{get_archive_store_dir, get_db_path, get_source_log_dir};

/// 外部フォルダ閲覧で受け入れるログ拡張子。
const EXTERNAL_LOG_EXTENSIONS: &[&str] = &[".txt", ".tar.zst"];

/// パス区切りやドライブ指定を含まない単一ファイル名だけを受け入れる。
fn is_plain_file_name(file_name: &str) -> bool {
    !file_name.is_empty() && file_name != "." && file_name != ".." && !file_name.contains('/') && !file_name.contains('\\') && !file_name.contains(':')
}

/// `StellaRecord` 管理下のアーカイブストアで扱うファイル名か判定する。
pub(crate) fn matches_managed_archive_name(file_name: &str) -> bool {
    is_plain_file_name(file_name) && file_name.starts_with("output_log_") && file_name.ends_with(".txt.tar.zst")
}

/// 選択された読込元の削除対象として扱えるソースログファイル名か判定する。
fn matches_source_log_file_name(file_name: &str) -> bool {
    is_plain_file_name(file_name) && file_name.starts_with("output_log_") && file_name.ends_with(".txt")
}

/// 管理アーカイブストア内のファイル名を絶対パスへ解決する。
///
/// IPC 引数はFrontendの一覧選択経由が通常だが、コマンド境界では
/// 文字列を信用せず、単一ファイル名だけを許可して archive dir 外への脱出を防ぐ。
pub(crate) fn resolve_managed_archive_path(archive_store_dir: &Path, file_name: &str) -> Result<PathBuf, String> {
    if matches_managed_archive_name(file_name) {
        Ok(archive_store_dir.join(file_name))
    } else {
        Err(format!(message!("Commands.Archive.resolveManagedArchivePath.archiveFileInvalid"), file_name = file_name))
    }
}

/// ライブ `VRChat` ファイルに必要な共有フラグでソースログを1件開く。
///
/// 元ログは変更してはならないため、`StellaRecord` は許可的な共有で読み取りのみ行い、
/// ソースファイルのリネームや削除は一切行わない。
fn open_source_log_for_read(path: &Path) -> Result<fs::File, String> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::{FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE};

        fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0).open(path).map_err(|err| utils::command_open_err(path, err))
    }

    #[cfg(not(windows))]
    {
        fs::File::open(path).map_err(|err| utils::command_open_err(path, err))
    }
}

/// 開かれた出力ファイルへ単一ログを圧縮し、エンコーダーとファイルを明示的に確定する。
///
/// `auto_finish` の Drop 時エラーを捨てず、zstd 終端と OS バッファの同期に成功した
/// ファイルだけを最終アーカイブ名へ昇格できるようにする。
fn compress_single_file_to_output(src: &Path, output: fs::File) -> Result<(), String> {
    let mut encoder = zstd::stream::Encoder::new(output, 3).map_err(|err| utils::command_err(message!("Commands.Archive.compressSingleFileToOutput.zstdFailed"), err))?;
    let file_name = src.file_name().ok_or_else(|| format!(message!("Commands.Archive.compressSingleFileToOutput.fileFailed"), src.display()))?;
    let input = open_source_log_for_read(src)?;
    let metadata = input.metadata().map_err(|err| utils::command_read_err(src, err))?;
    let mut header = tar::Header::new_gnu();
    header.set_metadata(&metadata);
    // ライブログの追記を tar ヘッダーの宣言サイズへ混入させず、開始時点の範囲を保存する。
    let mut snapshot = input.take(metadata.len());
    {
        let mut tar = tar::Builder::new(&mut encoder);
        tar.append_data(&mut header, file_name, &mut snapshot).map_err(|err| utils::command_err(message!("Commands.Archive.compressSingleFileToOutput.tarArchiveFailed"), err))?;
        if snapshot.limit() != 0 {
            return Err(message!("Commands.Archive.compressSingleFileToOutput.sourceChanged").to_string());
        }
        tar.finish().map_err(|err| utils::command_err(message!("Commands.Archive.compressSingleFileToOutput.tarArchiveCommitFailed"), err))?;
    }
    let output = encoder.finish().map_err(|err| utils::command_err(message!("Commands.Archive.compressSingleFileToOutput.zstdArchiveCommitFailed"), err))?;
    output.sync_all().map_err(|err| utils::command_err(message!("Commands.Archive.compressSingleFileToOutput.archiveSynchronizeFailed"), err))?;
    Ok(())
}

/// `VRChat` の命名規則に合致するソースログファイルを収集する。
///
/// # 引数
/// * `source_dir` - 取り込み開始時点で確定した `VRChat` ログdirectory。
///
/// # 戻り値
/// filesystem列挙順でソートされたソース `.txt` ログパス。
fn collect_source_logs(source_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(source_dir).map_err(|err| utils::command_read_err(source_dir, err))?;
    let mut paths = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                utils::log_warn(&format!(message!("Commands.Archive.collectSourceLogs.logReadFailed"), err = err));
                continue;
            }
        };

        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let is_txt_log = Path::new(name).extension().is_some_and(|extension| extension.eq_ignore_ascii_case("txt"));
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
/// `reader_a` が宣言サイズ分すべて存在し、`reader_b` の先頭と一致する場合だけ成功する。
fn streaming_prefix_equal(reader_a: &mut impl Read, reader_b: &mut impl Read, expected_size: u64) -> std::io::Result<bool> {
    const CHUNK: usize = 64 * 1024;
    let mut buf_a = vec![0u8; CHUNK];
    let mut buf_b = vec![0u8; CHUNK];
    let mut compared = 0u64;
    loop {
        let na = fill_buf(reader_a, &mut buf_a)?;
        if na == 0 {
            return Ok(compared == expected_size);
        }
        if na as u64 > expected_size.saturating_sub(compared) {
            return Ok(false);
        }
        let nb = fill_buf(reader_b, &mut buf_b[..na])?;
        if na != nb || buf_a[..na] != buf_b[..nb] {
            return Ok(false);
        }
        compared += na as u64;
    }
}

/// ソースログ1件に新規または更新 `.tar.zst` アーカイブが必要か計画する。
///
/// 既存アーカイブはソースログがアーカイブ済みバイト列を厳密に拡張している
/// 場合のみ置き換える。ログの先頭が書き換わっている場合はデータ破損の
/// 可能性があるため安全側に倒してスキップし、警告をログに記録する。
fn build_archive_sync_plan(source_path: &Path, archive_store_dir: &Path) -> Result<Option<ArchiveSyncPlan>, String> {
    let file_name = source_path.file_name().and_then(|name| name.to_str()).ok_or_else(|| format!(message!("Commands.Archive.buildArchiveSyncPlan.fileFailed"), source_path.display()))?;
    let archive_path = archive_store_dir.join(format!("{file_name}.tar.zst"));
    if !archive_path.exists() {
        return Ok(Some(ArchiveSyncPlan { source_path: source_path.to_path_buf(), archive_path, mode: ArchiveSyncMode::Create }));
    }

    // アーカイブエントリのサイズを tar ヘッダーから取得し、ソースファイルサイズと比較する。
    // 全内容をメモリに読み込まず、ストリーミングでバイト比較を行う。
    let archive_file = fs::File::open(&archive_path).map_err(|err| utils::command_open_err(&archive_path, err))?;
    let decoder = zstd::stream::Decoder::new(archive_file).map_err(|err| utils::command_err(message!("Commands.Archive.buildArchiveSyncPlan.zstdFailed"), err))?;
    let mut archive = tar::Archive::new(decoder);
    let mut entries = archive.entries().map_err(|err| utils::command_err(message!("Commands.Archive.buildArchiveSyncPlan.zstdEntryEntryListReadFailed"), err))?;
    let Some(entry_result) = entries.next() else {
        return Err(format!(message!("Commands.Archive.buildArchiveSyncPlan.archiveLogFileMissing"), archive_path.display()));
    };
    let mut entry = entry_result.map_err(|err| utils::command_err(message!("Commands.Archive.buildArchiveSyncPlan.zstdEntryReadFailed"), err))?;
    let archived_size = entry.header().size().map_err(|err| utils::command_err(message!("Commands.Archive.buildArchiveSyncPlan.archiveEntryReadFailed"), err))?;

    let mut source_file = open_source_log_for_read(source_path)?;
    let source_size = source_file.metadata().map_err(|err| utils::command_read_err(source_path, err))?.len();

    if source_size < archived_size {
        utils::log_warn(&format!(message!("Commands.Archive.buildArchiveSyncPlan.zstdLogArchiveUpdateFailed"), file_name = file_name));
        return Ok(None);
    }

    if source_size == archived_size {
        let equal = streaming_prefix_equal(&mut entry, &mut source_file, archived_size).map_err(|err| utils::command_read_err(source_path, err))?;
        if !equal {
            utils::log_warn(&format!(message!("Commands.Archive.buildArchiveSyncPlan.zstdUpdateFailed"), file_name = file_name));
        }
        return Ok(None);
    }

    // source_size > archived_size: ソースがアーカイブ済み内容を厳密に拡張しているか確認
    let extends = streaming_prefix_equal(&mut entry, &mut source_file, archived_size).map_err(|err| utils::command_read_err(source_path, err))?;
    if !extends {
        utils::log_warn(&format!(message!("Commands.Archive.buildArchiveSyncPlan.archiveNotExtended"), file_name = file_name));
        return Ok(None);
    }

    Ok(Some(ArchiveSyncPlan { source_path: source_path.to_path_buf(), archive_path, mode: ArchiveSyncMode::Replace }))
}

/// 完全に保存済みのソースログを、追記・差し替え・削除を禁止した状態で返す。
///
/// 照合から削除まで同じハンドルを保持し、Polaris の同名ファイル置換と競合させない。
/// リンク先の実体を削除しないよう、末尾の再解析ポイントは追跡しない。
fn open_archived_source_for_cleanup(source_path: &Path, archive_path: &Path) -> Result<Option<fs::File>, String> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Foundation::GENERIC_READ;
    use windows::Win32::Storage::FileSystem::{DELETE, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ};

    if !archive_path.exists() {
        return Ok(None);
    }

    let source_name = source_path.file_name().and_then(|name| name.to_str()).ok_or_else(|| format!(message!("Commands.Archive.isSourceLogExactlyArchived.fileFailed"), source_path.display()))?;
    let mut source_file =
        fs::OpenOptions::new().access_mode(GENERIC_READ.0 | DELETE.0).share_mode(FILE_SHARE_READ.0).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0).open(source_path).map_err(|err| utils::command_open_err(source_path, err))?;
    let source_metadata = source_file.metadata().map_err(|err| utils::command_read_err(source_path, err))?;
    if !source_metadata.is_file() {
        return Ok(None);
    }

    let archive_file = fs::File::open(archive_path).map_err(|err| utils::command_open_err(archive_path, err))?;
    let decoder = zstd::stream::Decoder::new(archive_file).map_err(|err| utils::command_err(message!("Commands.Archive.isSourceLogExactlyArchived.zstdFailed"), err))?;
    let mut archive = tar::Archive::new(decoder);
    let mut entries = archive.entries().map_err(|err| utils::command_err(message!("Commands.Archive.isSourceLogExactlyArchived.zstdEntryEntryListReadFailed"), err))?;
    let Some(entry_result) = entries.next() else {
        return Ok(None);
    };
    let mut entry = entry_result.map_err(|err| utils::command_err(message!("Commands.Archive.isSourceLogExactlyArchived.zstdEntryReadFailed"), err))?;
    let archived_name = entry.path().map_err(|err| utils::command_err(message!("Commands.Archive.isSourceLogExactlyArchived.zstdEntryReadFailed"), err))?.file_name().and_then(|name| name.to_str()).map(str::to_owned);
    if archived_name.as_deref() != Some(source_name) || !entry.header().entry_type().is_file() {
        utils::log_warn(&format!(message!("Commands.Archive.isSourceLogExactlyArchived.logArchiveFileDeleteFailed"), archived_name = archived_name.as_deref().unwrap_or_default(), source_name = source_name));
        return Ok(None);
    }
    let archived_size = entry.header().size().map_err(|err| utils::command_err(message!("Commands.Archive.isSourceLogExactlyArchived.archiveEntryReadFailed"), err))?;
    if source_metadata.len() != archived_size {
        return Ok(None);
    }

    let is_equal = streaming_prefix_equal(&mut entry, &mut source_file, archived_size).map_err(|err| utils::command_read_err(source_path, err))?;
    Ok(is_equal.then_some(source_file))
}

/// 照合済みハンドルが指すファイルを削除する。パス名による開き直しは行わない。
fn delete_open_source_log(source_file: fs::File, source_path: &Path) -> Result<(), String> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO};

    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true.into() };
    #[allow(clippy::cast_possible_truncation)]
    let size = std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32;
    unsafe { SetFileInformationByHandle(HANDLE(source_file.as_raw_handle()), FileDispositionInfo, (&raw const disposition).cast(), size) }.map_err(|err| utils::command_remove_err(source_path, err))?;
    drop(source_file);
    Ok(())
}

/// `Data` でアーカイブまたは更新が必要なソースログを収集する。
///
/// # 引数
/// * `source_dir` - 取り込み開始時点で確定した `VRChat` ログdirectory。
/// * `archive_store_dir` - `StellaRecord` が管理する `.tar.zst` ファイルの格納directory。
///
/// # 戻り値
/// 新規または安全に置き換え可能なログのアーカイブ同期計画（順序付き）。
pub(crate) fn collect_pending_archive_sync_plans(source_dir: &Path, archive_store_dir: &Path) -> Result<Vec<ArchiveSyncPlan>, String> {
    let mut plans = Vec::new();
    for source_path in collect_source_logs(source_dir)? {
        if let Some(plan) = build_archive_sync_plan(&source_path, archive_store_dir)? {
            plans.push(plan);
        }
    }
    Ok(plans)
}

/// ファイルまたはdirectoryツリーの合計サイズを再帰的に計算する。
///
/// ストレージパネルは単純な合計サイズを必要とするため、アーカイブ
/// directoryを走査し、読み取り不可の子は警告付きで許容する。
fn collect_directory_size(path: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];

    while let Some(current) = stack.pop() {
        let metadata = fs::metadata(&current).map_err(|err| utils::command_err(&format!(message!("Commands.Archive.collectDirectorySize.readFailed"), current.display()), err))?;

        if metadata.is_file() {
            total += metadata.len();
            continue;
        }

        let entries = fs::read_dir(&current).map_err(|err| utils::command_read_err(&current, err))?;

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    utils::log_warn(&format!(message!("Commands.Archive.collectDirectorySize.storageEntryReadFailed"), err = err));
                    continue;
                }
            };

            stack.push(entry.path());
        }
    }

    Ok(total)
}

/// 同じディレクトリの一時ファイルで、既存名を一度の操作で置き換える。
///
/// 旧名を先に退避せず、処理中断時に正規のアーカイブ名が欠落する区間を作らない。
fn replace_file_atomically(temp_path: &Path, target_path: &Path) -> Result<(), String> {
    fs::rename(temp_path, target_path).map_err(|err| utils::command_err(&format!(message!("Commands.Archive.replaceFileAtomically.failed"), target_path.display()), err))
}

/// 圧縮中に元ログが変わっても、既存アーカイブの本文を欠落させる置換は拒否する。
fn validate_archive_replacement(temp_path: &Path, target_path: &Path) -> Result<(), String> {
    let open_archive = |path: &Path| {
        let file = fs::File::open(path).map_err(|err| utils::command_open_err(path, err))?;
        let decoder = zstd::stream::Decoder::new(file).map_err(|err| utils::command_err(message!("Commands.Archive.buildArchiveSyncPlan.zstdFailed"), err))?;
        Ok::<_, String>(tar::Archive::new(decoder))
    };
    let mut original = open_archive(target_path)?;
    let mut staged = open_archive(temp_path)?;
    let mut original_entries = original.entries().map_err(|err| utils::command_err(message!("Commands.Archive.buildArchiveSyncPlan.zstdEntryEntryListReadFailed"), err))?;
    let mut staged_entries = staged.entries().map_err(|err| utils::command_err(message!("Commands.Archive.buildArchiveSyncPlan.zstdEntryEntryListReadFailed"), err))?;
    let changed = || message!("Commands.Archive.writeArchiveSafely.contentChanged").to_string();
    let mut original_entry = original_entries.next().ok_or_else(changed)?.map_err(|err| utils::command_err(message!("Commands.Archive.buildArchiveSyncPlan.zstdEntryReadFailed"), err))?;
    let mut staged_entry = staged_entries.next().ok_or_else(changed)?.map_err(|err| utils::command_err(message!("Commands.Archive.buildArchiveSyncPlan.zstdEntryReadFailed"), err))?;
    let original_size = original_entry.header().size().map_err(|err| utils::command_err(message!("Commands.Archive.buildArchiveSyncPlan.archiveEntryReadFailed"), err))?;
    let staged_size = staged_entry.header().size().map_err(|err| utils::command_err(message!("Commands.Archive.buildArchiveSyncPlan.archiveEntryReadFailed"), err))?;
    let original_name = original_entry.path().map_err(|err| utils::command_read_err(target_path, err))?;
    let staged_name = staged_entry.path().map_err(|err| utils::command_read_err(temp_path, err))?;
    if !original_entry.header().entry_type().is_file() || !staged_entry.header().entry_type().is_file() || original_name != staged_name || staged_size < original_size {
        return Err(changed());
    }
    if !streaming_prefix_equal(&mut original_entry, &mut staged_entry, original_size).map_err(|err| utils::command_read_err(target_path, err))? {
        return Err(changed());
    }
    Ok(())
}

/// 一時ファイルへアーカイブを完全生成してから最終名へ昇格する。
///
/// 新規作成でも最終パスへ直接書かない。圧縮・同期・リネームのいずれかが失敗した
/// 場合は `TempPath` が一時ファイルを除去し、最終名に部分ファイルを残さない。
fn write_archive_safely(source_path: &Path, target_path: &Path, mode: ArchiveSyncMode) -> Result<(), String> {
    let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
    let target_name = target_path.file_name().and_then(|name| name.to_str()).ok_or_else(|| format!(message!("Commands.Archive.writeArchiveSafely.archiveFileFailed"), target_path.display()))?;
    let staged = tempfile::Builder::new().prefix(&format!(".{target_name}.")).suffix(".tmp").tempfile_in(parent).map_err(|err| utils::command_create_err(parent, err))?;
    let (output, temp_path) = staged.into_parts();
    let temp_path_buf = temp_path.to_path_buf();

    compress_single_file_to_output(source_path, output)?;

    match mode {
        ArchiveSyncMode::Create => {
            fs::rename(&temp_path_buf, target_path).map_err(|err| utils::command_err(&format!(message!("Commands.Archive.writeArchiveSafely.archiveFailed"), target_path.display()), err))?;
        }
        ArchiveSyncMode::Replace => {
            validate_archive_replacement(&temp_path_buf, target_path)?;
            replace_file_atomically(&temp_path_buf, target_path)?;
        }
    }

    // rename 後は元の一時パスが存在しないため、TempPath の Drop は何も削除しない。
    drop(temp_path);
    Ok(())
}

/// ソースログを管理された `Data` アーカイブストアに同期する。
///
/// 元ログは削除もリネームもしない。`StellaRecord` は自身の `.tar.zst`
/// コピーの作成またはアトミック置き換えのみ行う。
pub(crate) fn sync_source_logs_into_archive_store(source_dir: &Path, archive_store_dir: &Path) -> Result<usize, String> {
    fs::create_dir_all(archive_store_dir).map_err(|err| utils::command_create_err(archive_store_dir, err))?;

    let plans = collect_pending_archive_sync_plans(source_dir, archive_store_dir)?;
    for plan in &plans {
        write_archive_safely(&plan.source_path, &plan.archive_path, plan.mode)?;
    }

    Ok(plans.len())
}

/// ビューアの重要度をチャンク転送用のコンパクトな `u8` にエンコードする。
///
/// Frontendがこれを CSS クラスにデコードするため、数値マッピングは
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
/// ワールドへの接続要求やプレイヤーのロード完了は入退室に含めない。
fn encode_log_category_u8(category: &str) -> u8 {
    match category {
        "world" => 1,
        "notification" => 2,
        "player_join" => 3,
        "player_left" => 4,
        "video" => 5,
        "photo" => 6,
        _ => 0,
    }
}

/// `.tar.zst` アーカイブの最初の tar エントリのソースファイル名だけを取得する。
///
/// アーカイブ内容は展開せず、ヘッダーのみ読み取る。
fn read_archive_source_name(archive_path: &Path) -> Result<String, String> {
    let file = fs::File::open(archive_path).map_err(|err| utils::command_open_err(archive_path, err))?;
    let decoder = zstd::stream::Decoder::new(file).map_err(|err| utils::command_err(message!("Commands.Archive.readArchiveSourceName.zstdFailed"), err))?;
    let mut archive = tar::Archive::new(decoder);
    let mut entries = archive.entries().map_err(|err| utils::command_err(message!("Commands.Archive.readArchiveSourceName.zstdEntryEntryListReadFailed"), err))?;
    let Some(entry) = entries.next() else {
        return Err(format!(message!("Commands.Archive.readArchiveSourceName.archiveLogFileMissing"), archive_path.display()));
    };
    let entry = entry.map_err(|err| utils::command_err(message!("Commands.Archive.readArchiveSourceName.zstdEntryReadFailed"), err))?;
    let entry_path = entry.path().map_err(|err| utils::command_err(message!("Commands.Archive.readArchiveSourceName.zstdEntryPathFailed"), err))?;
    entry_path.file_name().and_then(|value| value.to_str()).ok_or_else(|| message!("Commands.Archive.readArchiveSourceName.archiveFileFailed").to_string()).map(str::to_string)
}

/// 本文の対応項目との完全一致に用いる DB 登録済み文字列。
#[derive(Clone)]
struct DbKeywordMarker {
    text: String,
}

/// 先頭形式のない行は、本文の語句や直前行に依存せずデバッグとして扱う。
fn classify_log_level(line: &str) -> &'static str {
    let Some(header) = analyze::RE_LOG_HEADER.captures(line) else {
        return "debug";
    };
    match header.get(2).map(|value| value.as_str()) {
        Some("Error") => "error",
        Some("Warning") => "warning",
        Some("Debug") => "debug",
        Some("Info" | "Log") => "info",
        _ => "plain",
    }
}

/// 本文の対象項目と照合する表示名と通知本文を DB から取得する。
fn collect_db_keyword_markers(conn: &rusqlite::Connection, source_name: &str) -> Result<Vec<DbKeywordMarker>, String> {
    let mut markers = Vec::new();

    let sql = "
        SELECT world_name
          FROM visits
         WHERE session_id IN (SELECT id FROM sessions WHERE log_name = ?1)
           AND world_name IS NOT NULL AND trim(world_name) <> ''
        UNION
        SELECT fu.account_name
          FROM with_users wu
          JOIN find_users fu ON fu.vrchat_id = wu.vrchat_id
          JOIN visits v ON v.id = wu.visit_id
          JOIN sessions s ON s.id = v.session_id
         WHERE s.log_name = ?1
           AND fu.account_name IS NOT NULL AND trim(fu.account_name) <> ''
        UNION
        SELECT message
          FROM notifications
         WHERE session_id IN (SELECT id FROM sessions WHERE log_name = ?1)
           AND message IS NOT NULL AND trim(message) <> ''
        UNION
        SELECT sender_name
          FROM notifications
         WHERE session_id IN (SELECT id FROM sessions WHERE log_name = ?1)
           AND sender_name IS NOT NULL AND trim(sender_name) <> ''
    ";

    let mut stmt = conn.prepare(sql).map_err(|err| utils::command_err(message!("Commands.Archive.collectDbKeywordMarkers.prepareFailed"), err))?;

    let rows = stmt.query_map([source_name], |row| row.get::<_, String>(0)).map_err(|err| utils::command_err(message!("Commands.Archive.collectDbKeywordMarkers.failed"), err))?;

    for row in rows {
        match row {
            Ok(text) => {
                let trimmed = text.trim();
                if trimmed.len() >= 2 {
                    markers.push(DbKeywordMarker { text: trimmed.to_string() });
                }
            }
            Err(err) => utils::log_warn(&format!(message!("Commands.Archive.collectDbKeywordMarkers.logFailed"), err = err)),
        }
    }

    // 対象項目の完全一致に用いるため、文字列順に並べて重複を除く。
    markers.sort_by(|left, right| left.text.cmp(&right.text));
    markers.dedup_by(|left, right| left.text == right.text);
    Ok(markers)
}

/// 本文の文字位置を JavaScript の UTF-16 オフセットへ変換する。
/// `start` と `end` は本文内の UTF-8 境界を指し、`end` は範囲に含めない。
fn log_highlight_range(body: &str, start: usize, end: usize) -> LogViewerHighlight {
    LogViewerHighlight { start: body[..start].encode_utf16().count(), end: body[..end].encode_utf16().count() }
}

/// DB 登録項目の強調範囲を、その項目の位置から求める。
///
/// 同じ文字列が送信者名と通知本文の両方にあっても、選択した項目だけを強調する。
fn resolve_db_highlight(body: &str, category: &str, markers: &[DbKeywordMarker]) -> Option<LogViewerHighlight> {
    let field_range = |value: &str, start: usize| {
        let trimmed = value.trim();
        if !markers.iter().any(|marker| marker.text == trimmed) {
            return None;
        }
        let trimmed_start = start + value.len() - value.trim_start().len();
        let trimmed_end = trimmed_start + trimmed.len();
        Some(log_highlight_range(body, trimmed_start, trimmed_end))
    };

    match category {
        "world" => {
            let prefix = "[Behaviour] Entering Room: ";
            field_range(body.strip_prefix(prefix)?, prefix.len())
        }
        "player_join" | "player_left" => {
            let pattern = if category == "player_join" { &*analyze::RE_PLAYER_JOIN } else { &*analyze::RE_PLAYER_LEFT };
            let capture = pattern.captures(body)?;
            let display_name = capture.get(1)?;
            field_range(display_name.as_str(), display_name.start())
        }
        "notification" => {
            if let Some(capture) = analyze::RE_NOTIFICATION.captures(body) {
                // 通知本文を送信者名より優先し、他の項目に同じ文字列があっても位置を混同しない。
                for index in [6, 1] {
                    if let Some(value) = capture.get(index) {
                        if let Some(range) = field_range(value.as_str(), value.start()) {
                            return Some(range);
                        }
                    }
                }
                return None;
            }
            // 複数行通知の開始行では、確定している送信者名だけを候補にする。
            let prefix = "Received Notification: <Notification from username:";
            let (sender, _) = body.strip_prefix(prefix)?.split_once(", sender user id:")?;
            field_range(sender, prefix.len())
        }
        _ => None,
    }
}

/// `ProTV` の管理オブジェクト名とメッセージを分離する。名前が異なるプレイヤーは混同しない。
fn protv_source_message(plain_body: &str) -> Option<(&str, &str)> {
    let (tag, message) = plain_body.split_once("] ")?;
    if !tag.starts_with("[AT") {
        return None;
    }
    let (_, source) = tag.split_once('\t')?;
    let source = source.trim_start();
    source.starts_with("TVManager (").then_some((source, message))
}

/// `VVMW` の同期状態と PC 用 URL を取り出す。`Player` は利用者ではなく再生バックエンドを示す。
struct VvmwPlayback<'a> {
    player: u8,
    state: u8,
    url: &'a str,
}

impl<'a> VvmwPlayback<'a> {
    fn parse(body: &'a str) -> Option<Self> {
        let (player, message) = body.strip_prefix("[VVMW] Deserialized: Player=")?.split_once(", State=")?;
        let (state, message) = message.split_once(", Time=")?;
        let (_, message) = message.split_once(", Speed=")?;
        let (_, message) = message.split_once(", URL(PC)=")?;
        let (url, _) = message.split_once(", URL(Quest)=")?;
        let url = url.trim();
        if !analyze::RE_VIDEO_URL.find(url).is_some_and(|matched| matched.start() == 0 && matched.end() == url.len()) {
            return None;
        }
        Some(Self { player: player.parse().ok()?, state: state.parse().ok()?, url })
    }
}

/// 同じ動画の状態再送を除き、再生中と確認できた最初の行を強調する。
///
/// 空の待機状態には管理オブジェクトの識別子がなく、別プレイヤーの出力も混在するため、
/// 再生済み URL を一括解除しない。一時停止・時刻補正でも解除せず、再読み込みか退室で解除する。
#[derive(Default)]
struct VvmwPlaybackContext {
    playing: HashSet<(u8, String)>,
}

impl VvmwPlaybackContext {
    fn observe(&mut self, body: &str) -> bool {
        let body = analyze::RE_LOG_RICH_TEXT.replace_all(body, "");
        if body == "[Behaviour] OnLeftRoom" || body.strip_prefix("[Behaviour] Entering Room: ").is_some_and(|name| !name.trim().is_empty()) {
            self.playing.clear();
        }
        let Some(playback) = VvmwPlayback::parse(&body) else {
            return false;
        };
        if playback.player == 0 {
            return false;
        }
        let key = (playback.player, playback.url.to_string());
        match playback.state {
            0 | 1 => {
                self.playing.remove(&key);
                false
            }
            2 => self.playing.insert(key),
            _ => false,
        }
    }
}

/// 動画の識別・読み込み・再生を示す既知の出力だけを対象にする。
/// 汎用的な URL 解決、一時的な配信先への接続、エラー、時刻同期などの内部処理は含めない。
fn is_video_body(plain_body: &str) -> bool {
    let has_url = analyze::RE_VIDEO_URL.is_match(plain_body);
    if let Some(message) = plain_body.strip_prefix("[USharpVideo] ") {
        return has_url && ["Started video load for URL: ", "Started video: ", "Playing synced "].iter().any(|prefix| message.starts_with(prefix));
    }
    if let Some(message) = plain_body.strip_prefix("[YamaStream] ") {
        let message = if message.starts_with('[') { message.split_once("] ").map_or(message, |(_, value)| value) } else { message };
        return (has_url && ["Play track: ", "Load url: ", "Play url: ", "auto play: ", "On track synced: ", "Try get video info: "].iter().any(|prefix| message.starts_with(prefix)))
            || message.strip_prefix("Loaded video info from YouTube: ").is_some_and(|title| !title.trim().is_empty())
            || ["Video start.", "Video play.", "Video ready.", "Video end.", "Video stop."].iter().any(|event| message.ends_with(event));
    }
    if let Some(message) = plain_body.strip_prefix("[iwaSync3] ") {
        return matches!(message, "The video has started playing." | "The video is ready.");
    }
    if let Some(title) = plain_body.strip_prefix("[YTTL] title:") {
        return !title.trim().is_empty();
    }
    if let Some((_, message)) = protv_source_message(plain_body) {
        return (has_url && (message.contains("] loading URL") || message.contains(" Now Playing: ") || message.starts_with("Variable value is: ") || message.starts_with("Preparing implicit TV autoplay media: ")))
            || matches!(message, "Media starting playing." | "Buffering complete." | "Ending media playback." | "Stopping current media");
    }
    // VVMW の同期行は前後の再生状態を使うため、VvmwPlaybackContext で分類する。
    if let Some(message) = plain_body.strip_prefix("[JLChnToZ.VRC.VVMW.Core] Send Event ") {
        return matches!(message, "_OnVideoBeginLoad" | "_onVideoReady" | "OnVideoPlay" | "_onVideoStart" | "_onVideoEnd");
    }
    if plain_body.starts_with("[VideoTXL:") {
        return has_url && plain_body.split_once("] ").is_some_and(|(_, message)| message.starts_with("Play video ") || message.starts_with("Load Url: "));
    }
    if plain_body.starts_with("[KineL] Video ready # ") {
        return has_url;
    }
    has_url && (plain_body.starts_with("[VRCVideoReserveItem] start load url=[") || plain_body.starts_with("[VRCVideoReserveItem] finish load url=["))
}

/// 動画の URL・タイトルを DB に依存せず強調する。指定者の名前は範囲に含めない。
fn resolve_video_highlight(body: &str, is_forwarded_title: bool) -> Option<LogViewerHighlight> {
    if let Some(playback) = VvmwPlayback::parse(body) {
        // PC と Quest の URL が同じでも、PC で使う URL だけを強調する。
        let start = body.find(", URL(PC)=")? + ", URL(PC)=".len();
        let start = start + body[start..].len() - body[start..].trim_start().len();
        return Some(log_highlight_range(body, start, start + playback.url.len()));
    }
    let title_marker = if is_forwarded_title {
        Some("Variable value is: ")
    } else if body.starts_with("[YTTL] title:") {
        Some("[YTTL] title:")
    } else if body.contains("Loaded video info from YouTube: ") {
        Some("Loaded video info from YouTube: ")
    } else {
        None
    };
    if let Some(marker) = title_marker {
        let start = body.find(marker)? + marker.len();
        let value = &body[start..];
        let title = value.trim();
        if title.is_empty() {
            return None;
        }
        let start = start + value.len() - value.trim_start().len();
        return Some(log_highlight_range(body, start, start + title.len()));
    }

    // USharpVideo は URL の後ろ、ProTV は URL の前に指定者名が出る。
    let url_body = body.split_once(" requested by ").map_or(body, |(value, _)| value);
    let first = analyze::RE_VIDEO_URL.find(url_body)?;
    let last = analyze::RE_VIDEO_URL.find_iter(url_body).last()?;
    let start = if body.contains("TVManager (") { last.start() } else { first.start() };
    let end = if body.starts_with("[VRCVideoReserveItem] finish load url=[") { body.trim_end().len() } else { last.end() };
    Some(log_highlight_range(body, start, end))
}

/// `ProTV` の変更ブロックでは旧タイトルを除き、新タイトルの値だけを強調する。
fn resolve_protv_title_change(line: &str) -> Option<LogViewerHighlight> {
    let changes = line.trim_start().strip_prefix("Title change ")?;
    let (_, new_title) = changes.split_once(" -> ")?;
    let title = new_title.trim();
    if title.is_empty() {
        return None;
    }
    let start = line.len() - new_title.trim_start().len();
    Some(log_highlight_range(line, start, start + title.len()))
}

/// `ProTV` のタイトル値を、同じ管理オブジェクトの出力に限定して関連付ける。
#[derive(Default)]
struct VideoTitleContext {
    is_changes: bool,
    pending_source: Option<String>,
}

impl VideoTitleContext {
    fn highlight(&mut self, line: &str, body: Option<&str>) -> Option<LogViewerHighlight> {
        let previous_source = self.pending_source.take();
        if let Some(body) = body {
            let plain_body = analyze::RE_LOG_RICH_TEXT.replace_all(body, "");
            let protv = protv_source_message(&plain_body);
            self.is_changes = protv.is_some_and(|(_, message)| message == "Deserialization Changes");
            if let Some((source, message)) = protv {
                if message.starts_with("Forwarding variable OUT_TITLE to ") {
                    self.pending_source = Some(source.to_string());
                }
                if previous_source.as_deref() == Some(source) && message.starts_with("Variable value is: ") {
                    return resolve_video_highlight(body, true);
                }
            }
            None
        } else {
            if line.trim().is_empty() {
                self.is_changes = false;
            }
            if self.is_changes {
                resolve_protv_title_change(line)
            } else {
                None
            }
        }
    }
}

/// 先頭形式とイベント本文が一致する行だけをカテゴリに分類する。
///
/// DB 時刻の一致、初期化、ロード完了、音声処理などからイベントを推測しない。
fn classify_text_category(line: &str) -> Option<&'static str> {
    let header = analyze::RE_LOG_HEADER.captures(line)?;
    let body = header.get(3)?.as_str().trim_end();

    if body.starts_with("[Behaviour] OnPlayerJoinComplete ") && analyze::RE_PLAYER_JOIN_COMPLETE.is_match(body) {
        return None;
    }
    if body.starts_with("[Behaviour] OnPlayerJoined ") && analyze::RE_PLAYER_JOIN.find(body).is_some_and(|matched| matched.start() == 0 && matched.end() == body.len()) {
        return Some("player_join");
    }
    if body.starts_with("[Behaviour] OnPlayerLeft ") && analyze::RE_PLAYER_LEFT.find(body).is_some_and(|matched| matched.start() == 0 && matched.end() == body.len()) {
        return Some("player_left");
    }
    if body.starts_with("Received Notification: <Notification ") {
        return Some("notification");
    }
    if body.strip_prefix("[Behaviour] Entering Room: ").is_some_and(|name| !name.trim().is_empty()) || body == "[Behaviour] Successfully joined room" || body == "[Behaviour] OnLeftRoom" {
        return Some("world");
    }
    if body.starts_with("[VRC Camera] Took screenshot to: ") && analyze::RE_SCREENSHOT.is_match(body) {
        return Some("photo");
    }
    let plain_body = analyze::RE_LOG_RICH_TEXT.replace_all(body, "");
    if is_video_body(&plain_body) {
        return Some("video");
    }
    None
}

/// 本文のカテゴリと DB 登録項目の強調範囲を行単位で送信する。
///
/// 先頭形式のない行はデバッグとし、`ProTV` の変更ブロック内のタイトルだけを追加で強調する。
fn emit_log_viewer_chunks(mut reader: impl BufRead, session_id: String, db_keyword_markers: Option<Vec<DbKeywordMarker>>, app: AppHandle) {
    // 1イベントあたり500行で IPC オーバーヘッドを抑える。
    const CHUNK_SIZE: usize = 500;

    let mut timestamps: Vec<String> = Vec::with_capacity(CHUNK_SIZE);
    let mut levels: Vec<u8> = Vec::with_capacity(CHUNK_SIZE);
    let mut categories: Vec<u8> = Vec::with_capacity(CHUNK_SIZE);
    let mut raw_lines: Vec<String> = Vec::with_capacity(CHUNK_SIZE);
    let mut highlights: Vec<Option<LogViewerHighlight>> = Vec::with_capacity(CHUNK_SIZE);

    let flush = |ts: &mut Vec<String>, lv: &mut Vec<u8>, cat: &mut Vec<u8>, rl: &mut Vec<String>, hl: &mut Vec<Option<LogViewerHighlight>>, sid: &str, app: &AppHandle| -> bool {
        app.emit(
            "log_viewer_chunk",
            &LogViewerChunk { session_id: sid.to_string(), timestamps: std::mem::take(ts), levels: std::mem::take(lv), categories: std::mem::take(cat), raw_lines: std::mem::take(rl), highlights: std::mem::take(hl) },
        )
        .is_ok()
    };

    let mut line_buffer = Vec::new();
    let mut video_titles = VideoTitleContext::default();
    let mut vvmw_playback = VvmwPlaybackContext::default();
    loop {
        line_buffer.clear();
        match reader.read_until(b'\n', &mut line_buffer) {
            Ok(0) => break,
            Ok(_) => {}
            Err(err) => {
                utils::log_warn(&format!(message!("Commands.Archive.emitLogViewerChunks.logReadFailed"), err = err));
                break;
            }
        }

        let line = utils::decode_log_line_lossy(&line_buffer);
        let header = analyze::RE_LOG_HEADER.captures(&line);
        let timestamp =
            header.as_ref().and_then(|capture| capture.get(1)).and_then(|value| NaiveDateTime::parse_from_str(value.as_str(), "%Y.%m.%d %H:%M:%S").ok()).map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default();
        let level = classify_log_level(&line);
        let body = header.as_ref().and_then(|capture| capture.get(3)).map(|value| value.as_str());
        let title_highlight = video_titles.highlight(&line, body);
        let is_vvmw_playback = body.is_some_and(|body| vvmw_playback.observe(body));
        let category = if title_highlight.is_some() || is_vvmw_playback { "video" } else { classify_text_category(&line).unwrap_or("plain") };
        let highlight =
            title_highlight.or_else(|| body.and_then(|body| if category == "video" { resolve_video_highlight(body, false) } else { db_keyword_markers.as_deref().and_then(|markers| resolve_db_highlight(body, category, markers)) }));

        timestamps.push(timestamp);
        levels.push(encode_log_level_u8(level));
        categories.push(encode_log_category_u8(category));
        raw_lines.push(line);
        highlights.push(highlight);

        if raw_lines.len() >= CHUNK_SIZE && !flush(&mut timestamps, &mut levels, &mut categories, &mut raw_lines, &mut highlights, &session_id, &app) {
            return;
        }
    }

    if !raw_lines.is_empty() {
        flush(&mut timestamps, &mut levels, &mut categories, &mut raw_lines, &mut highlights, &session_id, &app);
    }

    utils::emit_event_warn(&app, "log_viewer_done", session_id);
}

/// インポート可能なアーカイブ済み `.tar.zst` ファイルを一覧表示する。
///
/// # Errors
/// アーカイブdirectoryを読み取れない場合にエラーを返す。
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
                utils::log_warn(&format!(message!("Commands.Archive.listArchiveFiles.readFailed"), err = err));
                continue;
            }
        };

        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            if path.is_file() && matches_managed_archive_name(name) {
                let size_bytes = match entry.metadata() {
                    Ok(metadata) => metadata.len(),
                    Err(err) => {
                        utils::log_warn(&format!(message!("Commands.Archive.listArchiveFiles.archiveMetadataReadFailed"), path.display(), err));
                        0
                    }
                };
                files.push(ArchiveFileItem { name: name.to_string(), size_bytes });
            }
        }
    }

    files.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(files)
}

/// ソース名に対応する DB 登録文字列を本文の強調用に取得する。
///
/// DB がなくても本文によるカテゴリ分類と重要度の表示は利用できる。
fn build_db_keyword_markers(source_name: &str) -> Option<Vec<DbKeywordMarker>> {
    match get_db_path() {
        Ok(db_path) if db_path.exists() => match rusqlite::Connection::open(&db_path) {
            Ok(conn) => match collect_db_keyword_markers(&conn, source_name) {
                Ok(markers) => Some(markers),
                Err(err) => {
                    utils::log_warn(&format!(message!("Commands.Archive.buildDbHints.logKeywordReadFailed"), err = err));
                    None
                }
            },
            Err(err) => {
                utils::log_warn(&format!(message!("Commands.Archive.buildDbHints.logDatabaseOpenFailed"), db_path.display(), err));
                None
            }
        },
        Ok(_) => None,
        Err(err) => {
            utils::log_warn(&format!(message!("Commands.Archive.buildDbHints.logDatabasePathFailed"), err = err));
            None
        }
    }
}

/// 圧縮済み `.tar.zst` ログを別スレッドでストリーミングしてビューアイベントを送出する。
fn spawn_compressed_log_stream(archive_path: PathBuf, session_id: String, db_keyword_markers: Option<Vec<DbKeywordMarker>>, app: AppHandle) {
    std::thread::spawn(move || {
        let app_fallback = app.clone();
        let sid_fallback = session_id.clone();
        let result: Result<(), String> = (|| {
            let file = fs::File::open(&archive_path).map_err(|err| utils::command_open_err(&archive_path, err))?;
            let decoder = zstd::stream::Decoder::new(file).map_err(|err| utils::command_err(message!("Commands.Archive.spawnCompressedLogStream.zstdFailed"), err))?;
            let mut archive = tar::Archive::new(decoder);
            let mut entries = archive.entries().map_err(|err| utils::command_err(message!("Commands.Archive.spawnCompressedLogStream.zstdEntryEntryListReadFailed"), err))?;
            let Some(entry_result) = entries.next() else {
                return Err(format!(message!("Commands.Archive.spawnCompressedLogStream.archiveLogFileMissing"), archive_path.display()));
            };
            let mut entry = entry_result.map_err(|err| utils::command_err(message!("Commands.Archive.spawnCompressedLogStream.zstdEntryReadFailed"), err))?;
            emit_log_viewer_chunks(BufReader::new(&mut entry), session_id, db_keyword_markers, app);
            Ok(())
        })();
        if let Err(err) = result {
            utils::log_warn(&format!(message!("Commands.Archive.spawnCompressedLogStream.logFailed"), err = err));
            utils::emit_event_warn(&app_fallback, "log_viewer_done", sid_fallback);
        }
    });
}

/// プレーンテキスト `.txt` ログを別スレッドでストリーミングしてビューアイベントを送出する。
fn spawn_plain_log_stream(log_path: PathBuf, session_id: String, db_keyword_markers: Option<Vec<DbKeywordMarker>>, app: AppHandle) {
    std::thread::spawn(move || {
        let app_fallback = app.clone();
        let sid_fallback = session_id.clone();
        let result: Result<(), String> = (|| {
            let file = open_source_log_for_read(&log_path)?;
            emit_log_viewer_chunks(BufReader::new(file), session_id, db_keyword_markers, app);
            Ok(())
        })();
        if let Err(err) = result {
            utils::log_warn(&format!(message!("Commands.Archive.spawnPlainLogStream.logFailed"), err = err));
            utils::emit_event_warn(&app_fallback, "log_viewer_done", sid_fallback);
        }
    });
}

/// アーカイブファイル1件のストリーミングログビューアセッションを開く。
///
/// メタデータを即座に返し、バックグラウンドスレッドから `log_viewer_chunk` /
/// `log_viewer_done` イベントを送出して UI が行を漸進的に描画できるようにする。
///
/// # Errors
/// アーカイブファイルが見つからない、またはデコードできない場合にエラーを返す。
#[tauri::command]
pub fn read_archive_log_viewer(file_name: String, session_id: String, app: AppHandle) -> Result<LogViewerMeta, String> {
    let archive_path = resolve_managed_archive_path(&get_archive_store_dir()?, &file_name)?;
    if !archive_path.exists() {
        return Err(format!(message!("Commands.Archive.readArchiveLogViewer.fileMissing"), file_name = file_name));
    }

    let source_name = read_archive_source_name(&archive_path)?;
    let db_keyword_markers = build_db_keyword_markers(&source_name);

    // セッション ID により、前回のストリームがまだ飛行中にユーザーが別ファイルに
    // 切り替えた場合、Frontendが古いチャンクイベントを破棄できる。
    spawn_compressed_log_stream(archive_path, session_id.clone(), db_keyword_markers, app);

    Ok(LogViewerMeta { session_id, archive_name: file_name, source_name })
}

/// 現在のアーカイブdirectoryサイズと設定済み上限値を算出する。
///
/// # Errors
/// アーカイブdirectoryパスを解決またはスキャンできない場合にエラーを返す。
#[tauri::command]
pub fn get_storage_status() -> Result<(u64, u64), String> {
    let archive_dir = get_archive_store_dir()?;
    let setting = config::load_polaris_setting();

    let total_size = if archive_dir.exists() { collect_directory_size(&archive_dir)? } else { 0 };

    Ok((total_size, setting.capacity_threshold_bytes))
}

/// 指定されたフォルダパスを OS シェルで開く。
///
/// # Errors
/// フォルダを開けない場合にエラーを返す。
#[tauri::command]
pub fn open_folder(path: &str) -> Result<(), String> {
    opener::open(path).map_err(|err| utils::command_err(message!("Commands.Archive.openFolder.directoryOpenFailed"), err))
}

/// `.tar.zst` 内のログと現在内容が一致し、安全に削除可能なソースログを一覧表示する。
///
/// # Errors
/// ソースログdirectoryまたはアーカイブdirectoryを読み取れない場合にエラーを返す。
#[tauri::command]
pub fn get_deletable_source_logs() -> Result<SourceLogCandidates, String> {
    let source_dir = get_source_log_dir()?;
    let archive_dir = get_archive_store_dir()?;

    let entries = fs::read_dir(&source_dir).map_err(|err| format!(message!("Commands.Archive.getDeletableSourceLogs.logDirectoryReadFailed"), err = err))?;

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
            let source_file = match open_archived_source_for_cleanup(&path, &archive_path) {
                Ok(Some(file)) => file,
                Ok(None) => return None,
                Err(err) => {
                    utils::log_warn(&format!(message!("Commands.Archive.getDeletableSourceLogs.logVerifyFailed"), err = err, name = name));
                    return None;
                }
            };
            let size_bytes = source_file.metadata().map(|m| m.len()).unwrap_or(0);
            Some(DeletableLogInfo { file_name: name, size_bytes })
        })
        .collect();

    results.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(SourceLogCandidates { source_log_path: source_dir.display().to_string(), logs: results })
}

/// 外部フォルダのファイル名がログビューア対応フォーマットに合致するか判定する。
///
/// `VRChat` の `output_log_*` 命名を満たし、生ログ `.txt` または `StellaRecord` の
/// `.tar.zst` アーカイブ拡張子を持つ場合のみ受け入れる。
fn matches_external_log_format(name: &str) -> bool {
    if !name.starts_with("output_log_") {
        return false;
    }
    EXTERNAL_LOG_EXTENSIONS.iter().any(|ext| name.ends_with(*ext))
}

/// ネイティブダイアログでユーザーにログファイルを複数選択させる。
///
/// # 戻り値
/// 選択されたファイルの絶対パス一覧。キャンセル時は空の `Vec`。
#[tauri::command]
pub fn pick_log_files() -> Result<Vec<String>, String> {
    platform::pick_log_files_dialog()
}

/// 外部ログファイル1件のストリーミングログビューアセッションを開く。
///
/// ユーザーが選択した任意のファイルパスから `output_log_*.txt` または
/// `*.tar.zst` を直接読み込む。`.txt` はそのまま、`.tar.zst` は zstd を解凍して
/// 内部の最初のエントリを使用する。
///
/// # Errors
/// ファイルパスが不正、ファイルが見つからない、または開けない場合にエラーを返す。
#[tauri::command]
pub fn read_external_log_viewer(file_path: String, session_id: String, app: AppHandle) -> Result<LogViewerMeta, String> {
    let log_path = PathBuf::from(&file_path);
    let file_name = log_path.file_name().and_then(|n| n.to_str()).ok_or_else(|| message!("Commands.Archive.readExternalLogViewer.fileReadFailed").to_string())?.to_string();

    if !matches_external_log_format(&file_name) {
        return Err(format!(message!("Commands.Archive.readExternalLogViewer.logText"), file_name = file_name));
    }
    if !log_path.is_file() {
        return Err(format!(message!("Commands.Archive.readExternalLogViewer.fileMissing"), file_path = file_path));
    }

    let is_compressed = file_name.ends_with(".tar.zst");
    let source_name = if is_compressed { read_archive_source_name(&log_path)? } else { file_name.clone() };

    let db_keyword_markers = build_db_keyword_markers(&source_name);

    if is_compressed {
        spawn_compressed_log_stream(log_path, session_id.clone(), db_keyword_markers, app);
    } else {
        spawn_plain_log_stream(log_path, session_id.clone(), db_keyword_markers, app);
    }

    Ok(LogViewerMeta { session_id, archive_name: file_path, source_name })
}

/// 各ファイルの `.tar.zst` 内容が現在の元ログと一致することを確認して削除する。
///
/// # Errors
/// ファイル名が不正、アーカイブが存在しない、または削除に失敗した場合にエラーを返す。
#[tauri::command]
pub fn delete_source_logs(source_log_path: String, file_names: Vec<String>) -> Result<usize, String> {
    let source_dir = get_source_log_dir()?;
    // 一覧取得後に設定が変わった場合、別フォルダの同名ファイルへ削除対象を移さない。
    if source_dir != Path::new(&source_log_path) {
        return Err(message!("Commands.Archive.deleteSourceLogs.sourceDirectoryChanged").to_string());
    }
    let archive_dir = get_archive_store_dir()?;

    let mut deleted_count: usize = 0;

    for file_name in &file_names {
        if !matches_source_log_file_name(file_name) {
            return Err(format!(message!("Commands.Archive.deleteSourceLogs.fileInvalid"), file_name = file_name));
        }
        let source_path = source_dir.join(file_name);
        if source_path.exists() {
            let archive_path = archive_dir.join(format!("{file_name}.tar.zst"));
            if !archive_path.exists() {
                return Err(format!(message!("Commands.Archive.deleteSourceLogs.archiveTarZstdMissing"), file_name = file_name));
            }
            let source_file = open_archived_source_for_cleanup(&source_path, &archive_path)?.ok_or_else(|| format!(message!("Commands.Archive.deleteSourceLogs.logArchiveSynchronizeFailed"), file_name = file_name))?;
            delete_open_source_log(source_file, &source_path)?;
            deleted_count += 1;
        }
    }

    Ok(deleted_count)
}

#[cfg(test)]
include!("archive_tests.rs");
