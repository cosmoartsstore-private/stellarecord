//! インポート制御: 拡張バッチインポート、デフォルト差分インポート、起動時インポート、
//! キャンセル。
//!
//! 全インポートエントリポイントはワーカースレッドを生成し即座に確認を
//! フロントエンドに返す。進捗は共有 `analyze-progress` イベントでストリーミングし、
//! 結果に関わらず `analyze-finished` イベントで完了を通知する。

use std::fs;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, State};

use crate::analyze;
use crate::utils;
use crate::AnalyzeCancelStatus;

use super::archive::{
    collect_pending_archive_sync_plans, resolve_managed_archive_path,
    sync_source_logs_into_archive_store,
};
use super::{emit_analyze_progress, get_archive_store_dir, get_db_path, get_source_log_dir};

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
    let cancel_flag = cancel_status.0.clone();
    cancel_flag.store(false, Ordering::SeqCst);

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

    std::thread::spawn(move || {
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
    let db_path = get_db_path()?;
    let source_dir = get_source_log_dir()?;
    let archive_store_dir = get_archive_store_dir()?;
    let cancel_flag = cancel_status.0.clone();
    cancel_flag.store(false, Ordering::SeqCst);

    std::thread::spawn(move || {
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
            Ok(()) => emit_analyze_progress(
                &app,
                "Data 内 zst アーカイブからの取り込みが完了しました。".to_string(),
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

    Ok(())
}

/// 実行中のインポートに次のキャンセルチェックポイントで停止するよう通知する。
///
/// インポートワーカーがアーカイブエントリ間でポーリングするグローバルな
/// `AtomicBool` を設定する。最もシンプルなクロススレッドキャンセルパターンで、
/// ワーカーが粗い作業単位間でのみチェックするためチャネルや非同期協調は不要。
#[tauri::command]
pub async fn cancel_analyze(cancel_status: State<'_, AnalyzeCancelStatus>) -> Result<(), String> {
    cancel_status.0.store(true, Ordering::SeqCst);
    Ok(())
}
