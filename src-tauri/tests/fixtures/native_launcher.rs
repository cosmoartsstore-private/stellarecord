#![windows_subsystem = "windows"]

// 外部アプリ起動テストの専用process。起動されたことだけを同じ専用directoryへ記録する。
fn main() -> std::io::Result<()> {
    let executable = std::env::current_exe()?;
    std::fs::write(executable.with_extension("invoked"), b"native launch reached")
}
