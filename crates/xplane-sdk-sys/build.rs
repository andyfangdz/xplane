use std::{env, path::Path};

fn main() {
    let sdk = Path::new(env!("CARGO_MANIFEST_DIR")).join("SDK");
    println!("cargo::metadata=root={}", sdk.display());
    println!("cargo::rerun-if-changed=SDK");
    match env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
        "windows" => {
            println!(
                "cargo::rustc-link-search=native={}",
                sdk.join("Libraries/Win").display()
            );
            println!("cargo::rustc-link-lib=XPLM_64");
            println!("cargo::rustc-link-lib=XPWidgets_64");
        }
        "macos" => {
            println!(
                "cargo::rustc-link-search=framework={}",
                sdk.join("Libraries/Mac").display()
            );
            println!("cargo::rustc-link-lib=framework=XPLM");
            println!("cargo::rustc-link-lib=framework=XPWidgets");
        }
        _ => {} // On Linux the simulator resolves plugin symbols at load time.
    }
}
