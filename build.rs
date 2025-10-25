use std::{env, path::Path, process::Command};

fn main() {
    if env::var("CARGO_FEATURE_GENERATE_MANPAGE").is_ok() {
        generate_manpage();
    }
}

fn generate_manpage() {
    pub const APP_NAME: &str = "shaderbg";
    pub const SCRIPT_PATH: &str = "doc/md2man.sh";

    let md_path = format!("doc/{APP_NAME}.1.md");

    if !Path::new(&md_path).exists() {
        println!(
            "cargo:warning=Markdown manpage source {} not found",
            md_path
        );
        return;
    }

    println!("cargo:rerun-if-changed={md_path}");
    println!("cargo:rerun-if-changed={SCRIPT_PATH}");
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{SCRIPT_PATH} doc/{APP_NAME}.1.md"))
        .status();

    match status {
        Ok(exit_status) => {
            if !exit_status.success() {
                println!(
                    "cargo:warning=Manpage generation failed with exit code: {:?}",
                    exit_status.code()
                );
            }
        }
        Err(e) => {
            println!(
                "cargo:warning=Failed to execute manpage generation script: {}",
                e
            );
            println!("cargo:warning=Please ensure pandoc and groff are installed");
        }
    }
}
