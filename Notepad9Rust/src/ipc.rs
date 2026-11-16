//! One Notepad9 window: a named mutex says whether we are first; later starts hand their
//! files to the running window through a named pipe and exit right away.

use std::io::{Read, Write};
use std::sync::mpsc::Sender;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, PIPE_ACCESS_INBOUND};
use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT};
use windows_sys::Win32::System::Threading::CreateMutexW;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn user() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| "user".into())
}

fn pipe_name() -> String {
    format!(r"\\.\pipe\Notepad9rs.{}", user())
}

/// True when this is the first Notepad9 of the user (the mutex stays owned until exit).
pub fn acquire_instance() -> bool {
    let name = wide(&format!("Notepad9rs.{}.Mutex", user()));
    unsafe {
        let h = CreateMutexW(std::ptr::null(), 1, name.as_ptr());
        if h.is_null() {
            return true;
        }
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

/// Give the files to the running window. False if it did not answer.
pub fn send_to_primary(files: &[String]) -> bool {
    crate::win::allow_foreground();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    loop {
        match std::fs::OpenOptions::new().write(true).open(pipe_name()) {
            Ok(mut f) => {
                let msg = if files.is_empty() { String::from("\n") } else { files.join("\n") };
                return f.write_all(msg.as_bytes()).is_ok();
            }
            Err(_) if std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(40)),
            Err(_) => return false,
        }
    }
}

/// Serve the pipe on a background thread; each message is a list of paths (may be empty).
pub fn serve(tx: Sender<Vec<String>>, ctx: eframe::egui::Context) {
    std::thread::spawn(move || {
        let name = wide(&pipe_name());
        loop {
            let h = unsafe {
                CreateNamedPipeW(name.as_ptr(), PIPE_ACCESS_INBOUND, PIPE_TYPE_BYTE | PIPE_WAIT, PIPE_UNLIMITED_INSTANCES,
                                 0, 65536, 0, std::ptr::null())
            };
            if h == INVALID_HANDLE_VALUE {
                std::thread::sleep(std::time::Duration::from_millis(500));
                continue;
            }
            let ok = unsafe { ConnectNamedPipe(h, std::ptr::null_mut()) } != 0 || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
            if ok {
                let mut data = Vec::new();
                let mut buf = [0u8; 8192];
                loop {
                    let mut read = 0u32;
                    let r = unsafe { ReadFile(h, buf.as_mut_ptr(), buf.len() as u32, &mut read, std::ptr::null_mut()) };
                    if r == 0 || read == 0 {
                        break;
                    }
                    data.extend_from_slice(&buf[..read as usize]);
                }
                let text = String::from_utf8_lossy(&data).to_string();
                let files: Vec<String> = text.split('\n').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                let _ = tx.send(files);
                ctx.request_repaint();
            }
            unsafe {
                DisconnectNamedPipe(h);
                CloseHandle(h);
            }
        }
    });
}

#[allow(dead_code)]
fn _read_unused(mut r: impl Read) {
    let _ = r.read(&mut [0u8; 1]);
}
