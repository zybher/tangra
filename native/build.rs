// Embeds the app icon into tangra.exe (Explorer, taskbar, Alt-Tab)
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut r = winresource::WindowsResource::new();
        r.set_icon("assets/icon.ico");
        r.compile().expect("embed icon");
    }
    println!("cargo:rerun-if-changed=assets/icon.ico");
}
