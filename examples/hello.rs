#![windows_subsystem = "windows"]

fn main() {
    if let Err(e) = ori_console::app::App::run() {
        let _ = std::fs::write("ori_console_error.log", format!("{e:?}"));
    }
}
