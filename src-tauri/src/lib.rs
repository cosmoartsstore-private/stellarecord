//! StellaRecord Tauri バックエンド。
//!
//! VRChat ログデータを管理する Tauri v2 デスクトップアプリの Rust 側。
//! 以下のモジュールを提供する:
//!
//! - **analyze** -- VRChat ログ解析と SQLite 取り込みパイプライン。
//! - **commands** -- React フロントエンドから呼び出される Tauri IPC コマンドハンドラ。
//! - **config** -- レジストリベースの設定読み書き。
//! - **models** -- バックエンドとフロントエンド間でやり取りする共有データ構造体。
//! - **platform** -- Windows 固有のユーティリティ（パニックフック、多重起動防止、
//!   レジストリ、プロセス起動）。
//! - **utils** -- ログ出力、エラー整形、レジストリ参照、イベント送信ヘルパー。

pub mod analyze;
pub mod commands;
pub mod config;
pub mod models;
pub mod platform;
pub mod utils;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::{WebviewUrl, WebviewWindowBuilder};

/// 長時間実行される解析タスクで共有するキャンセルフラグ。
pub struct AnalyzeCancelStatus(pub Arc<AtomicBool>);

/// `StellaRecord` Tauri アプリケーションを構築し実行する。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AnalyzeCancelStatus(Arc::new(AtomicBool::new(false))))
        .invoke_handler(tauri::generate_handler![
            commands::archive::list_archive_files,
            commands::archive::compress_logs,
            commands::archive::read_archive_log_viewer,
            commands::archive::get_pending_archive_log_count,
            commands::archive::get_storage_status,
            commands::archive::open_folder,
            commands::archive::get_deletable_source_logs,
            commands::archive::delete_source_logs,
            commands::archive::pick_log_folder,
            commands::archive::list_external_log_files,
            commands::archive::read_external_log_viewer,
            commands::database::get_db_tables,
            commands::database::get_db_table_data,
            commands::import::launch_enhanced_import,
            commands::import::launch_startup_archive_import,
            commands::import::cancel_analyze,
            commands::polaris::launch_external_app,
            commands::polaris::get_polaris_logs,
            commands::polaris::start_polaris,
            commands::polaris::get_polaris_status,
            commands::registry::pick_exe_file,
            commands::registry::register_app,
            commands::registry::unregister_app,
            commands::settings::get_management_settings,
            commands::settings::save_management_settings,
            commands::settings::read_registry_catalog,
        ])
        .setup(|app| {
            let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("STELLA RECORD")
                .inner_size(1280.0, 800.0)
                .maximized(true);

            if let Some(install_dir) = utils::get_stellarecord_install_dir() {
                builder = builder.data_directory(install_dir.join("Data").join("EBWebView"));
            }

            builder.build()?;
            Ok(())
        })
        .run(tauri::generate_context!());
    if let Err(err) = app {
        let msg = format!("Tauri アプリケーションの起動に失敗しました: {err}");
        utils::log_err(&msg);
        show_error_dialog(&msg);
    }
    #[allow(clippy::exit)]
    std::process::exit(0);
}

/// 致命的エラーを Windows メッセージボックスで表示する。
#[cfg(windows)]
fn show_error_dialog(msg: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let text = HSTRING::from(msg);
    let caption = HSTRING::from("StellaRecord");
    unsafe {
        MessageBoxW(None, &text, &caption, MB_ICONERROR | MB_OK);
    }
}

#[cfg(not(windows))]
fn show_error_dialog(_msg: &str) {}
