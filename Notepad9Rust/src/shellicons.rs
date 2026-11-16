//! File and folder icons from Windows (the same ones Explorer shows), loaded on a background
//! thread and cached: one icon per extension, per-file only for .exe / .ico / .lnk and similar.

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

/// Extensions whose icon depends on the file itself, not only on its type.
const OWN_ICON: &[&str] = &["exe", "ico", "lnk", "url", "cur", "ani", "scr", "msc", "appref-ms"];

struct Job {
    key: String,
    path: PathBuf,
    dir: bool,
    open: bool,
    small: bool,
}

#[derive(Default)]
pub struct ShellIcons {
    tex: HashMap<String, Option<TextureHandle>>,
    pending: HashSet<String>,
    tx: Option<Sender<Job>>,
    rx: Option<Receiver<(String, Option<ColorImage>)>>,
}

impl ShellIcons {
    /// Icon for a file or folder; None until it is loaded (a repaint is requested then).
    pub fn get(&mut self, ctx: &egui::Context, path: &Path, dir: bool, open: bool) -> Option<egui::TextureId> {
        self.collect(ctx);
        let small = ctx.pixels_per_point() <= 1.25;
        let size = if small { "s" } else { "l" };
        let key = if dir {
            format!("dir{}:{size}", if open { "_open" } else { "" })
        } else {
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if OWN_ICON.contains(&ext.as_str()) {
                format!("path:{}:{size}", path.display())
            } else {
                format!("ext:{ext}:{size}")
            }
        };
        if let Some(t) = self.tex.get(&key) {
            return t.as_ref().map(|t| t.id());
        }
        if self.pending.insert(key.clone()) {
            self.start(ctx);
            if let Some(tx) = &self.tx {
                let _ = tx.send(Job { key, path: path.to_path_buf(), dir, open, small });
            }
        }
        None
    }

    fn collect(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        while let Ok((key, img)) = rx.try_recv() {
            self.pending.remove(&key);
            let t = img.map(|img| ctx.load_texture(format!("shell:{key}"), img, TextureOptions::LINEAR));
            self.tex.insert(key, t);
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        if self.tx.is_some() {
            return;
        }
        let (tx, jobs) = channel::<Job>();
        let (done, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            unsafe {
                windows_sys::Win32::System::Com::CoInitializeEx(std::ptr::null(), windows_sys::Win32::System::Com::COINIT_APARTMENTTHREADED as u32);
            }
            while let Ok(j) = jobs.recv() {
                let img = load(&j);
                if done.send((j.key, img)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        self.tx = Some(tx);
        self.rx = Some(rx);
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn load(j: &Job) -> Option<ColorImage> {
    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL};
    use windows_sys::Win32::UI::Shell::*;
    let own = j.key.starts_with("path:");
    // generic icons by type are asked for with a made-up name, so nothing is read from disk
    let (name, attrs, mut flags) = if j.dir {
        ("folder".to_string(), FILE_ATTRIBUTE_DIRECTORY, SHGFI_USEFILEATTRIBUTES)
    } else if own {
        (j.path.display().to_string(), 0, 0)
    } else {
        let ext = j.path.extension().map(|e| format!("x.{}", e.to_string_lossy())).unwrap_or_else(|| "x".into());
        (ext, FILE_ATTRIBUTE_NORMAL, SHGFI_USEFILEATTRIBUTES)
    };
    flags |= SHGFI_ICON | if j.small { SHGFI_SMALLICON } else { SHGFI_LARGEICON };
    if j.open {
        flags |= SHGFI_OPENICON;
    }
    let w = wide(&name);
    let mut info: SHFILEINFOW = unsafe { std::mem::zeroed() };
    let ok = unsafe { SHGetFileInfoW(w.as_ptr(), attrs, &mut info, std::mem::size_of::<SHFILEINFOW>() as u32, flags) };
    if ok == 0 || info.hIcon.is_null() {
        return None;
    }
    let img = icon_to_image(info.hIcon);
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon(info.hIcon);
    }
    img
}

/// HICON → RGBA. Uses the icon's alpha; old icons without alpha use the AND mask.
fn icon_to_image(icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON) -> Option<ColorImage> {
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};
    unsafe {
        let mut ii: ICONINFO = std::mem::zeroed();
        if GetIconInfo(icon, &mut ii) == 0 {
            return None;
        }
        let cleanup = |ii: &ICONINFO| {
            if !ii.hbmColor.is_null() {
                DeleteObject(ii.hbmColor);
            }
            if !ii.hbmMask.is_null() {
                DeleteObject(ii.hbmMask);
            }
        };
        if ii.hbmColor.is_null() {
            cleanup(&ii);
            return None;
        }
        let mut bm: BITMAP = std::mem::zeroed();
        if GetObjectW(ii.hbmColor, std::mem::size_of::<BITMAP>() as i32, &mut bm as *mut BITMAP as *mut _) == 0 {
            cleanup(&ii);
            return None;
        }
        let (w, h) = (bm.bmWidth.max(1) as usize, bm.bmHeight.max(1) as usize);
        let dc = CreateCompatibleDC(std::ptr::null_mut());
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w as i32;
        bi.bmiHeader.biHeight = -(h as i32); // top-down
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB;
        let mut px = vec![0u8; w * h * 4];
        let got = GetDIBits(dc, ii.hbmColor, 0, h as u32, px.as_mut_ptr() as *mut _, &mut bi, DIB_RGB_COLORS);
        let mut mask = vec![0u8; w * h * 4];
        let has_mask = !ii.hbmMask.is_null()
            && GetDIBits(dc, ii.hbmMask, 0, h as u32, mask.as_mut_ptr() as *mut _, &mut bi, DIB_RGB_COLORS) != 0;
        DeleteDC(dc);
        cleanup(&ii);
        if got == 0 {
            return None;
        }
        let has_alpha = px.chunks_exact(4).any(|p| p[3] != 0);
        let mut rgba = Vec::with_capacity(w * h * 4);
        for (i, p) in px.chunks_exact(4).enumerate() {
            let a = if has_alpha {
                p[3]
            } else if has_mask && mask[i * 4] != 0 {
                0
            } else {
                255
            };
            // GDI gives premultiplied BGRA
            rgba.extend_from_slice(&[p[2], p[1], p[0], a]);
        }
        Some(if has_alpha {
            ColorImage::from_rgba_premultiplied([w, h], &rgba)
        } else {
            ColorImage::from_rgba_unmultiplied([w, h], &rgba)
        })
    }
}
