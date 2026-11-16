//! # Notepad9 plugin SDK
//!
//! A plugin is a Rust `cdylib` (`plugin.dll`) that implements [`Plugin`] and calls
//! [`export_plugin!`]. Put the DLL into `%APPDATA%\Notepad9\plugins` (or the `plugins`
//! folder next to Notepad9.exe) and restart Notepad9.
//!
//! ```ignore
//! use notepad9_plugin::*;
//!
//! #[derive(Default)]
//! struct Hello;
//!
//! impl Plugin for Hello {
//!     fn info() -> Info {
//!         Info { name: "Hello", version: "1.0", author: "me", description: "Says hello" }
//!     }
//!     fn init(&mut self, host: &Host) {
//!         host.add_menu_item("Hello/Say hello", "Ctrl+Alt+H", 1);
//!     }
//!     fn on_command(&mut self, host: &Host, command: u32) {
//!         if command == 1 {
//!             host.message("Hello", &format!("The text has {} lines", host.line_count()));
//!         }
//!     }
//! }
//!
//! export_plugin!(Hello);
//! ```

pub mod abi;

use abi::*;
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub use abi::{
    BLOCK_COLLAPSING, BLOCK_GROUP, BLOCK_HORIZONTAL, BLOCK_SCROLL, BLOCK_VERTICAL, BLOCK_WRAP, EVENT_CHAR_TYPED,
    EVENT_FILE_OPENED, EVENT_FILE_SAVED, EVENT_SELECTION_CHANGED, EVENT_STARTUP, EVENT_TAB_ACTIVATED,
    EVENT_TEXT_CHANGED, EVENT_TIMER, WINDOW_BOTTOM_PANEL, WINDOW_FLOATING, WINDOW_RIGHT_PANEL,
};

fn s(t: &str) -> N9Str {
    N9Str::new(t)
}

/// What the plugin manager shows about the plugin.
pub struct Info {
    pub name: &'static str,
    pub version: &'static str,
    pub author: &'static str,
    pub description: &'static str,
}

/// Where a plugin window appears.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WindowKind {
    /// a movable window over the editor
    Floating,
    /// a resizable panel on the right side
    RightPanel,
    /// a resizable panel above the status bar
    BottomPanel,
}

impl WindowKind {
    fn raw(self) -> u32 {
        match self {
            WindowKind::Floating => WINDOW_FLOATING,
            WindowKind::RightPanel => WINDOW_RIGHT_PANEL,
            WindowKind::BottomPanel => WINDOW_BOTTOM_PANEL,
        }
    }
}

/// Something that happened in the editor.
#[derive(Clone, Debug)]
pub enum Event {
    /// all plugins are loaded
    Startup,
    TabActivated { tab: u32 },
    FileOpened { tab: u32, path: String },
    FileSaved { tab: u32, path: String },
    /// the text of the active tab changed (at most once per frame)
    TextChanged { tab: u32 },
    SelectionChanged { tab: u32 },
    CharTyped { tab: u32, ch: char },
    /// see [`Host::set_timer`]
    Timer,
    Other(u32),
}

/// Access to Notepad9. Only use it inside the plugin callbacks (UI thread).
pub struct Host {
    api: &'static HostApi,
    id: u32,
}

impl Host {
    pub fn log(&self, text: &str) {
        (self.api.log)(self.id, s(text))
    }
    /// Short message in the status bar.
    pub fn show_info(&self, text: &str) {
        (self.api.show_info)(self.id, s(text))
    }
    /// Message box with an OK button.
    pub fn message(&self, title: &str, text: &str) {
        (self.api.message)(self.id, s(title), s(text))
    }

    /// Item in the Plugins menu. `path` "Group/Item" makes a submenu; `shortcut` like "Ctrl+Alt+W" or "".
    pub fn add_menu_item(&self, path: &str, shortcut: &str, command: u32) {
        (self.api.add_menu_item)(self.id, s(path), s(shortcut), command)
    }
    /// Toolbar button with a built-in icon ("find", "star", "about", "run" ...).
    pub fn add_toolbar_button(&self, icon: &str, tooltip: &str, command: u32) {
        (self.api.add_toolbar_button)(self.id, s(icon), std::ptr::null(), 0, s(tooltip), command)
    }
    /// Toolbar button with your own PNG icon (16×16 or 32×32 looks best).
    pub fn add_toolbar_button_png(&self, png: &'static [u8], tooltip: &str, command: u32) {
        (self.api.add_toolbar_button)(self.id, N9Str::EMPTY, png.as_ptr(), png.len(), s(tooltip), command)
    }

    /// Show a window; its content comes from [`Plugin::ui`] with the same `window` id.
    pub fn open_window(&self, window: u32, title: &str, kind: WindowKind, width: f32, height: f32) {
        (self.api.open_window)(self.id, window, s(title), kind.raw(), width, height)
    }
    pub fn close_window(&self, window: u32) {
        (self.api.close_window)(self.id, window)
    }
    pub fn is_window_open(&self, window: u32) -> bool {
        (self.api.is_window_open)(self.id, window)
    }
    pub fn toggle_window(&self, window: u32, title: &str, kind: WindowKind, width: f32, height: f32) {
        if self.is_window_open(window) {
            self.close_window(window)
        } else {
            self.open_window(window, title, kind, width, height)
        }
    }
    /// [`Event::Timer`] every `ms` milliseconds; 0 stops it.
    pub fn set_timer(&self, ms: u32) {
        (self.api.set_timer)(self.id, ms)
    }
    pub fn request_repaint(&self) {
        (self.api.request_repaint)()
    }

    // ---- tabs and files
    pub fn tab_count(&self) -> usize {
        (self.api.tab_count)() as usize
    }
    pub fn active_tab(&self) -> usize {
        (self.api.active_tab)() as usize
    }
    pub fn activate_tab(&self, index: usize) {
        (self.api.activate_tab)(index as u32)
    }
    pub fn tab_title(&self, index: usize) -> String {
        unsafe { (self.api.tab_title)(index as u32).to_string() }
    }
    /// Empty for a tab that was never saved.
    pub fn tab_path(&self, index: usize) -> String {
        unsafe { (self.api.tab_path)(index as u32).to_string() }
    }
    pub fn tab_modified(&self, index: usize) -> bool {
        (self.api.tab_modified)(index as u32)
    }
    pub fn new_tab(&self, text: &str) -> usize {
        (self.api.new_tab)(s(text)) as usize
    }
    pub fn open_file(&self, path: &str) -> bool {
        (self.api.open_file)(s(path))
    }
    pub fn save(&self) -> bool {
        (self.api.save_active)()
    }

    // ---- text of the active tab (positions are character indices)
    pub fn text(&self) -> String {
        unsafe { (self.api.get_text)().to_string() }
    }
    /// Replace the whole text (one undo step).
    pub fn set_text(&self, text: &str) {
        (self.api.set_text)(s(text))
    }
    pub fn text_len(&self) -> usize {
        (self.api.text_len)()
    }
    pub fn selection(&self) -> (usize, usize) {
        let (mut a, mut b) = (0usize, 0usize);
        (self.api.get_selection)(&mut a, &mut b);
        (a, b)
    }
    pub fn set_selection(&self, start: usize, end: usize) {
        (self.api.set_selection)(start, end)
    }
    pub fn selected_text(&self) -> String {
        unsafe { (self.api.selected_text)().to_string() }
    }
    pub fn replace_selection(&self, text: &str) {
        (self.api.replace_selection)(s(text))
    }
    pub fn replace_range(&self, start: usize, end: usize, text: &str) {
        (self.api.replace_range)(start, end, s(text))
    }
    pub fn insert_text(&self, at: usize, text: &str) {
        (self.api.insert_text)(at, s(text))
    }
    pub fn caret(&self) -> usize {
        (self.api.caret)()
    }
    pub fn line_count(&self) -> usize {
        (self.api.line_count)()
    }
    /// Text of a 0-based line, without its line break.
    pub fn line_text(&self, line: usize) -> String {
        unsafe { (self.api.line_text)(line).to_string() }
    }
    pub fn line_of(&self, pos: usize) -> usize {
        (self.api.line_of)(pos)
    }
    pub fn line_start(&self, line: usize) -> usize {
        (self.api.line_start)(line)
    }
    /// Move the caret to a 0-based line and scroll to it.
    pub fn goto_line(&self, line: usize) {
        (self.api.goto_line)(line)
    }
    /// Syntax of the active tab ("Rust", "Python", "Markdown" ...), empty for plain text.
    pub fn language(&self) -> String {
        unsafe { (self.api.language)().to_string() }
    }

    // ---- misc
    /// Built-in command: "save", "find", "replace", "wrap", "zoom_in", "console", "run" ...
    pub fn run_command(&self, command: &str) {
        (self.api.run_command)(s(command))
    }
    pub fn set_clipboard(&self, text: &str) {
        (self.api.clipboard_set)(s(text))
    }
    pub fn clipboard(&self) -> String {
        unsafe { (self.api.clipboard_get)().to_string() }
    }
    /// Run a command line in the Notepad9 console.
    pub fn console_run(&self, command: &str, dir: &str) {
        (self.api.console_run)(s(command), s(dir))
    }
    /// Setting of this plugin (empty if never set).
    pub fn setting(&self, key: &str) -> String {
        unsafe { (self.api.setting_get)(self.id, s(key)).to_string() }
    }
    pub fn set_setting(&self, key: &str, value: &str) {
        (self.api.setting_set)(self.id, s(key), s(value))
    }
    pub fn app_version(&self) -> String {
        unsafe { (self.api.app_version)().to_string() }
    }
}

/// Builds the content of a plugin window. Inputs keep their value between frames (per id).
pub struct Ui<'a> {
    api: &'a UiApi,
}

impl Ui<'_> {
    fn ctx(&self) -> *mut c_void {
        self.api.ctx
    }
    pub fn label(&mut self, text: &str) {
        (self.api.label)(self.ctx(), s(text), abi::LABEL_NORMAL, 0)
    }
    pub fn heading(&mut self, text: &str) {
        (self.api.label)(self.ctx(), s(text), abi::LABEL_HEADING, 0)
    }
    pub fn strong(&mut self, text: &str) {
        (self.api.label)(self.ctx(), s(text), abi::LABEL_STRONG, 0)
    }
    pub fn monospace(&mut self, text: &str) {
        (self.api.label)(self.ctx(), s(text), abi::LABEL_MONO, 0)
    }
    pub fn small(&mut self, text: &str) {
        (self.api.label)(self.ctx(), s(text), abi::LABEL_SMALL, 0)
    }
    /// Dimmed text.
    pub fn weak(&mut self, text: &str) {
        (self.api.label)(self.ctx(), s(text), abi::LABEL_WEAK, 0)
    }
    /// Text in a color (r, g, b).
    pub fn colored(&mut self, text: &str, rgb: (u8, u8, u8)) {
        let rgba = ((rgb.0 as u32) << 24) | ((rgb.1 as u32) << 16) | ((rgb.2 as u32) << 8) | 0xFF;
        (self.api.label)(self.ctx(), s(text), abi::LABEL_NORMAL, rgba)
    }
    /// True once after a click.
    pub fn button(&mut self, id: &str, text: &str) -> bool {
        (self.api.button)(self.ctx(), s(id), s(text))
    }
    /// Current state (starts as `initial`).
    pub fn checkbox(&mut self, id: &str, text: &str, initial: bool) -> bool {
        (self.api.checkbox)(self.ctx(), s(id), s(text), initial)
    }
    pub fn set_checkbox(&mut self, id: &str, value: bool) {
        (self.api.set_checkbox)(self.ctx(), s(id), value)
    }
    /// Single-line text field; returns its current text.
    pub fn text_input(&mut self, id: &str, hint: &str) -> String {
        unsafe { (self.api.text_input)(self.ctx(), s(id), s(hint), 0).to_string() }
    }
    /// Multi-line text field.
    pub fn text_area(&mut self, id: &str, hint: &str, rows: u32) -> String {
        unsafe { (self.api.text_input)(self.ctx(), s(id), s(hint), rows.max(1)).to_string() }
    }
    pub fn set_input(&mut self, id: &str, value: &str) {
        (self.api.set_input)(self.ctx(), s(id), s(value))
    }
    pub fn slider(&mut self, id: &str, text: &str, min: f64, max: f64, initial: f64) -> f64 {
        (self.api.slider)(self.ctx(), s(id), s(text), min, max, initial)
    }
    pub fn set_slider(&mut self, id: &str, value: f64) {
        (self.api.set_slider)(self.ctx(), s(id), value)
    }
    /// A list row; true once after a click.
    pub fn selectable(&mut self, id: &str, text: &str, selected: bool) -> bool {
        (self.api.selectable)(self.ctx(), s(id), s(text), selected)
    }
    pub fn link(&mut self, id: &str, text: &str) -> bool {
        (self.api.link)(self.ctx(), s(id), s(text))
    }
    pub fn separator(&mut self) {
        (self.api.separator)(self.ctx())
    }
    pub fn space(&mut self, amount: f32) {
        (self.api.space)(self.ctx(), amount)
    }
    pub fn progress(&mut self, fraction: f32, text: &str) {
        (self.api.progress)(self.ctx(), fraction, s(text))
    }
    fn block(&mut self, kind: u32, id: &str, title: &str, param: f32, f: impl FnOnce(&mut Self)) {
        (self.api.begin)(self.ctx(), kind, s(id), s(title), param);
        f(self);
        (self.api.end)(self.ctx());
    }
    /// Widgets side by side.
    pub fn horizontal(&mut self, f: impl FnOnce(&mut Self)) {
        self.block(BLOCK_HORIZONTAL, "", "", 0.0, f)
    }
    /// Side by side, wrapping to the next row when needed.
    pub fn wrap(&mut self, f: impl FnOnce(&mut Self)) {
        self.block(BLOCK_WRAP, "", "", 0.0, f)
    }
    pub fn vertical(&mut self, f: impl FnOnce(&mut Self)) {
        self.block(BLOCK_VERTICAL, "", "", 0.0, f)
    }
    /// Scrollable area, at most `max_height` tall.
    pub fn scroll(&mut self, id: &str, max_height: f32, f: impl FnOnce(&mut Self)) {
        self.block(BLOCK_SCROLL, id, "", max_height, f)
    }
    /// Section that can be folded by clicking its title.
    pub fn collapsing(&mut self, id: &str, title: &str, open: bool, f: impl FnOnce(&mut Self)) {
        self.block(BLOCK_COLLAPSING, id, title, if open { 1.0 } else { 0.0 }, f)
    }
    /// Framed group.
    pub fn group(&mut self, f: impl FnOnce(&mut Self)) {
        self.block(BLOCK_GROUP, "", "", 0.0, f)
    }
}

/// Implement this for your plugin type (which must also be `Default`).
pub trait Plugin: Default + 'static {
    fn info() -> Info;
    /// Register menu items, toolbar buttons, open windows ...
    fn init(&mut self, _host: &Host) {}
    /// A menu item / toolbar button / shortcut with this command id was used.
    fn on_command(&mut self, _host: &Host, _command: u32) {}
    fn on_event(&mut self, _host: &Host, _event: &Event) {}
    /// Content of window `window` (called every frame while it is open).
    fn ui(&mut self, _host: &Host, _window: u32, _ui: &mut Ui) {}
    fn shutdown(&mut self, _host: &Host) {}
}

#[doc(hidden)]
pub struct State<P> {
    plugin: P,
    host: Host,
}

fn guard(host: &Host, what: &str, f: impl FnOnce()) {
    if let Err(e) = catch_unwind(AssertUnwindSafe(f)) {
        let msg = e
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| e.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".into());
        host.log(&format!("panic in {what}: {msg}"));
        host.show_info(&format!("Plugin error in {what}: {msg}"));
    }
}

extern "C" fn tr_command<P: Plugin>(inst: *mut c_void, command: u32) {
    let st = unsafe { &mut *(inst as *mut State<P>) };
    let (plugin, host) = (&mut st.plugin, &st.host);
    guard(host, "on_command", || plugin.on_command(host, command));
}

extern "C" fn tr_event<P: Plugin>(inst: *mut c_void, ev: *const abi::Event) {
    let st = unsafe { &mut *(inst as *mut State<P>) };
    let ev = unsafe { &*ev };
    let path = unsafe { ev.path.to_string() };
    let event = match ev.kind {
        EVENT_STARTUP => Event::Startup,
        EVENT_TAB_ACTIVATED => Event::TabActivated { tab: ev.tab },
        EVENT_FILE_OPENED => Event::FileOpened { tab: ev.tab, path },
        EVENT_FILE_SAVED => Event::FileSaved { tab: ev.tab, path },
        EVENT_TEXT_CHANGED => Event::TextChanged { tab: ev.tab },
        EVENT_SELECTION_CHANGED => Event::SelectionChanged { tab: ev.tab },
        EVENT_CHAR_TYPED => Event::CharTyped { tab: ev.tab, ch: char::from_u32(ev.ch).unwrap_or('\0') },
        EVENT_TIMER => Event::Timer,
        k => Event::Other(k),
    };
    let (plugin, host) = (&mut st.plugin, &st.host);
    guard(host, "on_event", || plugin.on_event(host, &event));
}

extern "C" fn tr_ui<P: Plugin>(inst: *mut c_void, window: u32, ui: *const UiApi) {
    let st = unsafe { &mut *(inst as *mut State<P>) };
    let mut ui = Ui { api: unsafe { &*ui } };
    let (plugin, host) = (&mut st.plugin, &st.host);
    guard(host, "ui", || plugin.ui(host, window, &mut ui));
}

extern "C" fn tr_shutdown<P: Plugin>(inst: *mut c_void) {
    let st = unsafe { &mut *(inst as *mut State<P>) };
    let (plugin, host) = (&mut st.plugin, &st.host);
    guard(host, "shutdown", || plugin.shutdown(host));
}

#[doc(hidden)]
pub fn __entry<P: Plugin>(host: *const HostApi, id: u32) -> *const PluginDesc {
    if host.is_null() {
        return std::ptr::null();
    }
    let api: &'static HostApi = unsafe { &*host };
    if api.api_version < API_VERSION {
        return std::ptr::null();
    }
    let info = P::info();
    let state = Box::into_raw(Box::new(State { plugin: P::default(), host: Host { api, id } }));
    {
        let st = unsafe { &mut *state };
        let (plugin, host) = (&mut st.plugin, &st.host);
        guard(host, "init", || plugin.init(host));
    }
    let desc = PluginDesc {
        api_version: API_VERSION,
        name: N9Str::new(info.name),
        version: N9Str::new(info.version),
        author: N9Str::new(info.author),
        description: N9Str::new(info.description),
        instance: state as *mut c_void,
        on_command: Some(tr_command::<P>),
        on_event: Some(tr_event::<P>),
        on_ui: Some(tr_ui::<P>),
        shutdown: Some(tr_shutdown::<P>),
    };
    Box::into_raw(Box::new(desc))
}

/// Exports `n9_plugin_entry` for your plugin type.
#[macro_export]
macro_rules! export_plugin {
    ($ty:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn n9_plugin_entry(
            host: *const $crate::abi::HostApi,
            plugin: u32,
        ) -> *const $crate::abi::PluginDesc {
            $crate::__entry::<$ty>(host, plugin)
        }
    };
}
