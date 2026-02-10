#![windows_subsystem = "windows"]

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--print-config") {
        let config = ori_term::config::Config::default();
        match toml::to_string_pretty(&config) {
            Ok(s) => print!("{s}"),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("oriterm {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("oriterm {}", env!("CARGO_PKG_VERSION"));
        println!("A GPU-accelerated terminal emulator\n");
        println!("USAGE:");
        println!("    oriterm [OPTIONS]\n");
        println!("OPTIONS:");
        println!("    --print-config    Print the default configuration to stdout");
        println!("    --version, -V     Print version information");
        println!("    --help, -h        Print this help message");
        return;
    }

    if let Err(e) = ori_term::app::App::run() {
        let _ = std::fs::write("oriterm_error.log", format!("{e:?}"));
    }
}
