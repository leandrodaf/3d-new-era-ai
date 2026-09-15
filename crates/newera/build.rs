//! Embeds the app icon and the version details in the Windows executables, so
//! Explorer, the taskbar, the installer shortcuts and the Properties dialog
//! show the mark and the real name instead of the defaults.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/document.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        // Icon 1 is the executable's own (the lowest id is what Explorer
        // shows). Icon 2 is the document icon the installer points `.newera`
        // files at, as `newera-gui.exe,1` — the second icon group in the file.
        res.set_icon_with_id("assets/icon.ico", "1");
        res.set_icon_with_id("assets/document.ico", "2");
        res.set("ProductName", "3D New Era AI");
        res.set(
            "FileDescription",
            "3D New Era AI - home design editor with a built-in MCP server",
        );
        res.set("CompanyName", "3D New Era AI");
        res.set("LegalCopyright", "Leandro Ferreira - MIT or Apache-2.0");
        res.compile().expect("embed the Windows icon resource");
    }
}
