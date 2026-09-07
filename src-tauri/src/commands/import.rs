//! インポート制御: 拡張バッチインポート、デフォルト差分インポート、起動時インポート、
//! キャンセル。
//!
//! 全インポートエントリポイントはワーカースレッドを生成し即座に確認を
//! フロントエンドに返す。進捗は共有 `analyze-progress` イベントでストリーミングし、
//! 結果に関わらず `analyze-finished` イベントで完了を通知する。

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::analyze;
use crate::config;
use crate::utils;
use crate::AnalyzeCancelStatus;

use super::archive::{
    collect_pending_archive_sync_plans, resolve_managed_archive_path,
    sync_source_logs_into_archive_store,
};
use super::{emit_analyze_progress, get_archive_store_dir, get_db_path, get_source_log_dir};

/// 取り込みワーカー終了時に実行中フラグを必ず解除するためのガード。
struct AnalyzeRunGuard {
    running: Arc<AtomicBool>,
}

/// 差分インポート結果を最終進捗イベント用の文面へ変換する。
fn format_diff_import_completion(summary: &analyze::DiffImportSummary) -> (String, String) {
    let failed_count = summary.failed_filenames.len();
    if failed_count == 0 {
        return (
            "Data 内 zst アーカイブからの取り込みが完了しました。".to_string(),
            "100%".to_string(),
        );
    }

    let successful_count = summary.total_count.saturating_sub(failed_count);
    let mut filename_summary = summary
        .failed_filenames
        .iter()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .join("、");
    if failed_count > 5 {
        filename_summary.push_str("、ほか");
        filename_summary.push_str(&(failed_count - 5).to_string());
        filename_summary.push('件');
    }

    (
        format!(
            "{successful_count}件完了、{failed_count}件失敗（成功分は保存済み）: {filename_summary}"
        ),
        format!("{successful_count}/{}", summary.total_count),
    )
}

impl Drop for AnalyzeRunGuard {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// 新しい取り込みワーカーの開始権を取得し、共有キャンセルフラグを初期化する。
fn begin_analyze_run(
    cancel_status: &AnalyzeCancelStatus,
) -> Result<(Arc<AtomicBool>, AnalyzeRunGuard), String> {
    cancel_status
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "取り込みはすでに実行中です。完了後に再実行してください。".to_string())?;
    cancel_status.cancel.store(false, Ordering::SeqCst);
    Ok((
        cancel_status.cancel.clone(),
        AnalyzeRunGuard {
            running: cancel_status.running.clone(),
        },
    ))
}

/// ユーザー選択のアーカイブログをバックグラウンドスレッドでインポート開始する。
///
/// フロントエンドからアーカイブファイルピッカーで選択された `.tar.zst` ファイル名の
/// リストを受け取る。各ファイルを事前検証後、ワーカースレッドに引き渡し
/// アーカイブ処理ごとに `analyze-progress` イベントを送出する。
///
/// # Errors
/// 選択されたアーカイブが存在しない、または必要なパスを解決できない場合にエラーを返す。
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn launch_enhanced_import(
    app: AppHandle,
    file_names: Vec<String>,
    cancel_status: State<'_, AnalyzeCancelStatus>,
) -> Result<String, String> {
    let db_path = get_db_path()?;
    let zst_dir = get_archive_store_dir()?;
    let mut target_paths = Vec::new();
    let total = file_names.len();
    for file_name in file_names {
        let archive_path = resolve_managed_archive_path(&zst_dir, &file_name)?;
        if !archive_path.exists() {
            return Err(format!("ファイルが見つかりません: {file_name}"));
        }
        target_paths.push(archive_path);
    }
    target_paths.sort();

    let (cancel_flag, run_guard) = begin_analyze_run(&cancel_status)?;
    std::thread::spawn(move || {
        let _run_guard = run_guard;
        let result = analyze::run_enhanced_import_batch(
            &db_path,
            &target_paths,
            cancel_flag.as_ref(),
            |status, progress| {
                emit_analyze_progress(&app, status, progress, true);
            },
        );

        match result {
            Ok(()) => emit_analyze_progress(
                &app,
                format!("{total}件のインポートが完了しました。"),
                "100%".to_string(),
                false,
            ),
            Err(err) if err == analyze::ANALYZE_CANCELED_MESSAGE => emit_analyze_progress(
                &app,
                "キャンセルしました".to_string(),
                "0%".to_string(),
                false,
            ),
            Err(err) => {
                emit_analyze_progress(&app, format!("エラー: {err}"), "0%".to_string(), false);
            }
        }
        utils::emit_event_warn(&app, "analyze-finished", ());
    });

    Ok(format!("{total}件のアーカイブ同期を開始しました。"))
}
/// 起動時インポートを実行: ソースログを同期し、全アーカイブを差分インポートする。
///
/// `StellaRecord` 起動時に自動呼び出しされる。スプラッシュ画面に件数を表示できるよう
/// 概要を即座に返し、実際のインポートはバックグラウンドで実行する。
/// 未処理ログも既存アーカイブもない場合は完全にスキップする。
///
/// # Errors
/// データベースまたはアーカイブのパスを解決できない場合にエラーを返す。
#[tauri::command]
pub fn launch_startup_archive_import(
    app: AppHandle,
    cancel_status: State<'_, AnalyzeCancelStatus>,
) -> Result<(), String> {
    let preference = config::load_startup_import_preference();
    if !preference.preference_set || !preference.enabled {
        return Ok(());
    }

    let db_path = get_db_path()?;
    let source_dir = get_source_log_dir()?;
    let archive_store_dir = get_archive_store_dir()?;
    let (cancel_flag, run_guard) = begin_analyze_run(&cancel_status)?;
    std::thread::spawn(move || {
        let _run_guard = run_guard;
        let pending_count =
            match collect_pending_archive_sync_plans(&source_dir, &archive_store_dir) {
                Ok(plans) => plans.len(),
                Err(_) => 0,
            };

        let has_archives = archive_store_dir.is_dir()
            && fs::read_dir(&archive_store_dir)
                .map(|d| {
                    d.filter_map(std::result::Result::ok)
                        .any(|e| e.file_name().to_string_lossy().ends_with(".tar.zst"))
                })
                .unwrap_or(false);

        if pending_count == 0 && !has_archives {
            utils::emit_event_warn(&app, "analyze-finished", ());
            return;
        }

        if let Err(err) = sync_source_logs_into_archive_store(&source_dir, &archive_store_dir) {
            emit_analyze_progress(
                &app,
                format!("起動時 Data 同期に失敗しました: {err}"),
                "0%".to_string(),
                false,
            );
            utils::emit_event_warn(&app, "analyze-finished", ());
            return;
        }

        let result = analyze::run_diff_import(
            &db_path,
            &archive_store_dir,
            cancel_flag.as_ref(),
            |status, progress| {
                emit_analyze_progress(&app, status, progress, true);
            },
        );

        match result {
            Ok(summary) => {
                let (status, progress) = format_diff_import_completion(&summary);
                emit_analyze_progress(&app, status, progress, false);
            }
            Err(err) if err == analyze::ANALYZE_CANCELED_MESSAGE => emit_analyze_progress(
                &app,
                "キャンセルしました".to_string(),
                "0%".to_string(),
                false,
            ),
            Err(err) => {
                emit_analyze_progress(&app, format!("エラー: {err}"), "0%".to_string(), false);
            }
        }

        utils::emit_event_warn(&app, "analyze-finished", ());
    });

    Ok(())
}

/// 実行中のインポートに次のキャンセルチェックポイントで停止するよう通知する。
///
/// インポートワーカーがアーカイブエントリ間でポーリングするグローバルな
/// `AtomicBool` を設定する。最もシンプルなクロススレッドキャンセルパターンで、
/// ワーカーが粗い作業単位間でのみチェックするためチャネルや非同期協調は不要。
#[tauri::command]
pub async fn cancel_analyze(cancel_status: State<'_, AnalyzeCancelStatus>) -> Result<(), String> {
    cancel_status.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_import_completion_reports_partial_success() {
        let summary = analyze::DiffImportSummary {
            total_count: 3,
            failed_filenames: vec!["output_log_failed.txt".to_string()],
        };

        assert_eq!(
            format_diff_import_completion(&summary),
            (
                "2件完了、1件失敗（成功分は保存済み）: output_log_failed.txt".to_string(),
                "2/3".to_string(),
            )
        );
    }

    #[test]
    fn diff_import_completion_reports_full_success() {
        let summary = analyze::DiffImportSummary {
            total_count: 3,
            failed_filenames: Vec::new(),
        };

        assert_eq!(
            format_diff_import_completion(&summary),
            (
                "Data 内 zst アーカイブからの取り込みが完了しました。".to_string(),
                "100%".to_string(),
            )
        );
    }
}
