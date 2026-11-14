//! Embeds the Windows version info and application manifest into `agsync.exe`.

fn main() {
    #[cfg(windows)]
    embed();
}

#[cfg(windows)]
fn embed() {
    println!("cargo:rerun-if-changed=assets/agsync.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let mut res = winresource::WindowsResource::new();
    res.set("CompanyName", "agsync")
        .set(
            "FileDescription",
            "agsync - sync MCP servers and skills into coding agents",
        )
        .set("ProductName", "agsync")
        .set("FileVersion", &version)
        .set("ProductVersion", &version)
        .set("OriginalFilename", "agsync.exe")
        .set("InternalName", "agsync")
        .set("LegalCopyright", "Copyright (c) agsync contributors")
        .set_manifest_file("assets/agsync.manifest");
    res.compile().expect("embed the Windows resources");
}
