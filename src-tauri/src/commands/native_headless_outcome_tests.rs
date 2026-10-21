/// resource.libを含む所有PEから、抽出画像と実登録DBの再読込までを接続する。
#[test]
fn command_owned_icon_png_decodes_and_round_trips_through_catalog() -> NativeCommandTestResult {
    native_command_test("command_owned_icon_png_decodes_and_round_trips_through_catalog", |directory, _| {
        use std::os::windows::ffi::OsStrExt;
        use base64::Engine as _;
        use sha2::Digest as _;

        let executable = directory.path().join("画像を持つ所有PE ★.exe");
        fs::copy(std::env::current_exe()?, &executable)?;
        let original_sha = sha2::Sha256::digest(fs::read(&executable)?);
        let wide: Vec<u16> = executable.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let icons = unsafe { windows::Win32::UI::Shell::ExtractIconExW(windows::core::PCWSTR(wide.as_ptr()), -1, None, None, 0) };
        assert!(icons > 0, "所有PEに既存のproduction icon resourceがありません");
        let png = crate::platform::extract_exe_icon_png(&executable).ok_or("所有PEのiconを抽出できません")?;
        let decoded = image::load_from_memory_with_format(&png, image::ImageFormat::Png)?.to_rgba8();
        assert!(decoded.width() > 0 && decoded.height() > 0);
        assert!(decoded.pixels().any(|pixel| pixel.0[3] > 0));
        assert!(decoded.pixels().any(|pixel| Some(pixel) != decoded.pixels().next()));
        let unrelated = directory.path().join("保持する登録.exe");
        fs::write(&unrelated, b"owned unrelated non-PE")?;
        registry::register_app(unrelated.to_string_lossy().into_owned(), "対象外".to_string(), "保持する説明".to_string())?;
        registry::register_app(executable.to_string_lossy().into_owned(), "  画像付き  ".to_string(), "画像と同じ対象".to_string())?;
        let catalog = settings::read_registry_catalog();
        assert_eq!(catalog.apps.len(), 2);
        let target = catalog.apps.iter().find(|item| item.path == executable.to_string_lossy()).ok_or("登録対象がありません")?;
        assert_eq!(target.name, "画像付き");
        assert_eq!(target.description, "画像と同じ対象");
        let stored_png = base64::engine::general_purpose::STANDARD.decode(target.icon_data.as_deref().ok_or("保存iconがありません")?)?;
        assert_eq!(stored_png, png);
        assert_eq!(image::load_from_memory_with_format(&stored_png, image::ImageFormat::Png)?.to_rgba8(), decoded);
        let conn = Connection::open_with_flags(get_db_path()?, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let bytes = conn.query_row("SELECT icon FROM apps WHERE path=?1", [executable.to_string_lossy().as_ref()], |row| row.get::<_, Vec<u8>>(0))?;
        assert_eq!(bytes, stored_png);
        drop(conn);
        assert_eq!(native_viewer_json(settings::read_registry_catalog())?, native_viewer_json(&catalog)?);
        registry::unregister_app(executable.to_string_lossy().into_owned())?;
        let remaining = settings::read_registry_catalog();
        assert_eq!(remaining.apps.len(), 1);
        assert_eq!(remaining.apps[0].path, unrelated.to_string_lossy());
        assert_eq!(remaining.apps[0].description, "保持する説明");
        assert_eq!(sha2::Sha256::digest(fs::read(&executable)?), original_sha);
        assert_eq!(fs::read(&unrelated)?, b"owned unrelated non-PE");
        Ok(())
    })
}

/// 先行削除後の失敗を全体rollbackと誤認せず、同じ選択で残りだけを再試行する。
fn verify_cleanup_batch_failure_and_retry(changed: bool, directory: &tempfile::TempDir) -> NativeCommandTestResult {
    use std::os::windows::fs::OpenOptionsExt;

    let source = get_source_log_dir()?;
    let store = get_archive_store_dir()?;
    fs::create_dir_all(&source)?;
    fs::create_dir_all(&store)?;
    let names = ["output_log_2026-10-08_10-00-00.txt", "output_log_2026-10-08_11-00-00.txt", "output_log_2026-10-08_12-00-00.txt"];
    let bodies = [b"first owned log\n".as_slice(), b"second owned log\n".as_slice(), b"third owned log\n".as_slice()];
    for (name, body) in names.iter().zip(bodies) { fs::write(source.join(name), body)?; }
    fs::write(source.join("対象外.txt"), b"unrelated source")?;
    fs::write(store.join("retained.bin"), b"unrelated archive")?;
    assert_eq!(archive::sync_source_logs_into_archive_store(&source, &store)?, 3);
    let archives = names.iter().map(|name| fs::read(store.join(format!("{name}.tar.zst")))).collect::<std::io::Result<Vec<_>>>()?;
    let lock = if changed {
        fs::write(source.join(names[1]), b"changed after selection")?;
        None
    } else {
        // 読取りを許可しDELETE共有だけを拒否し、内容照合の後にremove_fileを失敗させる。
        Some(fs::OpenOptions::new().read(true).share_mode(3).open(source.join(names[1]))?)
    };
    let selection = names.map(ToString::to_string).to_vec();
    let error = archive::delete_source_logs(selection.clone()).err().ok_or("後続対象の故障が成功扱いです")?;
    assert!(error.contains(names[1]));
    assert!(error.contains(if changed { "現在内容がアーカイブと一致しません" } else { "削除に失敗しました" }));
    assert!(!source.join(names[0]).exists());
    assert_eq!(fs::read(source.join(names[1]))?, if changed { b"changed after selection".as_slice() } else { bodies[1] });
    assert_eq!(fs::read(source.join(names[2]))?, bodies[2]);
    assert_eq!(fs::read(source.join("対象外.txt"))?, b"unrelated source");
    for (name, bytes) in names.iter().zip(&archives) { assert_eq!(fs::read(store.join(format!("{name}.tar.zst")))?, *bytes); }
    assert_eq!(fs::read(store.join("retained.bin"))?, b"unrelated archive");
    drop(lock);
    if changed { fs::write(source.join(names[1]), bodies[1])?; }
    assert_eq!(archive::delete_source_logs(selection.clone())?, 2);
    assert_eq!(archive::delete_source_logs(selection)?, 0);
    for (index, name) in names.iter().enumerate() {
        assert!(!source.join(name).exists());
        assert_eq!(fs::read(store.join(format!("{name}.tar.zst")))?, archives[index]);
        native_busy_assert_archive(&store.join(format!("{name}.tar.zst")), name, std::str::from_utf8(bodies[index])?)?;
    }
    assert_eq!(fs::read(source.join("対象外.txt"))?, b"unrelated source");
    assert_eq!(fs::read(store.join("retained.bin"))?, b"unrelated archive");
    assert!(source.starts_with(directory.path()) && store.starts_with(directory.path()));
    Ok(())
}

#[test]
fn command_cleanup_changed_later_source_preserves_partial_result_and_retries_same_selection() -> NativeCommandTestResult {
    native_command_test("command_cleanup_changed_later_source_preserves_partial_result_and_retries_same_selection", |directory, _| verify_cleanup_batch_failure_and_retry(true, directory))
}

#[test]
fn command_cleanup_locked_later_source_preserves_partial_result_and_retries_same_selection() -> NativeCommandTestResult {
    native_command_test("command_cleanup_locked_later_source_preserves_partial_result_and_retries_same_selection", |directory, _| verify_cleanup_batch_failure_and_retry(false, directory))
}
