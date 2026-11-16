//! Windows inside the main window: Find/Replace, Go To, Find in Files, fonts, Customize, About ...

use crate::app::{App, ModalAction};
use crate::theme::{self, ThemeSettings, DEFS, GROUP_EDITOR, GROUP_SYNTAX, GROUP_UI, PRESETS};
use eframe::egui::{self, Align2, Color32, Key, RichText, Vec2};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// ===================================================================== find in files
#[derive(Clone)]
pub struct Hit {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize,
    pub display: String,
    pub text: String,
}

#[derive(Default)]
pub struct FindFiles {
    pub open: bool,
    pub find: String,
    pub folder: String,
    pub mask: String,
    pub match_case: bool,
    pub regex: bool,
    pub subfolders: bool,
    hits: Arc<Mutex<Vec<Hit>>>,
    status: Arc<Mutex<String>>,
    running: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    selected: Option<usize>,
    init: bool,
}

fn wildcard(mask: &str, name: &str) -> bool {
    fn m(p: &[char], s: &[char]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some('*'), _) => m(&p[1..], s) || (!s.is_empty() && m(p, &s[1..])),
            (Some('?'), Some(_)) => m(&p[1..], &s[1..]),
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b) && m(&p[1..], &s[1..]),
            _ => false,
        }
    }
    let p: Vec<char> = mask.chars().collect();
    let s: Vec<char> = name.chars().collect();
    m(&p, &s)
}

impl FindFiles {
    fn start(&mut self, ctx: &egui::Context) {
        let opts = crate::search::Opts { find: self.find.clone(), match_case: self.match_case, regex: self.regex, ..Default::default() };
        let re = match crate::search::build(&opts) {
            Ok(Some(r)) => r,
            Ok(None) => {
                *self.status.lock().unwrap() = "Enter text to find.".into();
                return;
            }
            Err(e) => {
                *self.status.lock().unwrap() = format!("Invalid expression: {e}");
                return;
            }
        };
        let root = PathBuf::from(self.folder.trim());
        if !root.is_dir() {
            *self.status.lock().unwrap() = "Folder not found.".into();
            return;
        }
        let masks: Vec<String> = self.mask.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let masks = if masks.is_empty() { vec!["*.*".to_string()] } else { masks };
        let recursive = self.subfolders;
        self.hits.lock().unwrap().clear();
        self.selected = None;
        self.running.store(true, Ordering::SeqCst);
        self.stop.store(false, Ordering::SeqCst);
        let (hits, status, running, stop, ctx) = (self.hits.clone(), self.status.clone(), self.running.clone(), self.stop.clone(), ctx.clone());
        std::thread::spawn(move || {
            let mut stack = vec![root.clone()];
            let (mut files, mut matched) = (0usize, 0usize);
            while let Some(dir) = stack.pop() {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(rd) = std::fs::read_dir(&dir) else { continue };
                for e in rd.flatten() {
                    let p = e.path();
                    let Ok(ft) = e.file_type() else { continue };
                    if ft.is_dir() {
                        if recursive {
                            stack.push(p);
                        }
                        continue;
                    }
                    let name = e.file_name().to_string_lossy().to_string();
                    if !masks.iter().any(|m| wildcard(m, &name) || (m == "*.*")) {
                        continue;
                    }
                    files += 1;
                    let Ok(meta) = e.metadata() else { continue };
                    if meta.len() > 20_000_000 {
                        continue;
                    }
                    let Ok(bytes) = std::fs::read(&p) else { continue };
                    if crate::textfile::looks_binary(&bytes) {
                        continue;
                    }
                    let (text, _, _) = crate::textfile::decode(&bytes);
                    let mut found = Vec::new();
                    for (li, line) in text.lines().enumerate() {
                        if let Some(m) = re.find(line) {
                            let rel = p.strip_prefix(&root).unwrap_or(&p).display().to_string();
                            let mut t = line.trim().to_string();
                            if t.chars().count() > 220 {
                                t = t.chars().take(220).collect::<String>() + "…";
                            }
                            found.push(Hit { path: p.clone(), line: li + 1, col: line[..m.start()].chars().count() + 1, display: format!("{rel}({}): ", li + 1), text: t });
                            if found.len() >= 500 {
                                break;
                            }
                        }
                    }
                    if !found.is_empty() {
                        matched += 1;
                        hits.lock().unwrap().extend(found);
                    }
                    if files % 50 == 0 {
                        *status.lock().unwrap() = format!("Searching... {files} files, {} matches", hits.lock().unwrap().len());
                        ctx.request_repaint();
                    }
                }
            }
            let n = hits.lock().unwrap().len();
            *status.lock().unwrap() = if stop.load(Ordering::SeqCst) {
                format!("Stopped. {n} matches.")
            } else {
                format!("{n} matches in {matched} of {files} files.")
            };
            running.store(false, Ordering::SeqCst);
            ctx.request_repaint();
        });
    }
}

// ===================================================================== fonts
#[derive(Clone, Copy, PartialEq)]
pub enum FontTarget {
    Editor,
    Second,
    Interface,
}

pub struct FontPick {
    pub target: FontTarget,
    pub filter: String,
    pub list: Vec<String>,
    pub current: String,
    /// font the dialog was opened with
    pub original: String,
    pub mono_only: bool,
    pub size: f32,
    pub sample: String,
    /// family loaded as the "preview" font
    pub loaded: Option<String>,
    pub scroll_to: bool,
    pub focus: bool,
}


impl FontPick {
    pub fn new(target: FontTarget, current: &str) -> Self {
        let list = crate::fonts::families();
        // the editor wants fixed-width fonts; show only those unless the current one is not
        let mono_only = target != FontTarget::Interface && crate::fonts::looks_monospace(current);
        Self {
            target,
            filter: String::new(),
            list,
            current: current.to_string(),
            original: current.to_string(),
            mono_only,
            size: if target == FontTarget::Interface { 14.0 } else { 16.0 },
            sample: String::new(),
            loaded: None,
            scroll_to: true,
            focus: true,
        }
    }
}

// ===================================================================== customize
pub struct Customize {
    pub page: usize,
    snapshot: ThemeSettings,
    snapshot_session: (bool, bool),
}

impl Customize {
    pub fn new(s: &crate::settings::AppSettings) -> Self {
        Self { page: 0, snapshot: s.theme.clone(), snapshot_session: (s.restore_session, s.hot_exit) }
    }
}

const PAGES: [&str; 8] = [
    "Theme presets", "Interface colors", "Editor colors", "Syntax colors", "Background", "Caret and pointer", "Fonts and sizes", "Tabs and status bar",
];

pub const STATUS_FIELDS: [(&str, &str); 13] = [
    ("ln", "Line"), ("col", "Column"), ("info", "Messages"), ("eval", "Eval (calculator)"), ("sel", "Selected characters"),
    ("sln", "Selected lines"), ("occ", "Occurrences"), ("bytes", "File size"), ("enc", "Encoding"), ("eol", "Line endings"),
    ("ins", "Insert / overwrite"), ("std", "Standard / secondary font"), ("scheme", "Syntax scheme"),
];

const SHORTCUTS: &[(&str, &str)] = &[
    ("Ctrl+N / Ctrl+T", "New tab"), ("Ctrl+W / Ctrl+F4", "Close tab"), ("Ctrl+Shift+T", "Reopen closed tab"),
    ("Ctrl+Tab / Ctrl+Shift+Tab", "Next / previous tab"), ("Ctrl+1 ... Ctrl+9", "Go to tab 1 ... last"),
    ("Ctrl+Shift+N", "New window"), ("Ctrl+O", "Open"), ("Ctrl+S", "Save"), ("F12 / Ctrl+Shift+S", "Save As"),
    ("Ctrl+Alt+S", "Save all"), ("Ctrl+R", "Reload from disk"), ("F5", "Run current file in the console"),
    ("Ctrl+`", "Show / hide console"), ("Ctrl+Space", "Complete word"), ("Ctrl+F / Ctrl+H", "Find / Replace"),
    ("F3 / Shift+F3", "Find next / previous"), ("Ctrl+Shift+F", "Find in files"), ("Ctrl+G", "Go to line"),
    ("Alt+Z", "Word wrap"), ("Ctrl+Alt+F", "Focused view"), ("Ctrl+Plus / Minus / 0", "Zoom in / out / reset"),
    ("Ctrl+D / Ctrl+Shift+D", "Duplicate / delete line"), ("Alt+Up / Alt+Down", "Move line"),
    ("Ctrl+Shift+U / Ctrl+U", "UPPERCASE / lowercase"), ("Ctrl+F5", "Insert date and time"), ("Insert", "Insert / overwrite"),
    ("Alt+T", "Always on top"), ("Markdown: Ctrl+B / I / K", "Bold / italic / link"), ("Markdown: Ctrl+Shift+H", "Heading level"),
    ("Markdown: Ctrl+Shift+V", "Show / hide preview"), ("Ctrl+Alt+P", "Customize interface"), ("Ctrl+Q", "Exit"),
];

fn win<'a>(title: &'a str, open: &'a mut bool) -> egui::Window<'a> {
    egui::Window::new(title).open(open).collapsible(false).resizable(false).pivot(Align2::CENTER_TOP)
        .default_pos(egui::pos2(500.0, 140.0))
}

impl App {
    pub fn dialogs(&mut self, ctx: &egui::Context) {
        self.find_window(ctx);
        self.goto_window(ctx);
        self.find_files_window(ctx);
        self.font_window(ctx);
        self.customize_window(ctx);
        self.about_window(ctx);
        self.modal_window(ctx);
    }

    fn find_window(&mut self, ctx: &egui::Context) {
        if !self.find.open {
            return;
        }
        let mut open = true;
        let title = if self.find.replace { "Replace" } else { "Find" };
        let mut act: Option<&str> = None;
        win(title, &mut open).show(ctx, |ui| {
            egui::Grid::new("find_grid").num_columns(2).spacing(Vec2::new(10.0, 8.0)).show(ui, |ui| {
                ui.label("Find what:");
                let r = ui.add(egui::TextEdit::singleline(&mut self.find.opts.find).desired_width(300.0).id(egui::Id::new("find_box")));
                if self.find.focus {
                    r.request_focus();
                    self.find.focus = false;
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    act = Some(if ui.input(|i| i.modifiers.shift) { "prev" } else { "next" });
                    r.request_focus();
                }
                ui.end_row();
                if self.find.replace {
                    ui.label("Replace with:");
                    ui.add(egui::TextEdit::singleline(&mut self.find.opts.replace).desired_width(300.0));
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                crate::widgets::checkbox(ui, &mut self.find.opts.match_case, "Match case");
                crate::widgets::checkbox(ui, &mut self.find.opts.whole_word, "Whole word");
                crate::widgets::checkbox(ui, &mut self.find.opts.regex, "Regular expression");
                crate::widgets::checkbox(ui, &mut self.find.opts.wrap, "Wrap around");
            });
            if self.find.replace {
                crate::widgets::checkbox(ui, &mut self.find.in_sel, "Replace All only inside the selection");
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if crate::widgets::button(ui, "Find Next").clicked() {
                    act = Some("next");
                }
                if crate::widgets::button(ui, "Find Previous").clicked() {
                    act = Some("prev");
                }
                if self.find.replace {
                    if crate::widgets::button(ui, "Replace").clicked() {
                        act = Some("replace");
                    }
                    if crate::widgets::button(ui, "Replace All").clicked() {
                        act = Some("all");
                    }
                }
                if crate::widgets::button(ui, "Count").clicked() {
                    act = Some("count");
                }
            });
            if !self.find.status.is_empty() {
                ui.label(RichText::new(&self.find.status).color(Color32::from_rgb(0xE0, 0xB0, 0x50)));
            }
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                act = Some("close");
            }
        });
        let i = self.active;
        match act {
            Some("next") => self.find_next(false),
            Some("prev") => self.find_next(true),
            Some("replace") => {
                let o = self.find.opts.clone();
                self.find.status = match crate::search::replace_current(&mut self.tabs[i].doc, &o) {
                    Ok(n) => n.unwrap_or_default(),
                    Err(e) => e,
                };
                self.tabs[i].view.reveal = true;
            }
            Some("all") => {
                let o = self.find.opts.clone();
                let in_sel = self.find.in_sel;
                self.find.status = match crate::search::replace_all(&mut self.tabs[i].doc, &o, in_sel) {
                    Ok(0) => format!("\"{}\" not found", o.find),
                    Ok(n) => format!("{n} occurrence(s) replaced"),
                    Err(e) => e,
                };
            }
            Some("count") => {
                let o = self.find.opts.clone();
                let text = self.tabs[i].doc.text();
                self.find.status = format!("{} match(es)", crate::search::count(&text, &o));
            }
            Some("close") => open = false,
            _ => {}
        }
        if !open {
            self.find.open = false;
            self.focus_editor();
        }
    }

    fn goto_window(&mut self, ctx: &egui::Context) {
        let Some(mut text) = self.goto.take() else { return };
        let mut open = true;
        let mut cancel = false;
        let mut go = false;
        let lines = self.tab().doc.len_lines();
        win("Go To", &mut open).show(ctx, |ui| {
            ui.label(format!("Line number (1 - {lines}), optionally line:column"));
            let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(260.0).id(egui::Id::new("goto_box")));
            r.request_focus();
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                go = true;
            }
            ui.horizontal(|ui| {
                if crate::widgets::button(ui, "Go").clicked() {
                    go = true;
                }
                if crate::widgets::button(ui, "Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    cancel = true;
                }
            });
        });
        if cancel {
            open = false;
        }
        if go {
            self.goto_line(&text);
        } else if open {
            self.goto = Some(text);
        } else {
            self.focus_editor();
        }
    }

    fn find_files_window(&mut self, ctx: &egui::Context) {
        if !self.ff.open {
            return;
        }
        if !self.ff.init {
            self.ff.init = true;
            self.ff.subfolders = true;
            self.ff.match_case = self.find.opts.match_case;
        }
        let mut open = true;
        let mut open_hit: Option<Hit> = None;
        egui::Window::new("Find in Files").open(&mut open).collapsible(false).resizable(true).default_size(Vec2::new(720.0, 460.0))
            .show(ctx, |ui| {
                egui::Grid::new("ff_grid").num_columns(3).spacing(Vec2::new(8.0, 6.0)).show(ui, |ui| {
                    ui.label("Find what:");
                    let r = ui.add(egui::TextEdit::singleline(&mut self.ff.find).desired_width(420.0));
                    let running = self.ff.running.load(Ordering::SeqCst);
                    if crate::widgets::button(ui, if running { "Stop" } else { "Search" }).clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
                        if running {
                            self.ff.stop.store(true, Ordering::SeqCst);
                        } else {
                            self.s.last_find_folder = self.ff.folder.clone();
                            self.s.last_find_mask = self.ff.mask.clone();
                            self.ff.start(ctx);
                        }
                    }
                    ui.end_row();
                    ui.label("Folder:");
                    ui.add(egui::TextEdit::singleline(&mut self.ff.folder).desired_width(420.0));
                    if crate::widgets::button(ui, "Browse...").clicked() {
                        if let Some(d) = rfd::FileDialog::new().set_directory(&self.ff.folder).pick_folder() {
                            self.ff.folder = d.display().to_string();
                        }
                    }
                    ui.end_row();
                    ui.label("File types:");
                    ui.add(egui::TextEdit::singleline(&mut self.ff.mask).desired_width(420.0)).on_hover_text("Separate masks with ; e.g. *.rs;*.toml");
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    crate::widgets::checkbox(ui, &mut self.ff.match_case, "Match case");
                    crate::widgets::checkbox(ui, &mut self.ff.regex, "Regular expression");
                    crate::widgets::checkbox(ui, &mut self.ff.subfolders, "Include subfolders");
                });
                ui.separator();
                let hits = self.ff.hits.lock().unwrap().clone();
                let link = self.pal.c("syn.attribute");
                egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - 24.0).show_rows(
                    ui, 18.0, hits.len(), |ui, range| {
                        for i in range {
                            let h = &hits[i];
                            let sel = self.ff.selected == Some(i);
                            let mut job = egui::text::LayoutJob::default();
                            let mono = egui::FontId::new(12.5, egui::FontFamily::Name("editor".into()));
                            job.append(&h.display, 0.0, egui::TextFormat { font_id: mono.clone(), color: link, ..Default::default() });
                            job.append(&h.text, 0.0, egui::TextFormat { font_id: mono, color: self.pal.c("ui.text"), ..Default::default() });
                            let r = ui.add(crate::widgets::Selectable::new(sel, job));
                            if r.clicked() {
                                self.ff.selected = Some(i);
                            }
                            if r.double_clicked() {
                                open_hit = Some(h.clone());
                            }
                        }
                    },
                );
                ui.label(RichText::new(self.ff.status.lock().unwrap().clone()).color(self.pal.c("ui.textDim")));
            });
        if let Some(h) = open_hit {
            let len = self.ff.find.chars().count();
            self.open_at(&h.path, h.line, h.col, len);
        }
        if !open {
            self.ff.stop.store(true, Ordering::SeqCst);
            self.ff.open = false;
        }
    }

    fn font_window(&mut self, ctx: &egui::Context) {
        // fixed sizes: the window never grows with its content
        const LIST_W: f32 = 230.0;
        const PREV_W: f32 = 360.0;
        const BODY_H: f32 = 290.0;
        let Some(fp) = self.font_dlg.as_mut() else { return };
        // the selected font is loaded as the "preview" family (usable from the next frame)
        if fp.loaded.as_deref() != Some(fp.current.as_str()) {
            let ed = if self.s.use_second_font { self.s.second_font_family.clone() } else { self.s.font_family.clone() };
            crate::fonts::install_with_preview(ctx, &self.s.theme.ui_font, &ed, Some(&fp.current));
            fp.loaded = Some(fp.current.clone());
            ctx.request_repaint();
        }
        let preview_ready = ctx.fonts(|f| f.families().contains(&egui::FontFamily::Name(crate::fonts::PREVIEW.into())));
        let pal = self.pal.clone();
        let mut open = true;
        let mut done: Option<bool> = None; // Some(true) = OK, Some(false) = Cancel
        let title = match fp.target { FontTarget::Editor => "Editor Font", FontTarget::Second => "Secondary Font", FontTarget::Interface => "Interface Font" };
        let center = ctx.screen_rect().center();
        egui::Window::new(title).id(egui::Id::new("font_dialog")).open(&mut open).collapsible(false).resizable(false)
            .pivot(Align2::CENTER_CENTER).default_pos(center).show(ctx, |ui| {
            let low = fp.filter.to_lowercase();
            let list: Vec<String> = fp.list.iter()
                .filter(|f| (low.is_empty() || f.to_lowercase().contains(&low)) && (!fp.mono_only || crate::fonts::looks_monospace(f)))
                .cloned().collect();

            // ---- search
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut fp.filter).hint_text("Search...").desired_width(LIST_W - 8.0));
                if fp.focus {
                    r.request_focus();
                    fp.focus = false;
                }
                if r.has_focus() && !list.is_empty() {
                    let (down, up) = ui.input(|i| (i.key_pressed(Key::ArrowDown), i.key_pressed(Key::ArrowUp)));
                    if down || up {
                        let pos = list.iter().position(|f| *f == fp.current);
                        let i = match pos {
                            Some(p) if down => (p + 1).min(list.len() - 1),
                            Some(p) => p.saturating_sub(1),
                            None => 0,
                        };
                        fp.current = list[i].clone();
                        fp.scroll_to = true;
                    }
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    done = Some(true);
                }
                crate::widgets::checkbox(ui, &mut fp.mono_only, "Monospaced only");
            });
            ui.add_space(4.0);

            ui.horizontal_top(|ui| {
                // ---- list
                ui.allocate_ui_with_layout(Vec2::new(LIST_W, BODY_H), egui::Layout::top_down(egui::Align::Min), |ui| {
                    egui::Frame::none().fill(pal.c("ui.field")).stroke(egui::Stroke::new(1.0_f32, pal.c("ui.fieldBorder")))
                        .inner_margin(egui::Margin::same(1.0)).show(ui, |ui| {
                        ui.set_width(LIST_W - 4.0);
                        ui.set_height(BODY_H - 4.0);
                        egui::ScrollArea::vertical().id_salt("font_list").max_height(BODY_H - 4.0).auto_shrink([false, false]).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            if list.is_empty() {
                                ui.label(RichText::new(" No fonts found").color(pal.c("ui.textDim")));
                            }
                            for f in &list {
                                let sel = *f == fp.current;
                                let text = if *f == fp.original { RichText::new(format!("{f}  ✔")) } else { RichText::new(f.as_str()) };
                                let r = ui.add_sized([LIST_W - 18.0, 21.0], crate::widgets::Selectable::new(sel, text));
                                if sel && fp.scroll_to {
                                    r.scroll_to_me(Some(egui::Align::Center));
                                }
                                if r.clicked() {
                                    fp.current = f.clone();
                                }
                                if r.double_clicked() {
                                    done = Some(true);
                                }
                            }
                        });
                    });
                });
                fp.scroll_to = false;

                // ---- preview
                ui.allocate_ui_with_layout(Vec2::new(PREV_W, BODY_H), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(PREV_W);
                    let family = if preview_ready && fp.loaded.as_deref() == Some(fp.current.as_str()) {
                        egui::FontFamily::Name(crate::fonts::PREVIEW.into())
                    } else {
                        egui::FontFamily::Proportional
                    };
                    let name = if fp.current.chars().count() > 26 { format!("{}…", fp.current.chars().take(25).collect::<String>()) } else { fp.current.clone() };
                    ui.label(RichText::new(name).font(egui::FontId::new(22.0, family.clone())).color(pal.c("ui.text")));
                    let kind = if crate::fonts::looks_monospace(&fp.current) { "Monospaced" } else { "Proportional" };
                    ui.label(RichText::new(kind).size(11.0).color(pal.c("ui.textDim")));
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label("Size:");
                        ui.add(crate::widgets::Slider::new(&mut fp.size, 8.0..=36.0).step_by(1.0).show_value(true));
                    });
                    ui.add_space(4.0);
                    let font = egui::FontId::new(fp.size, family.clone());
                    egui::Frame::none().fill(pal.c("ed.bg")).stroke(egui::Stroke::new(1.0_f32, pal.c("ui.line")))
                        .inner_margin(egui::Margin::same(8.0)).show(ui, |ui| {
                        ui.set_width(PREV_W - 18.0);
                        ui.set_height(BODY_H - 112.0);
                        egui::ScrollArea::both().id_salt("font_preview").max_height(BODY_H - 112.0).max_width(PREV_W - 18.0)
                            .auto_shrink([false, false]).show(ui, |ui| {
                            for line in ["AaBbCc  The quick brown fox", "Съешь же ещё этих булок", "0123456789  {}[]()<>", "il1I|  O0o  == != -> =>",
                                         "fn main() { let x = 42; }"] {
                                ui.add(egui::Label::new(RichText::new(line).font(font.clone()).color(pal.c("ed.fg"))).extend());
                            }
                            if !fp.sample.trim().is_empty() {
                                ui.add(egui::Label::new(RichText::new(fp.sample.as_str()).font(font.clone()).color(pal.c("syn.keyword"))).extend());
                            }
                        });
                    });
                    ui.add_space(4.0);
                    ui.add(egui::TextEdit::singleline(&mut fp.sample).hint_text("Your own sample text").desired_width(PREV_W - 10.0));
                });
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} fonts", list.len())).color(pal.c("ui.textDim")));
                ui.add_space(LIST_W + PREV_W - 330.0);
                let ok = crate::widgets::Button::new(RichText::new("OK").color(Color32::WHITE)).fill(pal.c("ui.accent")).min_size(Vec2::new(86.0, 26.0));
                if ui.add(ok).clicked() {
                    done = Some(true);
                }
                if ui.add(crate::widgets::Button::new("Cancel").min_size(Vec2::new(86.0, 26.0))).clicked() {
                    done = Some(false);
                }
            });
        });
        if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            done = Some(false);
        }
        let Some(fp) = self.font_dlg.as_mut() else { return };
        let (target, chosen) = (fp.target, fp.current.clone());
        match done {
            Some(true) => {
                match target {
                    FontTarget::Editor => {
                        self.s.font_family = chosen;
                        self.s.use_second_font = false;
                    }
                    FontTarget::Second => {
                        self.s.second_font_family = chosen;
                        self.s.use_second_font = true;
                    }
                    FontTarget::Interface => self.s.theme.ui_font = chosen,
                }
                self.font_dlg = None;
                self.fonts_dirty = true;
            }
            Some(false) => {
                self.font_dlg = None;
                self.fonts_dirty = true; // drop the preview font
            }
            None if !open => {
                self.font_dlg = None;
                self.fonts_dirty = true;
            }
            None => {}
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        if self.about {
            let mut open = true;
            let ms = self.startup.unwrap_or(0.0);
            win("About Notepad9", &mut open).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(img) = self.icons.image(ctx, "app", 64.0, true) {
                        ui.add(img);
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Notepad9").size(26.0).strong());
                        ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(self.pal.c("ui.textDim")));
                    });
                });
                ui.add_space(8.0);
                ui.label("A dark, fast text editor.\nRust · egui · ropey · syntect");
                ui.label("Free software under the MIT License: use, copy, modify\nand distribute it without restriction.");
                ui.add_space(6.0);
                ui.label(RichText::new(format!("Start-up time: {ms:.0} ms")).size(11.0).color(self.pal.c("ui.textDisabled")));
            });
            self.about = open;
        }
        if self.shortcuts {
            let mut open = true;
            win("Keyboard Shortcuts", &mut open).show(ctx, |ui| {
                egui::Grid::new("keys").num_columns(2).spacing(Vec2::new(24.0, 4.0)).show(ui, |ui| {
                    for (k, w) in SHORTCUTS {
                        ui.label(RichText::new(*k).color(self.pal.c("ui.textDim")));
                        ui.label(*w);
                        ui.end_row();
                    }
                });
            });
            self.shortcuts = open;
        }
    }

    fn modal_window(&mut self, ctx: &egui::Context) {
        let Some(m) = self.modal.as_ref() else { return };
        let mut chosen: Option<usize> = None;
        // dim the window behind the question
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("modal_dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter().rect_filled(r, 0.0, Color32::from_black_alpha(90));
        });
        egui::Window::new(&m.title).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, Vec2::ZERO).order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_min_width(340.0);
                ui.add_space(6.0);
                ui.label(&m.text);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    for (i, (label, _)) in m.buttons.iter().enumerate() {
                        let b = crate::widgets::Button::new(label.as_str()).min_size(Vec2::new(86.0, 26.0));
                        let b = if i == 0 { b.fill(self.pal.c("ui.accent")) } else { b };
                        if ui.add(b).clicked() {
                            chosen = Some(i);
                        }
                    }
                });
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    chosen = Some(0);
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    chosen = Some(m.buttons.len() - 1);
                }
            });
        if let Some(i) = chosen {
            let mut m = self.modal.take().unwrap();
            let action = std::mem::replace(&mut m.buttons[i].1, ModalAction::Nothing);
            self.modal_action(action);
        }
    }

    // ------------------------------------------------------------------ customize
    fn customize_window(&mut self, ctx: &egui::Context) {
        let Some(page) = self.customize.as_ref().map(|c| c.page) else { return };
        let mut open = true;
        let mut finish: Option<bool> = None;
        let mut new_page = page;
        let mut changed = false;
        egui::Window::new("Customize Interface").open(&mut open).collapsible(false).resizable(true)
            .default_size(Vec2::new(860.0, 580.0)).show(ctx, |ui| {
                egui::TopBottomPanel::bottom("cust_bottom").show_inside(ui, |ui| {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if crate::widgets::button(ui, "Import...").clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("Notepad9 theme", &["n9theme", "json"]).pick_file() {
                                if let Some(t) = std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str::<ThemeSettings>(&s).ok()) {
                                    self.s.theme = t;
                                    changed = true;
                                }
                            }
                        }
                        if crate::widgets::button(ui, "Export...").clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("Notepad9 theme", &["n9theme"]).set_file_name("My theme.n9theme").save_file() {
                                let _ = std::fs::write(p, serde_json::to_string_pretty(&self.s.theme).unwrap_or_default());
                            }
                        }
                        if crate::widgets::button(ui, "Reset everything").clicked() {
                            self.s.theme = ThemeSettings::default();
                            changed = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if crate::widgets::button(ui, "Cancel").clicked() {
                                finish = Some(false);
                            }
                            if ui.add(crate::widgets::Button::new("OK").fill(self.pal.c("ui.accent"))).clicked() {
                                finish = Some(true);
                            }
                        });
                    });
                });
                egui::SidePanel::left("cust_nav").resizable(false).exact_width(190.0).show_inside(ui, |ui| {
                    ui.add_space(6.0);
                    for (i, name) in PAGES.iter().enumerate() {
                        if crate::widgets::selectable(ui, i == new_page, *name).clicked() {
                            new_page = i;
                        }
                    }
                });
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        ui.add_space(6.0);
                        ui.heading(PAGES[new_page]);
                        ui.add_space(8.0);
                        changed |= self.customize_page(ui, new_page);
                    });
                });
            });
        if let Some(c) = self.customize.as_mut() {
            c.page = new_page;
        }
        if changed {
            self.theme_dirty = true;
            self.fonts_dirty = true;
        }
        let cancel = finish == Some(false) || (!open && finish.is_none());
        if cancel {
            if let Some(c) = self.customize.take() {
                self.s.theme = c.snapshot;
                self.s.restore_session = c.snapshot_session.0;
                self.s.hot_exit = c.snapshot_session.1;
                self.theme_dirty = true;
                self.fonts_dirty = true;
            }
        } else if finish == Some(true) {
            self.customize = None;
            self.s.save();
        }
    }

    fn color_row(&mut self, ui: &mut egui::Ui, id: &str, label: &str) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.add_sized(Vec2::new(230.0, 22.0), egui::Label::new(label).wrap());
            let mut c = self.pal.c(id);
            if egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::OnlyBlend).changed() {
                self.s.theme.colors.insert(id.to_string(), theme::hex(c));
                changed = true;
            }
            let mut hex = theme::hex(c);
            let r = ui.add(egui::TextEdit::singleline(&mut hex).desired_width(90.0).font(egui::TextStyle::Monospace));
            if r.changed() {
                if theme::parse(&hex).is_some() {
                    self.s.theme.colors.insert(id.to_string(), hex);
                    changed = true;
                }
            }
            if self.s.theme.colors.contains_key(id) && crate::widgets::button(ui, "Reset").on_hover_text("Back to the preset color").clicked() {
                self.s.theme.colors.remove(id);
                changed = true;
            }
        });
        if changed {
            self.pal = crate::theme::Palette::resolve(&self.s.theme);
        }
        changed
    }

    fn customize_page(&mut self, ui: &mut egui::Ui, page: usize) -> bool {
        let mut changed = false;
        let dim = self.pal.c("ui.textDim");
        match page {
            0 => {
                ui.label(RichText::new("Pick a starting point. Colors changed on the next pages are kept on top of the preset.").color(dim));
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    for name in PRESETS {
                        let c = |id: &str| theme::preset_color(name, id);
                        let selected = self.s.theme.preset == *name;
                        let (rect, r) = ui.allocate_exact_size(Vec2::new(176.0, 116.0), egui::Sense::click());
                        let p = ui.painter();
                        p.rect_filled(rect, 5.0, self.pal.c("ui.field"));
                        p.rect_stroke(rect, 5.0, egui::Stroke::new(2.0_f32, if selected { self.pal.c("ui.accent") } else { self.pal.c("ui.fieldBorder") }));
                        let inner = rect.shrink(7.0);
                        let prev = egui::Rect::from_min_size(inner.min, Vec2::new(inner.width(), 80.0));
                        p.rect_filled(egui::Rect::from_min_size(prev.min, Vec2::new(prev.width(), 9.0)), 0.0, c("ui.menuBar"));
                        p.rect_filled(egui::Rect::from_min_size(prev.min + Vec2::new(0.0, 9.0), Vec2::new(prev.width(), 9.0)), 0.0, c("ui.tabBar"));
                        let ed = egui::Rect::from_min_max(prev.min + Vec2::new(0.0, 18.0), prev.max - Vec2::new(0.0, 8.0));
                        p.rect_filled(ed, 0.0, c("ed.bg"));
                        p.rect_filled(egui::Rect::from_min_size(ed.min, Vec2::new(14.0, ed.height())), 0.0, c("ed.gutter"));
                        p.rect_filled(egui::Rect::from_min_size(egui::pos2(ed.left() + 14.0, ed.top() + 15.0), Vec2::new(ed.width() - 14.0, 10.0)), 0.0, c("ed.curLine"));
                        let bars = [(c("syn.keyword"), 34.0, c("syn.type"), 40.0), (c("ed.fg"), 18.0, c("syn.string"), 56.0),
                                    (c("syn.method"), 46.0, c("syn.number"), 14.0), (c("syn.comment"), 80.0, c("syn.comment"), 0.0)];
                        for (k, (a, wa, b, wb)) in bars.iter().enumerate() {
                            let y = ed.top() + 7.0 + k as f32 * 10.0;
                            p.rect_filled(egui::Rect::from_min_size(egui::pos2(ed.left() + 20.0, y), Vec2::new(*wa, 4.0)), 2.0, *a);
                            if *wb > 0.0 {
                                p.rect_filled(egui::Rect::from_min_size(egui::pos2(ed.left() + 25.0 + wa, y), Vec2::new(*wb, 4.0)), 2.0, *b);
                            }
                        }
                        p.rect_filled(egui::Rect::from_min_max(egui::pos2(prev.left(), prev.bottom() - 8.0), prev.max), 0.0, c("ui.status"));
                        p.text(egui::pos2(inner.left() + 2.0, inner.bottom() - 2.0), Align2::LEFT_BOTTOM, *name,
                               egui::FontId::proportional(13.0), self.pal.c("ui.text"));
                        if r.clicked() {
                            self.s.theme.preset = name.to_string();
                            self.s.theme.colors.clear();
                            changed = true;
                        }
                    }
                });
                if !self.s.theme.colors.is_empty() {
                    ui.label(RichText::new(format!("{} color(s) changed by hand on top of \"{}\".", self.s.theme.colors.len(), self.s.theme.preset)).color(dim));
                    if crate::widgets::button(ui, "Drop my color changes").clicked() {
                        self.s.theme.colors.clear();
                        changed = true;
                    }
                }
            }
            1..=3 => {
                let group = [GROUP_UI, GROUP_EDITOR, GROUP_SYNTAX][page - 1];
                ui.label(RichText::new("Click a swatch to pick a color (with transparency), or type #RRGGBB / #AARRGGBB.").color(dim));
                ui.add_space(6.0);
                for d in DEFS.iter().filter(|d| d.group == group) {
                    changed |= self.color_row(ui, d.id, d.label);
                }
                ui.add_space(8.0);
                if crate::widgets::button(ui, "Reset this page").clicked() {
                    for d in DEFS.iter().filter(|d| d.group == group) {
                        self.s.theme.colors.remove(d.id);
                    }
                    changed = true;
                }
            }
            4 => {
                let t = &mut self.s.theme;
                ui.label(RichText::new("A picture behind the text or behind the whole window. Colors with transparency let it show through more.").color(dim));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Show image:");
                    changed |= crate::widgets::radio_value(ui, &mut t.bg_target, "Off".into(), "Off").changed();
                    changed |= crate::widgets::radio_value(ui, &mut t.bg_target, "Editor".into(), "Behind the text").changed();
                    changed |= crate::widgets::radio_value(ui, &mut t.bg_target, "Window".into(), "Behind the whole window").changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Image file:");
                    changed |= ui.add(egui::TextEdit::singleline(&mut t.bg_image).desired_width(380.0)).changed();
                    if crate::widgets::button(ui, "Browse...").clicked() {
                        if let Some(p) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg"]).pick_file() {
                            t.bg_image = p.display().to_string();
                            if t.bg_target == "Off" {
                                t.bg_target = "Editor".into();
                            }
                            changed = true;
                        }
                    }
                    if crate::widgets::button(ui, "Clear").clicked() {
                        t.bg_image.clear();
                        changed = true;
                    }
                });
                changed |= ui.add(crate::widgets::Slider::new(&mut t.bg_opacity, 0.0..=1.0).text("Image opacity")).changed();
                ui.horizontal(|ui| {
                    ui.label("Size:");
                    for (v, l) in [("UniformToFill", "Cover"), ("Uniform", "Fit"), ("Fill", "Stretch"), ("None", "Original"), ("Tile", "Tile")] {
                        changed |= crate::widgets::radio_value(ui, &mut t.bg_stretch, v.into(), l).changed();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Position:");
                    for (v, l) in [("Center", "Center"), ("TopLeft", "Top left"), ("TopRight", "Top right"), ("BottomLeft", "Bottom left"), ("BottomRight", "Bottom right")] {
                        changed |= crate::widgets::radio_value(ui, &mut t.bg_align, v.into(), l).changed();
                    }
                });
                changed |= ui.add(crate::widgets::Slider::new(&mut t.panel_opacity, 0.0..=1.0).text("Panel opacity (whole-window image)")).changed();
                ui.add_space(10.0);
                changed |= self.color_row(ui, "ed.bg", "Text background");
                changed |= self.color_row(ui, "ui.window", "Window background");
            }
            5 => {
                {
                    let t = &mut self.s.theme;
                    ui.label(RichText::new("TEXT CURSOR (CARET)").size(11.0).color(dim));
                    ui.horizontal(|ui| {
                        ui.label("Shape:");
                        for (v, l) in [("Line", "Line |"), ("Block", "Block █"), ("Underline", "Underline _"), ("Box", "Frame ▯")] {
                            changed |= crate::widgets::radio_value(ui, &mut t.caret_style, v.into(), l).changed();
                        }
                    });
                    changed |= ui.add(crate::widgets::Slider::new(&mut t.caret_width, 1.0..=8.0).step_by(0.5).text("Thickness")).changed();
                    changed |= crate::widgets::checkbox(ui, &mut t.caret_blink, "Blink").changed();
                    changed |= ui.add(crate::widgets::Slider::new(&mut t.caret_blink_ms, 150..=1500).text("Blink speed (ms)")).changed();
                }
                changed |= self.color_row(ui, "ed.caret", "Caret color");
                ui.label(RichText::new("Overwrite mode (Insert key) always shows a block.").color(dim));
                ui.add_space(10.0);
                let t = &mut self.s.theme;
                ui.label(RichText::new("MOUSE POINTER OVER TEXT").size(11.0).color(dim));
                ui.horizontal(|ui| {
                    for (v, l) in [("IBeam", "I-beam"), ("Arrow", "Arrow"), ("Cross", "Cross"), ("Hand", "Hand"), ("Cell", "Cell"), ("Move", "Move")] {
                        changed |= crate::widgets::radio_value(ui, &mut t.pointer, v.into(), l).changed();
                    }
                });
            }
            6 => {
                ui.horizontal(|ui| {
                    ui.label(format!("Interface font: {}", self.s.theme.ui_font));
                    if crate::widgets::button(ui, "Choose...").clicked() {
                        self.font_dlg = Some(FontPick::new(FontTarget::Interface, &self.s.theme.ui_font));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(format!("Editor font: {}", self.s.font_family));
                    if crate::widgets::button(ui, "Choose...").clicked() {
                        self.font_dlg = Some(FontPick::new(FontTarget::Editor, &self.s.font_family));
                    }
                });
                let t = &mut self.s.theme;
                changed |= ui.add(crate::widgets::Slider::new(&mut t.ui_font_size, 9.0..=18.0).step_by(0.5).text("Interface text size")).changed();
                changed |= ui.add(crate::widgets::Slider::new(&mut self.s.font_size, 8.0..=40.0).step_by(0.5).text("Editor text size")).changed();
                changed |= ui.add(crate::widgets::Slider::new(&mut t.line_number_scale, 0.5..=1.2).step_by(0.05).text("Line number size")).changed();
                ui.horizontal(|ui| {
                    ui.label("Toolbar icons:");
                    for v in [16.0, 20.0, 24.0, 32.0] {
                        changed |= crate::widgets::radio_value(ui, &mut t.toolbar_icon_size, v, format!("{v:.0} px")).changed();
                    }
                });
                changed |= ui.add(crate::widgets::Slider::new(&mut t.scrollbar_size, 6.0..=24.0).step_by(1.0).text("Scrollbar width")).changed();
                ui.horizontal(|ui| {
                    ui.label("Window corners:");
                    for (v, l) in [("Round", "Round"), ("Small", "Slightly round"), ("Square", "Square")] {
                        changed |= crate::widgets::radio_value(ui, &mut t.corners, v.into(), l).changed();
                    }
                });
            }
            _ => {
                {
                    let t = &mut self.s.theme;
                    ui.horizontal(|ui| {
                        ui.label("Show tab bar:");
                        for (v, l) in [("Always", "Always"), ("Multiple", "Only with 2+ tabs"), ("Never", "Never")] {
                            changed |= crate::widgets::radio_value(ui, &mut t.tab_bar_mode, v.into(), l).changed();
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Tab bar position:");
                        changed |= crate::widgets::radio_value(ui, &mut t.tab_position, "Top".into(), "Top").changed();
                        changed |= crate::widgets::radio_value(ui, &mut t.tab_position, "Bottom".into(), "Bottom").changed();
                    });
                    changed |= crate::widgets::checkbox(ui, &mut t.tab_close_buttons, "Close buttons on tabs").changed();
                    changed |= ui.add(crate::widgets::Slider::new(&mut t.tab_max_width, 100.0..=400.0).step_by(10.0).text("Maximum tab width")).changed();
                }
                ui.add_space(8.0);
                ui.label(RichText::new("SESSION").size(11.0).color(dim));
                crate::widgets::checkbox(ui, &mut self.s.restore_session, "Reopen the files that were open");
                crate::widgets::checkbox(ui, &mut self.s.hot_exit, "Keep unsaved changes on exit instead of asking");
                ui.add_space(8.0);
                ui.label(RichText::new("STATUS BAR FIELDS").size(11.0).color(dim));
                ui.horizontal_wrapped(|ui| {
                    for (id, label) in STATUS_FIELDS {
                        let mut on = !self.s.theme.hidden_status.iter().any(|h| h == id);
                        if crate::widgets::checkbox(ui, &mut on, label).changed() {
                            self.s.theme.hidden_status.retain(|h| h != id);
                            if !on {
                                self.s.theme.hidden_status.push(id.to_string());
                            }
                            changed = true;
                        }
                    }
                });
            }
        }
        changed
    }

    /// Completion list under the caret.
    pub fn completion_popup(&mut self, ctx: &egui::Context) {
        if !self.complete.open {
            return;
        }
        let Some(r) = self.tab().view.caret_rect else { return };
        let mut accept = None;
        egui::Area::new(egui::Id::new("completion")).order(egui::Order::Foreground).fixed_pos(egui::pos2(r.left(), r.bottom() + 2.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).inner_margin(2.0).show(ui, |ui| {
                    ui.set_width(260.0);
                    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        for (i, it) in self.complete.items.iter().enumerate() {
                            let icon = if it.keyword { "ac_key" } else { "ac_word" };
                            let sel = i == self.complete.index;
                            let resp = ui.horizontal(|ui| {
                                if let Some(img) = self.icons.image(ctx, icon, 16.0, true) {
                                    ui.add(img);
                                }
                                ui.add(crate::widgets::Selectable::new(sel, RichText::new(&it.text).family(egui::FontFamily::Name("editor".into()))))
                            });
                            if sel {
                                resp.inner.scroll_to_me(None);
                            }
                            if resp.inner.clicked() {
                                accept = Some(i);
                            }
                        }
                    });
                });
            });
        if let Some(i) = accept {
            self.complete.index = i;
            let a = self.active;
            self.complete.accept(&mut self.tabs[a].doc);
            self.focus_editor();
        }
    }
}

#[allow(dead_code)]
fn _p(_: &Path) {}
