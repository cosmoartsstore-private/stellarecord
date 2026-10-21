use std::ffi::c_void;
use std::path::PathBuf;

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(key: isize, name: *const u16, options: u32, access: u32, result: *mut isize) -> i32;
    fn RegGetValueW(key: isize, subkey: *const u16, name: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, bytes: *mut u32) -> i32;
    fn RegOverridePredefKey(key: isize, replacement: isize) -> i32;
}
#[link(name = "kernel32")]
extern "system" { fn GetCurrentThreadId() -> u32; }
#[link(name = "user32")]
extern "system" {
    fn GetThreadDesktop(thread: u32) -> *mut c_void;
    fn GetUserObjectInformationW(object: *mut c_void, index: i32, data: *mut c_void, bytes: u32, required: *mut u32) -> i32;
}
fn wide(text: &str) -> Vec<u16> { text.encode_utf16().chain([0]).collect() }

// crate属性だけを除いた現行main.rsを取り込む。mainの本文は順序・呼出しとも編集しない。
mod production_entry {
    include!(env!("STELLA_STARTUP_ENTRY_SOURCE"));
    pub(super) fn execute() { main(); }
}

fn main() {
    let root = PathBuf::from(std::env::var_os("STELLA_STARTUP_TEST_ROOT").expect("専用領域がありません")).canonicalize().expect("専用領域を解決できません");
    assert_eq!(root.parent(), Some(std::env::temp_dir().canonicalize().expect("Temp領域を解決できません").as_path()));
    assert!(root.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("stellarecord-native-startup-")));
    assert!(std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none(), "WebView保存先の外部上書きがあります");
    assert!(std::env::var_os("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER").is_none(), "WebView runtimeの外部上書きがあります");
    let name = std::env::var("STELLA_STARTUP_TEST_DESKTOP").expect("専用desktop名がありません");
    let mut desktop = [0_u16; 512]; let mut required = 0;
    assert_ne!(unsafe { GetUserObjectInformationW(GetThreadDesktop(GetCurrentThreadId()), 2, desktop.as_mut_ptr().cast(), std::mem::size_of_val(&desktop) as u32, &mut required) }, 0);
    let length = desktop.iter().position(|value| *value == 0).expect("desktop名の終端がありません");
    assert_eq!(String::from_utf16(&desktop[..length]).expect("desktop名を読めません"), name);
    let path = std::env::var("STELLA_STARTUP_TEST_REGISTRY").expect("専用registry pathがありません");
    assert_eq!(path, format!("Software\\StellaRecord.Tests\\startup-{}", root.file_name().expect("専用領域名がありません").to_str().expect("専用領域名を読めません")));
    let hkcu = 0x8000_0001_u32 as i32 as isize; let mut key = 0;
    assert_eq!(unsafe { RegOpenKeyExW(hkcu, wide(&path).as_ptr(), 0, 0x2001f, &mut key) }, 0);
    let mut owner = [0_u16; 1024]; let mut bytes = std::mem::size_of_val(&owner) as u32;
    assert_eq!(unsafe { RegGetValueW(key, std::ptr::null(), wide("Owner").as_ptr(), 2, std::ptr::null_mut(), owner.as_mut_ptr().cast(), &mut bytes) }, 0);
    let length = owner.iter().position(|value| *value == 0).expect("所有markerの終端がありません");
    assert_eq!(PathBuf::from(String::from_utf16(&owner[..length]).expect("所有markerを読めません")).canonicalize().expect("所有領域を解決できません"), root);
    assert_eq!(unsafe { RegOverridePredefKey(hkcu, key) }, 0);
    let installed = wide("Software\\CosmoArtsStore\\StellaRecord"); let mut location = [0_u16; 1024]; let mut bytes = std::mem::size_of_val(&location) as u32;
    assert_eq!(unsafe { RegGetValueW(key, installed.as_ptr(), wide("InstallLocation").as_ptr(), 2, std::ptr::null_mut(), location.as_mut_ptr().cast(), &mut bytes) }, 0);
    let length = location.iter().position(|value| *value == 0).expect("保存先の終端がありません");
    assert_eq!(PathBuf::from(String::from_utf16(&location[..length]).expect("保存先を読めません")).canonicalize().expect("保存先を解決できません"), root);
    std::fs::write(root.join("startup.before.txt"), std::process::id().to_string()).expect("本体呼出し前を記録できません");
    production_entry::execute();
    std::fs::write(root.join("startup.returned.txt"), b"returned").expect("本体returnを記録できません");
}
