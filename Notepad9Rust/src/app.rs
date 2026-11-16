//! Application state: tabs, files, session, commands.

use crate::complete::Completion;
use crate::console::Console;
use crate::doc::{Doc, Sel};
use crate::highlight::{self, Highlighter};
use crate::icons::Icons;
use crate::mdview::Preview;
use crate::search::Opts;
use crate::session::{self, SessionData, SessionTab};
use crate::settings::AppSettings;
use crate::textfile::{self, Eol, FileEncoding};
use crate::theme::{self, Palette};
use crate::view::ViewState;
use eframe::egui::{self, Key};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Instant, SystemTime};

pub struct Tab {
    pub id: u64,
    pub doc: Doc,
    pub view: ViewState,
    pub hl: Highlighter,
    pub path: Option<PathBuf>,
    pub untitled: u32,
    pub enc: FileEncoding,
    pub eol: Eol,
    pub mtime: Option<SystemTime>,
    pub pending: Option<SessionTab>,
    /// syntax to resolve once the grammars finished loading
    pub want_syntax_for: Option<String>,
}

impl Tab {
    pub fn title(&self) -> String {
        match &self.path {
            Some(p) => p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            None if self.untitled <= 1 => "Untitled".into(),
            None => format!("Untitled {}", self.untitled),
        }
    }
    pub fn modified(&self) -> bool {
        self.doc.is_modified() || self.pending.as_ref().map_or(false, |p| p.modified)
    }
    pub fn pristine(&self) -> bool {
        self.path.is_none() && !self.doc.is_modified() && self.doc.len_chars() == 0 && self.pending.is_none()
    }
    pub fn dir(&self) -> Option<PathBuf> {
        self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()))
    }
    pub fn is_markdown(&self) -> bool {
        self.hl.syntax() == Some("Markdown")
            || self.path.as_ref().and_then(|p| p.extension()).map_or(false, |e| {
                let e = e.to_string_lossy().to_lowercase();
                e == "md" || e == "markdown"
            })
    }
}

pub enum ModalAction {
    Nothing,
    SaveClose(u64),
    ForceClose(u64),
    ExitSave(u64),
    ExitDiscard(u64),
    ExitCancel,
    Reload(u64),
    ResetSettings,
    RemovePlugin(usize),
}

pub struct Modal {
    pub title: String,
    pub text: String,
    pub buttons: Vec<(String, ModalAction)>,
}

#[derive(Default)]
pub struct FindState {
    pub open: bool,
    pub replace: bool,
    pub opts: Opts,
    pub in_sel: bool,
    pub status: String,
    pub focus: bool,
}

#[derive(Default)]
pub struct StatusCache {
    pub key: (u64, u64, usize, usize),
    pub occ: String,
    pub eval: Option<String>,
    pub bytes_version: (u64, u64),
    pub bytes: usize,
    pub bytes_time: f64,
}

pub struct App {
    pub s: AppSettings,
    pub pal: Palette,
    pub tabs: Vec<Tab>,
    pub active: usize,
    next_id: u64,
    pub closed: Vec<SessionTab>,
    pub icons: Icons,
    pub primary: bool,
    pub hwnd: Option<isize>,
    /// frames drawn while the window is still cloaked (start-up)
    pub reveal_frames: u32,
    pub revealed: bool,
    pub title: String,
    pub theme_dirty: bool,
    pub fonts_dirty: bool,
    pub info: Option<(String, f64)>,
    pub find: FindState,
    pub goto: Option<String>,
    pub ff: crate::dialogs::FindFiles,
    pub customize: Option<crate::dialogs::Customize>,
    pub font_dlg: Option<crate::dialogs::FontPick>,
    pub about: bool,
    pub shortcuts: bool,
    pub modal: Option<Modal>,
    pub status_popup: Option<&'static str>,
    pub console: Console,
    pub preview: Preview,
    pub complete: Completion,
    ipc: Receiver<Vec<String>>,
    pub allow_close: bool,
    pub exiting: bool,
    pub session_dirty: bool,
    last_session_save: f64,
    pub status: StatusCache,
    pub startup: Option<f64>,
    pub t0: Instant,
    was_focused: bool,
    pub bg: Option<(String, egui::TextureHandle)>,
    pub outer: Option<egui::Rect>,
    pub editor_focused: bool,
    pub tab_drag: Option<u64>,
    pub ctx: egui::Context,
    pub plugins: crate::plugins::Plugins,
}

fn now_secs(ctx: &egui::Context) -> f64 {
    ctx.input(|i| i.time)
}

impl App {
    pub fn new(cc: &eframe::CreationContext, settings: AppSettings, files: Vec<String>, primary: bool, t0: Instant) -> Self {
        let ctx = cc.egui_ctx.clone();
        let (tx, rx) = channel();
        if primary {
            crate::ipc::serve(tx, ctx.clone());
        }
        let editor_font = if settings.use_second_font { settings.second_font_family.clone() } else { settings.font_family.clone() };
        crate::fonts::install(&ctx, &settings.theme.ui_font, &editor_font);
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let pal = Palette::resolve(&settings.theme);
        theme::apply(&ctx, &pal, &settings.theme);
        let mut console = Console::default();
        console.height = settings.console_height;
        let mut app = Self {
            s: settings,
            pal,
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            closed: Vec::new(),
            icons: Icons::default(),
            primary,
            hwnd: None,
            reveal_frames: 0,
            revealed: false,
            title: String::new(),
            theme_dirty: false,
            fonts_dirty: false,
            info: None,
            find: FindState::default(),
            goto: None,
            ff: Default::default(),
            customize: None,
            font_dlg: None,
            about: false,
            shortcuts: false,
            modal: None,
            status_popup: None,
            console,
            preview: Preview::default(),
            complete: Completion::default(),
            ipc: rx,
            allow_close: false,
            exiting: false,
            session_dirty: false,
            last_session_save: 0.0,
            status: StatusCache::default(),
            startup: None,
            t0,
            was_focused: true,
            bg: None,
            outer: None,
            editor_focused: true,
            tab_drag: None,
            ctx: ctx.clone(),
            plugins: Default::default(),
        };
        app.find.opts = Opts {
            find: app.s.last_find.clone(),
            replace: app.s.last_replace.clone(),
            match_case: app.s.find_match_case,
            whole_word: app.s.find_whole_word,
            regex: app.s.find_regex,
            wrap: app.s.find_wrap,
        };
        if primary && app.s.restore_session {
            app.restore_session();
        }
        for f in files {
            app.open_path(Path::new(&f));
        }
        if app.tabs.is_empty() {
            app.new_tab();
        }
        app
    }

    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }
    pub fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    pub fn show_info(&mut self, text: impl Into<String>) {
        self.info = Some((text.into(), now_secs(&self.ctx)));
    }

    pub fn eol_str(&self) -> &'static str {
        self.tab().eol.as_str()
    }

    pub fn indent_unit(&self) -> String {
        if self.s.tabs_as_spaces { " ".repeat(self.s.tab_width.clamp(1, 16)) } else { "\t".into() }
    }

    // ===================================================================== tabs
    pub fn new_tab_with(&mut self, untitled: u32) -> usize {
        let n = if untitled > 0 {
            untitled
        } else {
            let mut n = 1;
            while self.tabs.iter().any(|t| t.path.is_none() && t.untitled == n) {
                n += 1;
            }
            n
        };
        let tab = Tab {
            id: self.next_id,
            doc: Doc::default(),
            view: ViewState::default(),
            hl: Highlighter::default(),
            path: None,
            untitled: n,
            enc: FileEncoding::Utf8,
            eol: Eol::CrLf,
            mtime: None,
            pending: None,
            want_syntax_for: None,
        };
        self.next_id += 1;
        let at = if self.tabs.is_empty() { 0 } else { (self.active + 1).min(self.tabs.len()) };
        self.tabs.insert(at, tab);
        self.session_dirty = true;
        at
    }

    pub fn new_tab(&mut self) {
        let i = self.new_tab_with(0);
        self.activate(i);
    }

    pub fn activate(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        self.active = i;
        self.complete.close();
        if self.tabs[i].pending.is_some() {
            self.load_pending(i);
        }
        self.focus_editor();
        self.check_external(i);
        self.plugin_event(notepad9_plugin::EVENT_TAB_ACTIVATED, i, "", 0);
    }

    pub fn focus_editor(&mut self) {
        let id = self.editor_id();
        self.ctx.memory_mut(|m| m.request_focus(id));
    }

    pub fn editor_id(&self) -> egui::Id {
        egui::Id::new(("editor", self.tab().id))
    }

    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    /// Close a tab; asks first when it has unsaved changes.
    pub fn close_tab(&mut self, i: usize, ask: bool) {
        if i >= self.tabs.len() {
            return;
        }
        if ask && self.tabs[i].modified() {
            self.activate(i);
            let t = &self.tabs[i];
            self.modal = Some(Modal {
                title: "Notepad9".into(),
                text: format!("Save changes to \"{}\"?", t.title()),
                buttons: vec![
                    ("Save".into(), ModalAction::SaveClose(t.id)),
                    ("Don't Save".into(), ModalAction::ForceClose(t.id)),
                    ("Cancel".into(), ModalAction::Nothing),
                ],
            });
            return;
        }
        let t = self.tabs.remove(i);
        if let Some(p) = &t.path {
            self.closed.push(t.pending.clone().unwrap_or(SessionTab {
                path: Some(p.display().to_string()),
                caret: t.doc.sel.head,
                top_line: Some(t.view.row_line(t.view.top)),
                ..Default::default()
            }));
        }
        self.session_dirty = true;
        if self.tabs.is_empty() {
            self.new_tab();
        } else {
            let n = if self.active > i || self.active >= self.tabs.len() { self.active.saturating_sub(1) } else { self.active };
            self.activate(n.min(self.tabs.len() - 1));
        }
    }

    pub fn close_tabs(&mut self, ids: Vec<u64>) {
        for id in ids {
            if self.modal.is_some() {
                break;
            }
            if let Some(i) = self.index_of(id) {
                self.close_tab(i, true);
            }
        }
    }

    pub fn reopen_closed(&mut self) {
        while let Some(st) = self.closed.pop() {
            if let Some(p) = st.path.clone() {
                if Path::new(&p).exists() && self.open_path(Path::new(&p)) {
                    let t = self.tab_mut();
                    t.doc.sel = Sel::caret(st.caret.min(t.doc.len_chars()));
                    if let Some(l) = st.top_line {
                        t.view.top = l;
                    }
                    return;
                }
            }
        }
        self.show_info("No closed tabs to reopen");
    }

    pub fn switch_tab(&mut self, d: i32) {
        let n = self.tabs.len() as i32;
        if n < 2 {
            return;
        }
        let i = ((self.active as i32 + d) % n + n) % n;
        self.activate(i as usize);
    }

    // ===================================================================== files
    /// Open a file in a tab (switches to it if it is already open).
    pub fn open_path(&mut self, path: &Path) -> bool {
        let path = std::fs::canonicalize(path).map(strip_unc).unwrap_or_else(|_| path.to_path_buf());
        if let Some(i) = self.tabs.iter().position(|t| t.path.as_deref().map_or(false, |p| same_path(p, &path))) {
            self.activate(i);
            return true;
        }
        let reuse = !self.tabs.is_empty() && self.tab().pristine();
        let i = if reuse { self.active } else { self.new_tab_with(0) };
        match self.load_into(i, &path, None) {
            Ok(()) => {
                self.activate(i);
                true
            }
            Err(e) => {
                if !reuse {
                    self.tabs.remove(i);
                    if self.active >= self.tabs.len() {
                        self.active = self.tabs.len().saturating_sub(1);
                    }
                }
                self.message(format!("Could not open \"{}\".\n\n{e}", path.display()));
                false
            }
        }
    }

    pub fn message(&mut self, text: String) {
        self.modal = Some(Modal { title: "Notepad9".into(), text, buttons: vec![("OK".into(), ModalAction::Nothing)] });
    }

    pub fn load_into(&mut self, i: usize, path: &Path, forced: Option<FileEncoding>) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let (text, enc, eol) = match forced {
            Some(f) => {
                let (t, e) = textfile::decode_as(&bytes, f);
                let eol = textfile::detect_eol(&t);
                (t, e, eol)
            }
            None => textfile::decode(&bytes),
        };
        let auto = self.s.auto_detect_scheme;
        let t = &mut self.tabs[i];
        t.doc.set_text(&text);
        t.doc.mark_saved();
        t.view = ViewState::default();
        t.path = Some(path.to_path_buf());
        t.enc = enc;
        t.eol = eol;
        t.mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        t.pending = None;
        t.hl.set_syntax(None);
        t.want_syntax_for = if enc != FileEncoding::Binary && auto { Some(path.display().to_string()) } else { None };
        let p = path.display().to_string();
        self.s.add_recent(&p);
        self.session_dirty = true;
        self.plugin_event(notepad9_plugin::EVENT_FILE_OPENED, i, &p, 0);
        if enc == FileEncoding::Binary {
            self.show_info("Binary file: opened byte for byte, saving keeps every byte");
        }
        Ok(())
    }

    pub fn open_dialog(&mut self) {
        let mut d = rfd::FileDialog::new().set_title("Open")
            .add_filter("All Files", &["*"])
            .add_filter("Text Files", &["txt", "log", "ini", "md"])
            .add_filter("Source Code", &["cs", "cpp", "h", "c", "rs", "java", "js", "ts", "py", "ps1", "go"])
            .add_filter("Web / XML", &["html", "htm", "css", "xml", "xaml", "json"]);
        if let Some(dir) = self.tab().dir() {
            d = d.set_directory(dir);
        }
        if let Some(files) = d.pick_files() {
            for f in files {
                self.open_path(&f);
            }
        }
    }

    pub fn reload(&mut self, enc: Option<FileEncoding>) {
        let i = self.active;
        let Some(p) = self.tabs[i].path.clone() else { return };
        let caret = self.tabs[i].doc.sel.head;
        let top = self.tabs[i].view.top;
        match self.load_into(i, &p, enc) {
            Ok(()) => {
                let t = &mut self.tabs[i];
                t.doc.sel = Sel::caret(caret.min(t.doc.len_chars()));
                t.view.top = top;
            }
            Err(e) => self.message(e),
        }
    }

    /// Save the active tab. Returns false if cancelled or failed.
    pub fn save(&mut self) -> bool {
        match self.tab().path.clone() {
            Some(p) => self.write_to(&p, true),
            None => self.save_as(false),
        }
    }

    pub fn save_as(&mut self, copy: bool) -> bool {
        let t = self.tab();
        let name = t.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or("Untitled.txt".into());
        let mut d = rfd::FileDialog::new().set_title(if copy { "Save Copy" } else { "Save As" }).set_file_name(name)
            .add_filter("All Files", &["*"]).add_filter("Text Files", &["txt"]);
        if let Some(dir) = t.dir() {
            d = d.set_directory(dir);
        }
        let Some(p) = d.save_file() else { return false };
        if copy {
            return self.write_to(&p, false);
        }
        if !self.write_to(&p, true) {
            return false;
        }
        let ps = p.display().to_string();
        let auto = self.s.auto_detect_scheme;
        let t = self.tab_mut();
        t.path = Some(p);
        t.mtime = t.path.as_ref().and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        if auto {
            t.want_syntax_for = Some(ps.clone());
        }
        self.s.add_recent(&ps);
        self.session_dirty = true;
        true
    }

    fn write_to(&mut self, p: &Path, mark: bool) -> bool {
        let i = self.active;
        let text = self.tabs[i].doc.text();
        let (enc, eol) = (self.tabs[i].enc, self.tabs[i].eol);
        match textfile::write(p, &text, enc, eol) {
            Ok(()) => {
                if mark {
                    let t = &mut self.tabs[i];
                    t.doc.mark_saved();
                    t.mtime = std::fs::metadata(p).and_then(|m| m.modified()).ok();
                    self.plugin_event(notepad9_plugin::EVENT_FILE_SAVED, i, &p.display().to_string(), 0);
                }
                self.show_info(format!("Saved {}", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
                self.session_dirty = true;
                true
            }
            Err(e) => {
                self.message(format!("Could not save \"{}\".\n\n{e}", p.display()));
                false
            }
        }
    }

    pub fn save_all(&mut self) {
        let cur = self.active;
        let ids: Vec<u64> = self.tabs.iter().filter(|t| t.modified()).map(|t| t.id).collect();
        for id in ids {
            if let Some(i) = self.index_of(id) {
                self.activate(i);
                if !self.save() {
                    return;
                }
            }
        }
        self.activate(cur.min(self.tabs.len() - 1));
    }

    pub fn check_external(&mut self, i: usize) {
        if !self.s.detect_file_changes || self.modal.is_some() {
            return;
        }
        let t = &mut self.tabs[i];
        let Some(p) = &t.path else { return };
        let Ok(m) = std::fs::metadata(p).and_then(|m| m.modified()) else { return };
        if Some(m) == t.mtime || t.mtime.is_none() {
            return;
        }
        t.mtime = Some(m);
        let text = if t.modified() {
            format!("\"{}\" was changed by another program.\nReload it and lose your changes?", t.title())
        } else {
            format!("\"{}\" was changed by another program.\nReload it?", t.title())
        };
        let id = t.id;
        self.modal = Some(Modal { title: "Notepad9".into(), text, buttons: vec![("Reload".into(), ModalAction::Reload(id)), ("Keep".into(), ModalAction::Nothing)] });
    }

    pub fn modal_action(&mut self, a: ModalAction) {
        match a {
            ModalAction::Nothing => {}
            ModalAction::SaveClose(id) => {
                if let Some(i) = self.index_of(id) {
                    self.activate(i);
                    if self.save() {
                        self.close_tab(i, false);
                    }
                }
            }
            ModalAction::ForceClose(id) => {
                if let Some(i) = self.index_of(id) {
                    self.close_tab(i, false);
                }
            }
            ModalAction::ExitSave(id) => {
                if let Some(i) = self.index_of(id) {
                    self.activate(i);
                    if self.save() {
                        self.continue_exit();
                    } else {
                        self.exiting = false;
                    }
                }
            }
            ModalAction::ExitDiscard(id) => {
                if let Some(i) = self.index_of(id) {
                    let t = &mut self.tabs[i];
                    t.doc.mark_saved();
                    t.pending = None;
                }
                self.continue_exit();
            }
            ModalAction::ExitCancel => self.exiting = false,
            ModalAction::Reload(id) => {
                if let Some(i) = self.index_of(id) {
                    self.active = i;
                    self.tabs[i].doc.mark_saved();
                    self.reload(None);
                }
            }
            ModalAction::RemovePlugin(pi) => self.plugin_remove_confirmed(pi),
            ModalAction::ResetSettings => {
                AppSettings::reset();
                self.s.save_on_exit = false;
                self.show_info("Settings will be reset on the next start");
            }
        }
    }

    // ===================================================================== exit / session
    /// Called when the window wants to close. Returns true when it may close now.
    pub fn request_exit(&mut self) -> bool {
        let hot = self.primary && self.s.restore_session && self.s.hot_exit;
        if hot {
            self.finish_exit();
            return true;
        }
        self.exiting = true;
        self.continue_exit();
        self.allow_close
    }

    fn continue_exit(&mut self) {
        if let Some(i) = self.tabs.iter().position(|t| t.modified()) {
            self.activate(i);
            let t = &self.tabs[i];
            self.modal = Some(Modal {
                title: "Notepad9".into(),
                text: format!("Save changes to \"{}\"?", t.title()),
                buttons: vec![
                    ("Save".into(), ModalAction::ExitSave(t.id)),
                    ("Don't Save".into(), ModalAction::ExitDiscard(t.id)),
                    ("Cancel".into(), ModalAction::ExitCancel),
                ],
            });
            return;
        }
        self.finish_exit();
        self.allow_close = true;
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn finish_exit(&mut self) {
        let hot = self.s.restore_session && self.s.hot_exit;
        if self.primary {
            if self.s.restore_session { self.save_session(hot) } else { session::clear() }
        }
        self.plugins_shutdown();
        self.console.kill();
        self.s.console_height = self.console.height;
        if let Some(r) = self.outer {
            self.s.left = Some(r.left());
            self.s.top = Some(r.top());
            self.s.width = r.width();
            self.s.height = r.height();
        }
        self.s.last_find = self.find.opts.find.clone();
        self.s.last_replace = self.find.opts.replace.clone();
        self.s.find_match_case = self.find.opts.match_case;
        self.s.find_whole_word = self.find.opts.whole_word;
        self.s.find_regex = self.find.opts.regex;
        self.s.find_wrap = self.find.opts.wrap;
        if self.s.save_on_exit {
            self.s.save();
        }
        self.allow_close = true;
    }

    pub fn save_session(&mut self, backups: bool) {
        let mut data = SessionData::default();
        let mut texts = Vec::new();
        for (i, t) in self.tabs.iter_mut().enumerate() {
            if let Some(p) = &t.pending {
                let mut st = p.clone();
                let saved = if backups { p.backup.as_deref().and_then(session::read_backup) } else { None };
                st.modified = st.modified && saved.is_some();
                if st.path.is_none() && !st.modified {
                    continue;
                }
                if i == self.active {
                    data.active = data.tabs.len();
                }
                if let Some(s) = saved {
                    texts.push((data.tabs.len(), s));
                }
                data.tabs.push(st);
                continue;
            }
            let modified = t.doc.is_modified();
            if t.path.is_none() && (!backups || t.doc.len_chars() == 0) {
                continue;
            }
            if i == self.active {
                data.active = data.tabs.len();
            }
            if backups && modified {
                texts.push((data.tabs.len(), t.doc.text().to_string()));
            }
            data.tabs.push(SessionTab {
                path: t.path.as_ref().map(|p| p.display().to_string()),
                untitled_number: t.untitled,
                backup: None,
                modified: backups && modified,
                caret: t.doc.sel.head,
                top_line: Some(t.view.row_line(t.view.top)),
                encoding: t.enc,
                eol: t.eol,
                scheme: t.hl.syntax().map(|s| s.to_string()),
            });
        }
        session::save(data, texts);
        self.session_dirty = false;
    }

    /// Recreate the tabs of the last session; only the active one is read now (fast start).
    fn restore_session(&mut self) {
        let data = session::load();
        let mut ids = Vec::new();
        for mut st in data.tabs {
            let has_backup = st.backup.as_deref().map_or(false, session::backup_exists);
            let file_ok = st.path.as_deref().map_or(false, |p| Path::new(p).exists());
            if !has_backup && !file_ok {
                continue;
            }
            if !has_backup {
                st.backup = None;
                st.modified = false;
            }
            let i = self.new_tab_with(if st.path.is_none() { st.untitled_number } else { 0 });
            self.active = i;
            let t = &mut self.tabs[i];
            t.path = st.path.as_ref().map(PathBuf::from);
            ids.push(t.id);
            t.pending = Some(st);
        }
        if !ids.is_empty() {
            let i = self.index_of(ids[data.active.min(ids.len() - 1)]).unwrap_or(0);
            self.activate(i);
        }
        self.session_dirty = false;
    }

    fn load_pending(&mut self, i: usize) {
        let Some(st) = self.tabs[i].pending.take() else { return };
        let text = st.backup.as_deref().and_then(session::read_backup);
        if let Some(text) = text {
            let t = &mut self.tabs[i];
            t.doc.set_text(&text);
            t.path = st.path.as_ref().map(PathBuf::from);
            t.enc = st.encoding;
            t.eol = st.eol;
            t.mtime = t.path.as_ref().and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
            t.hl.set_syntax(None);
            match &st.scheme {
                Some(s) => t.hl.set_syntax(Some(s.clone())),
                None => t.want_syntax_for = st.path.clone(),
            }
            t.doc.force_modified = st.modified;
        } else if let Some(p) = st.path.clone() {
            if let Err(e) = self.load_into(i, Path::new(&p), None) {
                self.message(format!("Could not open \"{p}\".\n\n{e}"));
                return;
            }
            if let Some(s) = &st.scheme {
                self.tabs[i].want_syntax_for = None;
                self.tabs[i].hl.set_syntax(Some(s.clone()));
            }
        }
        let t = &mut self.tabs[i];
        t.doc.sel = Sel::caret(st.caret.min(t.doc.len_chars()));
        if let Some(l) = st.top_line {
            t.view.top = l.min(t.doc.len_lines().saturating_sub(1));
        } else {
            t.view.reveal = true;
        }
    }

    // ===================================================================== commands
    pub fn run(&mut self, id: &str) {
        let ctx = self.ctx.clone();
        match id {
            "new" => self.new_tab(),
            "window_new" => {
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).arg("/new").spawn();
                }
            }
            "open" => self.open_dialog(),
            "reload" => {
                if self.tab().modified() {
                    let id = self.tab().id;
                    self.modal = Some(Modal { title: "Notepad9".into(), text: format!("Discard changes and reload \"{}\"?", self.tab().title()),
                        buttons: vec![("Reload".into(), ModalAction::Reload(id)), ("Cancel".into(), ModalAction::Nothing)] });
                } else {
                    self.reload(None);
                }
            }
            "save" => {
                self.save();
            }
            "save_as" => {
                self.save_as(false);
            }
            "save_copy" => {
                self.save_as(true);
            }
            "save_all" => self.save_all(),
            "close_tab" => self.close_tab(self.active, true),
            "close_all" => {
                let ids = self.tabs.iter().map(|t| t.id).collect();
                self.close_tabs(ids);
            }
            "reopen_tab" => self.reopen_closed(),
            "next_tab" => self.switch_tab(1),
            "prev_tab" => self.switch_tab(-1),
            "exit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            "open_folder" => {
                if let Some(p) = &self.tab().path {
                    let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", p.display())).spawn();
                }
            }
            "terminal_ext" => {
                use std::os::windows::process::CommandExt;
                let dir = self.tab().dir().unwrap_or_else(|| PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()));
                let _ = std::process::Command::new("cmd.exe").current_dir(dir).creation_flags(0x10).spawn();
            }
            "print" => {
                let t = self.tab();
                let tmp = std::env::temp_dir().join(format!("Notepad9 - {}.txt", t.title()));
                let text = self.tabs[self.active].doc.text();
                if std::fs::write(&tmp, textfile::encode(&text, FileEncoding::Utf8Bom, Eol::CrLf)).is_ok() {
                    crate::win::shell("print", &tmp.display().to_string(), None);
                }
            }
            "console" => {
                self.console.visible = !self.console.visible;
                if self.console.visible {
                    let dir = self.tab().dir();
                    self.console.ensure_shell(dir.as_deref(), &ctx);
                    self.console.focus_input = true;
                } else {
                    self.focus_editor();
                }
            }
            "run" => self.run_file(),
            // ---- edit
            "undo" => self.tab_mut().doc.undo(),
            "redo" => self.tab_mut().doc.redo(),
            "cut" | "copy" => {
                let eol = self.eol_str();
                let d = &mut self.tabs[self.active].doc;
                let text = if d.sel.is_empty() {
                    let l = d.line_of(d.sel.head);
                    format!("{}{eol}", d.line_string(l))
                } else {
                    d.selected_text()
                };
                ctx.copy_text(text);
                if id == "cut" {
                    if d.sel.is_empty() {
                        let l = d.line_of(d.sel.head);
                        let (a, b) = (d.line_start(l), d.line_next(l));
                        d.replace(a, b, "");
                    } else {
                        d.backspace(false);
                    }
                }
            }
            "paste" => self.paste_from_clipboard(),
            "delete" => self.tab_mut().doc.delete(false),
            "select_all" => self.tab_mut().doc.select_all(),
            "dup_line" => crate::lineops::duplicate(&mut self.tabs[self.active].doc),
            "del_line" => crate::lineops::delete_lines(&mut self.tabs[self.active].doc),
            "line_up" => crate::lineops::move_lines(&mut self.tabs[self.active].doc, true),
            "line_down" => crate::lineops::move_lines(&mut self.tabs[self.active].doc, false),
            "sort_asc" | "sort_desc" | "uniq" | "no_empty" | "join" | "tabs2spaces" | "spaces2tabs" | "trim" => {
                let tw = self.s.tab_width;
                let nl = self.eol_str();
                crate::lineops::transform_lines(&mut self.tabs[self.active].doc, id, tw, nl);
            }
            "upper" | "lower" | "title" | "invert" => crate::lineops::transform_case(&mut self.tabs[self.active].doc, id),
            "datetime" => {
                let s = crate::lineops::now_string();
                self.tab_mut().doc.type_text(&s, false);
            }
            "complete" => {
                let syn = self.tab().hl.syntax().map(|s| s.to_string());
                let i = self.active;
                self.complete.update(&mut self.tabs[i].doc, syn.as_deref(), false);
            }
            // ---- markdown
            "md_bold" => crate::mdtools::wrap(&mut self.tabs[self.active].doc, "**", "bold"),
            "md_italic" => crate::mdtools::wrap(&mut self.tabs[self.active].doc, "*", "italic"),
            "md_strike" => crate::mdtools::wrap(&mut self.tabs[self.active].doc, "~~", "text"),
            "md_code" => crate::mdtools::wrap(&mut self.tabs[self.active].doc, "`", "code"),
            "md_heading" => crate::mdtools::heading(&mut self.tabs[self.active].doc),
            "md_codeblock" => {
                let nl = self.eol_str();
                crate::mdtools::code_block(&mut self.tabs[self.active].doc, nl);
            }
            "md_link" => crate::mdtools::link(&mut self.tabs[self.active].doc),
            "md_image" => self.insert_image(),
            "md_ul" => crate::mdtools::toggle_list(&mut self.tabs[self.active].doc, crate::mdtools::ListKind::Bullet),
            "md_ol" => crate::mdtools::toggle_list(&mut self.tabs[self.active].doc, crate::mdtools::ListKind::Numbered),
            "md_task" => crate::mdtools::toggle_list(&mut self.tabs[self.active].doc, crate::mdtools::ListKind::Task),
            "md_quote" => crate::mdtools::toggle_list(&mut self.tabs[self.active].doc, crate::mdtools::ListKind::Quote),
            "md_table" => {
                let nl = self.eol_str();
                crate::mdtools::table(&mut self.tabs[self.active].doc, nl);
            }
            "md_hr" => {
                let nl = self.eol_str();
                crate::mdtools::rule(&mut self.tabs[self.active].doc, nl);
            }
            "md_preview" => self.s.md_preview = !self.s.md_preview,
            "md_sync" => self.s.md_sync_scroll = !self.s.md_sync_scroll,
            // ---- search
            "find" | "replace" => {
                self.find.open = true;
                self.find.replace = id == "replace";
                self.find.focus = true;
                self.find.status.clear();
                let sel = self.tab().doc.selected_text();
                if !sel.is_empty() && sel.chars().count() < 500 && !sel.contains('\n') {
                    self.find.opts.find = sel;
                }
            }
            "find_next" | "find_prev" => {
                if self.find.opts.find.is_empty() {
                    let sel = self.tab().doc.selected_text();
                    if !sel.is_empty() && !sel.contains('\n') {
                        self.find.opts.find = sel;
                    }
                }
                if self.find.opts.find.is_empty() {
                    self.run("find");
                } else {
                    self.find_next(id == "find_prev");
                }
            }
            "find_files" => {
                self.ff.open = true;
                if self.ff.folder.is_empty() {
                    self.ff.folder = self.tab().dir().map(|d| d.display().to_string()).unwrap_or_else(|| self.s.last_find_folder.clone());
                }
                if self.ff.mask.is_empty() {
                    self.ff.mask = self.s.last_find_mask.clone();
                }
                let sel = self.tab().doc.selected_text();
                if !sel.is_empty() && !sel.contains('\n') {
                    self.ff.find = sel;
                }
            }
            "goto" => {
                let t = self.tab();
                self.goto = Some(format!("{}", t.doc.line_of(t.doc.sel.head) + 1));
            }
            "focus" => self.toggle_focus_view(),
            // ---- view
            "toggle_toolbar" => self.s.show_toolbar = !self.s.show_toolbar,
            "toggle_status" => self.s.show_status_bar = !self.s.show_status_bar,
            "wrap" => self.s.word_wrap = !self.s.word_wrap,
            "toggle_lnum" => self.s.line_numbers = !self.s.line_numbers,
            "toggle_ws" => self.s.show_whitespace = !self.s.show_whitespace,
            "toggle_eol" => self.s.show_eol = !self.s.show_eol,
            "ruler" => self.s.long_line_marker = !self.s.long_line_marker,
            "toggle_curline" => self.s.highlight_current_line = !self.s.highlight_current_line,
            "toggle_occ" => self.s.mark_occurrences = !self.s.mark_occurrences,
            "zoom_in" => self.zoom(self.s.zoom * 1.1),
            "zoom_out" => self.zoom(self.s.zoom / 1.1),
            "zoom_reset" => self.zoom(1.0),
            "pin" => {
                self.s.always_on_top = !self.s.always_on_top;
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.s.always_on_top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                }));
            }
            // ---- appearance / settings
            "customize" => self.customize = Some(crate::dialogs::Customize::new(&self.s)),
            "font" => self.font_dlg = Some(crate::dialogs::FontPick::new(crate::dialogs::FontTarget::Editor, &self.s.font_family)),
            "font2" => self.font_dlg = Some(crate::dialogs::FontPick::new(crate::dialogs::FontTarget::Second, &self.s.second_font_family)),
            "toggle_2nd" => {
                self.s.use_second_font = !self.s.use_second_font;
                self.fonts_dirty = true;
            }
            "toggle_tabspaces" => self.s.tabs_as_spaces = !self.s.tabs_as_spaces,
            "toggle_autoscheme" => self.s.auto_detect_scheme = !self.s.auto_detect_scheme,
            "toggle_autoindent" => self.s.auto_indent = !self.s.auto_indent,
            "toggle_autocomplete" => self.s.auto_complete = !self.s.auto_complete,
            "toggle_detect" => self.s.detect_file_changes = !self.s.detect_file_changes,
            "toggle_remember" => {
                self.s.remember_recent = !self.s.remember_recent;
                if !self.s.remember_recent {
                    self.s.recent.clear();
                }
            }
            "toggle_saveonexit" => self.s.save_on_exit = !self.s.save_on_exit,
            "toggle_session" => self.s.restore_session = !self.s.restore_session,
            "toggle_hotexit" => self.s.hot_exit = !self.s.hot_exit,
            "save_settings" => {
                self.s.save();
                self.show_info("Settings saved");
            }
            "settings_folder" => {
                let _ = std::fs::create_dir_all(crate::settings::folder());
                let _ = std::process::Command::new("explorer.exe").arg(crate::settings::folder()).spawn();
            }
            "reset_settings" => {
                self.modal = Some(Modal {
                    title: "Notepad9".into(),
                    text: "Reset all settings to defaults? They are used the next time Notepad9 starts.".into(),
                    buttons: vec![("Reset".into(), ModalAction::ResetSettings), ("Cancel".into(), ModalAction::Nothing)],
                });
            }
            "star_add" => {
                if let Some(p) = &self.tab().path {
                    let p = p.display().to_string();
                    if !self.s.favorites.iter().any(|f| f.eq_ignore_ascii_case(&p)) {
                        self.s.favorites.push(p);
                    }
                    self.show_info("Added to favorites");
                }
            }
            "shortcuts" => self.shortcuts = true,
            "about" => self.about = true,
            "ins" => {
                let d = &mut self.tab_mut().doc;
                d.overwrite = !d.overwrite;
            }
            "eval_copy" => {
                if let Some(v) = self.status.eval.clone() {
                    ctx.copy_text(v.clone());
                    self.show_info(format!("Result copied: {v}"));
                }
            }
            "fold" => self.show_info("Code folding is not available in this version"),
            "plugin_manager" => self.plugins.manager_open = true,
            "plugin_install" => self.install_plugin_dialog(),
            "plugins_folder" => {
                let dir = crate::plugins::user_dir();
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::process::Command::new("explorer.exe").arg(dir).spawn();
            }
            _ => {}
        }
        if !matches!(id, "find" | "replace" | "goto" | "find_files" | "console" | "customize" | "font" | "font2" | "about" | "shortcuts"
            | "plugin_manager" | "plugins_folder" | "plugin_install") {
            self.tab_mut().view.reveal = true;
        }
    }

    fn paste_from_clipboard(&mut self) {
        let Ok(text) = arboard::Clipboard::new().and_then(|mut c| c.get_text()) else { return };
        let t = &mut self.tabs[self.active];
        let text = if t.enc == FileEncoding::Binary { text } else { textfile::normalize_eol(&text, t.eol) };
        t.doc.type_text(&text, false);
        self.focus_editor();
    }

    pub fn zoom(&mut self, z: f32) {
        self.s.zoom = if (z - 1.0).abs() < 0.04 { 1.0 } else { z.clamp(0.3, 5.0) };
        self.show_info(format!("Zoom {:.0}%", self.s.zoom * 100.0));
    }

    pub fn find_next(&mut self, back: bool) {
        let o = self.find.opts.clone();
        let i = self.active;
        match crate::search::find_next(&mut self.tabs[i].doc, &o, back) {
            Ok(note) => {
                self.find.status = note.clone().unwrap_or_default();
                if let Some(n) = note {
                    self.show_info(n);
                }
                let t = &mut self.tabs[i];
                let line = t.doc.line_of(t.doc.sel.head);
                if t.view.filter.is_some() {
                    t.view.filter = None;
                }
                t.view.center = Some(line);
            }
            Err(e) => {
                self.find.status = e.clone();
                self.show_info(e);
            }
        }
    }

    pub fn goto_line(&mut self, text: &str) {
        let parts: Vec<&str> = text.split([':', ',', ' ']).filter(|s| !s.is_empty()).collect();
        let Some(Ok(line)) = parts.first().map(|s| s.parse::<usize>()) else { return };
        let col = parts.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
        let t = self.tab_mut();
        let l = line.clamp(1, t.doc.len_lines()) - 1;
        let c = (col.max(1) - 1).min(t.doc.line_len(l));
        t.doc.sel = Sel::caret(t.doc.line_start(l) + c);
        t.view.center = Some(l);
        self.focus_editor();
    }

    pub fn open_at(&mut self, path: &Path, line: usize, col: usize, len: usize) {
        if !self.open_path(path) {
            return;
        }
        let t = self.tab_mut();
        let l = line.clamp(1, t.doc.len_lines().max(1)) - 1;
        let s = t.doc.line_start(l) + (col.max(1) - 1).min(t.doc.line_len(l));
        t.doc.sel = Sel { anchor: s, head: (s + len).min(t.doc.len_chars()) };
        t.view.center = Some(l);
        self.focus_editor();
    }

    fn toggle_focus_view(&mut self) {
        let i = self.active;
        if self.tabs[i].view.filter.is_some() {
            self.tabs[i].view.filter = None;
            self.show_info("Focused view off");
            return;
        }
        let sel = self.tabs[i].doc.selected_text();
        let term = if !sel.is_empty() && !sel.contains('\n') { sel } else { self.find.opts.find.clone() };
        if term.is_empty() {
            self.show_info("Focused view: select some text first");
            return;
        }
        let low = term.to_lowercase();
        let t = &mut self.tabs[i];
        let lines: Vec<usize> = (0..t.doc.len_lines()).filter(|&l| t.doc.line_string(l).to_lowercase().contains(&low)).collect();
        if lines.is_empty() {
            self.show_info(format!("\"{term}\" not found"));
            return;
        }
        let n = lines.len();
        let caret_line = t.doc.line_of(t.doc.sel.head);
        if !lines.contains(&caret_line) {
            let s = t.doc.line_start(lines[0]);
            t.doc.sel = Sel::caret(s);
        }
        t.view.filter = Some(lines);
        t.view.top = 0;
        t.view.top_off = 0.0;
        t.view.reveal = true;
        self.show_info(format!("Focused view: {n} line(s) with \"{term}\"  (Esc to exit)"));
    }

    fn run_file(&mut self) {
        if self.tab().path.is_none() || self.tab().modified() {
            if !self.save() {
                return;
            }
        }
        let Some(p) = self.tab().path.clone() else { return };
        match crate::runner::for_path(&p) {
            crate::runner::RunKind::Preview => self.s.md_preview = true,
            crate::runner::RunKind::Shell => {
                if crate::win::shell("open", &p.display().to_string(), p.parent().map(|d| d.display().to_string()).as_deref()) {
                    self.show_info(format!("Opened {}", self.tab().title()));
                }
            }
            crate::runner::RunKind::Console(cmd) => {
                self.console.visible = true;
                let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
                let ctx = self.ctx.clone();
                self.console.run(&cmd, &dir, &ctx);
            }
        }
    }

    fn insert_image(&mut self) {
        let mut d = rfd::FileDialog::new().set_title("Insert Image")
            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "svg", "webp", "bmp"]).add_filter("All Files", &["*"]);
        let dir = self.tab().dir();
        if let Some(dir) = &dir {
            d = d.set_directory(dir);
        }
        let Some(p) = d.pick_file() else { return };
        let mut s = p.display().to_string();
        if let Some(dir) = dir {
            if let Ok(rel) = p.strip_prefix(&dir) {
                s = rel.display().to_string();
            }
        }
        crate::mdtools::image(&mut self.tabs[self.active].doc, &s.replace('\\', "/"));
    }

    pub fn set_encoding(&mut self, e: FileEncoding) {
        let t = self.tab_mut();
        if t.enc != e {
            t.enc = e;
            t.doc.force_modified = true;
        }
    }

    pub fn set_eol(&mut self, eol: Eol) {
        let i = self.active;
        if self.tabs[i].eol == eol {
            return;
        }
        if self.tabs[i].enc == FileEncoding::Binary {
            self.show_info("Line endings are not changed in binary files");
            return;
        }
        let t = &mut self.tabs[i];
        t.eol = eol;
        let text = t.doc.text();
        let conv = textfile::normalize_eol(&text, eol);
        if *conv != **text {
            let caret_line = t.doc.line_of(t.doc.sel.head);
            let col = t.doc.col_of(t.doc.sel.head);
            let n = t.doc.len_chars();
            t.doc.replace(0, n, &conv);
            let s = t.doc.line_start(caret_line.min(t.doc.len_lines() - 1));
            t.doc.sel = Sel::caret(s + col.min(t.doc.line_len(t.doc.line_of(s))));
        }
        t.doc.force_modified = true;
    }

    pub fn set_syntax(&mut self, name: Option<String>) {
        let t = self.tab_mut();
        t.want_syntax_for = None;
        t.hl.set_syntax(name);
    }

    // ===================================================================== per-frame housekeeping
    pub fn tick(&mut self, ctx: &egui::Context) {
        let now = now_secs(ctx);
        if self.startup.is_none() {
            self.startup = Some(self.t0.elapsed().as_secs_f64() * 1000.0);
        }
        // files sent by "Edit in Notepad9" / a second start
        while let Ok(files) = self.ipc.try_recv() {
            for f in files {
                self.open_path(Path::new(&f));
            }
            if let Some(h) = self.hwnd {
                crate::win::bring_to_front(h);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        self.console.poll();
        // dropped files
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for p in dropped {
            if p.is_file() {
                self.open_path(&p);
            }
        }
        // came back to the window: was the file changed outside?
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if focused && !self.was_focused {
            self.check_external(self.active);
        }
        self.was_focused = focused;
        // syntax resolved once the grammars are loaded
        if highlight::ready().is_some() {
            for t in &mut self.tabs {
                if let Some(p) = t.want_syntax_for.take() {
                    t.hl.set_syntax(highlight::syntax_for_path(&p));
                }
            }
        } else if self.tabs.iter().any(|t| t.want_syntax_for.is_some()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        // window placement for the next start
        ctx.input(|i| {
            let v = i.viewport();
            if v.maximized == Some(false) && v.minimized != Some(true) {
                if let Some(r) = v.outer_rect {
                    self.outer = Some(r);
                }
            }
            if let Some(m) = v.maximized {
                self.s.maximized = m;
            }
        });
        // crash safety: keep the session on disk while something changed
        if self.session_dirty && now - self.last_session_save > 20.0 {
            self.last_session_save = now;
            if self.primary && self.s.restore_session && self.s.hot_exit {
                self.save_session(true);
            }
        }
        if let Some((_, t)) = &self.info {
            if now - t > 4.0 {
                self.info = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(500));
            }
        }
    }

    /// Theme or font changes are applied once per frame.
    pub fn apply_appearance(&mut self, ctx: &egui::Context) {
        if self.fonts_dirty {
            self.fonts_dirty = false;
            let ed = if self.s.use_second_font { self.s.second_font_family.clone() } else { self.s.font_family.clone() };
            crate::fonts::install(ctx, &self.s.theme.ui_font, &ed);
            self.theme_dirty = true;
        }
        if self.theme_dirty {
            self.theme_dirty = false;
            self.pal = Palette::resolve(&self.s.theme);
            theme::apply(ctx, &self.pal, &self.s.theme);
            self.style_title_bar();
            for t in &mut self.tabs {
                t.view.reset_layout();
            }
        }
    }

    pub fn style_title_bar(&self) {
        if let Some(h) = self.hwnd {
            let corners = match self.s.theme.corners.as_str() { "Square" => 1, "Small" => 3, _ => 2 };
            crate::win::style_title_bar(h, self.pal.c("ui.title"), self.pal.c("ui.titleText"), self.pal.c("ui.border"),
                                        !self.pal.is_light(), corners);
        }
    }
}

fn strip_unc(p: PathBuf) -> PathBuf {
    let s = p.display().to_string();
    match s.strip_prefix(r"\\?\") {
        Some(r) if !r.starts_with("UNC") => PathBuf::from(r),
        _ => p,
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.display().to_string().eq_ignore_ascii_case(&b.display().to_string())
}

/// Editor-level shortcuts (exact modifier match). (ctrl, shift, alt, key, command)
pub const KEYS: &[(bool, bool, bool, Key, &str)] = &[
    (true, false, false, Key::N, "new"),
    (true, false, false, Key::T, "new"),
    (true, false, false, Key::W, "close_tab"),
    (true, false, false, Key::F4, "close_tab"),
    (true, true, false, Key::T, "reopen_tab"),
    (true, false, false, Key::Tab, "next_tab"),
    (true, true, false, Key::Tab, "prev_tab"),
    (true, false, false, Key::PageDown, "next_tab"),
    (true, false, false, Key::PageUp, "prev_tab"),
    (true, true, false, Key::N, "window_new"),
    (true, false, false, Key::O, "open"),
    (true, false, false, Key::R, "reload"),
    (true, false, false, Key::S, "save"),
    (true, true, false, Key::S, "save_as"),
    (false, false, false, Key::F12, "save_as"),
    (true, false, true, Key::S, "save_all"),
    (true, false, false, Key::P, "print"),
    (true, false, false, Key::Q, "exit"),
    (true, false, false, Key::Z, "undo"),
    (true, false, false, Key::Y, "redo"),
    (true, true, false, Key::Z, "redo"),
    (true, false, false, Key::A, "select_all"),
    (true, false, false, Key::F, "find"),
    (true, false, false, Key::H, "replace"),
    (false, false, false, Key::F3, "find_next"),
    (false, true, false, Key::F3, "find_prev"),
    (true, true, false, Key::F, "find_files"),
    (true, false, false, Key::G, "goto"),
    (true, false, true, Key::F, "focus"),
    (false, false, true, Key::Z, "wrap"),
    (true, false, false, Key::Plus, "zoom_in"),
    (true, false, false, Key::Equals, "zoom_in"),
    (true, true, false, Key::Equals, "zoom_in"),
    (true, false, false, Key::Minus, "zoom_out"),
    (true, false, false, Key::Num0, "zoom_reset"),
    (false, false, true, Key::T, "pin"),
    (true, false, false, Key::D, "dup_line"),
    (true, true, false, Key::D, "del_line"),
    (false, false, true, Key::ArrowUp, "line_up"),
    (false, false, true, Key::ArrowDown, "line_down"),
    (true, true, false, Key::U, "upper"),
    (true, false, false, Key::U, "lower"),
    (true, false, false, Key::F5, "datetime"),
    (false, false, false, Key::F1, "shortcuts"),
    (false, false, false, Key::F5, "run"),
    (true, false, false, Key::Backtick, "console"),
    (true, false, false, Key::Space, "complete"),
    (true, false, false, Key::B, "md_bold"),
    (true, false, false, Key::I, "md_italic"),
    (true, false, false, Key::K, "md_link"),
    (true, true, false, Key::H, "md_heading"),
    (true, true, false, Key::V, "md_preview"),
    (true, false, true, Key::P, "customize"),
];

/// Commands that act on the text: only while the editor has the keyboard.
pub fn editor_only(id: &str) -> bool {
    matches!(id, "undo" | "redo" | "select_all" | "dup_line" | "del_line" | "line_up" | "line_down" | "upper" | "lower"
        | "datetime" | "complete" | "md_bold" | "md_italic" | "md_link" | "md_heading" | "wrap" | "focus")
}
