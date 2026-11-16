//! Side panel with the folder tree ("Explorer"): Windows file / folder icons, folders read when
//! they are opened, file operations from the context menu (new, rename, Recycle Bin ...).

use crate::app::{App, Modal, ModalAction};
use crate::shellicons::ShellIcons;
use crate::widgets::{self, Button};
use eframe::egui::{self, pos2, vec2, Color32, Key, Rect, RichText, Sense, Shape, Stroke, TextStyle};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub dir: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum EditKind {
    NewFile,
    NewFolder,
    Rename,
}

/// Name being typed for a new file / folder or a rename.
pub struct Edit {
    pub kind: EditKind,
    /// folder the item is (or goes) in
    pub dir: PathBuf,
    /// item being renamed
    pub target: Option<PathBuf>,
    pub name: String,
    pub focus: bool,
}

#[derive(Default)]
pub struct Sidebar {
    children: HashMap<PathBuf, Vec<Entry>>,
    pub expanded: HashSet<PathBuf>,
    pub icons: ShellIcons,
    pub selected: Option<PathBuf>,
    pub edit: Option<Edit>,
    last_active: Option<PathBuf>,
    scroll_to: Option<PathBuf>,
}

enum Row {
    Item(usize, Entry),
    NewItem(usize),
}

enum Act {
    Click(Entry),
    Open(PathBuf),
    New(PathBuf, EditKind),
    Rename(PathBuf),
    Delete(PathBuf),
    Copy(String),
    Reveal(PathBuf),
    Prompt(PathBuf),
    FindIn(PathBuf),
    Refresh,
    CollapseAll,
    CloseFolder,
    Commit,
    Cancel,
}

fn list_dir(dir: &Path) -> Vec<Entry> {
    use std::os::windows::fs::MetadataExt;
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<Entry> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name == ".git" || name.eq_ignore_ascii_case("$RECYCLE.BIN") || name == "System Volume Information" {
                return None;
            }
            let md = e.metadata().ok()?;
            let attr = md.file_attributes();
            if attr & 0x2 != 0 && attr & 0x4 != 0 {
                return None; // hidden system files (desktop.ini ...)
            }
            let dir = md.is_dir() || (md.file_type().is_symlink() && e.path().is_dir());
            Some(Entry { name, path: e.path(), dir })
        })
        .collect();
    v.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    v
}

impl Sidebar {
    /// Forget what was read; folders are read again when shown.
    pub fn refresh(&mut self) {
        self.children.clear();
    }

    pub fn reset(&mut self) {
        self.children.clear();
        self.expanded.clear();
        self.selected = None;
        self.edit = None;
        self.last_active = None;
        self.scroll_to = None;
    }

    fn kids(&mut self, dir: &Path) -> &Vec<Entry> {
        self.children.entry(dir.to_path_buf()).or_insert_with(|| list_dir(dir))
    }

    fn push_rows(&mut self, dir: &Path, depth: usize, out: &mut Vec<Row>) {
        let kids = self.kids(dir).clone();
        for e in kids {
            let open = e.dir && self.expanded.contains(&e.path);
            let p = e.path.clone();
            out.push(Row::Item(depth, e));
            if open {
                self.push_rows(&p, depth + 1, out);
            }
        }
    }

    fn rows(&mut self, root: &Path) -> Vec<Row> {
        let mut out = Vec::new();
        self.push_rows(root, 0, &mut out);
        if let Some(ed) = &self.edit {
            if ed.kind != EditKind::Rename {
                if ed.dir == root {
                    out.insert(0, Row::NewItem(0));
                } else if let Some(i) = out.iter().position(|r| matches!(r, Row::Item(_, e) if e.path == ed.dir)) {
                    let d = if let Row::Item(d, _) = &out[i] { *d } else { 0 };
                    out.insert(i + 1, Row::NewItem(d + 1));
                }
            }
        }
        out
    }

    /// Open the folders down to `file` and select it.
    fn reveal(&mut self, root: &Path, file: &Path) {
        if !file.starts_with(root) {
            return;
        }
        let mut d = file.parent();
        while let Some(p) = d {
            if p == root || !p.starts_with(root) {
                break;
            }
            self.expanded.insert(p.to_path_buf());
            d = p.parent();
        }
        self.selected = Some(file.to_path_buf());
        self.scroll_to = Some(file.to_path_buf());
    }
}

fn chevron(p: &egui::Painter, c: egui::Pos2, open: bool, color: Color32) {
    let s = 3.6;
    let pts = if open {
        vec![pos2(c.x - s, c.y - s * 0.55), pos2(c.x + s, c.y - s * 0.55), pos2(c.x, c.y + s * 0.65)]
    } else {
        vec![pos2(c.x - s * 0.55, c.y - s), pos2(c.x + s * 0.65, c.y), pos2(c.x - s * 0.55, c.y + s)]
    };
    p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

impl App {
    /// Show `dir` in the sidebar.
    pub fn set_folder(&mut self, dir: &Path) {
        self.s.sidebar_root = dir.display().to_string();
        self.s.show_sidebar = true;
        self.side.reset();
    }

    pub fn sidebar_root(&self) -> Option<PathBuf> {
        (!self.s.sidebar_root.is_empty()).then(|| PathBuf::from(&self.s.sidebar_root))
    }

    pub fn sidebar_panel(&mut self, ctx: &egui::Context, cmds: &mut Vec<String>) {
        let frame = egui::Frame::none().fill(self.pal.c("ui.panel")).inner_margin(egui::Margin { left: 0.0, right: 0.0, top: 4.0, bottom: 2.0 });
        let r = egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(self.s.sidebar_width.clamp(150.0, 700.0))
            .width_range(150.0..=700.0)
            .frame(frame)
            .show(ctx, |ui| self.sidebar_ui(ui, cmds));
        self.s.sidebar_width = r.response.rect.width();
    }

    fn side_tool(&mut self, ui: &mut egui::Ui, icon: Option<&str>, text: &str, tip: &str) -> bool {
        let ctx = ui.ctx().clone();
        let r = match icon.and_then(|i| self.icons.image(&ctx, i, 16.0, true)) {
            Some(img) => ui.add(Button::image(img).frame(false)),
            None => ui.add(Button::new(text).frame(false)),
        };
        r.on_hover_text(tip).clicked()
    }

    fn sidebar_ui(&mut self, ui: &mut egui::Ui, cmds: &mut Vec<String>) {
        let pal = self.pal.clone();
        let ctx = ui.ctx().clone();
        let root = self.sidebar_root();
        let mut acts: Vec<Act> = Vec::new();

        // ---- header
        ui.horizontal(|ui| {
            ui.set_height(24.0);
            ui.add_space(12.0);
            ui.label(RichText::new("EXPLORER").size(11.5).strong().color(pal.c("ui.textDim")));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                ui.spacing_mut().item_spacing.x = 1.0;
                if self.side_tool(ui, None, "✕", "Hide the sidebar (Ctrl+Shift+E)") {
                    cmds.push("sidebar".into());
                }
                if let Some(root) = &root {
                    if self.side_tool(ui, Some("fold"), "−", "Collapse all folders") {
                        acts.push(Act::CollapseAll);
                    }
                    if self.side_tool(ui, Some("redo"), "⟳", "Refresh") {
                        acts.push(Act::Refresh);
                    }
                    let target = self.new_item_dir(root);
                    if self.side_tool(ui, Some("open"), "+", "New folder") {
                        acts.push(Act::New(target.clone(), EditKind::NewFolder));
                    }
                    if self.side_tool(ui, Some("new"), "+", "New file") {
                        acts.push(Act::New(target, EditKind::NewFile));
                    }
                }
            });
        });

        let Some(root) = root else {
            ui.add_space(18.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No folder is open").color(pal.c("ui.textDim")));
                ui.add_space(10.0);
                if ui.add(Button::new("Open Folder...").fill(pal.c("ui.accent")).min_size(vec2(180.0, 28.0))).clicked() {
                    cmds.push("open_dir".into());
                }
                if let Some(d) = self.tab().dir() {
                    ui.add_space(4.0);
                    if ui.add(Button::new("Folder of Current File").min_size(vec2(180.0, 28.0))).clicked() {
                        cmds.push(format!("open_dir_at:{}", d.display()));
                    }
                }
            });
            return;
        };

        // follow the active tab
        let active = self.tab().path.clone();
        if active != self.side.last_active {
            self.side.last_active = active.clone();
            if let Some(f) = &active {
                self.side.reveal(&root, f);
            }
        }

        // ---- root row
        let root_name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| root.display().to_string());
        let (rr, rresp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
        {
            let p = ui.painter();
            if rresp.hovered() {
                p.rect_filled(rr.shrink2(vec2(4.0, 0.0)), 5.0, pal.c("ui.menuHover"));
            }
            let icon_rect = Rect::from_center_size(pos2(rr.left() + 22.0, rr.center().y), vec2(16.0, 16.0));
            if let Some(t) = self.side.icons.get(&ctx, &root, true, true) {
                p.image(t, icon_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            let g = p.layout_no_wrap(root_name.to_uppercase(), egui::FontId::proportional(12.0), pal.c("ui.text"));
            let pos = pos2(icon_rect.right() + 7.0, rr.center().y - g.size().y / 2.0);
            p.with_clip_rect(rr).galley(pos, g, pal.c("ui.text"));
        }
        let rresp = rresp.on_hover_text(root.display().to_string());
        rresp.context_menu(|ui| {
            if ui.add(Button::new("New File...")).clicked() {
                acts.push(Act::New(root.clone(), EditKind::NewFile));
                ui.close_menu();
            }
            if ui.add(Button::new("New Folder...")).clicked() {
                acts.push(Act::New(root.clone(), EditKind::NewFolder));
                ui.close_menu();
            }
            ui.separator();
            self.common_menu(ui, &root, &root, &mut acts);
            ui.separator();
            if ui.add(Button::new("Refresh")).clicked() {
                acts.push(Act::Refresh);
                ui.close_menu();
            }
            if ui.add(Button::new("Close Folder")).clicked() {
                acts.push(Act::CloseFolder);
                ui.close_menu();
            }
        });

        // ---- tree
        let rows = self.side.rows(&root);
        let row_h = 23.0;
        let scroll_idx = self.side.scroll_to.take().and_then(|p| rows.iter().position(|r| matches!(r, Row::Item(_, e) if e.path == p)));
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut area = egui::ScrollArea::vertical().id_salt("side_tree").auto_shrink([false, false]);
        if let Some(i) = scroll_idx {
            let view = ui.available_height();
            let y = i as f32 * row_h;
            let cur = ui.ctx().data(|d| d.get_temp::<f32>(egui::Id::new("side_tree_off"))).unwrap_or(0.0);
            if y < cur || y + row_h > cur + view {
                area = area.vertical_scroll_offset((y - view / 2.0).max(0.0));
            }
        }
        let font = TextStyle::Body.resolve(ui.style());
        let line = pal.c("ui.line");
        let text = pal.c("ui.text");
        let dim = pal.c("ui.textDim");
        let accent = pal.c("ui.accent");
        let hover = pal.c("ui.menuHover");
        let sel_fill = widgets::mix(pal.c("ui.panel"), accent, if pal.is_light() { 0.18 } else { 0.30 });
        let out = area.show_rows(ui, row_h, rows.len(), |ui, range| {
            for idx in range {
                let (depth, entry) = match &rows[idx] {
                    Row::Item(d, e) => (*d, Some(e.clone())),
                    Row::NewItem(d) => (*d, None),
                };
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::click());
                let indent = 12.0 + depth as f32 * 14.0;
                let painter = ui.painter().with_clip_rect(rect);
                // indent guides
                for d in 0..depth {
                    let x = (12.0 + d as f32 * 14.0 + 6.0 + rect.left()).round() + 0.5;
                    painter.vline(x, rect.y_range(), Stroke::new(1.0_f32, line.gamma_multiply(0.6)));
                }
                let chev_c = pos2(rect.left() + indent + 6.0, rect.center().y);
                let icon_rect = Rect::from_center_size(pos2(rect.left() + indent + 22.0, rect.center().y), vec2(16.0, 16.0));
                let text_x = icon_rect.right() + 6.0;
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));

                let Some(e) = entry else {
                    // name for a new file / folder
                    let kind = self.side.edit.as_ref().map(|e| e.kind).unwrap_or(EditKind::NewFile);
                    let dir = kind == EditKind::NewFolder;
                    let probe = self.side.edit.as_ref().map(|e| e.dir.join(if e.name.is_empty() { "x" } else { e.name.as_str() })).unwrap_or_default();
                    if let Some(t) = self.side.icons.get(&ctx, &probe, dir, false) {
                        painter.image(t, icon_rect, uv, Color32::WHITE);
                    }
                    let er = Rect::from_min_max(pos2(text_x - 3.0, rect.top() + 1.0), pos2(rect.right() - 6.0, rect.bottom() - 1.0));
                    self.side_name_edit(ui, er, &mut acts);
                    continue;
                };

                let is_active = active.as_deref() == Some(e.path.as_path());
                let is_sel = self.side.selected.as_deref() == Some(e.path.as_path());
                let hov = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.08);
                let body = rect.shrink2(vec2(4.0, 0.5));
                if is_active || is_sel {
                    painter.rect_filled(body, 5.0, if is_active { sel_fill } else { widgets::mix(hover, sel_fill, 0.35) });
                } else if hov > 0.0 {
                    painter.rect_filled(body, 5.0, hover.gamma_multiply(hov));
                }
                if is_active {
                    let bar = Rect::from_center_size(pos2(body.left() + 1.5, body.center().y), vec2(3.0, row_h - 9.0));
                    painter.rect_filled(bar, 1.5, accent);
                }
                let open = e.dir && self.side.expanded.contains(&e.path);
                if e.dir {
                    chevron(&painter, chev_c, open, if resp.hovered() { text } else { dim });
                }
                if let Some(t) = self.side.icons.get(&ctx, &e.path, e.dir, open) {
                    painter.image(t, icon_rect, uv, Color32::WHITE);
                }
                let renaming = self.side.edit.as_ref().is_some_and(|ed| ed.kind == EditKind::Rename && ed.target.as_deref() == Some(e.path.as_path()));
                if renaming {
                    let er = Rect::from_min_max(pos2(text_x - 3.0, rect.top() + 1.0), pos2(rect.right() - 6.0, rect.bottom() - 1.0));
                    self.side_name_edit(ui, er, &mut acts);
                } else {
                    let col = if e.name.starts_with('.') { dim } else { text };
                    let g = painter.layout_no_wrap(e.name.clone(), font.clone(), col);
                    painter.galley(pos2(text_x, rect.center().y - g.size().y / 2.0), g, col);
                }
                if resp.clicked() {
                    acts.push(Act::Click(e.clone()));
                }
                let dir_of = if e.dir { e.path.clone() } else { e.path.parent().map(Path::to_path_buf).unwrap_or_else(|| root.clone()) };
                resp.context_menu(|ui| {
                    if !e.dir && ui.add(Button::new("Open")).clicked() {
                        acts.push(Act::Open(e.path.clone()));
                        ui.close_menu();
                    }
                    if ui.add(Button::new("New File...")).clicked() {
                        acts.push(Act::New(dir_of.clone(), EditKind::NewFile));
                        ui.close_menu();
                    }
                    if ui.add(Button::new("New Folder...")).clicked() {
                        acts.push(Act::New(dir_of.clone(), EditKind::NewFolder));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add(Button::new("Rename").shortcut_text("F2")).clicked() {
                        acts.push(Act::Rename(e.path.clone()));
                        ui.close_menu();
                    }
                    if ui.add(Button::new("Delete").shortcut_text("Del")).clicked() {
                        acts.push(Act::Delete(e.path.clone()));
                        ui.close_menu();
                    }
                    ui.separator();
                    self.common_menu(ui, &e.path, &dir_of, &mut acts);
                });
            }
        });
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("side_tree_off"), out.state.offset.y));

        // empty space below the tree: context menu for the root folder
        let rest = ui.available_rect_before_wrap();
        if rest.height() > 4.0 {
            let r = ui.allocate_rect(rest, Sense::click());
            if r.clicked() {
                self.side.selected = None;
            }
            r.context_menu(|ui| {
                if ui.add(Button::new("New File...")).clicked() {
                    acts.push(Act::New(root.clone(), EditKind::NewFile));
                    ui.close_menu();
                }
                if ui.add(Button::new("New Folder...")).clicked() {
                    acts.push(Act::New(root.clone(), EditKind::NewFolder));
                    ui.close_menu();
                }
            });
        }

        // keys while the mouse is over the tree and no text field is active
        if self.side.edit.is_none() && ui.ui_contains_pointer() && !ctx.wants_keyboard_input() {
            if let Some(sel) = self.side.selected.clone() {
                if ctx.input(|i| i.key_pressed(Key::F2)) {
                    acts.push(Act::Rename(sel.clone()));
                }
                if ctx.input(|i| i.key_pressed(Key::Delete)) {
                    acts.push(Act::Delete(sel));
                }
            }
        }

        for a in acts {
            self.side_apply(a, &root, cmds);
        }
    }

    /// Folder a new item from the header buttons goes into.
    fn new_item_dir(&self, root: &Path) -> PathBuf {
        match &self.side.selected {
            Some(p) if p.is_dir() => p.clone(),
            Some(p) => p.parent().map(Path::to_path_buf).unwrap_or_else(|| root.to_path_buf()),
            None => root.to_path_buf(),
        }
    }

    fn common_menu(&mut self, ui: &mut egui::Ui, path: &Path, dir: &Path, acts: &mut Vec<Act>) {
        let root = self.sidebar_root().unwrap_or_default();
        if ui.add(Button::new("Copy Path")).clicked() {
            acts.push(Act::Copy(path.display().to_string()));
            ui.close_menu();
        }
        if ui.add(Button::new("Copy Relative Path")).clicked() {
            let rel = path.strip_prefix(&root).map(|p| p.display().to_string()).unwrap_or_else(|_| path.display().to_string());
            acts.push(Act::Copy(if rel.is_empty() { ".".into() } else { rel }));
            ui.close_menu();
        }
        ui.separator();
        if ui.add(Button::new("Reveal in File Explorer")).clicked() {
            acts.push(Act::Reveal(path.to_path_buf()));
            ui.close_menu();
        }
        if ui.add(Button::new("Open Command Prompt Here")).clicked() {
            acts.push(Act::Prompt(dir.to_path_buf()));
            ui.close_menu();
        }
        if ui.add(Button::new("Find in Folder...")).clicked() {
            acts.push(Act::FindIn(dir.to_path_buf()));
            ui.close_menu();
        }
    }

    fn side_name_edit(&mut self, ui: &mut egui::Ui, rect: Rect, acts: &mut Vec<Act>) {
        let Some(ed) = self.side.edit.as_mut() else { return };
        let id = egui::Id::new("side_name_edit");
        let mut out = None;
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
            out = Some(egui::TextEdit::singleline(&mut ed.name).id(id).desired_width(rect.width()).margin(vec2(3.0, 1.0)).show(ui));
        });
        let Some(mut out) = out else { return };
        if ed.focus {
            ed.focus = false;
            out.response.request_focus();
            // select the name without its extension, like Explorer
            let stem = if ed.kind == EditKind::Rename && !ed.name.starts_with('.') {
                ed.name.rfind('.').unwrap_or(ed.name.chars().count())
            } else {
                ed.name.len()
            };
            let stem = ed.name[..stem.min(ed.name.len())].chars().count();
            out.state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(stem))));
            out.state.store(ui.ctx(), id);
            return;
        }
        if out.response.lost_focus() {
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                acts.push(Act::Cancel);
            } else {
                acts.push(Act::Commit);
            }
        }
    }

    fn side_apply(&mut self, a: Act, root: &Path, cmds: &mut Vec<String>) {
        match a {
            Act::Click(e) => {
                self.side.selected = Some(e.path.clone());
                if e.dir {
                    if !self.side.expanded.remove(&e.path) {
                        self.side.expanded.insert(e.path);
                    }
                    // the tree takes the keyboard (F2 / Del) instead of the editor
                    self.ctx.memory_mut(|m| {
                        if let Some(id) = m.focused() {
                            m.surrender_focus(id);
                        }
                    });
                } else {
                    cmds.push(format!("open:{}", e.path.display()));
                }
            }
            Act::Open(p) => cmds.push(format!("open:{}", p.display())),
            Act::New(dir, kind) => {
                if dir != root {
                    self.side.expanded.insert(dir.clone());
                }
                self.side.edit = Some(Edit { kind, dir, target: None, name: String::new(), focus: true });
            }
            Act::Rename(p) => {
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let dir = p.parent().map(Path::to_path_buf).unwrap_or_else(|| root.to_path_buf());
                self.side.edit = Some(Edit { kind: EditKind::Rename, dir, target: Some(p), name, focus: true });
            }
            Act::Delete(p) => {
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let what = if p.is_dir() { "the folder" } else { "" };
                self.modal = Some(Modal {
                    title: "Delete".into(),
                    text: format!("Move {what} \"{name}\" to the Recycle Bin?").replace("  ", " "),
                    buttons: vec![("Move to Recycle Bin".into(), ModalAction::DeletePath(p)), ("Cancel".into(), ModalAction::Nothing)],
                });
            }
            Act::Copy(s) => {
                self.ctx.copy_text(s);
                self.show_info("Path copied");
            }
            Act::Reveal(p) => {
                let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", p.display())).spawn();
            }
            Act::Prompt(dir) => {
                use std::os::windows::process::CommandExt;
                let _ = std::process::Command::new("cmd.exe").current_dir(dir).creation_flags(0x10).spawn();
            }
            Act::FindIn(dir) => {
                self.ff.folder = dir.display().to_string();
                cmds.push("find_files".into());
            }
            Act::Refresh => self.side.refresh(),
            Act::CollapseAll => self.side.expanded.clear(),
            Act::CloseFolder => cmds.push("close_dir".into()),
            Act::Cancel => self.side.edit = None,
            Act::Commit => self.side_commit(),
        }
    }

    fn side_commit(&mut self) {
        let Some(ed) = self.side.edit.take() else { return };
        let name = ed.name.trim().to_string();
        if name.is_empty() {
            return;
        }
        if name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']) {
            self.show_info("A name can't contain \\ / : * ? \" < > |");
            return;
        }
        let new = ed.dir.join(&name);
        match ed.kind {
            EditKind::NewFile => match std::fs::OpenOptions::new().write(true).create_new(true).open(&new) {
                Ok(_) => {
                    self.side.children.remove(&ed.dir);
                    self.side.selected = Some(new.clone());
                    self.open_path(&new);
                }
                Err(e) => self.show_info(format!("Can't create {name}: {e}")),
            },
            EditKind::NewFolder => match std::fs::create_dir(&new) {
                Ok(()) => {
                    self.side.children.remove(&ed.dir);
                    self.side.expanded.insert(new.clone());
                    self.side.selected = Some(new);
                }
                Err(e) => self.show_info(format!("Can't create {name}: {e}")),
            },
            EditKind::Rename => {
                let Some(old) = ed.target else { return };
                if old == new {
                    return;
                }
                match std::fs::rename(&old, &new) {
                    Ok(()) => {
                        // open tabs follow the file (or everything inside a renamed folder)
                        for t in &mut self.tabs {
                            if let Some(p) = t.path.clone() {
                                if p == old {
                                    t.path = Some(new.clone());
                                } else if let Ok(rest) = p.strip_prefix(&old) {
                                    t.path = Some(new.join(rest));
                                }
                            }
                        }
                        let moved: Vec<PathBuf> = self.side.expanded.iter().filter(|p| p.starts_with(&old)).cloned().collect();
                        for p in moved {
                            self.side.expanded.remove(&p);
                            if let Ok(rest) = p.strip_prefix(&old) {
                                self.side.expanded.insert(new.join(rest));
                            }
                        }
                        self.side.refresh();
                        self.side.selected = Some(new);
                        self.side.last_active = None;
                    }
                    Err(e) => self.show_info(format!("Can't rename: {e}")),
                }
            }
        }
    }

    /// Confirmed from the "Delete" question.
    pub fn delete_path_confirmed(&mut self, p: &Path) {
        if crate::win::recycle(p) {
            self.side.refresh();
            if self.side.selected.as_deref() == Some(p) {
                self.side.selected = None;
            }
        } else {
            self.show_info(format!("Couldn't move {} to the Recycle Bin", p.display()));
        }
    }
}
