fn main() {
    // The host and target may differ even when building on a Mac.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
        || std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("aarch64")
    {
        return;
    }
    println!("cargo:rerun-if-changed=src/bridge.m");
    println!("cargo:rerun-if-changed=src/video.m");
    cc::Build::new()
        .files(["src/bridge.m", "src/video.m"])
        .flag("-fobjc-arc")
        .flag("-mmacosx-version-min=11.0")
        .compile("newera_metal_bridge");
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=Foundation");
}
