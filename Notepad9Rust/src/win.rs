//! Small Windows helpers: themed title bar, foreground window, ShellExecute.

use eframe::egui::Color32;
use std::ffi::c_void;
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, IsIconic, SetForegroundWindow, ShowWindow, SW_MAXIMIZE, SW_RESTORE, SW_SHOWNORMAL,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn colorref(c: Color32) -> u32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    r as u32 | ((g as u32) << 8) | ((b as u32) << 16)
}

/// Native title bar in the theme colors (Windows 11), dark mode on Windows 10.
pub fn style_title_bar(hwnd: isize, caption: Color32, text: Color32, border: Color32, dark: bool, corners: u32) {
    let h = hwnd as *mut c_void;
    let set = |attr: u32, v: u32| unsafe {
        DwmSetWindowAttribute(h, attr, &v as *const u32 as *const c_void, 4);
    };
    set(20, dark as u32); // DWMWA_USE_IMMERSIVE_DARK_MODE
    set(35, colorref(caption)); // DWMWA_CAPTION_COLOR
    set(36, colorref(text)); // DWMWA_TEXT_COLOR
    set(34, colorref(border)); // DWMWA_BORDER_COLOR
    set(33, corners); // DWMWA_WINDOW_CORNER_PREFERENCE
}

/// Hide the window from the screen without hiding it from Windows (DWMWA_CLOAK). Used at start-up
/// so the first frames, while the layout settles, are never seen.
pub fn cloak(hwnd: isize, on: bool) {
    let v: i32 = on as i32;
    unsafe {
        DwmSetWindowAttribute(hwnd as *mut c_void, 13, &v as *const i32 as *const c_void, 4);
    }
}

/// Normal maximize: fills the work area, the taskbar stays visible.
pub fn maximize(hwnd: isize) {
    unsafe {
        ShowWindow(hwnd as *mut c_void, SW_MAXIMIZE);
    }
}

pub fn allow_foreground() {
    unsafe {
        AllowSetForegroundWindow(u32::MAX);
    }
}

pub fn bring_to_front(hwnd: isize) {
    let h = hwnd as *mut c_void;
    unsafe {
        if IsIconic(h) != 0 {
            ShowWindow(h, SW_RESTORE);
        }
        SetForegroundWindow(h);
    }
}

/// Open a file / URL / folder with its associated program ("open", "print", "explore").
pub fn shell(verb: &str, target: &str, dir: Option<&str>) -> bool {
    let v = wide(verb);
    let t = wide(target);
    let d = dir.map(wide);
    let r = unsafe {
        ShellExecuteW(std::ptr::null_mut(), v.as_ptr(), t.as_ptr(), std::ptr::null(),
                      d.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()), SW_SHOWNORMAL)
    };
    r as isize > 32
}
