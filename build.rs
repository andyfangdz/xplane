use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let configured = env::var_os("XPLM_SDK_PATH").map(PathBuf::from);
    let bundled = manifest
        .parent()
        .unwrap_or(Path::new("."))
        .join("sr20-g6-custom-fm")
        .join("XPSDK430")
        .join("SDK");
    let sdk = configured.unwrap_or(bundled);
    let library_dir = sdk.join("Libraries").join("Win");
    let import_library = library_dir.join("XPLM_64.lib");

    if !import_library.is_file() {
        panic!(
            "XPLM_64.lib was not found at {}. Set XPLM_SDK_PATH to the SDK directory.",
            import_library.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", library_dir.display());
    println!("cargo:rustc-link-lib=dylib=XPLM_64");
    println!("cargo:rerun-if-env-changed=XPLM_SDK_PATH");
}
