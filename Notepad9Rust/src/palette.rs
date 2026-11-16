//! Command palette (Ctrl+Shift+P): type a few letters of any command and press Enter.

use crate::app::App;
use eframe::egui::{self, pos2, vec2, Color32, FontId, Key, Rect, Sense, Stroke};

/// (group, label, shortcut, command)
pub const COMMANDS: &[(&str, &str, &str, &str)] = &[
    ("File", "New Tab", "Ctrl+T", "new"),
    ("File", "New Window", "Ctrl+Shift+N", "window_new"),
    ("File", "Open File...", "Ctrl+O", "open"),
    ("File", "Open Folder...", "", "open_dir"),
    ("File", "Close Folder", "", "close_dir"),
    ("File", "Reload", "Ctrl+R", "reload"),
    ("File", "Close Tab", "Ctrl+W", "close_tab"),
    ("File", "Close All Tabs", "", "close_all"),
    ("File", "Reopen Closed Tab", "Ctrl+Shift+T", "reopen_tab"),
    ("File", "Save", "Ctrl+S", "save"),
    ("File", "Save As...", "F12", "save_as"),
    ("File", "Save Copy...", "", "save_copy"),
    ("File", "Save All", "Ctrl+Alt+S", "save_all"),
    ("File", "Add to Favorites", "", "star_add"),
    ("File", "Clear Favorites", "", "clear_favorites"),
    ("File", "Clear Recent Files", "", "clear_recent"),
    ("File", "Run Current File", "F5", "run"),
    ("File", "External Command Prompt", "", "terminal_ext"),
    ("File", "Open Containing Folder", "", "open_folder"),
    ("File", "Print...", "Ctrl+P", "print"),
    ("File", "Exit", "Ctrl+Q", "exit"),
    ("Edit", "Undo", "Ctrl+Z", "undo"),
    ("Edit", "Redo", "Ctrl+Y", "redo"),
    ("Edit", "Cut", "Ctrl+X", "cut"),
    ("Edit", "Copy", "Ctrl+C", "copy"),
    ("Edit", "Paste", "Ctrl+V", "paste"),
    ("Edit", "Delete", "Del", "delete"),
    ("Edit", "Select All", "Ctrl+A", "select_all"),
    ("Edit", "Insert Date/Time", "Ctrl+F5", "datetime"),
    ("Edit", "Complete Word", "Ctrl+Space", "complete"),
    ("Lines", "Duplicate Line", "Ctrl+D", "dup_line"),
    ("Lines", "Delete Line", "Ctrl+Shift+D", "del_line"),
    ("Lines", "Move Line Up", "Alt+Up", "line_up"),
    ("Lines", "Move Line Down", "Alt+Down", "line_down"),
    ("Lines", "Sort Ascending", "", "sort_asc"),
    ("Lines", "Sort Descending", "", "sort_desc"),
    ("Lines", "Remove Duplicate Lines", "", "uniq"),
    ("Lines", "Remove Empty Lines", "", "no_empty"),
    ("Lines", "Join Lines", "", "join"),
    ("Convert", "UPPERCASE", "Ctrl+Shift+U", "upper"),
    ("Convert", "lowercase", "Ctrl+U", "lower"),
    ("Convert", "Title Case", "", "title"),
    ("Convert", "Invert Case", "", "invert"),
    ("Convert", "Tabs to Spaces", "", "tabs2spaces"),
    ("Convert", "Leading Spaces to Tabs", "", "spaces2tabs"),
    ("Convert", "Trim Trailing Whitespace", "", "trim"),
    ("Markdown", "Bold", "Ctrl+B", "md_bold"),
    ("Markdown", "Italic", "Ctrl+I", "md_italic"),
    ("Markdown", "Strikethrough", "", "md_strike"),
    ("Markdown", "Heading", "Ctrl+Shift+H", "md_heading"),
    ("Markdown", "Inline Code", "", "md_code"),
    ("Markdown", "Code Block", "", "md_codeblock"),
    ("Markdown", "Link", "Ctrl+K", "md_link"),
    ("Markdown", "Image...", "", "md_image"),
    ("Markdown", "Bullet List", "", "md_ul"),
    ("Markdown", "Numbered List", "", "md_ol"),
    ("Markdown", "Task List", "", "md_task"),
    ("Markdown", "Quote", "", "md_quote"),
    ("Markdown", "Table", "", "md_table"),
    ("Markdown", "Horizontal Rule", "", "md_hr"),
    ("Search", "Find...", "Ctrl+F", "find"),
    ("Search", "Find Next", "F3", "find_next"),
    ("Search", "Find Previous", "Shift+F3", "find_prev"),
    ("Search", "Replace...", "Ctrl+H", "replace"),
    ("Search", "Find in Files...", "Ctrl+Shift+F", "find_files"),
    ("Search", "Go to Line...", "Ctrl+G", "goto"),
    ("Search", "Focused View", "Ctrl+Alt+F", "focus"),
    ("View", "Sidebar (Explorer)", "Ctrl+Shift+E", "sidebar"),
    ("View", "Toolbar", "", "toggle_toolbar"),
    ("View", "Status Bar", "", "toggle_status"),
    ("View", "Console", "Ctrl+`", "console"),
    ("View", "Markdown Preview", "Ctrl+Shift+V", "md_preview"),
    ("View", "Sync Preview Scrolling", "", "md_sync"),
    ("View", "Word Wrap", "Alt+Z", "wrap"),
    ("View", "Line Numbers", "", "toggle_lnum"),
    ("View", "Show Whitespace", "", "toggle_ws"),
    ("View", "Show Line Endings", "", "toggle_eol"),
    ("View", "Long Line Marker", "", "ruler"),
    ("View", "Highlight Current Line", "", "toggle_curline"),
    ("View", "Mark Occurrences", "", "toggle_occ"),
    ("View", "Zoom In", "Ctrl++", "zoom_in"),
    ("View", "Zoom Out", "Ctrl+-", "zoom_out"),
    ("View", "Reset Zoom", "Ctrl+0", "zoom_reset"),
    ("View", "Always on Top", "Alt+T", "pin"),
    ("Appearance", "Customize Interface...", "Ctrl+Alt+P", "customize"),
    ("Appearance", "Font...", "", "font"),
    ("Appearance", "Secondary Font...", "", "font2"),
    ("Appearance", "Use Secondary Font", "", "toggle_2nd"),
    ("Appearance", "Auto-detect Syntax Scheme", "", "toggle_autoscheme"),
    ("Appearance", "Indent with Spaces", "", "toggle_tabspaces"),
    ("Settings", "Auto Indent", "", "toggle_autoindent"),
    ("Settings", "Auto Completion", "", "toggle_autocomplete"),
    ("Settings", "Restore Session on Start", "", "toggle_session"),
    ("Settings", "Keep Unsaved Changes on Exit", "", "toggle_hotexit"),
    ("Settings", "Detect External File Changes", "", "toggle_detect"),
    ("Settings", "Remember Recent Files", "", "toggle_remember"),
    ("Settings", "Save Settings on Exit", "", "toggle_saveonexit"),
    ("Settings", "Save Settings Now", "", "save_settings"),
    ("Settings", "Open Settings Folder", "", "settings_folder"),
    ("Settings", "Reset Settings...", "", "reset_settings"),
    ("Plugins", "Install Plugin...", "", "plugin_install"),
    ("Plugins", "Plugin Manager...", "", "plugin_manager"),
    ("Plugins", "Open Plugins Folder", "", "plugins_folder"),
    ("Help", "Keyboard Shortcuts", "F1", "shortcuts"),
    ("Help", "About Notepad9", "", "about"),
];

#[derive(Default)]
pub struct CommandPalette {
    pub open: bool,
    pub query: String,
    pub index: usize,
    focus: bool,
    scroll: bool,
}

impl CommandPalette {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.query.clear();
        self.index = 0;
        self.focus = self.open;
        self.scroll = true;
    }
}

#[derive(Clone)]
struct Item {
    group: String,
    label: String,
    shortcut: String,
    cmd: String,
    check: Option<bool>,
    recent: bool,
}

/// Letters of `needle` found in order in `hay`. Score favours word starts and runs.
fn fuzzy(hay: &str, needle: &str) -> Option<(i32, Vec<usize>)> {
    let h: Vec<char> = hay.chars().collect();
    let hl: Vec<char> = hay.to_lowercase().chars().collect();
    if hl.len() != h.len() {
        return None;
    }
    let mut score = 0;
    let mut hits = Vec::new();
    let mut pos = 0;
    let mut last: Option<usize> = None;
    for word in needle.split_whitespace() {
        for nc in word.to_lowercase().chars() {
            // prefer a word start, otherwise the next match
            let mut found = None;
            for i in pos..hl.len() {
                if hl[i] == nc {
                    let start = i == 0 || !h[i - 1].is_alphanumeric() || (h[i].is_uppercase() && h[i - 1].is_lowercase());
                    if start && last.map_or(true, |l| i != l + 1) {
                        found = Some((i, true));
                        break;
                    }
                    if found.is_none() {
                        found = Some((i, start));
                    }
                    if last.is_some_and(|l| i == l + 1) {
                        break;
                    }
                }
            }
            let (i, start) = found?;
            score += 10;
            if start {
                score += 18;
            }
            if last.is_some_and(|l| i == l + 1) {
                score += 14;
            }
            score -= (i - pos).min(10) as i32;
            hits.push(i);
            last = Some(i);
            pos = i + 1;
        }
    }
    if hay.to_lowercase().contains(&needle.trim().to_lowercase()) {
        score += 40;
    }
    score -= (h.len() / 8) as i32;
    Some((score, hits))
}

fn take(i: &mut egui::InputState, key: Key) -> bool {
    i.consume_key(egui::Modifiers::NONE, key)
}

impl App {
    fn palette_checked(&self, id: &str) -> Option<bool> {
        let s = &self.s;
        Some(match id {
            "sidebar" => s.show_sidebar,
            "toggle_toolbar" => s.show_toolbar,
            "toggle_status" => s.show_status_bar,
            "console" => self.console.visible,
            "md_preview" => s.md_preview,
            "md_sync" => s.md_sync_scroll,
            "wrap" => s.word_wrap,
            "toggle_lnum" => s.line_numbers,
            "toggle_ws" => s.show_whitespace,
            "toggle_eol" => s.show_eol,
            "ruler" => s.long_line_marker,
            "toggle_curline" => s.highlight_current_line,
            "toggle_occ" => s.mark_occurrences,
            "pin" => s.always_on_top,
            "toggle_2nd" => s.use_second_font,
            "toggle_autoscheme" => s.auto_detect_scheme,
            "toggle_tabspaces" => s.tabs_as_spaces,
            "toggle_autoindent" => s.auto_indent,
            "toggle_autocomplete" => s.auto_complete,
            "toggle_session" => s.restore_session,
            "toggle_hotexit" => s.hot_exit,
            "toggle_detect" => s.detect_file_changes,
            "toggle_remember" => s.remember_recent,
            "toggle_saveonexit" => s.save_on_exit,
            _ => return None,
        })
    }

    fn palette_items(&self, searching: bool) -> Vec<Item> {
        let mk = |g: &str, l: &str, sc: &str, c: &str, check: Option<bool>| Item {
            group: g.into(), label: l.into(), shortcut: sc.into(), cmd: c.into(), check, recent: false,
        };
        let mut v: Vec<Item> = COMMANDS.iter().map(|(g, l, sc, c)| mk(g, l, sc, c, self.palette_checked(c))).collect();
        // plugin commands
        for (pi, p) in self.plugins.list.iter().enumerate() {
            if !p.is_loaded() {
                continue;
            }
            for m in &p.menu {
                let label = m.path.replace('/', " › ");
                v.push(mk(&p.name, label.trim(), &m.shortcut, &format!("plugin_cmd:{pi}:{}", m.command), None));
            }
        }
        if searching {
            for t in &self.tabs {
                v.push(mk("Tab", &t.title(), "", &format!("activate:{}", t.id), None));
            }
            for p in &self.s.recent {
                let name = std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.clone());
                v.push(mk("Open Recent", &name, "", &format!("open:{p}"), None));
            }
            for p in crate::theme::PRESETS {
                let cur = self.s.theme.preset == *p && self.s.theme.colors.is_empty();
                v.push(mk("Theme", p, "", &format!("theme:{p}"), Some(cur)));
            }
            let cur = self.tab().hl.syntax().map(|s| s.to_string());
            v.push(mk("Syntax", "Plain Text", "", "scheme:", Some(cur.is_none())));
            if crate::highlight::ready().is_some() {
                for n in crate::highlight::all_syntax_names() {
                    let c = cur.as_deref() == Some(n.as_str());
                    v.push(mk("Syntax", &n, "", &format!("scheme:{n}"), Some(c)));
                }
            }
            for e in crate::textfile::ALL_ENCODINGS {
                let name = crate::textfile::encoding_name(e);
                v.push(mk("Encoding", &format!("Save as {name}"), "", &format!("enc:{}", u8::from(e)), Some(e == self.tab().enc)));
            }
            for (e, l) in [(crate::textfile::Eol::CrLf, "Windows (CR+LF)"), (crate::textfile::Eol::Lf, "Unix (LF)"), (crate::textfile::Eol::Cr, "Mac (CR)")] {
                v.push(mk("Line Endings", l, "", &format!("eol:{}", u8::from(e)), Some(e == self.tab().eol)));
            }
            for w in [2, 3, 4, 8] {
                v.push(mk("Tab Width", &w.to_string(), "", &format!("tabw:{w}"), Some(self.s.tab_width == w)));
            }
        }
        v
    }

    pub fn palette_window(&mut self, ctx: &egui::Context, cmds: &mut Vec<String>) {
        if !self.palette.open {
            return;
        }
        let pal = self.pal.clone();
        let q = self.palette.query.trim().to_string();
        let searching = !q.is_empty();
        let all = self.palette_items(searching);

        // filter and rank
        let mut list: Vec<(Item, Vec<usize>)> = Vec::new();
        if searching {
            let mut scored: Vec<(i32, usize, Item, Vec<usize>)> = Vec::new();
            for (n, it) in all.into_iter().enumerate() {
                let hay = format!("{}: {}", it.group, it.label);
                if let Some((mut s, hits)) = fuzzy(&hay, &q) {
                    if self.s.palette_recent.contains(&it.cmd) {
                        s += 6;
                    }
                    scored.push((s, n, it, hits));
                }
            }
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            list = scored.into_iter().map(|(_, _, it, h)| (it, h)).collect();
        } else {
            for c in &self.s.palette_recent {
                if let Some(it) = all.iter().find(|i| &i.cmd == c) {
                    let mut it = it.clone();
                    it.recent = true;
                    list.push((it, Vec::new()));
                }
            }
            for it in all {
                if !self.s.palette_recent.contains(&it.cmd) {
                    list.push((it, Vec::new()));
                }
            }
        }
        let n = list.len();
        let p = &mut self.palette;
        if p.index >= n {
            p.index = n.saturating_sub(1);
        }

        // keys (taken before the text field sees them)
        let mut run: Option<usize> = None;
        let mut close = false;
        ctx.input_mut(|i| {
            if take(i, Key::ArrowDown) && n > 0 {
                p.index = (p.index + 1) % n;
                p.scroll = true;
            }
            if take(i, Key::ArrowUp) && n > 0 {
                p.index = (p.index + n - 1) % n;
                p.scroll = true;
            }
            if take(i, Key::PageDown) && n > 0 {
                p.index = (p.index + 10).min(n - 1);
                p.scroll = true;
            }
            if take(i, Key::PageUp) {
                p.index = p.index.saturating_sub(10);
                p.scroll = true;
            }
            if take(i, Key::Enter) && n > 0 {
                run = Some(p.index);
            }
            if take(i, Key::Escape) {
                close = true;
            }
        });

        let screen = ctx.screen_rect();
        let w = (screen.width() - 40.0).clamp(280.0, 640.0);
        let pos = pos2(screen.center().x - w / 2.0, screen.top() + 52.0);
        let row_h = 30.0;
        let mut area_rect = Rect::NOTHING;
        egui::Area::new(egui::Id::new("command_palette")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            let frame = egui::Frame::popup(ui.style()).fill(pal.c("ui.popup")).stroke(Stroke::new(1.0_f32, pal.c("ui.popupBorder")))
                .rounding(10.0).inner_margin(egui::Margin::same(8.0));
            let r = frame.show(ui, |ui| {
                ui.set_width(w - 16.0);
                let te = egui::TextEdit::singleline(&mut p.query)
                    .hint_text("Type a command, a theme, a file...")
                    .font(FontId::proportional(15.0))
                    .margin(vec2(10.0, 7.0))
                    .desired_width(f32::INFINITY);
                let out = ui.add(te);
                if p.focus {
                    out.request_focus();
                    p.focus = false;
                }
                if out.changed() {
                    p.index = 0;
                    p.scroll = true;
                }
                ui.add_space(6.0);
                if n == 0 {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("   No matching commands").color(pal.c("ui.textDim")));
                    ui.add_space(6.0);
                    return;
                }
                let accent = pal.c("ui.accent");
                let text = pal.c("ui.text");
                let dim = pal.c("ui.textDim");
                let hover = pal.c("ui.menuHover");
                let sel_fill = crate::widgets::mix(pal.c("ui.popup"), accent, if pal.is_light() { 0.16 } else { 0.30 });
                let font = FontId::proportional(14.0);
                let small = FontId::proportional(12.0);
                egui::ScrollArea::vertical().max_height(row_h * 12.0).auto_shrink([false, true]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (k, (it, hits)) in list.iter().enumerate() {
                        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::click());
                        if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                            p.index = k;
                        }
                        if resp.clicked() {
                            run = Some(k);
                        }
                        let sel = k == p.index;
                        if sel && p.scroll {
                            resp.scroll_to_me(None);
                        }
                        if !ui.is_rect_visible(rect) {
                            continue;
                        }
                        let painter = ui.painter().with_clip_rect(rect);
                        if sel {
                            painter.rect_filled(rect, 6.0, sel_fill);
                            painter.rect_filled(Rect::from_center_size(pos2(rect.left() + 2.0, rect.center().y), vec2(3.0, row_h - 12.0)), 1.5, accent);
                        } else if resp.hovered() {
                            painter.rect_filled(rect, 6.0, hover);
                        }
                        // check mark slot
                        let slot = Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), vec2(14.0, 14.0));
                        if it.check == Some(true) {
                            let pts = vec![slot.lerp_inside(vec2(0.15, 0.55)), slot.lerp_inside(vec2(0.42, 0.8)), slot.lerp_inside(vec2(0.88, 0.25))];
                            painter.add(egui::Shape::line(pts, Stroke::new(2.0_f32, accent)));
                        }
                        // "Group: Label" with the matched letters highlighted
                        let full = format!("{}: {}", it.group, it.label);
                        let glen = it.group.chars().count() + 2;
                        let mut job = egui::text::LayoutJob::default();
                        for (ci, ch) in full.chars().enumerate() {
                            let hit = hits.contains(&ci);
                            let base = if ci < glen { dim } else { text };
                            let fmt = egui::TextFormat {
                                font_id: font.clone(),
                                color: if hit { accent } else { base },
                                underline: if hit { Stroke::new(1.0_f32, accent) } else { Stroke::NONE },
                                ..Default::default()
                            };
                            let mut b = [0u8; 4];
                            job.append(ch.encode_utf8(&mut b), 0.0, fmt);
                        }
                        let g = ui.fonts(|f| f.layout_job(job));
                        painter.galley(pos2(rect.left() + 32.0, rect.center().y - g.size().y / 2.0), g, text);
                        // shortcut chip / "recently used"
                        let right = if !it.shortcut.is_empty() { Some((it.shortcut.clone(), true)) } else if it.recent { Some(("recently used".to_string(), false)) } else { None };
                        if let Some((s, chip)) = right {
                            let g = ui.fonts(|f| f.layout_no_wrap(s, small.clone(), dim));
                            let gr = Rect::from_min_size(pos2(rect.right() - 12.0 - g.size().x, rect.center().y - g.size().y / 2.0), g.size());
                            if chip {
                                painter.rect(gr.expand2(vec2(6.0, 3.0)), 4.0, pal.c("ui.field"), Stroke::new(1.0_f32, pal.c("ui.fieldBorder")));
                            }
                            painter.galley(gr.min, g, dim);
                        }
                    }
                });
                p.scroll = false;
            });
            area_rect = r.response.rect;
        });

        // click outside closes it
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(pp) = ctx.input(|i| i.pointer.interact_pos()) {
                if !area_rect.contains(pp) {
                    close = true;
                }
            }
        }
        if let Some(k) = run {
            let cmd = list[k].0.cmd.clone();
            self.palette.open = false;
            self.s.palette_recent.retain(|c| c != &cmd);
            self.s.palette_recent.insert(0, cmd.clone());
            self.s.palette_recent.truncate(8);
            self.focus_editor();
            cmds.push(cmd);
        } else if close {
            self.palette.open = false;
            self.focus_editor();
        }
        let _ = Color32::TRANSPARENT;
    }
}
