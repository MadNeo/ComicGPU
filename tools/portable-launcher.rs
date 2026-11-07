//! Single-file Windows distribution of the GNU build, including its WebView2 loader.
//! Build using tools/build-portable.sh after compiling the desktop application.
#![windows_subsystem = "windows"]
use std::{
    fs, io,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const APPLICATION: &[u8] = include_bytes!(env!("COMICGPU_APP_EXE"));
const LOADER: &[u8] = include_bytes!(env!("COMICGPU_WEBVIEW_LOADER"));

struct WorkingDirectory(PathBuf);
impl Drop for WorkingDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn start() -> io::Result<()> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("ComicGPU-{}-{stamp}", std::process::id()));
    fs::create_dir(&directory)?;
    let working = WorkingDirectory(directory);
    let application = working.0.join("ComicGPU.exe");
    fs::write(&application, APPLICATION)?;
    fs::write(working.0.join("WebView2Loader.dll"), LOADER)?;
    // Keep the payload available until the application has closed.
    let result = Command::new(application).current_dir(&working.0).status()?;
    if !result.success() {
        return Err(io::Error::other(format!("ComicGPU is afgesloten met code {result}. Controleer of Microsoft Edge WebView2 Runtime is geïnstalleerd.")));
    }
    Ok(())
}
#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(
        window: *mut std::ffi::c_void,
        text: *const u16,
        title: *const u16,
        flags: u32,
    ) -> i32;
}
fn main() {
    if let Err(error) = start() {
        let message: Vec<u16> = format!("ComicGPU kon niet starten.\n\n{error}")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let title: Vec<u16> = "ComicGPU".encode_utf16().chain(Some(0)).collect();
        unsafe {
            MessageBoxW(std::ptr::null_mut(), message.as_ptr(), title.as_ptr(), 0x10);
        }
        std::process::exit(1);
    }
}
