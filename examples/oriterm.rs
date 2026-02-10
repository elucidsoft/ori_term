#![windows_subsystem = "windows"]

fn main() {
    if let Err(e) = ori_term::app::App::run() {
        let _ = std::fs::write("oriterm_error.log", format!("{e:?}"));
    }
}
