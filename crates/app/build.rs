// Stamps the Windows .exe with the app icon and version info. No-op elsewhere.
fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icon.ico")
        .set("ProductName", "WhatsApp Broadcaster")
        .set("FileDescription", "WhatsApp Broadcaster")
        .set("CompanyName", "rikad")
        .set("LegalCopyright", "rikad");
    res.compile().expect("failed to embed Windows resources");
}
