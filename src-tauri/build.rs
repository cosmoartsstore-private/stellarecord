#[path = "build/messages.rs"]
mod messages;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    messages::compile("messages/messages.ja.properties")?;
    tauri_build::build();
    // unit testから既存のmanifest resourceをリンクするため、生成先だけを検索先に加える。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-search=native={}", std::env::var("OUT_DIR")?);
    }
    Ok(())
}
