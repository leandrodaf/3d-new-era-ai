fn main() {
    // The host and target may differ even when building on a Mac.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
        || std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("aarch64")
    {
        return;
    }
    println!("cargo:rerun-if-changed=src/bridge.m");
    println!("cargo:rerun-if-changed=src/video.m");
    let mut build = cc::Build::new();
    build
        .files(["src/bridge.m", "src/video.m"])
        .flag("-fobjc-arc")
        .flag("-mmacosx-version-min=11.0");
    let compiler = build.get_compiler();
    build.compile("newera_metal_bridge");
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=Foundation");
    // `@available(macOS 12.0, *)` compiles to a call of
    // `__isPlatformVersionAtLeast`, which lives in clang's own runtime
    // library. clang links it when it drives the link; rustc links with `cc`
    // but leaves it out, so a release binary failed with an undefined symbol.
    // Link it from the directory the compiler that built the bridge reports.
    let runtime = std::process::Command::new(compiler.path())
        .arg("--print-runtime-dir")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|dir| std::path::Path::new(dir).join("libclang_rt.osx.a").exists());
    match runtime {
        Some(dir) => {
            println!("cargo:rustc-link-search=native={dir}");
            println!("cargo:rustc-link-lib=static=clang_rt.osx");
        }
        None => println!(
            "cargo:warning=clang's runtime library was not found: `@available` may not link"
        ),
    }
}
