use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use crate::utils;

/// 予期しない異常終了をログファイルに記録するパニックフックを設定する。
///
/// `StellaRecord` はコンソールなしで動作し得るため、フロントエンドが
/// パニック詳細を表示できなくても障害を診断可能にする。
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = match info.location() {
            Some(location) => format!("{}:{}", location.file(), location.line()),
            None => "場所を特定できませんでした".to_string(),
        };
        let payload = info.payload();
        let message = if let Some(text) = payload.downcast_ref::<&str>() {
            text.to_string()
        } else if let Some(text) = payload.downcast_ref::<String>() {
            text.clone()
        } else {
            "panic メッセージを取得できませんでした".to_string()
        };

        utils::log_err(&format!("[PANIC] {message} ({location})"));
    }));
}

/// `StellaRecord` の多重起動を防止する。
///
/// 共有データベースとバックグラウンド処理を所有するため、単一インスタンス実行で
/// 重複書き込みや UI 状態の不整合を回避する。
///
/// # 戻り値
/// なし。別のインスタンスが既に起動中の場合はプロセスを即終了する。
pub fn ensure_single_instance() {
    #[cfg(windows)]
    {
        use std::mem::ManuallyDrop;

        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        use windows::Win32::System::Threading::CreateMutexW;

        let mutex_name: Vec<u16> = "Local\\StellaRecord_SingleInstance\0"
            .encode_utf16()
            .collect();
        let mutex = match unsafe { CreateMutexW(None, true, PCWSTR(mutex_name.as_ptr())) } {
            Ok(mutex) => mutex,
            Err(err) => {
                utils::log_warn(&format!(
                    "単一起動ガードを初期化できませんでした。多重起動を防げない可能性があります: {err}"
                ));
                return;
            }
        };
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            std::process::exit(0);
        }
        // プロセス終了までハンドルを保持し、他インスタンスが検出できるようにする。
        let _ = ManuallyDrop::new(mutex);
    }
}

/// Windows でコンソールウィンドウを表示せずに外部プロセスを起動する。
///
/// # エラー
/// 対象の実行ファイルを起動できない場合にエラーを返す。
pub fn launch_external_process(path: &str) -> Result<(), String> {
    let mut cmd = Command::new(path);
    // CREATE_NO_WINDOW (0x0800_0000) により、非コンソールプロセスから
    // GUI アプリを起動する際のコンソール表示を抑制する。
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);

    cmd.spawn()
        .map_err(|err| utils::command_err(&format!("起動に失敗しました [{path}]"), err))?;
    Ok(())
}

/// Windows スタートアップ一覧への登録・解除を行う。
///
/// # エラー
/// スタートアップのレジストリキーを更新できない場合にエラーを返す。
pub fn set_startup_enabled(value_name: &str, enabled: bool) -> Result<(), String> {
    let run_key = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .map_err(|err| utils::command_err("Run キーを開けませんでした", err))?
        .0;

    if enabled {
        let executable = std::env::current_exe().map_err(|err| {
            utils::command_err("自分自身の実行ファイルパスを取得できませんでした", err)
        })?;
        let command = format!("\"{}\"", executable.display());
        run_key
            .set_value(value_name, &command)
            .map_err(|err| utils::command_err("自動起動の登録に失敗しました", err))?;
    } else if let Err(err) = run_key.delete_value(value_name) {
        if err.kind() != std::io::ErrorKind::NotFound {
            return Err(utils::command_err("自動起動の解除に失敗しました", err));
        }
    }

    Ok(())
}

/// exe ファイルからアイコンを抽出し、PNG バイト列として返す。
pub fn extract_exe_icon_png(exe_path: &Path) -> Option<Vec<u8>> {
    use std::io::Cursor;
    use std::os::windows::ffi::OsStrExt;

    use image::{ImageFormat, RgbaImage};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::UI::Shell::ExtractIconExW;
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo};

    let wide_path: Vec<u16> = exe_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut large_icon = windows::Win32::UI::WindowsAndMessaging::HICON::default();
    let count = unsafe {
        ExtractIconExW(
            windows::core::PCWSTR(wide_path.as_ptr()),
            0,
            Some(&raw mut large_icon),
            None,
            1,
        )
    };
    if count == 0 || large_icon.is_invalid() {
        return None;
    }

    let result = (|| -> Option<Vec<u8>> {
        let mut icon_info = windows::Win32::UI::WindowsAndMessaging::ICONINFO::default();
        unsafe { GetIconInfo(large_icon, &raw mut icon_info) }.ok()?;

        let color_bmp = icon_info.hbmColor;
        let mask_bmp = icon_info.hbmMask;

        let cleanup_bitmaps = || unsafe {
            let _ = DeleteObject(color_bmp);
            let _ = DeleteObject(mask_bmp);
        };

        let mut bmp = BITMAP::default();
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let bmp_size = std::mem::size_of::<BITMAP>() as i32;
        if unsafe { GetObjectW(color_bmp, bmp_size, Some((&raw mut bmp).cast())) } == 0 {
            cleanup_bitmaps();
            return None;
        }

        let width = bmp.bmWidth;
        let height = bmp.bmHeight;
        if width <= 0 || height <= 0 {
            cleanup_bitmaps();
            return None;
        }

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (w, h) = (width as u32, height as u32);

        #[allow(clippy::cast_possible_truncation)]
        let mut info_header = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };

        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let hdc = unsafe { CreateCompatibleDC(None) };

        let scan_result = unsafe {
            GetDIBits(
                hdc,
                color_bmp,
                0,
                h,
                Some(pixels.as_mut_ptr().cast()),
                (&raw mut info_header).cast(),
                DIB_RGB_COLORS,
            )
        };

        let _ = unsafe { DeleteDC(hdc) };
        cleanup_bitmaps();

        if scan_result == 0 {
            return None;
        }

        // BGRA → RGBA
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }

        let img = RgbaImage::from_raw(w, h, pixels)?;
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).ok()?;
        Some(buf.into_inner())
    })();

    unsafe {
        let _ = DestroyIcon(large_icon);
    }
    result
}

/// exe の Windows VersionInfo から表示名を抽出する。
///
/// `FileDescription` を優先し、空または取得できない場合は `ProductName` にフォールバックし、
/// それも取得できない場合は拡張子を除いたファイル名を返す。完全に解決できない場合は `None`。
#[cfg(windows)]
pub fn read_exe_display_name(exe_path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;

    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };

    let wide_path: Vec<u16> = exe_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let file_stem_fallback = exe_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned());

    let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(wide_path.as_ptr()), None) };
    if size == 0 {
        return file_stem_fallback;
    }

    let mut buffer = vec![0u8; size as usize];
    if unsafe {
        GetFileVersionInfoW(
            PCWSTR(wide_path.as_ptr()),
            0,
            size,
            buffer.as_mut_ptr().cast(),
        )
    }
    .is_err()
    {
        return file_stem_fallback;
    }

    // 利用可能な翻訳一覧を取得し、最初の言語/コードページ組を採用する。
    let mut translations_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
    let mut translations_len: u32 = 0;
    let translation_subblock: Vec<u16> = "\\VarFileInfo\\Translation\0".encode_utf16().collect();
    let translations_ok = unsafe {
        VerQueryValueW(
            buffer.as_ptr().cast(),
            PCWSTR(translation_subblock.as_ptr()),
            &raw mut translations_ptr,
            &raw mut translations_len,
        )
    };

    let mut candidate_paths: Vec<(u16, u16)> = Vec::new();
    if translations_ok.as_bool() && !translations_ptr.is_null() && translations_len >= 4 {
        let count = (translations_len as usize) / 4;
        let pairs =
            unsafe { std::slice::from_raw_parts(translations_ptr.cast::<u16>(), count * 2) };
        for chunk in pairs.chunks_exact(2) {
            candidate_paths.push((chunk[0], chunk[1]));
        }
    }
    // 英語 (US) / Unicode のフォールバックも試す。
    candidate_paths.push((0x0409, 0x04B0));
    candidate_paths.push((0x0000, 0x04B0));

    let lookup_string = |lang: u16, codepage: u16, field: &str| -> Option<String> {
        let subblock = format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\{field}\0");
        let wide: Vec<u16> = subblock.encode_utf16().collect();
        let mut value_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut value_len: u32 = 0;
        let result = unsafe {
            VerQueryValueW(
                buffer.as_ptr().cast(),
                PCWSTR(wide.as_ptr()),
                &raw mut value_ptr,
                &raw mut value_len,
            )
        };
        if !result.as_bool() || value_ptr.is_null() || value_len == 0 {
            return None;
        }
        let slice = unsafe {
            std::slice::from_raw_parts(value_ptr.cast::<u16>(), value_len as usize)
        };
        // 末尾の NUL を除去
        let trimmed = match slice.iter().position(|c| *c == 0) {
            Some(pos) => &slice[..pos],
            None => slice,
        };
        let text = String::from_utf16_lossy(trimmed);
        let trimmed_text = text.trim();
        if trimmed_text.is_empty() {
            None
        } else {
            Some(trimmed_text.to_string())
        }
    };

    for field in ["FileDescription", "ProductName"] {
        for (lang, codepage) in &candidate_paths {
            if let Some(value) = lookup_string(*lang, *codepage, field) {
                return Some(value);
            }
        }
    }

    file_stem_fallback
}

#[cfg(not(windows))]
pub fn read_exe_display_name(exe_path: &Path) -> Option<String> {
    exe_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
}

/// ネイティブファイルダイアログで exe ファイルを選択する。
pub fn pick_exe_file_dialog() -> Result<Option<String>, String> {
    use std::os::windows::ffi::OsStringExt;

    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
    };

    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if hr.is_err() {
        return Err(format!("COM 初期化に失敗しました: {hr}"));
    }

    let result = (|| -> Result<Option<String>, String> {
        let dialog: IFileOpenDialog = unsafe {
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
        }
        .map_err(|e| format!("ファイルダイアログを作成できませんでした: {e}"))?;

        let filter_name = HSTRING::from("実行ファイル (*.exe)");
        let filter_spec = HSTRING::from("*.exe");
        let filters = [COMDLG_FILTERSPEC {
            pszName: PCWSTR(filter_name.as_ptr()),
            pszSpec: PCWSTR(filter_spec.as_ptr()),
        }];

        unsafe {
            dialog
                .SetFileTypes(&filters)
                .map_err(|e| format!("フィルター設定に失敗しました: {e}"))?;
            dialog
                .SetOptions(FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST)
                .map_err(|e| format!("オプション設定に失敗しました: {e}"))?;
        }

        let hr = unsafe { dialog.Show(None) };
        if hr.is_err() {
            return Ok(None);
        }

        let item = unsafe { dialog.GetResult() }
            .map_err(|e| format!("選択結果を取得できませんでした: {e}"))?;
        let display_name = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }
            .map_err(|e| format!("ファイルパスを取得できませんでした: {e}"))?;

        let path = unsafe {
            let len = (0..).take_while(|&i| *display_name.0.add(i) != 0).count();
            let slice = std::slice::from_raw_parts(display_name.0, len);
            std::ffi::OsString::from_wide(slice)
                .to_string_lossy()
                .into_owned()
        };

        unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(display_name.0.cast())) };

        Ok(Some(path))
    })();

    unsafe { CoUninitialize() };
    result
}

/// ネイティブフォルダ選択ダイアログでディレクトリを選択する。
pub fn pick_folder_dialog() -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;

        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::UI::Shell::{
            FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS,
            SIGDN_FILESYSPATH,
        };

        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr.is_err() {
            return Err(format!("COM 初期化に失敗しました: {hr}"));
        }

        let result = (|| -> Result<Option<String>, String> {
            let dialog: IFileOpenDialog =
                unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) }
                    .map_err(|e| format!("フォルダダイアログを作成できませんでした: {e}"))?;

            unsafe {
                dialog
                    .SetOptions(FOS_PICKFOLDERS | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST)
                    .map_err(|e| format!("オプション設定に失敗しました: {e}"))?;
            }

            let hr = unsafe { dialog.Show(None) };
            if hr.is_err() {
                return Ok(None);
            }

            let item = unsafe { dialog.GetResult() }
                .map_err(|e| format!("選択結果を取得できませんでした: {e}"))?;
            let display_name = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }
                .map_err(|e| format!("フォルダパスを取得できませんでした: {e}"))?;

            let path = unsafe {
                let len = (0..).take_while(|&i| *display_name.0.add(i) != 0).count();
                let slice = std::slice::from_raw_parts(display_name.0, len);
                std::ffi::OsString::from_wide(slice)
                    .to_string_lossy()
                    .into_owned()
            };

            unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(display_name.0.cast())) };

            Ok(Some(path))
        })();

        unsafe { CoUninitialize() };
        result
    }

    #[cfg(not(windows))]
    {
        Err("このプラットフォームではフォルダ選択ダイアログを利用できません。".to_string())
    }
}
