//! Plugin host: loads plugin DLLs (Rust cdylibs built with the `notepad9_plugin` SDK),
//! hands them the `HostApi` table and draws the windows they describe.
//!
//! Plugins are searched in `%APPDATA%\Notepad9\plugins` and `<exe folder>\plugins`
//! (`name.dll` or `name\name.dll`). A plugin calls back into the editor only on the UI
//! thread, inside one of its callbacks; the editor is reached through a pointer that is set
//! just for the duration of that call.

use crate::app::{App, Modal, ModalAction};
use crate::doc::Sel;
use crate::theme::Palette;
use eframe::egui::{self, Color32, Key, RichText, Stroke, Vec2};
use notepad9_plugin::abi::{self, HostApi, N9Str, UiApi};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

// ====================================================================== data

pub enum Status {
    Loaded,
    Disabled,
    Failed(String),
    /// file renamed away; gone after a restart
    Removed,
}

#[derive(Clone, Copy)]
struct Calls {
    inst: *mut c_void,
    on_command: Option<extern "C" fn(*mut c_void, u32)>,
    on_event: Option<extern "C" fn(*mut c_void, *const abi::Event)>,
    on_ui: Option<extern "C" fn(*mut c_void, u32, *const UiApi)>,
    shutdown: Option<extern "C" fn(*mut c_void)>,
}

impl Default for Calls {
    fn default() -> Self {
        Self { inst: std::ptr::null_mut(), on_command: None, on_event: None, on_ui: None, shutdown: None }
    }
}

pub struct MenuItem {
    pub path: String,
    pub shortcut: String,
    pub key: Option<(bool, bool, bool, Key)>,
    pub command: u32,
}

pub struct ToolItem {
    pub icon: String,
    png: Option<Vec<u8>>,
    /// decoded lazily: None = not tried yet
    tex: Option<Option<egui::TextureHandle>>,
    pub tooltip: String,
    pub command: u32,
}

/// What a plugin window shows, rebuilt every frame by the plugin's `on_ui`.
enum Node {
    Label { text: String, style: u32, rgba: u32 },
    Button { id: String, text: String },
    Checkbox { id: String, text: String },
    TextInput { id: String, hint: String, rows: u32 },
    Slider { id: String, text: String, min: f64, max: f64 },
    Selectable { id: String, text: String, selected: bool },
    Link { id: String, text: String },
    Separator,
    Space(f32),
    Progress(f32, String),
    Block { kind: u32, id: String, title: String, param: f32, children: Vec<Node> },
}

/// Values of the inputs of one window, kept by the editor between frames.
#[derive(Default)]
struct WinState {
    clicked: HashSet<String>,
    bools: HashMap<String, bool>,
    nums: HashMap<String, f64>,
    texts: HashMap<String, String>,
}

struct PWindow {
    id: u32,
    title: String,
    kind: u32,
    size: Vec2,
    open: bool,
    nodes: Vec<Node>,
    state: WinState,
}

pub struct PluginRec {
    pub file: PathBuf,
    pub stem: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub status: Status,
    calls: Calls,
    pub menu: Vec<MenuItem>,
    pub toolbar: Vec<ToolItem>,
    windows: Vec<PWindow>,
    timer_ms: u32,
    next_timer: Option<Instant>,
    settings: Option<BTreeMap<String, String>>,
    settings_dirty: bool,
    pub log: Vec<String>,
}

impl PluginRec {
    fn new(file: PathBuf, stem: String) -> Self {
        Self {
            file,
            name: stem.clone(),
            stem,
            version: String::new(),
            author: String::new(),
            description: String::new(),
            status: Status::Failed("not loaded".into()),
            calls: Calls::default(),
            menu: Vec::new(),
            toolbar: Vec::new(),
            windows: Vec::new(),
            timer_ms: 0,
            next_timer: None,
            settings: None,
            settings_dirty: false,
            log: Vec::new(),
        }
    }
    pub fn is_loaded(&self) -> bool {
        matches!(self.status, Status::Loaded)
    }
    fn removed(&self) -> bool {
        matches!(self.status, Status::Removed)
    }
    fn push_log(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > 300 {
            self.log.drain(..100);
        }
    }
    fn settings(&mut self) -> &mut BTreeMap<String, String> {
        let stem = self.stem.clone();
        self.settings.get_or_insert_with(|| {
            std::fs::read_to_string(data_file(&stem)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
        })
    }
    fn save_settings(&mut self) {
        if !self.settings_dirty {
            return;
        }
        self.settings_dirty = false;
        if let Some(s) = &self.settings {
            let f = data_file(&self.stem);
            if let Some(dir) = f.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(text) = serde_json::to_string_pretty(s) {
                let _ = std::fs::write(f, text);
            }
        }
    }
}

struct Ev {
    target: Option<usize>,
    kind: u32,
    tab: u32,
    path: String,
    ch: u32,
}

#[derive(Default)]
pub struct Plugins {
    pub list: Vec<PluginRec>,
    /// the plugin folders were scanned
    pub loaded: bool,
    /// at least one plugin is running
    active: bool,
    stopped: bool,
    queue: Vec<Ev>,
    messages: std::collections::VecDeque<(String, String)>,
    last_text: (u64, u64),
    last_sel: (u64, Sel),
    last_settings_save: Option<Instant>,
    pub manager_open: bool,
}

impl Plugins {
    /// Shortcuts registered by plugins: (plugin, command, ctrl, shift, alt, key).
    pub fn shortcuts(&self) -> Vec<(usize, u32, bool, bool, bool, Key)> {
        let mut v = Vec::new();
        for (pi, p) in self.list.iter().enumerate().filter(|(_, p)| p.is_loaded()) {
            for m in &p.menu {
                if let Some((c, s, a, k)) = m.key {
                    v.push((pi, m.command, c, s, a, k));
                }
            }
        }
        v
    }
}

// ====================================================================== folders

pub fn user_dir() -> PathBuf {
    crate::settings::folder().join("plugins")
}

pub fn app_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|d| d.join("plugins"))
}

fn data_file(stem: &str) -> PathBuf {
    crate::settings::folder().join("plugin-data").join(format!("{stem}.json"))
}

fn stem_of(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

/// Finish what Install / Remove could not do while the DLL was loaded:
/// `x.dll.update` replaces `x.dll`, `x.dll.removed` is deleted.
fn housekeeping() {
    let Ok(rd) = std::fs::read_dir(user_dir()) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        if name.ends_with(".dll.removed") {
            let _ = std::fs::remove_file(&p);
        } else if name.ends_with(".dll.update") {
            let target = p.with_extension("");
            let _ = std::fs::remove_file(&target);
            let _ = std::fs::rename(&p, &target);
        }
    }
}

/// `plugins\*.dll` and `plugins\<name>\<name>.dll`; the user folder wins over the program folder.
fn discover() -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for dir in [Some(user_dir()), app_dir()].into_iter().flatten() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut found = Vec::new();
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Some(n) = p.file_name() {
                    let dll = p.join(format!("{}.dll", n.to_string_lossy()));
                    if dll.is_file() {
                        found.push(dll);
                    }
                }
            } else if p.extension().map_or(false, |x| x.eq_ignore_ascii_case("dll")) {
                found.push(p);
            }
        }
        found.sort();
        for p in found {
            if seen.insert(stem_of(&p).to_lowercase()) {
                out.push(p);
            }
        }
    }
    out
}

fn open_library(path: &Path) -> Result<abi::EntryFn, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::FreeLibrary;
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_WITH_ALTERED_SEARCH_PATH};
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let h = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH) };
    if h.is_null() {
        return Err(format!("could not load the DLL: {}", std::io::Error::last_os_error()));
    }
    match unsafe { GetProcAddress(h, abi::ENTRY_NAME.as_ptr()) } {
        // the library stays loaded for the life of the process
        Some(f) => Ok(unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, abi::EntryFn>(f) }),
        None => {
            unsafe { FreeLibrary(h) };
            Err("not a Notepad9 plugin (n9_plugin_entry is missing)".into())
        }
    }
}

/// "Ctrl+Alt+W", "Ctrl+Shift+F9", "F8" ... Plain letters need Ctrl or Alt.
pub fn parse_shortcut(s: &str) -> Option<(bool, bool, bool, Key)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (mut ctrl, mut shift, mut alt) = (false, false, false);
    let mut key = None;
    let (body, plus) = match s.strip_suffix("++") {
        Some(b) => (b, true),
        None => (s, false),
    };
    for part in body.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            _ => key = key_from(part),
        }
    }
    if plus {
        key = Some(Key::Plus);
    }
    let key = key?;
    let name = key.name();
    let fkey = name.len() > 1 && name.starts_with('F') && name[1..].chars().all(|c| c.is_ascii_digit());
    if !ctrl && !alt && !fkey {
        return None;
    }
    Some((ctrl, shift, alt, key))
}

fn key_from(part: &str) -> Option<Key> {
    Key::from_name(part).or_else(|| Key::from_name(&part.to_uppercase())).or_else(|| {
        let mut c = part.chars();
        let first = c.next()?;
        Key::from_name(&format!("{}{}", first.to_uppercase(), c.as_str().to_lowercase()))
    })
}

// ====================================================================== calling plugins

thread_local! {
    static APP: Cell<*mut App> = const { Cell::new(std::ptr::null_mut()) };
    /// last string handed to a plugin; valid until the next host call
    static RET: RefCell<Arc<String>> = RefCell::new(Arc::new(String::new()));
}

static CTX: OnceLock<egui::Context> = OnceLock::new();

/// Run a plugin callback with the editor reachable from the host functions.
/// Returns None when already inside a plugin call (no re-entry).
fn call<R>(app: &mut App, f: impl FnOnce() -> R) -> Option<R> {
    if app.plugins.stopped || !APP.with(|a| a.get()).is_null() {
        return None;
    }
    let p: *mut App = app;
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            APP.with(|a| a.set(std::ptr::null_mut()));
        }
    }
    APP.with(|a| a.set(p));
    let _reset = Reset;
    Some(f())
}

/// The editor, inside a host function. None outside plugin callbacks or on other threads.
fn with<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    let p = APP.with(|a| a.get());
    if p.is_null() {
        return None;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(unsafe { &mut *p }))).ok()
}

fn ret_arc(a: Arc<String>) -> N9Str {
    RET.with(|r| {
        let mut r = r.borrow_mut();
        *r = a;
        N9Str { ptr: r.as_ptr(), len: r.len() }
    })
}

fn ret(s: Option<String>) -> N9Str {
    match s {
        Some(s) if !s.is_empty() => ret_arc(Arc::new(s)),
        _ => N9Str::EMPTY,
    }
}

fn st(s: N9Str) -> String {
    unsafe { s.to_string() }
}

/// Text from a plugin gets the line endings of the active file.
fn norm(a: &App, text: &str) -> String {
    let t = a.tab();
    if t.enc == crate::textfile::FileEncoding::Binary || !text.contains(['\r', '\n']) {
        text.to_string()
    } else {
        crate::textfile::normalize_eol(text, t.eol)
    }
}

fn plugin_mut(a: &mut App, plugin: u32) -> Option<&mut PluginRec> {
    a.plugins.list.get_mut(plugin as usize)
}

fn edited(a: &mut App) {
    a.session_dirty = true;
    a.tab_mut().view.reveal = true;
    a.ctx.request_repaint();
}

// ====================================================================== host functions

extern "C" fn h_log(plugin: u32, text: N9Str) {
    let t = st(text);
    with(|a| {
        if let Some(p) = plugin_mut(a, plugin) {
            p.push_log(t);
        }
    });
}

extern "C" fn h_show_info(_plugin: u32, text: N9Str) {
    let t = st(text);
    with(|a| a.show_info(t));
}

extern "C" fn h_message(plugin: u32, title: N9Str, text: N9Str) {
    let (title, text) = (st(title), st(text));
    with(|a| {
        let title = if title.is_empty() { a.plugins.list.get(plugin as usize).map(|p| p.name.clone()).unwrap_or_default() } else { title };
        a.plugins.messages.push_back((title, text));
        a.ctx.request_repaint();
    });
}

extern "C" fn h_add_menu_item(plugin: u32, path: N9Str, shortcut: N9Str, command: u32) {
    let (path, shortcut) = (st(path), st(shortcut));
    with(|a| {
        let Some(p) = plugin_mut(a, plugin) else { return };
        let key = parse_shortcut(&shortcut);
        if key.is_none() && !shortcut.trim().is_empty() {
            p.push_log(format!("unsupported shortcut \"{shortcut}\" (use Ctrl/Alt + key, or F1..F24)"));
        }
        p.menu.retain(|m| !(m.command == command && m.path == path));
        let shortcut = if key.is_some() { shortcut } else { String::new() };
        p.menu.push(MenuItem { path, shortcut, key, command });
    });
}

extern "C" fn h_add_toolbar_button(plugin: u32, icon: N9Str, png: *const u8, png_len: usize, tooltip: N9Str, command: u32) {
    let (icon, tooltip) = (st(icon), st(tooltip));
    let png = if png.is_null() || png_len == 0 || png_len > 4 << 20 { None } else { Some(unsafe { std::slice::from_raw_parts(png, png_len) }.to_vec()) };
    with(|a| {
        if let Some(p) = plugin_mut(a, plugin) {
            p.toolbar.retain(|t| t.command != command);
            p.toolbar.push(ToolItem { icon, png, tex: None, tooltip, command });
        }
    });
}

extern "C" fn h_open_window(plugin: u32, window: u32, title: N9Str, kind: u32, width: f32, height: f32) {
    let title = st(title);
    with(|a| {
        let Some(p) = plugin_mut(a, plugin) else { return };
        let kind = kind.min(abi::WINDOW_BOTTOM_PANEL);
        let w = if width.is_finite() && width >= 40.0 { width } else { 320.0 };
        let h = if height.is_finite() && height >= 40.0 { height } else { 260.0 };
        match p.windows.iter_mut().find(|x| x.id == window) {
            Some(x) => {
                x.open = true;
                x.kind = kind;
                if !title.is_empty() {
                    x.title = title;
                }
            }
            None => p.windows.push(PWindow {
                id: window,
                title: if title.is_empty() { p.name.clone() } else { title },
                kind,
                size: Vec2::new(w, h),
                open: true,
                nodes: Vec::new(),
                state: WinState::default(),
            }),
        }
        a.ctx.request_repaint();
    });
}

extern "C" fn h_close_window(plugin: u32, window: u32) {
    with(|a| {
        if let Some(w) = plugin_mut(a, plugin).and_then(|p| p.windows.iter_mut().find(|x| x.id == window)) {
            w.open = false;
        }
        a.ctx.request_repaint();
    });
}

extern "C" fn h_is_window_open(plugin: u32, window: u32) -> bool {
    with(|a| plugin_mut(a, plugin).map_or(false, |p| p.windows.iter().any(|x| x.id == window && x.open))).unwrap_or(false)
}

extern "C" fn h_set_timer(plugin: u32, ms: u32) {
    with(|a| {
        if let Some(p) = plugin_mut(a, plugin) {
            p.timer_ms = if ms == 0 { 0 } else { ms.max(15) };
            p.next_timer = None;
        }
        a.ctx.request_repaint();
    });
}

/// The only host function that may be called from any thread.
extern "C" fn h_request_repaint() {
    if let Some(c) = CTX.get() {
        c.request_repaint();
    }
}

extern "C" fn h_tab_count() -> u32 {
    with(|a| a.tabs.len() as u32).unwrap_or(0)
}

extern "C" fn h_active_tab() -> u32 {
    with(|a| a.active as u32).unwrap_or(0)
}

extern "C" fn h_activate_tab(index: u32) {
    with(|a| {
        if (index as usize) < a.tabs.len() {
            a.activate(index as usize);
            a.ctx.request_repaint();
        }
    });
}

extern "C" fn h_tab_title(index: u32) -> N9Str {
    ret(with(|a| a.tabs.get(index as usize).map(|t| t.title())).flatten())
}

extern "C" fn h_tab_path(index: u32) -> N9Str {
    ret(with(|a| a.tabs.get(index as usize).and_then(|t| t.path.as_ref().map(|p| p.display().to_string()))).flatten())
}

extern "C" fn h_tab_modified(index: u32) -> bool {
    with(|a| a.tabs.get(index as usize).map_or(false, |t| t.modified())).unwrap_or(false)
}

extern "C" fn h_new_tab(text: N9Str) -> u32 {
    let text = st(text);
    with(|a| {
        let i = a.new_tab_with(0);
        if !text.is_empty() {
            let text = crate::textfile::normalize_eol(&text, a.tabs[i].eol);
            let d = &mut a.tabs[i].doc;
            d.replace(0, 0, &text);
            d.sel = Sel::caret(0);
        }
        a.activate(i);
        a.ctx.request_repaint();
        i as u32
    })
    .unwrap_or(0)
}

extern "C" fn h_open_file(path: N9Str) -> bool {
    let path = st(path);
    with(|a| {
        let ok = !path.is_empty() && a.open_path(Path::new(&path));
        a.ctx.request_repaint();
        ok
    })
    .unwrap_or(false)
}

extern "C" fn h_save_active() -> bool {
    with(|a| a.save()).unwrap_or(false)
}

extern "C" fn h_get_text() -> N9Str {
    match with(|a| a.tab_mut().doc.text()) {
        Some(t) if !t.is_empty() => ret_arc(t),
        _ => N9Str::EMPTY,
    }
}

extern "C" fn h_set_text(text: N9Str) {
    let text = st(text);
    with(|a| {
        let text = norm(a, &text);
        let d = &mut a.tab_mut().doc;
        let sel = d.sel;
        let n = d.len_chars();
        d.replace(0, n, &text);
        let len = d.len_chars();
        d.sel = Sel { anchor: sel.anchor.min(len), head: sel.head.min(len) };
        edited(a);
    });
}

extern "C" fn h_text_len() -> usize {
    with(|a| a.tab().doc.len_chars()).unwrap_or(0)
}

extern "C" fn h_get_selection(start: *mut usize, end: *mut usize) {
    let (s, e) = with(|a| a.tab().doc.sel.range()).unwrap_or((0, 0));
    unsafe {
        if !start.is_null() {
            *start = s;
        }
        if !end.is_null() {
            *end = e;
        }
    }
}

extern "C" fn h_set_selection(start: usize, end: usize) {
    with(|a| {
        let d = &mut a.tab_mut().doc;
        let n = d.len_chars();
        d.sel = Sel { anchor: start.min(n), head: end.min(n) };
        d.preferred_x = None;
        a.tab_mut().view.reveal = true;
        a.ctx.request_repaint();
    });
}

extern "C" fn h_selected_text() -> N9Str {
    ret(with(|a| a.tab().doc.selected_text()))
}

extern "C" fn h_replace_selection(text: N9Str) {
    let text = st(text);
    with(|a| {
        let text = norm(a, &text);
        let d = &mut a.tab_mut().doc;
        let (s, e) = d.sel.range();
        d.replace_select(s, e, &text);
        edited(a);
    });
}

extern "C" fn h_replace_range(start: usize, end: usize, text: N9Str) {
    let text = st(text);
    with(|a| {
        let text = norm(a, &text);
        let d = &mut a.tab_mut().doc;
        let n = d.len_chars();
        let (s, e) = (start.min(end).min(n), start.max(end).min(n));
        d.replace(s, e, &text);
        edited(a);
    });
}

extern "C" fn h_insert_text(at: usize, text: N9Str) {
    let text = st(text);
    if text.is_empty() {
        return;
    }
    with(|a| {
        let text = norm(a, &text);
        let d = &mut a.tab_mut().doc;
        let at = at.min(d.len_chars());
        d.insert_at(at, &text);
        edited(a);
    });
}

extern "C" fn h_caret() -> usize {
    with(|a| a.tab().doc.sel.head).unwrap_or(0)
}

extern "C" fn h_line_count() -> usize {
    with(|a| a.tab().doc.len_lines()).unwrap_or(0)
}

extern "C" fn h_line_text(line: usize) -> N9Str {
    ret(with(|a| {
        let d = &a.tab().doc;
        if line < d.len_lines() { d.line_string(line) } else { String::new() }
    }))
}

extern "C" fn h_line_of(pos: usize) -> usize {
    with(|a| {
        let d = &a.tab().doc;
        d.line_of(pos.min(d.len_chars()))
    })
    .unwrap_or(0)
}

extern "C" fn h_line_start(line: usize) -> usize {
    with(|a| {
        let d = &a.tab().doc;
        d.line_start(line.min(d.len_lines().saturating_sub(1)))
    })
    .unwrap_or(0)
}

extern "C" fn h_goto_line(line: usize) {
    with(|a| {
        let t = a.tab_mut();
        let l = line.min(t.doc.len_lines().saturating_sub(1));
        t.doc.sel = Sel::caret(t.doc.line_start(l));
        t.doc.preferred_x = None;
        t.view.filter = None;
        t.view.center = Some(l);
        a.focus_editor();
        a.ctx.request_repaint();
    });
}

extern "C" fn h_language() -> N9Str {
    ret(with(|a| a.tab().hl.syntax().map(|s| s.to_string())).flatten())
}

extern "C" fn h_run_command(command: N9Str) {
    let c = st(command);
    if c.is_empty() || c.starts_with('@') || c.starts_with("plugin_cmd:") {
        return;
    }
    with(|a| {
        a.exec(&c);
        a.ctx.request_repaint();
    });
}

extern "C" fn h_clipboard_set(text: N9Str) {
    let t = st(text);
    with(|a| {
        if arboard::Clipboard::new().and_then(|mut c| c.set_text(t.clone())).is_err() {
            a.ctx.copy_text(t);
        }
    });
}

extern "C" fn h_clipboard_get() -> N9Str {
    if APP.with(|a| a.get()).is_null() {
        return N9Str::EMPTY;
    }
    ret(arboard::Clipboard::new().and_then(|mut c| c.get_text()).ok())
}

extern "C" fn h_console_run(command: N9Str, dir: N9Str) {
    let (cmd, dir) = (st(command), st(dir));
    if cmd.trim().is_empty() {
        return;
    }
    with(|a| {
        let dir = if dir.is_empty() {
            a.tab().dir().unwrap_or_else(|| PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()))
        } else {
            PathBuf::from(dir)
        };
        a.console.visible = true;
        let ctx = a.ctx.clone();
        a.console.run(&cmd, &dir, &ctx);
    });
}

extern "C" fn h_setting_get(plugin: u32, key: N9Str) -> N9Str {
    let key = st(key);
    ret(with(|a| plugin_mut(a, plugin).and_then(|p| p.settings().get(&key).cloned())).flatten())
}

extern "C" fn h_setting_set(plugin: u32, key: N9Str, value: N9Str) {
    let (key, value) = (st(key), st(value));
    if key.is_empty() {
        return;
    }
    with(|a| {
        if let Some(p) = plugin_mut(a, plugin) {
            if p.settings().get(&key) != Some(&value) {
                p.settings().insert(key, value);
                p.settings_dirty = true;
            }
        }
    });
}

extern "C" fn h_app_version() -> N9Str {
    N9Str::new(env!("CARGO_PKG_VERSION"))
}

static HOST: HostApi = HostApi {
    api_version: abi::API_VERSION,
    size: std::mem::size_of::<HostApi>() as u32,
    log: h_log,
    show_info: h_show_info,
    message: h_message,
    add_menu_item: h_add_menu_item,
    add_toolbar_button: h_add_toolbar_button,
    open_window: h_open_window,
    close_window: h_close_window,
    is_window_open: h_is_window_open,
    set_timer: h_set_timer,
    request_repaint: h_request_repaint,
    tab_count: h_tab_count,
    active_tab: h_active_tab,
    activate_tab: h_activate_tab,
    tab_title: h_tab_title,
    tab_path: h_tab_path,
    tab_modified: h_tab_modified,
    new_tab: h_new_tab,
    open_file: h_open_file,
    save_active: h_save_active,
    get_text: h_get_text,
    set_text: h_set_text,
    text_len: h_text_len,
    get_selection: h_get_selection,
    set_selection: h_set_selection,
    selected_text: h_selected_text,
    replace_selection: h_replace_selection,
    replace_range: h_replace_range,
    insert_text: h_insert_text,
    caret: h_caret,
    line_count: h_line_count,
    line_text: h_line_text,
    line_of: h_line_of,
    line_start: h_line_start,
    goto_line: h_goto_line,
    language: h_language,
    run_command: h_run_command,
    clipboard_set: h_clipboard_set,
    clipboard_get: h_clipboard_get,
    console_run: h_console_run,
    setting_get: h_setting_get,
    setting_set: h_setting_set,
    app_version: h_app_version,
};

// ====================================================================== UI builder (UiApi)

struct Builder {
    stack: Vec<(u32, String, String, f32, Vec<Node>)>,
    root: Vec<Node>,
    state: WinState,
    ret: String,
    count: usize,
}

impl Builder {
    fn push(&mut self, n: Node) {
        self.count += 1;
        if self.count > 50_000 {
            return;
        }
        match self.stack.last_mut() {
            Some(b) => b.4.push(n),
            None => self.root.push(n),
        }
    }
    fn close_block(&mut self) {
        if let Some((kind, id, title, param, children)) = self.stack.pop() {
            self.push(Node::Block { kind, id, title, param, children });
        }
    }
    fn finish(mut self) -> (Vec<Node>, WinState) {
        while !self.stack.is_empty() {
            self.close_block();
        }
        self.state.clicked.clear();
        (self.root, self.state)
    }
}

fn bld<'a>(ctx: *mut c_void) -> Option<&'a mut Builder> {
    unsafe { (ctx as *mut Builder).as_mut() }
}

fn wid(id: N9Str, text: &str) -> String {
    let id = st(id);
    if id.is_empty() { text.to_string() } else { id }
}

extern "C" fn u_label(ctx: *mut c_void, text: N9Str, style: u32, rgba: u32) {
    if let Some(b) = bld(ctx) {
        b.push(Node::Label { text: st(text), style, rgba });
    }
}

extern "C" fn u_button(ctx: *mut c_void, id: N9Str, text: N9Str) -> bool {
    let Some(b) = bld(ctx) else { return false };
    let text = st(text);
    let id = wid(id, &text);
    let hit = b.state.clicked.remove(&id);
    b.push(Node::Button { id, text });
    hit
}

extern "C" fn u_checkbox(ctx: *mut c_void, id: N9Str, text: N9Str, initial: bool) -> bool {
    let Some(b) = bld(ctx) else { return initial };
    let text = st(text);
    let id = wid(id, &text);
    let v = *b.state.bools.entry(id.clone()).or_insert(initial);
    b.push(Node::Checkbox { id, text });
    v
}

extern "C" fn u_set_checkbox(ctx: *mut c_void, id: N9Str, value: bool) {
    if let Some(b) = bld(ctx) {
        b.state.bools.insert(st(id), value);
    }
}

extern "C" fn u_text_input(ctx: *mut c_void, id: N9Str, hint: N9Str, rows: u32) -> N9Str {
    let Some(b) = bld(ctx) else { return N9Str::EMPTY };
    let hint = st(hint);
    let id = wid(id, &hint);
    b.ret = b.state.texts.entry(id.clone()).or_default().clone();
    b.push(Node::TextInput { id, hint, rows: rows.min(200) });
    N9Str { ptr: b.ret.as_ptr(), len: b.ret.len() }
}

extern "C" fn u_set_input(ctx: *mut c_void, id: N9Str, value: N9Str) {
    if let Some(b) = bld(ctx) {
        b.state.texts.insert(st(id), st(value));
    }
}

extern "C" fn u_slider(ctx: *mut c_void, id: N9Str, text: N9Str, min: f64, max: f64, initial: f64) -> f64 {
    let Some(b) = bld(ctx) else { return initial };
    let text = st(text);
    let id = wid(id, &text);
    let (min, max) = if min.is_finite() && max.is_finite() { (min.min(max), min.max(max)) } else { (0.0, 1.0) };
    let init = if initial.is_finite() { initial.clamp(min, max) } else { min };
    let v = *b.state.nums.entry(id.clone()).or_insert(init);
    b.push(Node::Slider { id, text, min, max });
    v
}

extern "C" fn u_set_slider(ctx: *mut c_void, id: N9Str, value: f64) {
    if let Some(b) = bld(ctx) {
        if value.is_finite() {
            b.state.nums.insert(st(id), value);
        }
    }
}

extern "C" fn u_selectable(ctx: *mut c_void, id: N9Str, text: N9Str, selected: bool) -> bool {
    let Some(b) = bld(ctx) else { return false };
    let text = st(text);
    let id = wid(id, &text);
    let hit = b.state.clicked.remove(&id);
    b.push(Node::Selectable { id, text, selected });
    hit
}

extern "C" fn u_link(ctx: *mut c_void, id: N9Str, text: N9Str) -> bool {
    let Some(b) = bld(ctx) else { return false };
    let text = st(text);
    let id = wid(id, &text);
    let hit = b.state.clicked.remove(&id);
    b.push(Node::Link { id, text });
    hit
}

extern "C" fn u_separator(ctx: *mut c_void) {
    if let Some(b) = bld(ctx) {
        b.push(Node::Separator);
    }
}

extern "C" fn u_space(ctx: *mut c_void, amount: f32) {
    if let Some(b) = bld(ctx) {
        b.push(Node::Space(if amount.is_finite() { amount.clamp(0.0, 400.0) } else { 0.0 }));
    }
}

extern "C" fn u_progress(ctx: *mut c_void, fraction: f32, text: N9Str) {
    if let Some(b) = bld(ctx) {
        b.push(Node::Progress(if fraction.is_finite() { fraction.clamp(0.0, 1.0) } else { 0.0 }, st(text)));
    }
}

extern "C" fn u_begin(ctx: *mut c_void, kind: u32, id: N9Str, title: N9Str, param: f32) {
    if let Some(b) = bld(ctx) {
        if b.stack.len() < 64 {
            b.stack.push((kind, st(id), st(title), if param.is_finite() { param } else { 0.0 }, Vec::new()));
        }
    }
}

extern "C" fn u_end(ctx: *mut c_void) {
    if let Some(b) = bld(ctx) {
        b.close_block();
    }
}

// ====================================================================== drawing plugin windows

fn render(ui: &mut egui::Ui, nodes: &[Node], st: &mut WinState, base: egui::Id, pal: &Palette, changed: &mut bool) {
    for n in nodes {
        match n {
            Node::Label { text, style, rgba } => {
                let mut rt = RichText::new(text.as_str());
                rt = match *style {
                    abi::LABEL_HEADING => rt.size(17.0).strong(),
                    abi::LABEL_MONO => rt.monospace(),
                    abi::LABEL_SMALL => rt.size(11.0),
                    abi::LABEL_WEAK => rt.color(pal.c("ui.textDim")),
                    abi::LABEL_STRONG => rt.strong(),
                    _ => rt,
                };
                if *rgba != 0 {
                    rt = rt.color(Color32::from_rgba_unmultiplied((rgba >> 24) as u8, (rgba >> 16) as u8, (rgba >> 8) as u8, *rgba as u8));
                }
                ui.label(rt);
            }
            Node::Button { id, text } => {
                if crate::widgets::button(ui, text.as_str()).clicked() {
                    st.clicked.insert(id.clone());
                    *changed = true;
                }
            }
            Node::Checkbox { id, text } => {
                let v = st.bools.entry(id.clone()).or_insert(false);
                if crate::widgets::checkbox(ui, v, text.as_str()).changed() {
                    *changed = true;
                }
            }
            Node::TextInput { id, hint, rows } => {
                let v = st.texts.entry(id.clone()).or_default();
                let te = if *rows == 0 { egui::TextEdit::singleline(v) } else { egui::TextEdit::multiline(v).desired_rows(*rows as usize) };
                let te = te.hint_text(hint.as_str()).id(base.with(("input", id.as_str()))).desired_width(f32::INFINITY);
                if ui.add(te).changed() {
                    *changed = true;
                }
            }
            Node::Slider { id, text, min, max } => {
                let v = st.nums.entry(id.clone()).or_insert(*min);
                if ui.add(crate::widgets::Slider::new(v, *min..=*max).text(text.as_str())).changed() {
                    *changed = true;
                }
            }
            Node::Selectable { id, text, selected } => {
                if crate::widgets::selectable(ui, *selected, text.as_str()).clicked() {
                    st.clicked.insert(id.clone());
                    *changed = true;
                }
            }
            Node::Link { id, text } => {
                if ui.link(text.as_str()).clicked() {
                    st.clicked.insert(id.clone());
                    *changed = true;
                }
            }
            Node::Separator => {
                ui.separator();
            }
            Node::Space(a) => ui.add_space(*a),
            Node::Progress(f, t) => {
                let mut p = egui::ProgressBar::new(*f);
                if !t.is_empty() {
                    p = p.text(t.as_str());
                }
                ui.add(p);
            }
            Node::Block { kind, id, title, param, children } => {
                let mut inner = |ui: &mut egui::Ui| render(ui, children, st, base, pal, changed);
                match *kind {
                    abi::BLOCK_HORIZONTAL => {
                        ui.horizontal(|ui| inner(ui));
                    }
                    abi::BLOCK_WRAP => {
                        ui.horizontal_wrapped(|ui| inner(ui));
                    }
                    abi::BLOCK_SCROLL => {
                        let h = if *param > 0.0 { *param } else { f32::INFINITY };
                        egui::ScrollArea::vertical().id_salt(base.with(("scroll", id.as_str()))).max_height(h).auto_shrink([false, true])
                            .show(ui, |ui| inner(ui));
                    }
                    abi::BLOCK_COLLAPSING => {
                        egui::CollapsingHeader::new(title.as_str()).id_salt(base.with(("fold", id.as_str(), title.as_str())))
                            .default_open(*param >= 0.5).show(ui, |ui| inner(ui));
                    }
                    abi::BLOCK_GROUP => {
                        ui.group(|ui| {
                            ui.set_width(ui.available_width());
                            inner(ui)
                        });
                    }
                    _ => {
                        ui.vertical(|ui| inner(ui));
                    }
                }
            }
        }
    }
}

/// Title row of a docked plugin panel. True when its close button was clicked.
fn panel_header(ui: &mut egui::Ui, title: &str, pal: &Palette) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong().color(pal.c("ui.text")));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(crate::widgets::Button::new(RichText::new("×").size(15.0)).frame(false)).on_hover_text("Close").clicked() {
                close = true;
            }
        });
    });
    ui.separator();
    close
}

// ====================================================================== App side

impl App {
    /// Scan the plugin folders and start every enabled plugin (once).
    pub fn load_plugins(&mut self) {
        if self.plugins.loaded {
            return;
        }
        self.plugins.loaded = true;
        let _ = CTX.set(self.ctx.clone());
        housekeeping();
        let disabled: Vec<String> = self.s.disabled_plugins.iter().map(|s| s.to_lowercase()).collect();
        for path in discover() {
            let off = disabled.contains(&stem_of(&path).to_lowercase());
            self.load_one(path, off);
        }
        let (tid, ver, sel) = {
            let t = self.tab();
            (t.id, t.doc.version, t.doc.sel)
        };
        self.plugins.last_text = (tid, ver);
        self.plugins.last_sel = (tid, sel);
        self.plugin_event(abi::EVENT_STARTUP, 0, "", 0);
        self.plugins_dispatch();
        self.ctx.request_repaint();
    }

    /// Add one plugin to the list and start it (unless disabled). Returns its index.
    fn load_one(&mut self, path: PathBuf, disabled: bool) -> usize {
        let stem = stem_of(&path);
        let mut rec = PluginRec::new(path.clone(), stem);
        let id = self.plugins.list.len();
        if disabled {
            rec.status = Status::Disabled;
            self.plugins.list.push(rec);
            return id;
        }
        self.plugins.list.push(rec);
        let entry = match open_library(&path) {
            Ok(e) => e,
            Err(e) => {
                self.plugins.list[id].status = Status::Failed(e);
                return id;
            }
        };
        let desc = call(self, || unsafe { entry(&HOST, id as u32) }).unwrap_or(std::ptr::null());
        let p = &mut self.plugins.list[id];
        if desc.is_null() {
            p.status = Status::Failed("the plugin did not start (it may need a newer Notepad9)".into());
            p.menu.clear();
            p.toolbar.clear();
            p.windows.clear();
            p.timer_ms = 0;
            return id;
        }
        let d = unsafe { &*desc };
        let name = unsafe { d.name.to_string() };
        if !name.is_empty() {
            p.name = name;
        }
        unsafe {
            p.version = d.version.to_string();
            p.author = d.author.to_string();
            p.description = d.description.to_string();
        }
        p.calls = Calls { inst: d.instance, on_command: d.on_command, on_event: d.on_event, on_ui: d.on_ui, shutdown: d.shutdown };
        p.status = Status::Loaded;
        self.plugins.active = true;
        id
    }

    /// "Install Plugin...": pick DLL files, copy them to the user plugin folder and start them now.
    pub fn install_plugin_dialog(&mut self) {
        let Some(files) = rfd::FileDialog::new().set_title("Install Notepad9 Plugin")
            .add_filter("Notepad9 plugin (*.dll)", &["dll"]).pick_files() else { return };
        for f in files {
            self.install_plugin(&f);
        }
    }

    pub fn install_plugin(&mut self, src: &Path) {
        let stem = stem_of(src);
        let dir = user_dir();
        let dest = dir.join(format!("{stem}.dll"));
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.message(format!("Could not create the plugin folder\n{}\n\n{e}", dir.display()));
            return;
        }
        // a quick look before copying: is it a Notepad9 plugin at all?
        let looks_ok = std::fs::read(src).map_or(false, |b| b.starts_with(b"MZ") && b.windows(15).any(|w| w == b"n9_plugin_entry"));
        if !looks_ok {
            self.message(format!("\"{}\" is not a Notepad9 plugin (no n9_plugin_entry in it).", src.display()));
            return;
        }
        self.s.disabled_plugins.retain(|s| !s.eq_ignore_ascii_case(&stem));
        let running = self.plugins.list.iter().any(|p| p.stem.eq_ignore_ascii_case(&stem) && p.is_loaded());
        let same_file = std::fs::canonicalize(src).ok() == std::fs::canonicalize(&dest).ok() && dest.exists();
        if running {
            // the loaded DLL is locked: the new one replaces it on the next start
            if !same_file {
                if let Err(e) = std::fs::copy(src, dest.with_extension("dll.update")) {
                    self.message(format!("Could not copy the plugin.\n\n{e}"));
                    return;
                }
            }
            self.s.save();
            self.message(format!("A plugin named \"{stem}\" is already running.\nThe new version is used after Notepad9 restarts."));
            return;
        }
        if !same_file {
            if let Err(e) = std::fs::copy(src, &dest) {
                self.message(format!("Could not copy the plugin to\n{}\n\n{e}", dest.display()));
                return;
            }
        }
        self.s.save();
        // an entry for it that is disabled / failed / removed is replaced
        let old = self.plugins.list.iter().position(|p| p.stem.eq_ignore_ascii_case(&stem));
        let id = self.load_one(dest, false);
        if let Some(o) = old {
            // keep indices stable: the old entry only stops being listed
            self.plugins.list[o].status = Status::Removed;
            self.plugins.list[o].file = PathBuf::new();
        }
        match &self.plugins.list[id].status {
            Status::Loaded => {
                let name = self.plugins.list[id].name.clone();
                self.plugins.queue.push(Ev { target: Some(id), kind: abi::EVENT_STARTUP, tab: 0, path: String::new(), ch: 0 });
                self.plugins_dispatch();
                self.show_info(format!("Plugin \"{name}\" installed"));
            }
            Status::Failed(e) => {
                let e = e.clone();
                self.message(format!("The plugin was copied but did not start:\n{e}"));
            }
            _ => {}
        }
        self.ctx.request_repaint();
    }

    /// Delete a plugin from disk (a running one is renamed and goes away after a restart).
    fn remove_plugin(&mut self, pi: usize) {
        let Some(p) = self.plugins.list.get(pi) else { return };
        let file = p.file.clone();
        let loaded = p.is_loaded();
        let res = if loaded {
            let to = PathBuf::from(format!("{}.removed", file.display()));
            let _ = std::fs::remove_file(&to);
            std::fs::rename(&file, &to)
        } else {
            std::fs::remove_file(&file)
        };
        match res {
            Ok(()) => {
                let p = &mut self.plugins.list[pi];
                if loaded {
                    // keeps running until the restart, but its menu and buttons go now
                    p.menu.clear();
                    p.toolbar.clear();
                    for w in &mut p.windows {
                        w.open = false;
                    }
                }
                p.status = Status::Removed;
                let name = p.name.clone();
                self.show_info(format!("Plugin \"{name}\" removed"));
            }
            Err(e) => self.message(format!("Could not remove\n{}\n\n{e}", file.display())),
        }
    }

    /// Queue an event for all plugins.
    pub fn plugin_remove_confirmed(&mut self, pi: usize) {
        self.remove_plugin(pi);
    }

    pub fn plugin_event(&mut self, kind: u32, tab: usize, path: &str, ch: u32) {
        if !self.plugins.active {
            return;
        }
        if kind == abi::EVENT_TAB_ACTIVATED {
            self.plugins.queue.retain(|e| e.kind != abi::EVENT_TAB_ACTIVATED);
        }
        self.plugins.queue.push(Ev { target: None, kind, tab: tab as u32, path: path.to_string(), ch });
    }

    /// A menu item / toolbar button / shortcut of a plugin.
    pub fn plugin_command(&mut self, pi: usize, command: u32) {
        let Some(p) = self.plugins.list.get(pi) else { return };
        if !p.is_loaded() {
            return;
        }
        let (inst, f) = (p.calls.inst, p.calls.on_command);
        if let Some(f) = f {
            call(self, || f(inst, command));
            self.ctx.request_repaint();
        }
    }

    /// Deliver queued events (a few rounds: plugins may cause new ones).
    pub fn plugins_dispatch(&mut self) {
        let mut any = false;
        for _ in 0..4 {
            if self.plugins.queue.is_empty() {
                break;
            }
            let q = std::mem::take(&mut self.plugins.queue);
            for ev in &q {
                let raw = abi::Event { kind: ev.kind, tab: ev.tab, path: N9Str::new(&ev.path), ch: ev.ch };
                for pi in 0..self.plugins.list.len() {
                    if ev.target.map_or(false, |t| t != pi) {
                        continue;
                    }
                    let p = &self.plugins.list[pi];
                    if !p.is_loaded() {
                        continue;
                    }
                    let (inst, f) = (p.calls.inst, p.calls.on_event);
                    if let Some(f) = f {
                        call(self, || f(inst, &raw));
                        any = true;
                    }
                }
            }
        }
        if any || !self.plugins.queue.is_empty() {
            self.ctx.request_repaint();
        }
    }

    /// Before the panels: let every open plugin window describe its content.
    pub fn plugins_build_ui(&mut self) {
        if !self.plugins.active {
            return;
        }
        for pi in 0..self.plugins.list.len() {
            let p = &self.plugins.list[pi];
            if !p.is_loaded() {
                continue;
            }
            let Some(on_ui) = p.calls.on_ui else { continue };
            let inst = p.calls.inst;
            let ids: Vec<u32> = p.windows.iter().filter(|w| w.open).map(|w| w.id).collect();
            for wid in ids {
                let Some(w) = self.plugins.list[pi].windows.iter_mut().find(|w| w.id == wid) else { continue };
                let state = std::mem::take(&mut w.state);
                let mut b = Builder { stack: Vec::new(), root: Vec::new(), state, ret: String::new(), count: 0 };
                let api = UiApi {
                    ctx: &mut b as *mut Builder as *mut c_void,
                    label: u_label,
                    button: u_button,
                    checkbox: u_checkbox,
                    set_checkbox: u_set_checkbox,
                    text_input: u_text_input,
                    set_input: u_set_input,
                    slider: u_slider,
                    selectable: u_selectable,
                    link: u_link,
                    separator: u_separator,
                    space: u_space,
                    progress: u_progress,
                    begin: u_begin,
                    end: u_end,
                    set_slider: u_set_slider,
                };
                call(self, || on_ui(inst, wid, &api));
                let (nodes, state) = b.finish();
                if let Some(w) = self.plugins.list[pi].windows.iter_mut().find(|w| w.id == wid) {
                    w.nodes = nodes;
                    w.state = state;
                }
            }
        }
    }

    /// Docked plugin windows (right / bottom). Must run before the central panel.
    pub fn plugins_panels(&mut self, ctx: &egui::Context) {
        if !self.plugins.active {
            return;
        }
        let pal = self.pal.clone();
        let screen = ctx.screen_rect();
        let mut changed = false;
        for (pi, p) in self.plugins.list.iter_mut().enumerate() {
            if !p.is_loaded() {
                continue;
            }
            for w in p.windows.iter_mut().filter(|w| w.open && w.kind != abi::WINDOW_FLOATING) {
                let id = egui::Id::new(("plugin_panel", pi, w.id, w.kind));
                let frame = egui::Frame::none().fill(pal.c("ui.panel")).stroke(Stroke::new(1.0_f32, pal.c("ui.line")))
                    .inner_margin(egui::Margin::same(6.0));
                let mut close = false;
                let body = |ui: &mut egui::Ui| {
                    if panel_header(ui, &w.title, &pal) {
                        close = true;
                    }
                    egui::ScrollArea::vertical().id_salt(id.with("scroll")).auto_shrink([false, false]).show(ui, |ui| {
                        render(ui, &w.nodes, &mut w.state, id, &pal, &mut changed);
                    });
                };
                if w.kind == abi::WINDOW_RIGHT_PANEL {
                    egui::SidePanel::right(id).resizable(true).default_width(w.size.x)
                        .width_range(120.0..=(screen.width() * 0.75).max(160.0)).frame(frame).show(ctx, body);
                } else {
                    egui::TopBottomPanel::bottom(id).resizable(true).default_height(w.size.y)
                        .height_range(50.0..=(screen.height() * 0.75).max(80.0)).frame(frame).show(ctx, body);
                }
                if close {
                    w.open = false;
                    changed = true;
                }
            }
        }
        if changed {
            ctx.request_repaint();
        }
    }

    /// Floating plugin windows.
    pub fn plugins_windows(&mut self, ctx: &egui::Context) {
        if !self.plugins.active {
            return;
        }
        let pal = self.pal.clone();
        let mut changed = false;
        for (pi, p) in self.plugins.list.iter_mut().enumerate() {
            if !p.is_loaded() {
                continue;
            }
            for w in p.windows.iter_mut().filter(|w| w.open && w.kind == abi::WINDOW_FLOATING) {
                let id = egui::Id::new(("plugin_window", pi, w.id));
                let mut open = true;
                egui::Window::new(w.title.as_str()).id(id).open(&mut open).collapsible(false).resizable(true).default_size(w.size)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().id_salt(id.with("scroll")).auto_shrink([false, true]).show(ui, |ui| {
                            render(ui, &w.nodes, &mut w.state, id, &pal, &mut changed);
                        });
                    });
                if !open {
                    w.open = false;
                    changed = true;
                }
            }
        }
        if changed {
            ctx.request_repaint();
        }
    }

    /// End of frame: notice text / caret changes, fire timers, show messages, deliver events.
    pub fn plugins_after_frame(&mut self) {
        if !self.plugins.active {
            return;
        }
        let (tid, ver, sel) = {
            let t = self.tab();
            (t.id, t.doc.version, t.doc.sel)
        };
        let i = self.active;
        if self.plugins.last_text.0 == tid && self.plugins.last_text.1 != ver {
            self.plugin_event(abi::EVENT_TEXT_CHANGED, i, "", 0);
        }
        self.plugins.last_text = (tid, ver);
        if self.plugins.last_sel.0 == tid && self.plugins.last_sel.1 != sel {
            self.plugin_event(abi::EVENT_SELECTION_CHANGED, i, "", 0);
        }
        self.plugins.last_sel = (tid, sel);

        let now = Instant::now();
        let mut wait: Option<Duration> = None;
        let Plugins { list, queue, .. } = &mut self.plugins;
        for (pi, p) in list.iter_mut().enumerate() {
            if !p.is_loaded() || p.timer_ms == 0 {
                continue;
            }
            let period = Duration::from_millis(p.timer_ms as u64);
            let next = p.next_timer.get_or_insert(now + period);
            if now >= *next {
                queue.push(Ev { target: Some(pi), kind: abi::EVENT_TIMER, tab: 0, path: String::new(), ch: 0 });
                *next = now + period;
            }
            let left = next.saturating_duration_since(now);
            wait = Some(wait.map_or(left, |w| w.min(left)));
        }
        if let Some(w) = wait {
            self.ctx.request_repaint_after(w);
        }

        if self.modal.is_none() {
            if let Some((title, text)) = self.plugins.messages.pop_front() {
                self.modal = Some(Modal { title, text, buttons: vec![("OK".into(), ModalAction::Nothing)] });
            }
        }

        let due = self.plugins.last_settings_save.map_or(true, |t| t.elapsed() > Duration::from_secs(5));
        if due && self.plugins.list.iter().any(|p| p.settings_dirty) {
            self.plugins.last_settings_save = Some(Instant::now());
            for p in &mut self.plugins.list {
                p.save_settings();
            }
        }

        self.plugins_dispatch();
    }

    /// Notepad9 is closing.
    pub fn plugins_shutdown(&mut self) {
        if self.plugins.stopped {
            return;
        }
        for pi in 0..self.plugins.list.len() {
            let p = &self.plugins.list[pi];
            if !p.is_loaded() {
                continue;
            }
            let (inst, f) = (p.calls.inst, p.calls.shutdown);
            if let Some(f) = f {
                call(self, || f(inst));
            }
        }
        for p in &mut self.plugins.list {
            p.save_settings();
        }
        self.plugins.stopped = true;
    }

    // ------------------------------------------------------------------ menu, toolbar, manager

    pub fn plugins_menu(&mut self, ui: &mut egui::Ui, cmds: &mut Vec<String>) {
        let mut any = false;
        for pi in 0..self.plugins.list.len() {
            let p = &self.plugins.list[pi];
            if !p.is_loaded() || p.menu.is_empty() {
                continue;
            }
            any = true;
            // (submenu, label, shortcut, command)
            let items: Vec<(String, String, String, u32)> = p.menu.iter().map(|m| {
                let (g, l) = match m.path.split_once('/') {
                    Some((g, l)) => (g.trim().to_string(), l.trim().to_string()),
                    None => (String::new(), m.path.trim().to_string()),
                };
                (g, l, m.shortcut.clone(), m.command)
            }).collect();
            let mut groups: Vec<String> = Vec::new();
            for (g, ..) in &items {
                if !g.is_empty() && !groups.contains(g) {
                    groups.push(g.clone());
                }
            }
            for (_, l, sc, c) in items.iter().filter(|i| i.0.is_empty()) {
                self.mi(ui, cmds, l, sc, None, true, &format!("plugin_cmd:{pi}:{c}"));
            }
            for g in groups {
                ui.menu_button(crate::widgets::sub(ui, &g), |ui| {
                    for (_, l, sc, c) in items.iter().filter(|i| i.0 == g) {
                        self.mi(ui, cmds, l, sc, None, true, &format!("plugin_cmd:{pi}:{c}"));
                    }
                });
            }
        }
        if !any {
            ui.add_enabled(false, crate::widgets::Button::new("      (no plugins installed)"));
        }
        ui.separator();
        self.mi(ui, cmds, "Install Plugin...", "", Some("open"), true, "plugin_install");
        self.mi(ui, cmds, "Plugin Manager...", "", Some("settings"), true, "plugin_manager");
        self.mi(ui, cmds, "Open Plugins Folder", "", None, true, "plugins_folder");
    }

    /// Toolbar buttons added by plugins (after the built-in ones).
    pub fn plugins_toolbar(&mut self, ui: &mut egui::Ui, cmds: &mut Vec<String>) {
        if !self.plugins.list.iter().any(|p| p.is_loaded() && !p.toolbar.is_empty()) {
            return;
        }
        self.tool_sep(ui);
        let ctx = ui.ctx().clone();
        for pi in 0..self.plugins.list.len() {
            if !self.plugins.list[pi].is_loaded() {
                continue;
            }
            for ti in 0..self.plugins.list[pi].toolbar.len() {
                let it = &mut self.plugins.list[pi].toolbar[ti];
                if it.tex.is_none() {
                    it.tex = Some(it.png.as_ref().and_then(|b| {
                        let img = image::load_from_memory(b).ok()?.to_rgba8();
                        let size = [img.width() as usize, img.height() as usize];
                        let img = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                        Some(ctx.load_texture(format!("plugin_icon_{pi}_{ti}"), img, egui::TextureOptions::LINEAR))
                    }));
                }
                let tex = it.tex.clone().flatten();
                let (icon, tip, cmd) = (it.icon.clone(), it.tooltip.clone(), it.command);
                let (rect, resp, size) = self.tool_frame(ui, true, false);
                if let Some(t) = tex {
                    let r = egui::Rect::from_center_size(rect.center(), Vec2::splat(size));
                    ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE);
                } else if !icon.is_empty() && self.icons.get(&ctx, &icon, size, true).is_some() {
                    self.icons.paint(ui, &icon, rect, size, true);
                } else {
                    let letter: String = tip.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or("P".into());
                    ui.painter().rect_stroke(egui::Rect::from_center_size(rect.center(), Vec2::splat(size - 2.0)), 3.0,
                                             Stroke::new(1.0_f32, self.pal.c("ui.textDim")));
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, letter, egui::FontId::proportional(size * 0.62), self.pal.c("ui.text"));
                }
                let tip = if tip.is_empty() { self.plugins.list[pi].name.clone() } else { tip };
                if resp.on_hover_text(tip).clicked() {
                    cmds.push(format!("plugin_cmd:{pi}:{cmd}"));
                    cmds.push("@focus_editor".into());
                }
            }
        }
    }

    pub fn plugin_manager_window(&mut self, ctx: &egui::Context) {
        if !self.plugins.manager_open {
            return;
        }
        let mut open = true;
        let mut toggles: Vec<(String, bool)> = Vec::new();
        let mut open_folder = false;
        let mut install = false;
        let mut remove: Option<usize> = None;
        let dim = self.pal.c("ui.textDim");
        let disabled: Vec<String> = self.s.disabled_plugins.iter().map(|s| s.to_lowercase()).collect();
        let restart_needed = self.plugins.list.iter().any(|p| {
            let off = disabled.contains(&p.stem.to_lowercase());
            (off && p.is_loaded()) || (!off && matches!(p.status, Status::Disabled))
        });
        egui::Window::new("Plugin Manager").open(&mut open).collapsible(false).resizable(true).default_size(Vec2::new(640.0, 460.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{} plugin(s)", self.plugins.list.iter().filter(|p| !p.removed()).count()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::widgets::button(ui, "Open Plugins Folder").clicked() {
                            open_folder = true;
                        }
                        let b = crate::widgets::Button::new(RichText::new("Install Plugin...").color(Color32::WHITE)).fill(self.pal.c("ui.accent"));
                        if ui.add(b).on_hover_text("Choose a plugin .dll: it is copied to your plugin folder and started").clicked() {
                            install = true;
                        }
                    });
                });
                let mut folders = format!("Plugins are loaded from:\n  {}", user_dir().display());
                if let Some(d) = app_dir() {
                    folders.push_str(&format!("\n  {}", d.display()));
                }
                ui.label(RichText::new(folders).size(11.5).color(dim));
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - 64.0).show(ui, |ui| {
                    if self.plugins.list.iter().all(|p| p.removed()) {
                        ui.add_space(12.0);
                        ui.label("No plugins installed.");
                        ui.label(RichText::new("Click \"Install Plugin...\" and choose a plugin .dll file.").color(dim));
                    }
                    for (pi, p) in self.plugins.list.iter().enumerate() {
                        if p.removed() {
                            continue;
                        }
                        ui.group(|ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let mut en = !disabled.contains(&p.stem.to_lowercase());
                                if crate::widgets::checkbox(ui, &mut en, "").on_hover_text("Enabled (applies after a restart)").changed() {
                                    toggles.push((p.stem.clone(), en));
                                }
                                ui.label(RichText::new(&p.name).strong().size(14.0));
                                if !p.version.is_empty() {
                                    ui.label(RichText::new(format!("v{}", p.version)).color(dim));
                                }
                                if !p.author.is_empty() {
                                    ui.label(RichText::new(format!("by {}", p.author)).color(dim));
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let (text, color) = match &p.status {
                                        Status::Loaded => ("Running", Color32::from_rgb(110, 190, 110)),
                                        Status::Disabled => ("Disabled", dim),
                                        Status::Failed(_) => ("Error", Color32::from_rgb(230, 100, 90)),
                                        Status::Removed => ("Removed", dim),
                                    };
                                    if crate::widgets::small_button(ui, "Remove").on_hover_text("Delete this plugin from disk").clicked() {
                                        remove = Some(pi);
                                    }
                                    ui.label(RichText::new(text).color(color));
                                });
                            });
                            if !p.description.is_empty() {
                                ui.label(p.description.as_str());
                            }
                            if let Status::Failed(e) = &p.status {
                                ui.label(RichText::new(e).color(Color32::from_rgb(230, 100, 90)));
                            }
                            ui.label(RichText::new(p.file.display().to_string()).size(11.0).color(dim));
                            if !p.log.is_empty() {
                                egui::CollapsingHeader::new(format!("Log ({})", p.log.len())).id_salt(("plugin_log", &p.stem)).show(ui, |ui| {
                                    egui::ScrollArea::vertical().max_height(140.0).stick_to_bottom(true).show(ui, |ui| {
                                        for l in &p.log {
                                            ui.label(RichText::new(l).monospace().size(11.5));
                                        }
                                    });
                                });
                            }
                        });
                    }
                });
                ui.separator();
                if restart_needed {
                    ui.label(RichText::new("Restart Notepad9 to apply the changes.").color(self.pal.c("ui.accent")));
                }
                ui.label(RichText::new("Plugins are native code and run with the same rights as Notepad9. Install only plugins you trust.")
                    .size(11.5).color(dim));
            });
        for (stem, en) in toggles {
            self.s.disabled_plugins.retain(|s| !s.eq_ignore_ascii_case(&stem));
            if !en {
                self.s.disabled_plugins.push(stem);
            }
            self.s.save();
        }
        if open_folder {
            self.run("plugins_folder");
        }
        if install {
            self.install_plugin_dialog();
        }
        if let Some(pi) = remove {
            let name = self.plugins.list[pi].name.clone();
            self.modal = Some(Modal {
                title: "Remove Plugin".into(),
                text: format!("Delete the plugin \"{name}\" from disk?"),
                buttons: vec![("Remove".into(), ModalAction::RemovePlugin(pi)), ("Cancel".into(), ModalAction::Nothing)],
            });
        }
        self.plugins.manager_open = open;
    }
}
