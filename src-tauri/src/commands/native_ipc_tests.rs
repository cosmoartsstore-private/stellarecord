// 実BrowserのFrontend service・Tauri SDK・command dispatchを、専用HKCUとDataで接続する。
use std::borrow::Cow;
use tauri::utils::assets::{AssetKey, AssetsIter, CspHash};

struct NativeProbeAssets { files: std::collections::BTreeMap<String, Vec<u8>> }
impl tauri::Assets<tauri::Wry> for NativeProbeAssets {
    fn get(&self, key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        self.files.get(key.as_ref()).map(|bytes| Cow::Borrowed(bytes.as_slice()))
    }
    fn iter(&self) -> Box<AssetsIter<'_>> {
        Box::new(self.files.iter().map(|(name, bytes)| (Cow::Borrowed(name.as_str()), Cow::Borrowed(bytes.as_slice()))))
    }
    fn csp_hashes(&self, _: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> { Box::new(std::iter::empty()) }
}

#[derive(serde::Serialize)]
struct NativeProbeInputs {
    database_path: String, log_archive_path: String, owned_exe: String, archive_name: String, archive_bytes: u64,
    source_name: String, raw_lines: Vec<String>, external_path: String, external_lines: Vec<String>, invalid_external_path: String,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct NativeProbeReport { ok: bool, error: String, checks: Vec<String>, #[serde(default)] picker_results: Vec<Vec<String>>, #[serde(default)] viewer_failures: Vec<NativeViewerFailure>, #[serde(default)] viewer_starts: Vec<NativeViewerStart>, #[serde(default)] viewer_done_sessions: Vec<String>, #[serde(default)] viewer_chunks: Vec<NativeViewerChunkSample> }

#[derive(serde::Serialize, serde::Deserialize)]
struct NativeViewerChunkSample { session_id: String, rows: usize, first: String, last: String }

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
struct NativeViewerFailure { file_path: String, error: String }

#[derive(serde::Serialize, serde::Deserialize)]
struct NativeViewerStart { file_path: String, session_id: String, source_name: String }

#[derive(serde::Serialize)]
#[allow(clippy::struct_excessive_bools, reason = "既存の独立した故障fixture条件を本体UIへの診断入力として保持するため")]
struct NativeUiProbeInputs { database_path: String, log_archive_path: String, archive_name: String, source_name: String, raw_lines: Vec<String>, native_picker: bool, external_read_failure: bool, external_stream_lock: bool, external_stream_partial: bool, external_paths: Vec<String>, external_lines: Vec<String> }

/// 本体と検証用builderのcommand登録を照合し、検証側だけへ追加・改名した登録を通さない。
fn native_ipc_registration_matches_production() -> Result<(), Box<dyn std::error::Error>> {
    fn handlers(source: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let (_, list) = source.rsplit_once(".invoke_handler(tauri::generate_handler![").ok_or("command登録がありません")?;
        let (list, _) = list.split_once("])").ok_or("command登録の終端がありません")?;
        Ok(list.split(',').map(str::trim).filter(|item| !item.is_empty()).map(|item| item.trim_start_matches("crate::").to_string()).collect())
    }
    let production = handlers(include_str!("../lib.rs"))?; let test = handlers(include_str!("native_ipc_tests.rs"))?;
    assert_eq!(production.len(), 24); assert_eq!(production, test); Ok(())
}

fn native_browser_probe<T: serde::Serialize>(directory: &std::path::Path, inputs: T, mode: &str) -> Result<NativeProbeReport, Box<dyn std::error::Error>> {
    use std::os::windows::fs::OpenOptionsExt;
    use tauri::ipc::IpcResponse;
    native_ipc_registration_matches_production()?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().ok_or("Frontend rootがありません")?;
    let output = directory.join("browser-probe.js");
    let compiled = Command::new("node").arg(root.join("src/test/native-ipc/build-probe.ts")).arg(&output).arg(mode).creation_flags(0x0800_0000).output()?;
    assert!(compiled.status.success(), "Frontend probeをbuildできません: {}", String::from_utf8_lossy(&compiled.stderr));
    if mode == "ui-picker" { fs::write(directory.join("ui-picker.child-stage.txt"), "frontend-bundled")?; }
    let names = tauri::ipc::InvokeResponseBody::Json(fs::read_to_string(directory.join("browser-probe-assets.json"))?).deserialize::<Vec<String>>()?;
    let mut assets = std::collections::BTreeMap::new(); let boundary = directory.canonicalize()?;
    for name in names {
        assert!(name == "browser-probe.js" || name == "browser-probe.css" || name.starts_with("probe-assets/"));
        let path = directory.join(&name).canonicalize()?; assert!(path.starts_with(&boundary));
        assets.insert(format!("/{name}"), fs::read(path)?);
    }
    assert!(assets.contains_key("/browser-probe.js"));
    let stylesheet = if mode == "service" { "" } else { assert!(assets.contains_key("/browser-probe.css")); "<link rel=\"stylesheet\" href=\"browser-probe.css\">" };
    let tauri::ipc::InvokeResponseBody::Json(json) = inputs.body()? else { return Err("probe入力がJSONではありません".into()); };
    let html = format!("<!doctype html><html lang=\"ja\"><head><meta charset=\"UTF-8\"><title>STELLA_NATIVE_IPC_BOOT</title>{stylesheet}</head><body><div id=\"root\"></div><script id=\"probe-input\" type=\"application/json\">{}</script><script src=\"browser-probe.js\"></script></body></html>", json.replace('<', "\\u003c"));
    assets.insert("/index.html".to_string(), html.into_bytes());
    let mut context = tauri::generate_context!(); context.config_mut().app.windows.clear(); context.config_mut().build.dev_url = None;
    context.set_assets(Box::new(NativeProbeAssets { files: assets }));
    let (sender, receiver) = std::sync::mpsc::channel(); let profile = directory.join("EBWebView");
    assert!(profile.starts_with(directory)); let progress_root = directory.to_path_buf();
    let native_picker = mode == "ui-picker"; let stream_lock_path = directory.join("output_log_選択 ★ 直接.txt");
    let stream_lock = std::sync::Mutex::new(None::<fs::File>);
    let state = crate::AnalyzeCancelStatus { cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)), running: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)) };
    let app = tauri::Builder::default().any_thread().plugin(tauri_plugin_shell::init()).manage(state)
        .invoke_handler(tauri::generate_handler![
            crate::commands::archive::list_archive_files,
            crate::commands::archive::read_archive_log_viewer,
            crate::commands::archive::get_storage_status,
            crate::commands::archive::open_folder,
            crate::commands::archive::get_deletable_source_logs,
            crate::commands::archive::delete_source_logs,
            crate::commands::archive::pick_log_files,
            crate::commands::archive::read_external_log_viewer,
            crate::commands::database::get_db_tables,
            crate::commands::database::get_db_table_data,
            crate::commands::import::launch_enhanced_import,
            crate::commands::import::launch_startup_archive_import,
            crate::commands::import::cancel_analyze,
            crate::commands::registry::launch_external_app,
            crate::commands::registry::pick_exe_file,
            crate::commands::registry::extract_exe_display_name,
            crate::commands::registry::register_app,
            crate::commands::registry::unregister_app,
            crate::commands::settings::get_management_settings,
            crate::commands::settings::save_management_settings,
            crate::commands::settings::get_startup_import_settings,
            crate::commands::settings::save_startup_import_preference,
            crate::commands::settings::read_registry_catalog,
            crate::commands::log_client_error,
        ])
        .setup(move |app| {
            let window = tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into())).visible(false).inner_size(1280.0, 900.0).data_directory(profile.clone())
                .on_document_title_changed(move |window, title| {
                    if let Some(stage) = title.strip_prefix("STELLA_NATIVE_IPC_PROGRESS:") {
                        assert!(matches!(stage, "picker-initial-cancel" | "picker-select" | "picker-retry-select" | "picker-selected-cancel" | "stream-lock-acquire-before-read" | "stream-lock-release-before-retry"));
                        assert!(fs::write(progress_root.join("ui-picker.progress.txt"), stage).is_ok(), "pickerの開始段階を保存できません");
                        if stage.starts_with("stream-lock-") {
                            // 専用windowから要求された検証用fileだけをlockし、実SDKの呼出し前に完了を返す。
                            assert!(native_picker && stream_lock_path.parent() == Some(progress_root.as_path()));
                            let result = (|| -> NativeCommandTestResult {
                                let mut owned = stream_lock.lock().map_err(|_| "専用stream lockの状態を取得できません")?;
                                let state = if stage == "stream-lock-acquire-before-read" {
                                    assert!(owned.is_none()); *owned = Some(fs::OpenOptions::new().read(true).share_mode(0).open(&stream_lock_path)?); "held"
                                } else { drop(owned.take().ok_or("解除対象の専用stream lockがありません")?); "released" };
                                fs::write(progress_root.join("ui-picker.stream-lock-state.txt"), state)?; drop(owned);
                                window.eval("window.__STELLA_NATIVE_STREAM_LOCK_READY__?.()")?; Ok(())
                            })();
                            assert!(result.is_ok(), "専用stream lockの操作に失敗しました: {result:?}");
                        }
                    }
                    if let Some(stage) = title.strip_prefix("STELLA_NATIVE_IPC_UI_CHECK:") {
                        assert!(stage.len() <= 100 && stage.chars().all(|value| value.is_ascii_lowercase() || value == '-'));
                        assert!(fs::write(progress_root.join("ui-picker.last-check.txt"), stage).is_ok(), "画面の完了段階を保存できません");
                    }
                    if let Some(result) = title.strip_prefix("STELLA_NATIVE_IPC_RESULT:") { let _ = sender.send(result.to_string()); }
                }).build()?;
            assert!(!window.is_visible()?);
            Ok(())
        }).build(context)?;
    if mode == "ui-picker" { fs::write(directory.join("ui-picker.child-stage.txt"), "native-builder-created")?; }
    let handle = app.handle().clone();
    let supervisor = std::thread::spawn(move || {
        let result = receiver.recv_timeout(std::time::Duration::from_secs(60)); handle.exit(i32::from(result.is_err())); result
    });
    let exit_code = app.run_return(|_, _| {});
    let result = supervisor.join().map_err(|_| "専用IPC監視threadが異常終了しました")??;
    assert_eq!(exit_code, 0);
    if mode == "ui-picker" { fs::write(directory.join("ui-picker.report.json"), &result)?; }
    let report = tauri::ipc::InvokeResponseBody::Json(result).deserialize::<NativeProbeReport>()?;
    assert!(report.ok, "Browserの確認が失敗しました。完了={:?}: {}", report.checks, report.error); Ok(report)
}

#[test]
fn command_browser_ipc_connects_actual_frontend_services() -> NativeCommandTestResult {
    native_command_test("command_browser_ipc_connects_actual_frontend_services", |directory, _| {
        let name = "output_log_2026-10-08_12-00-00.txt"; let archive_name = create_native_import_archive(name, import_log())?;
        let archive_bytes = fs::metadata(get_archive_store_dir()?.join(&archive_name))?.len();
        drop(open_initialized_main_db()?); let owned_exe = directory.path().join("OwnedApp.exe"); fs::write(&owned_exe, b"owned fixture")?;
        let database_path = directory.path().join("db/current.sqlite"); let log_archive_path = directory.path().join("archive");
        assert_eq!(get_db_path()?, database_path); assert_eq!(get_archive_store_dir()?, log_archive_path);
        let external_path = directory.path().join("output_log_2026-10-08_13-00-00.txt"); let invalid_external_path = directory.path().join("external.txt");
        let external_lines = (0..501).map(|index| format!("2026.10.08 13:00:00 Log - 専用の外部ログ{index}")).collect::<Vec<_>>();
        fs::write(&external_path, external_lines.join("\n"))?; fs::write(&invalid_external_path, b"invalid fixture")?;
        let report = native_browser_probe(directory.path(), NativeProbeInputs {
            database_path: database_path.display().to_string(), log_archive_path: log_archive_path.display().to_string(), owned_exe: owned_exe.display().to_string(),
            archive_name, archive_bytes, source_name: name.to_string(), raw_lines: import_log().lines().map(str::to_string).collect(),
            external_path: external_path.display().to_string(), external_lines: external_lines.clone(), invalid_external_path: invalid_external_path.display().to_string(),
        }, "service")?;
        assert_eq!(report.checks, ["settings-roundtrip", "catalog-and-database", "archive-and-storage", "invalid-ipc-arguments", "import-events-and-unlisten", "archive-viewer-events", "external-viewer-events"]);
        assert_eq!(config::load_polaris_setting().capacity_threshold_bytes, 512 * 1_048_576);
        assert_eq!(fs::read_to_string(get_source_log_dir()?.join(name))?, import_log());
        assert_eq!(fs::read_to_string(&external_path)?, external_lines.join("\n"));
        assert_eq!(fs::read(&invalid_external_path)?, b"invalid fixture");
        let conn = Connection::open(get_db_path()?)?; assert_eq!(conn.query_row("SELECT count(*) FROM apps", [], |row| row.get::<_, i64>(0))?, 0);
        assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 1);
        assert_eq!(conn.query_row("SELECT count(*) FROM visits", [], |row| row.get::<_, i64>(0))?, 1); Ok(())
    })
}

/// 本体のmain・App・hookを通し、画面の保存・解除・取込・再表示と永続化を照合する。
fn native_ui_probe(name: &str, mode: &str) -> NativeCommandTestResult {
    native_command_test(name, |directory, _| verify_native_ui_probe(directory.path(), mode, false, false, false, false))
}

#[allow(clippy::fn_params_excessive_bools, reason = "既存fixtureの診断条件を維持し、途中stream故障の条件だけを追加するため")]
fn verify_native_ui_probe(directory: &std::path::Path, mode: &str, native_picker: bool, external_read_failure: bool, external_stream_lock: bool, external_stream_partial: bool) -> NativeCommandTestResult {
    assert!([external_read_failure, external_stream_lock, external_stream_partial].into_iter().filter(|value| *value).count() <= 1);
    assert!((!external_read_failure && !external_stream_lock && !external_stream_partial) || native_picker);
    let source_name = "output_log_2026-10-08_12-00-00.txt"; let archive_name = create_native_import_archive(source_name, import_log())?;
    drop(open_initialized_main_db()?); let owned_exe = directory.join("OwnedApp.exe"); fs::write(&owned_exe, b"owned fixture")?;
    registry::register_app(owned_exe.display().to_string(), "検証用アプリ".to_string(), "保存した説明".to_string())?;
    let original_archive = fs::read(get_archive_store_dir()?.join(&archive_name))?;
    let report = native_browser_probe(directory, NativeUiProbeInputs {
        database_path: get_db_path()?.display().to_string(), log_archive_path: get_archive_store_dir()?.display().to_string(),
        archive_name: archive_name.clone(), source_name: source_name.to_string(), raw_lines: import_log().lines().map(str::to_string).collect(),
        native_picker, external_read_failure, external_stream_lock, external_stream_partial, external_paths: if native_picker { vec![directory.join("output_log_選択 ★ 直接.txt").display().to_string(), directory.join("output_log_選択 ★ 圧縮.tar.zst").display().to_string()] } else { vec![] },
        external_lines: if native_picker { (0..501).map(|index| format!("2026.10.08 13:00:00 Log - 外部ログ{index} 日本語と\"引用符\"")).collect() } else { vec![] },
    }, mode)?;
    let mut expected = vec!["initial-consent-and-render", "launcher-unregister", "settings-validation-save-and-theme", "archive-selection-and-import", "viewer-filter-close-and-reopen", "database-preview-and-sort"];
    if native_picker {
        expected.push("native-external-picker-cancel");
        if external_read_failure { expected.extend(["native-external-picker-select-and-read-failure", "native-external-picker-retry-after-read-failure"]); }
        else if external_stream_lock { expected.extend(["native-external-picker-select-and-stream-open-failure", "native-external-picker-retry-same-file-after-lock"]); }
        else if external_stream_partial { expected.extend(["native-external-picker-select-and-partial-stream-failure", "native-external-picker-retry-same-file-after-partial-read"]); }
        else { expected.push("native-external-picker-select-and-view"); }
        expected.extend(["native-external-picker-cancel-after-selection", "external-selection-clear-and-default-viewer"]);
    }
    assert_eq!(report.checks, expected);
    if external_read_failure {
        let path = directory.join("output_log_選択 ★ 圧縮.tar.zst").display().to_string();
        assert_eq!(report.viewer_failures, [NativeViewerFailure { file_path: path.clone(), error: format!("アーカイブ内にログファイルがありません: {path}") }]);
    } else { assert!(report.viewer_failures.is_empty()); }
    if external_stream_lock {
        let path = directory.join("output_log_選択 ★ 直接.txt").display().to_string(); assert_eq!(report.viewer_starts.len(), 2);
        for started in &report.viewer_starts { assert_eq!(started.file_path, path); assert_eq!(started.source_name, "output_log_選択 ★ 直接.txt"); assert!(report.viewer_done_sessions.contains(&started.session_id)); }
        assert_ne!(report.viewer_starts[0].session_id, report.viewer_starts[1].session_id);
    }
    if external_stream_partial {
        let path = directory.join("output_log_選択 ★ 圧縮.tar.zst").display().to_string(); assert_eq!(report.viewer_starts.len(), 2);
        for (index, started) in report.viewer_starts.iter().enumerate() {
            assert_eq!(started.file_path, path); assert_eq!(started.source_name, "output_log_選択 ★ 直接.txt");
            assert!(report.viewer_done_sessions.contains(&started.session_id));
            let chunks = report.viewer_chunks.iter().filter(|chunk| chunk.session_id == started.session_id).collect::<Vec<_>>();
            assert_eq!(chunks.iter().map(|chunk| chunk.rows).collect::<Vec<_>>(), [500, if index == 0 { 1 } else { 2 }]);
            assert_eq!(chunks[0].first, "2026.10.08 13:00:00 Log - 外部ログ0 日本語と\"引用符\"");
            assert_eq!(chunks[0].last, "2026.10.08 13:00:00 Log - 外部ログ499 日本語と\"引用符\"");
            assert_eq!(chunks[1].first, "2026.10.08 13:00:00 Log - 外部ログ500 日本語と\"引用符\"");
            assert_eq!(chunks[1].last, if index == 0 { "2026.10.08 13:00:00 Log - 外部ログ500 日本語と\"引用符\"" } else { "2026.10.08 13:00:00 Log - 修復後の最終行" });
        }
        assert_ne!(report.viewer_starts[0].session_id, report.viewer_starts[1].session_id);
    }
    assert_eq!(config::load_polaris_setting().capacity_threshold_bytes, 512 * 1_048_576);
    let preference = settings::get_startup_import_settings()?; assert!(!preference.enabled && preference.preference_set);
    assert_eq!(fs::read_to_string(get_source_log_dir()?.join(source_name))?, import_log());
    assert_eq!(fs::read(get_archive_store_dir()?.join(archive_name))?, original_archive); assert_eq!(fs::read(owned_exe)?, b"owned fixture");
    let conn = Connection::open(get_db_path()?)?; assert_eq!(conn.query_row("SELECT count(*) FROM apps", [], |row| row.get::<_, i64>(0))?, 0);
    assert_eq!(conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get::<_, i64>(0))?, 1);
    assert_eq!(conn.query_row("SELECT count(*) FROM visits", [], |row| row.get::<_, i64>(0))?, 1);
    Ok(())
}

#[test]
fn command_browser_ui_connects_actual_main_in_development() -> NativeCommandTestResult {
    native_ui_probe("command_browser_ui_connects_actual_main_in_development", "ui")
}

#[test]
fn command_browser_ui_connects_actual_main_in_production() -> NativeCommandTestResult {
    native_ui_probe("command_browser_ui_connects_actual_main_in_production", "ui-production")
}
