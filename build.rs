//! Build script: on Windows, embed the application icon into the `.exe` so it
//! shows up in Explorer and the taskbar. No-op on other platforms.
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon/icon.ico");
        let _ = res.compile();
    }
}
