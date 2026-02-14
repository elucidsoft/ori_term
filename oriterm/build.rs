use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let assets = workspace_root.join("assets");

    // Embed the application icon into the Windows executable
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let rc_path = assets.join("icon.rc");
        let res_path = format!("{out_dir}/icon.res");

        // Determine the correct windres binary for the target
        let target = std::env::var("TARGET").unwrap_or_default();
        let windres = if target.contains("x86_64") && target.contains("gnu") {
            "x86_64-w64-mingw32-windres"
        } else {
            "windres"
        };

        let status = Command::new(windres)
            .args([
                "--include-dir",
                assets.to_str().unwrap(),
                rc_path.to_str().unwrap(),
                "-O",
                "coff",
                "-o",
                &res_path,
            ])
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:rustc-link-arg-bins={res_path}");
                println!("cargo:rerun-if-changed={}", rc_path.display());
                println!(
                    "cargo:rerun-if-changed={}",
                    assets.join("icon.ico").display()
                );
                println!(
                    "cargo:rerun-if-changed={}",
                    assets.join("oriterm.manifest").display()
                );
            }
            Ok(s) => {
                eprintln!("warning: windres exited with {s}, exe will have no icon");
            }
            Err(e) => {
                eprintln!("warning: failed to run {windres}: {e}, exe will have no icon");
            }
        }
    }

    // Decode PNG to raw RGBA at build time so runtime doesn't need the image crate
    let png_path = assets.join("icon-256.png");
    let png_bytes = std::fs::read(&png_path).expect("read assets/icon-256.png");
    let img = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png)
        .expect("decode icon PNG");
    let rgba = img.into_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let mut out = Vec::with_capacity(8 + rgba.len());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&rgba);
    std::fs::write(format!("{out_dir}/icon_rgba.bin"), &out).expect("write icon_rgba.bin");
    println!("cargo:rerun-if-changed={}", png_path.display());
}
