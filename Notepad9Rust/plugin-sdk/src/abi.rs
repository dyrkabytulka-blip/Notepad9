//! The binary interface between Notepad9 and a plugin DLL.
//!
//! Plain C ABI on purpose: a plugin built with any Rust version (or any egui version)
//! works with any Notepad9 that speaks the same `API_VERSION`. Structs only grow at the end.

use std::ffi::c_void;

/// Version of this interface. A plugin asks for at least this version.
pub const API_VERSION: u32 = 1;

/// UTF-8 string slice passed across the DLL boundary (not NUL-terminated).
///
/// Strings given *to* the host are borrowed for the duration of the call.
/// Strings returned *by* the host stay valid until the next host call; the SDK copies them.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct N9Str {
    pub ptr: *const u8,
    pub len: usize,
}

impl N9Str {
    pub const EMPTY: N9Str = N9Str { ptr: std::ptr::null(), len: 0 };

    pub fn new(s: &str) -> N9Str {
        N9Str { ptr: s.as_ptr(), len: s.len() }
    }

    /// # Safety
    /// `ptr` must point to `len` valid UTF-8 bytes (or be null with len 0).
    pub unsafe fn as_str<'a>(&self) -> &'a str {
        if self.ptr.is_null() || self.len == 0 {
            return "";
        }
        let bytes = std::slice::from_raw_parts(self.ptr, self.len);
        std::str::from_utf8(bytes).unwrap_or("")
    }

    /// # Safety
    /// See [`N9Str::as_str`].
    pub unsafe fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

// ---------------------------------------------------------------- constants

/// Kinds of plugin windows.
pub const WINDOW_FLOATING: u32 = 0;
pub const WINDOW_RIGHT_PANEL: u32 = 1;
pub const WINDOW_BOTTOM_PANEL: u32 = 2;

/// Event kinds (`Event::kind`).
pub const EVENT_STARTUP: u32 = 1;
pub const EVENT_TAB_ACTIVATED: u32 = 2;
pub const EVENT_FILE_OPENED: u32 = 3;
pub const EVENT_FILE_SAVED: u32 = 4;
pub const EVENT_TEXT_CHANGED: u32 = 5;
pub const EVENT_TIMER: u32 = 6;
pub const EVENT_CHAR_TYPED: u32 = 7;
pub const EVENT_SELECTION_CHANGED: u32 = 8;

/// Label styles.
pub const LABEL_NORMAL: u32 = 0;
pub const LABEL_HEADING: u32 = 1;
pub const LABEL_MONO: u32 = 2;
pub const LABEL_SMALL: u32 = 3;
pub const LABEL_WEAK: u32 = 4;
pub const LABEL_STRONG: u32 = 5;

/// Layout blocks (`UiApi::begin`).
pub const BLOCK_HORIZONTAL: u32 = 0;
pub const BLOCK_VERTICAL: u32 = 1;
pub const BLOCK_SCROLL: u32 = 2;
pub const BLOCK_COLLAPSING: u32 = 3;
pub const BLOCK_GROUP: u32 = 4;
pub const BLOCK_WRAP: u32 = 5;

#[repr(C)]
pub struct Event {
    pub kind: u32,
    /// index of the tab concerned
    pub tab: u32,
    /// file path (FileOpened / FileSaved), may be empty
    pub path: N9Str,
    /// typed character (CharTyped)
    pub ch: u32,
}

// ---------------------------------------------------------------- host → plugin

/// Functions the host gives to the plugin. `plugin` is the handle received in `n9_plugin_entry`.
/// Valid on the UI thread, inside the plugin callbacks (and during `n9_plugin_entry`).
#[repr(C)]
pub struct HostApi {
    pub api_version: u32,
    /// size of this struct in bytes (newer hosts are bigger)
    pub size: u32,

    // ---- messages
    pub log: extern "C" fn(plugin: u32, text: N9Str),
    pub show_info: extern "C" fn(plugin: u32, text: N9Str),
    pub message: extern "C" fn(plugin: u32, title: N9Str, text: N9Str),

    // ---- registering UI
    /// "Group/Item" puts the item in a submenu of the Plugins menu. `shortcut` like "Ctrl+Alt+W" (may be empty).
    pub add_menu_item: extern "C" fn(plugin: u32, path: N9Str, shortcut: N9Str, command: u32),
    /// `icon` is a built-in icon name ("find", "star" ...) or empty; `png` optional PNG bytes (used when given).
    pub add_toolbar_button: extern "C" fn(plugin: u32, icon: N9Str, png: *const u8, png_len: usize, tooltip: N9Str, command: u32),
    pub open_window: extern "C" fn(plugin: u32, window: u32, title: N9Str, kind: u32, width: f32, height: f32),
    pub close_window: extern "C" fn(plugin: u32, window: u32),
    pub is_window_open: extern "C" fn(plugin: u32, window: u32) -> bool,
    /// call `on_event(EVENT_TIMER)` every `ms` milliseconds (0 = off)
    pub set_timer: extern "C" fn(plugin: u32, ms: u32),
    pub request_repaint: extern "C" fn(),

    // ---- tabs and files
    pub tab_count: extern "C" fn() -> u32,
    pub active_tab: extern "C" fn() -> u32,
    pub activate_tab: extern "C" fn(index: u32),
    pub tab_title: extern "C" fn(index: u32) -> N9Str,
    pub tab_path: extern "C" fn(index: u32) -> N9Str,
    pub tab_modified: extern "C" fn(index: u32) -> bool,
    /// new tab with this text; returns its index
    pub new_tab: extern "C" fn(text: N9Str) -> u32,
    pub open_file: extern "C" fn(path: N9Str) -> bool,
    pub save_active: extern "C" fn() -> bool,

    // ---- text of the active tab (positions are character indices)
    pub get_text: extern "C" fn() -> N9Str,
    pub set_text: extern "C" fn(text: N9Str),
    pub text_len: extern "C" fn() -> usize,
    pub get_selection: extern "C" fn(start: *mut usize, end: *mut usize),
    pub set_selection: extern "C" fn(start: usize, end: usize),
    pub selected_text: extern "C" fn() -> N9Str,
    pub replace_selection: extern "C" fn(text: N9Str),
    pub replace_range: extern "C" fn(start: usize, end: usize, text: N9Str),
    pub insert_text: extern "C" fn(at: usize, text: N9Str),
    pub caret: extern "C" fn() -> usize,
    pub line_count: extern "C" fn() -> usize,
    pub line_text: extern "C" fn(line: usize) -> N9Str,
    pub line_of: extern "C" fn(pos: usize) -> usize,
    pub line_start: extern "C" fn(line: usize) -> usize,
    /// 0-based line; scrolls it into view
    pub goto_line: extern "C" fn(line: usize),
    /// syntax name of the active tab ("Rust", "Markdown" ...) or empty
    pub language: extern "C" fn() -> N9Str,

    // ---- misc
    /// run a built-in command: "save", "find", "wrap", "zoom_in", "console" ...
    pub run_command: extern "C" fn(command: N9Str),
    pub clipboard_set: extern "C" fn(text: N9Str),
    pub clipboard_get: extern "C" fn() -> N9Str,
    /// run a command line in the Notepad9 console (dir may be empty)
    pub console_run: extern "C" fn(command: N9Str, dir: N9Str),
    /// per-plugin settings kept in %APPDATA%\Notepad9\plugin-data\<plugin file name>.json
    pub setting_get: extern "C" fn(plugin: u32, key: N9Str) -> N9Str,
    pub setting_set: extern "C" fn(plugin: u32, key: N9Str, value: N9Str),
    pub app_version: extern "C" fn() -> N9Str,
}

// ---------------------------------------------------------------- building a window

/// Immediate-mode UI builder given to `on_ui`. Valid only during that call.
///
/// Values of inputs (checkbox, text, slider) are kept by the host per window and id;
/// a button returns true once after it was clicked.
#[repr(C)]
pub struct UiApi {
    pub ctx: *mut c_void,
    pub label: extern "C" fn(ctx: *mut c_void, text: N9Str, style: u32, rgba: u32),
    pub button: extern "C" fn(ctx: *mut c_void, id: N9Str, text: N9Str) -> bool,
    pub checkbox: extern "C" fn(ctx: *mut c_void, id: N9Str, text: N9Str, initial: bool) -> bool,
    pub set_checkbox: extern "C" fn(ctx: *mut c_void, id: N9Str, value: bool),
    /// `rows` 0 = single line
    pub text_input: extern "C" fn(ctx: *mut c_void, id: N9Str, hint: N9Str, rows: u32) -> N9Str,
    pub set_input: extern "C" fn(ctx: *mut c_void, id: N9Str, value: N9Str),
    pub slider: extern "C" fn(ctx: *mut c_void, id: N9Str, text: N9Str, min: f64, max: f64, initial: f64) -> f64,
    pub selectable: extern "C" fn(ctx: *mut c_void, id: N9Str, text: N9Str, selected: bool) -> bool,
    pub link: extern "C" fn(ctx: *mut c_void, id: N9Str, text: N9Str) -> bool,
    pub separator: extern "C" fn(ctx: *mut c_void),
    pub space: extern "C" fn(ctx: *mut c_void, amount: f32),
    pub progress: extern "C" fn(ctx: *mut c_void, fraction: f32, text: N9Str),
    /// start a block (BLOCK_*); `param` = max height for scroll, 1.0 = open by default for collapsing
    pub begin: extern "C" fn(ctx: *mut c_void, kind: u32, id: N9Str, title: N9Str, param: f32),
    pub end: extern "C" fn(ctx: *mut c_void),
    /// change the value a slider shows
    pub set_slider: extern "C" fn(ctx: *mut c_void, id: N9Str, value: f64),
}

// ---------------------------------------------------------------- plugin → host

/// Returned by `n9_plugin_entry`. Lives as long as the process.
#[repr(C)]
pub struct PluginDesc {
    pub api_version: u32,
    pub name: N9Str,
    pub version: N9Str,
    pub author: N9Str,
    pub description: N9Str,
    pub instance: *mut c_void,
    pub on_command: Option<extern "C" fn(instance: *mut c_void, command: u32)>,
    pub on_event: Option<extern "C" fn(instance: *mut c_void, event: *const Event)>,
    pub on_ui: Option<extern "C" fn(instance: *mut c_void, window: u32, ui: *const UiApi)>,
    pub shutdown: Option<extern "C" fn(instance: *mut c_void)>,
}

/// Signature of the function every plugin DLL exports as `n9_plugin_entry`.
pub type EntryFn = unsafe extern "C" fn(host: *const HostApi, plugin: u32) -> *const PluginDesc;

/// Name of the exported entry function.
pub const ENTRY_NAME: &[u8] = b"n9_plugin_entry\0";
