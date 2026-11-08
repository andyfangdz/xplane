fn main() {
    println!("cargo:rerun-if-changed=src/seh.c");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        cc::Build::new().file("src/seh.c").compile("xphost_seh");
    }
}
