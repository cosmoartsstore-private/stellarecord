#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    stella_record_ui::platform::install_panic_hook();
    stella_record_ui::platform::ensure_single_instance();
    stella_record_ui::run();
}
