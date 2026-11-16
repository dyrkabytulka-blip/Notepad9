//! Main window drawing: menu, toolbar, tabs, Markdown bar, editor, console, status bar.

use crate::app::{editor_only, App, KEYS};
use crate::textfile::{self, Eol, ALL_ENCODINGS};
use crate::theme;
use crate::view::{self, CaretShape, ViewCfg};
use eframe::egui::{self, Align2, Color32, CursorIcon, FontId, Key, Pos2, Rect, Rounding, Sense, Stroke, Vec2};

type Cmds = Vec<String>;

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.hwnd.is_none() {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(h) = frame.window_handle() {
                if let RawWindowHandle::Win32(w) = h.as_raw() {
                    let h = w.hwnd.get();
                    self.hwnd = Some(h);
                    // still hidden here: cloak it, maximize it, show it once the layout has settled
                    crate::win::cloak(h, true);
                    if self.s.maximized {
                        crate::win::maximize(h);
                    }
                    self.style_title_bar();
                }
            }
        }
        self.tick(ctx);

        // closing the window
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && !self.request_exit() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }

        let mut cmds: Cmds = Vec::new();
        if self.modal.is_none() {
            self.shortcuts_input(ctx, &mut cmds);
        }
        // Ctrl+1..9 selects a tab
        for (n, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9].iter().enumerate() {
            if ctx.input_mut(|i| take_key(i, true, false, false, *key)) {
                let i = if n == 8 { self.tabs.len() - 1 } else { n };
                if i < self.tabs.len() {
                    self.activate(i);
                }
            }
        }
        self.editor_keys(ctx);

        // plugin windows describe their content before anything is drawn
        self.plugins_build_ui();
        // whole-window background picture: first, so the menu and bars are drawn over it
        if self.s.theme.window_image_active() {
            if let Some(tex) = self.bg_texture(ctx) {
                let r = ctx.screen_rect();
                paint_image(&ctx.layer_painter(egui::LayerId::background()), &tex, r, &self.s.theme);
            }
        }
        self.menu_bar(ctx, &mut cmds);
        if self.s.show_toolbar {
            self.toolbar(ctx, &mut cmds);
        }
        let tabs_visible = match self.s.theme.tab_bar_mode.as_str() {
            "Never" => false,
            "Multiple" => self.tabs.len() > 1,
            _ => true,
        };
        let tabs_bottom = self.s.theme.tab_position == "Bottom";
        if self.s.show_status_bar {
            self.status_bar(ctx, &mut cmds);
        }
        // the sidebar runs from the toolbar down to the status bar; tabs sit right of it
        if self.s.show_sidebar {
            self.sidebar_panel(ctx, &mut cmds);
        }
        if tabs_visible && !tabs_bottom {
            self.tab_bar(ctx, &mut cmds, false);
        }
        if tabs_visible && tabs_bottom {
            self.tab_bar(ctx, &mut cmds, true);
        }
        if self.console.visible {
            self.console_panel(ctx, &mut cmds);
        }
        self.plugins_panels(ctx);
        let md = self.tab().is_markdown();
        if md && self.s.md_preview {
            self.preview_panel(ctx);
        }
        self.central(ctx, &mut cmds, md);

        self.dialogs(ctx);
        self.plugins_windows(ctx);
        self.plugin_manager_window(ctx);
        self.completion_popup(ctx);
        self.palette_window(ctx, &mut cmds);

        for c in cmds {
            self.exec(&c);
        }
        self.plugins_after_frame();

        // title
        let t = self.tab();
        let title = format!("{}{} - Notepad9", if t.modified() { "* " } else { "" }, t.title());
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        self.apply_appearance(ctx);

        // plugins start after the first frame is on screen
        if !self.plugins.loaded {
            self.load_plugins();
        }

        // start-up: the window becomes visible only after a few frames (size, plugin panels,
        // syntax colors are all in place by then), so it does not flicker
        if !self.revealed {
            self.reveal_frames += 1;
            let settled = self.reveal_frames >= 4 && crate::highlight::ready().is_some();
            if settled || self.reveal_frames > 60 || self.t0.elapsed().as_millis() > 2000 {
                self.revealed = true;
                if let Some(h) = self.hwnd {
                    crate::win::cloak(h, false);
                }
            } else {
                ctx.request_repaint();
            }
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let [r, g, b, _] = self.pal.c("ui.window").to_srgba_unmultiplied();
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
    }
}

/// Remove a key press with exactly these modifiers from the input. True if it was there.
fn take_key(i: &mut egui::InputState, ctrl: bool, shift: bool, alt: bool, key: Key) -> bool {
    let mut found = false;
    i.events.retain(|e| {
        if let egui::Event::Key { key: k, pressed: true, modifiers, .. } = e {
            if *k == key && modifiers.ctrl == ctrl && modifiers.shift == shift && modifiers.alt == alt && !found {
                found = true;
                return false;
            }
        }
        true
    });
    found
}

impl App {
    /// Runs a command id, or one of the parametrized ones ("open:<path>", "enc:<n>" ...).
    pub fn exec(&mut self, c: &str) {
        if let Some((k, v)) = c.split_once(':') {
            match k {
                "open" => {
                    self.open_path(std::path::Path::new(v));
                }
                "enc" => self.set_encoding((v.parse::<u8>().unwrap_or(1)).into()),
                "reload_as" => self.reload(Some((v.parse::<u8>().unwrap_or(1)).into())),
                "eol" => self.set_eol((v.parse::<u8>().unwrap_or(0)).into()),
                "scheme" => self.set_syntax(if v.is_empty() { None } else { Some(v.to_string()) }),
                "tabw" => self.s.tab_width = v.parse().unwrap_or(4),
                "activate" => {
                    if let Some(i) = v.parse().ok().and_then(|id| self.index_of(id)) {
                        self.activate(i);
                    }
                }
                "close" => {
                    if let Some(i) = v.parse().ok().and_then(|id| self.index_of(id)) {
                        self.close_tab(i, true);
                    }
                }
                "close_others" => {
                    let id: u64 = v.parse().unwrap_or(0);
                    let ids = self.tabs.iter().filter(|t| t.id != id).map(|t| t.id).collect();
                    self.close_tabs(ids);
                }
                "close_right" => {
                    if let Some(i) = v.parse().ok().and_then(|id| self.index_of(id)) {
                        let ids = self.tabs.iter().skip(i + 1).map(|t| t.id).collect();
                        self.close_tabs(ids);
                    }
                }
                "copy_path" => self.ctx.copy_text(v.to_string()),
                "open_dir_at" => self.set_folder(std::path::Path::new(v)),
                "theme" => {
                    self.s.theme.preset = v.to_string();
                    self.s.theme.colors.clear();
                    self.theme_dirty = true;
                }
                "fav_remove" => self.s.favorites.retain(|f| f != v),
                "plugin_cmd" => {
                    if let Some((pi, cmd)) = v.split_once(':') {
                        if let (Ok(pi), Ok(cmd)) = (pi.parse::<usize>(), cmd.parse::<u32>()) {
                            self.plugin_command(pi, cmd);
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        match c {
            "clear_recent" => self.s.recent.clear(),
            "clear_favorites" => self.s.favorites.clear(),
            _ => self.run(c),
        }
    }

    fn shortcuts_input(&mut self, ctx: &egui::Context, cmds: &mut Cmds) {
        let md = self.tab().is_markdown();
        let editor = self.editor_focused;
        let nothing_focused = ctx.memory(|m| m.focused().is_none());
        for (ctrl, shift, alt, key, id) in KEYS {
            if editor_only(id) && !(editor || nothing_focused) {
                continue;
            }
            if matches!(*id, "md_bold" | "md_italic" | "md_link" | "md_heading" | "md_preview") && !md {
                continue;
            }
            if ctx.input_mut(|i| take_key(i, *ctrl, *shift, *alt, *key)) {
                cmds.push(id.to_string());
            }
        }
        for (pi, cmd, ctrl, shift, alt, key) in self.plugins.shortcuts() {
            if ctx.input_mut(|i| take_key(i, ctrl, shift, alt, key)) {
                cmds.push(format!("plugin_cmd:{pi}:{cmd}"));
            }
        }
        if self.tab().view.filter.is_some() && editor && ctx.input_mut(|i| take_key(i, false, false, false, Key::Escape)) {
            cmds.push("focus".into());
        }
    }

    /// Keys handled before the editor sees them: completion list, Markdown Enter / Tab.
    fn editor_keys(&mut self, ctx: &egui::Context) {
        if !self.editor_focused || self.modal.is_some() {
            return;
        }
        let i = self.active;
        if self.complete.open {
            let n = self.complete.items.len();
            if ctx.input_mut(|inp| take_key(inp, false, false, false, Key::ArrowDown)) {
                self.complete.index = (self.complete.index + 1).min(n.saturating_sub(1));
            }
            if ctx.input_mut(|inp| take_key(inp, false, false, false, Key::ArrowUp)) {
                self.complete.index = self.complete.index.saturating_sub(1);
            }
            if ctx.input_mut(|inp| take_key(inp, false, false, false, Key::PageDown)) {
                self.complete.index = (self.complete.index + 8).min(n.saturating_sub(1));
            }
            if ctx.input_mut(|inp| take_key(inp, false, false, false, Key::PageUp)) {
                self.complete.index = self.complete.index.saturating_sub(8);
            }
            if ctx.input_mut(|inp| take_key(inp, false, false, false, Key::Enter) || take_key(inp, false, false, false, Key::Tab)) {
                self.complete.accept(&mut self.tabs[i].doc);
            }
            if ctx.input_mut(|inp| take_key(inp, false, false, false, Key::Escape)) {
                self.complete.close();
            }
            return;
        }
        if self.tabs[i].is_markdown() {
            let nl = self.tabs[i].eol.as_str();
            let unit = self.indent_unit();
            let enter = ctx.input(|inp| inp.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if modifiers.is_none())));
            if enter && crate::mdtools::handle_enter(&mut self.tabs[i].doc, nl) {
                ctx.input_mut(|inp| take_key(inp, false, false, false, Key::Enter));
                self.tabs[i].view.reveal = true;
            }
            let tab = ctx.input(|inp| inp.events.iter().find_map(|e| match e {
                egui::Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if !modifiers.ctrl && !modifiers.alt => Some(modifiers.shift),
                _ => None,
            }));
            if let Some(shift) = tab {
                if crate::mdtools::handle_tab(&mut self.tabs[i].doc, shift, &unit) {
                    ctx.input_mut(|inp| take_key(inp, false, shift, false, Key::Tab));
                }
            }
        }
    }

    // ===================================================================== menu
    pub(crate) fn mi(&mut self, ui: &mut egui::Ui, cmds: &mut Cmds, label: &str, sc: &str, icon: Option<&str>, enabled: bool, id: &str) {
        let img = icon.and_then(|i| self.icons.image(ui.ctx(), i, 16.0, enabled));
        let mut b = match img {
            Some(img) => crate::widgets::Button::image_and_text(img, label),
            None => crate::widgets::Button::new(label),
        };
        if !sc.is_empty() {
            b = b.shortcut_text(sc);
        }
        if ui.add_enabled(enabled, b).clicked() {
            cmds.push(id.to_string());
            ui.close_menu();
        }
    }

    fn mc(&mut self, ui: &mut egui::Ui, cmds: &mut Cmds, label: &str, sc: &str, checked: bool, id: &str) {
        let mut b = crate::widgets::Button::new(label).checked(checked);
        if !sc.is_empty() {
            b = b.shortcut_text(sc);
        }
        if ui.add(b).clicked() {
            cmds.push(id.to_string());
            ui.close_menu();
        }
    }

    fn menu_bar(&mut self, ctx: &egui::Context, cmds: &mut Cmds) {
        let fill = self.pal.c("ui.menuBar");
        let line = self.pal.c("ui.line");
        egui::TopBottomPanel::top("menu").frame(egui::Frame::none().fill(fill).inner_margin(egui::Margin { left: 2.0, right: 2.0, top: 1.0, bottom: 1.0 })
            .stroke(Stroke::NONE)).show_separator_line(false).show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.style_mut().spacing.button_padding = Vec2::new(8.0, 2.0);
                let t_has_path = self.tab().path.is_some();
                let (can_undo, can_redo, has_sel) = {
                    let d = &self.tab().doc;
                    (d.can_undo(), d.can_redo(), !d.sel.is_empty())
                };
                ui.menu_button("File", |ui| {
                    self.mi(ui, cmds, "New Tab", "Ctrl+T", Some("new"), true, "new");
                    self.mi(ui, cmds, "New Window", "Ctrl+Shift+N", Some("window_new"), true, "window_new");
                    self.mi(ui, cmds, "Open...", "Ctrl+O", Some("open"), true, "open");
                    self.mi(ui, cmds, "Open Folder...", "", None, true, "open_dir");
                    let has_root = !self.s.sidebar_root.is_empty();
                    self.mi(ui, cmds, "Close Folder", "", None, has_root, "close_dir");
                    self.mi(ui, cmds, "Reload", "Ctrl+R", None, t_has_path, "reload");
                    ui.menu_button(crate::widgets::sub(ui, "Recent Files"), |ui| {
                        let recent = self.s.recent.clone();
                        if recent.is_empty() {
                            ui.add_enabled(false, crate::widgets::Button::new("(empty)"));
                        }
                        for (i, p) in recent.iter().enumerate() {
                            let label = if i < 9 { format!("{}  {p}", i + 1) } else { p.clone() };
                            if ui.add_enabled(std::path::Path::new(p).exists(), crate::widgets::Button::new(label)).clicked() {
                                cmds.push(format!("open:{p}"));
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        if crate::widgets::button(ui, "Clear List").clicked() {
                            cmds.push("clear_recent".into());
                            ui.close_menu();
                        }
                    });
                    ui.separator();
                    self.mi(ui, cmds, "Close Tab", "Ctrl+W", None, true, "close_tab");
                    self.mi(ui, cmds, "Close All Tabs", "", None, true, "close_all");
                    self.mi(ui, cmds, "Reopen Closed Tab", "Ctrl+Shift+T", None, !self.closed.is_empty(), "reopen_tab");
                    ui.separator();
                    self.mi(ui, cmds, "Save", "Ctrl+S", Some("save"), true, "save");
                    self.mi(ui, cmds, "Save As...", "F12", Some("save_as"), true, "save_as");
                    self.mi(ui, cmds, "Save Copy...", "", None, true, "save_copy");
                    self.mi(ui, cmds, "Save All", "Ctrl+Alt+S", None, true, "save_all");
                    ui.separator();
                    ui.menu_button(crate::widgets::sub(ui, "Encoding"), |ui| self.encoding_menu(ui, cmds));
                    ui.menu_button(crate::widgets::sub(ui, "Line Endings"), |ui| self.eol_menu(ui, cmds));
                    ui.separator();
                    ui.menu_button(crate::widgets::sub(ui, "Favorites"), |ui| self.favorites_menu(ui, cmds));
                    self.mi(ui, cmds, "Add to Favorites", "", Some("star_add"), t_has_path, "star_add");
                    ui.separator();
                    self.mi(ui, cmds, "Run", "F5", Some("run"), true, "run");
                    self.mi(ui, cmds, "Console", "Ctrl+`", Some("terminal"), true, "console");
                    self.mi(ui, cmds, "External Command Prompt", "", None, true, "terminal_ext");
                    self.mi(ui, cmds, "Open Containing Folder", "", None, t_has_path, "open_folder");
                    ui.separator();
                    self.mi(ui, cmds, "Print...", "Ctrl+P", Some("print"), true, "print");
                    ui.separator();
                    self.mi(ui, cmds, "Exit", "Ctrl+Q", Some("exit"), true, "exit");
                });
                ui.menu_button("Edit", |ui| {
                    self.mi(ui, cmds, "Undo", "Ctrl+Z", Some("undo"), can_undo, "undo");
                    self.mi(ui, cmds, "Redo", "Ctrl+Y", Some("redo"), can_redo, "redo");
                    ui.separator();
                    self.mi(ui, cmds, "Cut", "Ctrl+X", Some("cut"), true, "cut");
                    self.mi(ui, cmds, "Copy", "Ctrl+C", Some("copy"), true, "copy");
                    self.mi(ui, cmds, "Paste", "Ctrl+V", Some("paste"), true, "paste");
                    self.mi(ui, cmds, "Delete", "Del", None, has_sel, "delete");
                    ui.separator();
                    self.mi(ui, cmds, "Select All", "Ctrl+A", None, true, "select_all");
                    ui.separator();
                    ui.menu_button(crate::widgets::sub(ui, "Lines"), |ui| {
                        self.mi(ui, cmds, "Duplicate Line", "Ctrl+D", None, true, "dup_line");
                        self.mi(ui, cmds, "Delete Line", "Ctrl+Shift+D", None, true, "del_line");
                        self.mi(ui, cmds, "Move Line Up", "Alt+Up", None, true, "line_up");
                        self.mi(ui, cmds, "Move Line Down", "Alt+Down", None, true, "line_down");
                        ui.separator();
                        self.mi(ui, cmds, "Sort Ascending", "", None, true, "sort_asc");
                        self.mi(ui, cmds, "Sort Descending", "", None, true, "sort_desc");
                        self.mi(ui, cmds, "Remove Duplicate Lines", "", None, true, "uniq");
                        self.mi(ui, cmds, "Remove Empty Lines", "", None, true, "no_empty");
                        self.mi(ui, cmds, "Join Lines", "", None, true, "join");
                    });
                    ui.menu_button(crate::widgets::sub(ui, "Convert"), |ui| {
                        self.mi(ui, cmds, "UPPERCASE", "Ctrl+Shift+U", None, true, "upper");
                        self.mi(ui, cmds, "lowercase", "Ctrl+U", None, true, "lower");
                        self.mi(ui, cmds, "Title Case", "", None, true, "title");
                        self.mi(ui, cmds, "iNVERT cASE", "", None, true, "invert");
                        ui.separator();
                        self.mi(ui, cmds, "Tabs to Spaces", "", None, true, "tabs2spaces");
                        self.mi(ui, cmds, "Leading Spaces to Tabs", "", None, true, "spaces2tabs");
                        self.mi(ui, cmds, "Trim Trailing Whitespace", "", None, true, "trim");
                    });
                    ui.separator();
                    self.mi(ui, cmds, "Insert Date/Time", "Ctrl+F5", None, true, "datetime");
                    self.mi(ui, cmds, "Complete Word", "Ctrl+Space", Some("ac_word"), true, "complete");
                    ui.separator();
                    ui.menu_button(crate::widgets::sub(ui, "Markdown"), |ui| {
                        for (l, sc, icon, id) in [("Bold", "Ctrl+B", "md_bold", "md_bold"), ("Italic", "Ctrl+I", "md_italic", "md_italic"),
                            ("Strikethrough", "", "md_strike", "md_strike"), ("Heading", "Ctrl+Shift+H", "md_heading", "md_heading"),
                            ("Inline Code", "", "md_code", "md_code"), ("Code Block", "", "md_codeblock", "md_codeblock"),
                            ("Link", "Ctrl+K", "md_link", "md_link"), ("Image...", "", "md_image", "md_image"),
                            ("Bullet List", "", "md_ul", "md_ul"), ("Numbered List", "", "md_ol", "md_ol"), ("Task List", "", "md_task", "md_task"),
                            ("Quote", "", "md_quote", "md_quote"), ("Table", "", "md_table", "md_table"), ("Horizontal Rule", "", "md_hr", "md_hr")] {
                            self.mi(ui, cmds, l, sc, Some(icon), true, id);
                        }
                    });
                });
                ui.menu_button("Search", |ui| {
                    self.mi(ui, cmds, "Find...", "Ctrl+F", Some("find"), true, "find");
                    self.mi(ui, cmds, "Find Next", "F3", None, true, "find_next");
                    self.mi(ui, cmds, "Find Previous", "Shift+F3", None, true, "find_prev");
                    self.mi(ui, cmds, "Replace...", "Ctrl+H", Some("replace"), true, "replace");
                    ui.separator();
                    self.mi(ui, cmds, "Find in Files...", "Ctrl+Shift+F", Some("find_files"), true, "find_files");
                    ui.separator();
                    self.mi(ui, cmds, "Go To...", "Ctrl+G", Some("goto"), true, "goto");
                    ui.separator();
                    let fv = self.tab().view.filter.is_some();
                    self.mc(ui, cmds, "Focused View", "Ctrl+Alt+F", fv, "focus");
                });
                ui.menu_button("View", |ui| {
                    self.mi(ui, cmds, "Command Palette...", "Ctrl+Shift+P", None, true, "palette");
                    let side = self.s.show_sidebar;
                    self.mc(ui, cmds, "Sidebar (Explorer)", "Ctrl+Shift+E", side, "sidebar");
                    ui.separator();
                    let s = &self.s;
                    let items = [("Toolbar", "", s.show_toolbar, "toggle_toolbar"), ("Status Bar", "", s.show_status_bar, "toggle_status"),
                        ("Console", "Ctrl+`", self.console.visible, "console"), ("Markdown Preview", "Ctrl+Shift+V", s.md_preview, "md_preview"),
                        ("Sync Preview Scrolling", "", s.md_sync_scroll, "md_sync")];
                    for (l, sc, c, id) in items {
                        self.mc(ui, cmds, l, sc, c, id);
                    }
                    ui.separator();
                    let s = &self.s;
                    let items = [("Word Wrap", "Alt+Z", s.word_wrap, "wrap"), ("Line Numbers", "", s.line_numbers, "toggle_lnum"),
                        ("Show Whitespace", "", s.show_whitespace, "toggle_ws"), ("Show Line Endings", "", s.show_eol, "toggle_eol"),
                        ("Long Line Marker", "", s.long_line_marker, "ruler"), ("Highlight Current Line", "", s.highlight_current_line, "toggle_curline"),
                        ("Mark Occurrences", "", s.mark_occurrences, "toggle_occ")];
                    for (l, sc, c, id) in items {
                        self.mc(ui, cmds, l, sc, c, id);
                    }
                    ui.separator();
                    self.mi(ui, cmds, "Zoom In", "Ctrl++", Some("zoom_in"), true, "zoom_in");
                    self.mi(ui, cmds, "Zoom Out", "Ctrl+-", Some("zoom_out"), true, "zoom_out");
                    self.mi(ui, cmds, "Reset Zoom", "Ctrl+0", Some("zoom_reset"), true, "zoom_reset");
                    ui.separator();
                    let top = self.s.always_on_top;
                    self.mc(ui, cmds, "Always on Top", "Alt+T", top, "pin");
                });
                ui.menu_button("Appearance", |ui| {
                    self.mi(ui, cmds, "Customize Interface...", "Ctrl+Alt+P", Some("settings"), true, "customize");
                    ui.menu_button(crate::widgets::sub(ui, "Theme"), |ui| {
                        for p in theme::PRESETS {
                            let cur = self.s.theme.preset == *p && self.s.theme.colors.is_empty();
                            if ui.add(crate::widgets::Button::new(*p).checked(cur)).clicked() {
                                self.s.theme.preset = p.to_string();
                                self.s.theme.colors.clear();
                                self.theme_dirty = true;
                                ui.close_menu();
                            }
                        }
                    });
                    ui.separator();
                    self.mi(ui, cmds, "Font...", "", Some("font"), true, "font");
                    self.mi(ui, cmds, "Secondary Font...", "", None, true, "font2");
                    let sec = self.s.use_second_font;
                    self.mc(ui, cmds, "Use Secondary Font", "", sec, "toggle_2nd");
                    ui.separator();
                    ui.menu_button(crate::widgets::sub(ui, "Syntax Scheme"), |ui| self.scheme_menu(ui, cmds));
                    let auto = self.s.auto_detect_scheme;
                    self.mc(ui, cmds, "Auto-detect Scheme", "", auto, "toggle_autoscheme");
                    ui.separator();
                    ui.menu_button(crate::widgets::sub(ui, "Tab Width"), |ui| {
                        for w in [2, 3, 4, 8] {
                            let cur = self.s.tab_width == w;
                            self.mc(ui, cmds, &w.to_string(), "", cur, &format!("tabw:{w}"));
                        }
                    });
                    let sp = self.s.tabs_as_spaces;
                    self.mc(ui, cmds, "Indent with Spaces", "", sp, "toggle_tabspaces");
                });
                ui.menu_button("Settings", |ui| {
                    let s = &self.s;
                    let items = [("Auto Indent", s.auto_indent, "toggle_autoindent"), ("Auto Completion", s.auto_complete, "toggle_autocomplete"),
                        ("Restore Session on Start", s.restore_session, "toggle_session"), ("Keep Unsaved Changes on Exit", s.hot_exit, "toggle_hotexit"),
                        ("Detect External File Changes", s.detect_file_changes, "toggle_detect"), ("Remember Recent Files", s.remember_recent, "toggle_remember"),
                        ("Save Settings on Exit", s.save_on_exit, "toggle_saveonexit")];
                    for (l, c, id) in items {
                        self.mc(ui, cmds, l, "", c, id);
                    }
                    ui.separator();
                    self.mi(ui, cmds, "Save Settings Now", "", Some("settings"), true, "save_settings");
                    self.mi(ui, cmds, "Open Settings Folder", "", None, true, "settings_folder");
                    self.mi(ui, cmds, "Reset Settings...", "", None, true, "reset_settings");
                });
                ui.menu_button("Plugins", |ui| self.plugins_menu(ui, cmds));
                ui.menu_button("Help", |ui| {
                    self.mi(ui, cmds, "Keyboard Shortcuts", "F1", None, true, "shortcuts");
                    ui.separator();
                    self.mi(ui, cmds, "About Notepad9", "", Some("about"), true, "about");
                });
            });
        });
        // separator line under the menu, like the reference
        let r = ctx.available_rect();
        ctx.layer_painter(egui::LayerId::background()).hline(r.left()..=r.right(), r.top() - 0.5, Stroke::new(1.0_f32, line));
    }

    fn encoding_menu(&mut self, ui: &mut egui::Ui, cmds: &mut Cmds) {
        let cur = self.tab().enc;
        ui.add_enabled(false, crate::widgets::Button::new("Save as"));
        for e in ALL_ENCODINGS {
            self.mc(ui, cmds, &textfile::encoding_name(e), "", e == cur, &format!("enc:{}", u8::from(e)));
        }
        ui.separator();
        ui.add_enabled(false, crate::widgets::Button::new("Reload as"));
        let can = self.tab().path.is_some();
        for e in [textfile::FileEncoding::Utf8, textfile::FileEncoding::Ansi, textfile::FileEncoding::Utf16Le,
                  textfile::FileEncoding::Utf16Be, textfile::FileEncoding::Binary] {
            self.mi(ui, cmds, &textfile::encoding_name(e), "", None, can, &format!("reload_as:{}", u8::from(e)));
        }
    }

    fn eol_menu(&mut self, ui: &mut egui::Ui, cmds: &mut Cmds) {
        let cur = self.tab().eol;
        for (e, l) in [(Eol::CrLf, "Windows (CR+LF)"), (Eol::Lf, "Unix (LF)"), (Eol::Cr, "Mac (CR)")] {
            self.mc(ui, cmds, l, "", e == cur, &format!("eol:{}", u8::from(e)));
        }
    }

    fn scheme_menu(&mut self, ui: &mut egui::Ui, cmds: &mut Cmds) {
        let cur = self.tab().hl.syntax().map(|s| s.to_string());
        egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
            self.mc(ui, cmds, "Text Files", "", cur.is_none(), "scheme:");
            ui.separator();
            if crate::highlight::ready().is_none() {
                ui.add_enabled(false, crate::widgets::Button::new("(loading...)"));
                return;
            }
            for n in crate::highlight::all_syntax_names() {
                let c = cur.as_deref() == Some(n.as_str());
                self.mc(ui, cmds, &n, "", c, &format!("scheme:{n}"));
            }
        });
    }

    fn favorites_menu(&mut self, ui: &mut egui::Ui, cmds: &mut Cmds) {
        let favs = self.s.favorites.clone();
        if favs.is_empty() {
            ui.add_enabled(false, crate::widgets::Button::new("(no favorites yet)"));
        }
        for p in favs {
            let name = std::path::Path::new(&p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let r = ui.add_enabled(std::path::Path::new(&p).exists(), crate::widgets::Button::new(format!("★  {name}")).shortcut_text(&p));
            if r.clicked() {
                cmds.push(format!("open:{p}"));
                ui.close_menu();
            }
        }
        ui.separator();
        let has = self.tab().path.is_some();
        self.mi(ui, cmds, "Add Current File", "", Some("star_add"), has, "star_add");
        if let Some(p) = self.tab().path.as_ref().map(|p| p.display().to_string()) {
            if self.s.favorites.contains(&p) && crate::widgets::button(ui, "      Remove Current File").clicked() {
                cmds.push(format!("fav_remove:{p}"));
                ui.close_menu();
            }
        }
        if crate::widgets::button(ui, "      Clear Favorites").clicked() {
            cmds.push("clear_favorites".into());
            ui.close_menu();
        }
    }

    // ===================================================================== toolbar
    fn tool_button(&mut self, ui: &mut egui::Ui, icon: &str, tip: &str, enabled: bool, checked: bool) -> egui::Response {
        let (rect, resp, size) = self.tool_frame(ui, enabled, checked);
        self.icons.paint(ui, icon, rect, size, enabled);
        resp.on_hover_text(tip)
    }

    /// Space and hover / pressed look of a toolbar button; returns (rect, response, icon size).
    pub(crate) fn tool_frame(&self, ui: &mut egui::Ui, enabled: bool, checked: bool) -> (Rect, egui::Response, f32) {
        let size = self.s.theme.toolbar_icon_size.clamp(12.0, 48.0);
        let bs = size + 9.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(bs, bs), if enabled { Sense::click() } else { Sense::hover() });
        let hovered = resp.hovered() && enabled;
        let pressed = resp.is_pointer_button_down_on() && enabled;
        let k = ui.ctx().animate_bool_with_time(resp.id.with("tool_hov"), checked || hovered || pressed, 0.10);
        if k > 0.0 {
            let (fill, stroke) = if pressed {
                (self.pal.c("ui.toolPressed"), theme::darken(self.pal.c("ui.toolPressed"), 0.2))
            } else {
                (self.pal.c("ui.toolHover"), self.pal.c("ui.toolHoverBorder"))
            };
            let r = rect.shrink(1.0);
            ui.painter().rect(r, Rounding::same(6.0), fill.gamma_multiply(k), Stroke::new(1.0_f32, stroke.gamma_multiply(k)));
            if checked {
                // accent marker under a "pressed in" toggle button
                let bar = Rect::from_center_size(egui::pos2(r.center().x, r.bottom() - 2.0), Vec2::new(r.width() * 0.4, 2.5));
                ui.painter().rect_filled(bar, Rounding::same(1.5), self.pal.c("ui.accent"));
            }
        }
        (rect, resp, size)
    }

    pub(crate) fn tool_sep(&self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(7.0, 22.0), Sense::hover());
        let tb = self.pal.c("ui.toolbar");
        let light = theme::luminance(tb) > 0.6;
        let dark = if light { theme::darken(tb, 0.25) } else { theme::darken(tb, 0.5) };
        let lite = if light { theme::lighten(tb, 0.6) } else { theme::lighten(tb, 0.3) };
        let x = rect.center().x.round();
        ui.painter().vline(x - 0.5, rect.y_range(), Stroke::new(1.0_f32, dark));
        ui.painter().vline(x + 0.5, rect.y_range(), Stroke::new(1.0_f32, lite));
    }

    fn toolbar(&mut self, ctx: &egui::Context, cmds: &mut Cmds) {
        let fill = self.pal.c("ui.toolbar");
        egui::TopBottomPanel::top("toolbar").frame(egui::Frame::none().fill(fill).inner_margin(egui::Margin { left: 3.0, right: 3.0, top: 1.0, bottom: 1.0 }))
            .show_separator_line(false).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(1.0, 1.0);
                let t = self.tab();
                let has_text = t.doc.len_chars() > 0;
                let has_sel = !t.doc.sel.is_empty();
                let (modified, can_undo, can_redo, has_path) = (t.modified(), t.doc.can_undo(), t.doc.can_redo(), t.path.is_some());
                let focus = t.view.filter.is_some();
                let fav = t.path.as_ref().map_or(false, |p| self.s.favorites.iter().any(|f| f.eq_ignore_ascii_case(&p.display().to_string())));
                let s = &self.s;
                let (wrap, ruler, pin, console) = (s.word_wrap, s.long_line_marker, s.always_on_top, self.console.visible);
                let items: Vec<Option<(&str, &str, bool, bool, &str)>> = vec![
                    Some(("new", "New Tab (Ctrl+T)", true, false, "new")),
                    Some(("window_new", "New Window (Ctrl+Shift+N)", true, false, "window_new")),
                    None,
                    Some(("open", "Open (Ctrl+O)", true, false, "open")),
                    Some(("save", "Save (Ctrl+S)", modified, false, "save")),
                    Some(("save_as", "Save As (F12)", true, false, "save_as")),
                    Some(("recent", "Recent Files", true, false, "@recent")),
                    None,
                    Some(("undo", "Undo (Ctrl+Z)", can_undo, false, "undo")),
                    Some(("redo", "Redo (Ctrl+Y)", can_redo, false, "redo")),
                    None,
                    Some(("cut", "Cut (Ctrl+X)", has_sel, false, "cut")),
                    Some(("copy", "Copy (Ctrl+C)", has_sel, false, "copy")),
                    Some(("paste", "Paste (Ctrl+V)", true, false, "paste")),
                    None,
                    Some(("find", "Find (Ctrl+F)", has_text, false, "find")),
                    Some(("replace", "Replace (Ctrl+H)", has_text, false, "replace")),
                    None,
                    Some(("find_files", "Find in Files (Ctrl+Shift+F)", true, false, "find_files")),
                    None,
                    Some(("wrap", "Word Wrap (Alt+Z)", true, wrap, "wrap")),
                    None,
                    Some(("fold", "Toggle Folds", false, false, "fold")),
                    Some(("mark", "Focused View: only lines with the selection (Ctrl+Alt+F)", has_text || focus, focus, "focus")),
                    None,
                    Some(("star", "Favorites", true, false, "@fav")),
                    Some(("star_add", "Add to Favorites", has_path && !fav, false, "star_add")),
                    None,
                    Some(("zoom_in", "Zoom In (Ctrl++)", true, false, "zoom_in")),
                    Some(("zoom_reset", "Reset Zoom (Ctrl+0)", true, false, "zoom_reset")),
                    Some(("zoom_out", "Zoom Out (Ctrl+-)", true, false, "zoom_out")),
                    None,
                    Some(("ruler", "Long Line Marker", true, ruler, "ruler")),
                    None,
                    Some(("terminal", "Console (Ctrl+`)", true, console, "console")),
                    Some(("run", "Run current file (F5)", true, false, "run")),
                    None,
                    Some(("pin", "Always on Top (Alt+T)", true, pin, "pin")),
                    None,
                    Some(("exit", "Exit (Ctrl+Q)", true, false, "exit")),
                ];
                for it in items {
                    match it {
                        None => self.tool_sep(ui),
                        Some((icon, tip, en, ch, id)) => {
                            let r = self.tool_button(ui, icon, tip, en, ch);
                            if id == "@recent" || id == "@fav" {
                                let pid = egui::Id::new(id);
                                if r.clicked() {
                                    ui.memory_mut(|m| m.toggle_popup(pid));
                                }
                                egui::popup::popup_below_widget(ui, pid, &r, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                                    ui.set_min_width(260.0);
                                    if id == "@recent" {
                                        let recent = self.s.recent.clone();
                                        if recent.is_empty() {
                                            ui.add_enabled(false, crate::widgets::Button::new("(empty)"));
                                        }
                                        for p in recent {
                                            if crate::widgets::button(ui, &p).clicked() {
                                                cmds.push(format!("open:{p}"));
                                            }
                                        }
                                    } else {
                                        self.favorites_menu(ui, cmds);
                                    }
                                });
                            } else if r.clicked() {
                                cmds.push(id.to_string());
                                cmds.push("@focus_editor".into());
                            }
                        }
                    }
                }
                self.plugins_toolbar(ui, cmds);
            });
        });
        if cmds.iter().any(|c| c == "@focus_editor") {
            cmds.retain(|c| c != "@focus_editor");
            self.focus_editor();
        }
    }

    // ===================================================================== tabs
    fn tab_bar(&mut self, ctx: &egui::Context, cmds: &mut Cmds, bottom: bool) {
        let pal = self.pal.clone();
        let panel = if bottom { egui::TopBottomPanel::bottom("tabs") } else { egui::TopBottomPanel::top("tabs") };
        panel.exact_height(30.0).frame(egui::Frame::none().fill(pal.c("ui.tabBar"))).show_separator_line(false).show(ctx, |ui| {
            let full = ui.max_rect();
            ui.painter().hline(full.x_range(), if bottom { full.top() + 0.5 } else { full.bottom() - 0.5 }, Stroke::new(1.0_f32, pal.c("ui.line")));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let list_w = 30.0;
                let avail = ui.available_width() - list_w;
                egui::ScrollArea::horizontal().id_salt("tab_scroll").max_width(avail).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            let maxw = self.s.theme.tab_max_width.clamp(80.0, 600.0);
                            let font = FontId::proportional(12.5);
                            let n = self.tabs.len();
                            let mut swap: Option<(usize, usize)> = None;
                            for i in 0..n {
                                let (id, title, modified, path) = {
                                    let t = &self.tabs[i];
                                    (t.id, t.title(), t.modified(), t.path.as_ref().map(|p| p.display().to_string()))
                                };
                                let active = i == self.active;
                                let tw = ui.fonts(|f| f.layout_no_wrap(title.clone(), font.clone(), Color32::WHITE).size().x);
                                let close_w = if self.s.theme.tab_close_buttons { 22.0 } else { 6.0 };
                                let w = (tw + 22.0 + close_w).clamp(90.0, maxw);
                                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 30.0), Sense::click_and_drag());
                                let hovered = resp.hovered();
                                let bg = if active { pal.c("ui.tabActive") } else if hovered { pal.c("ui.tabHover") } else { pal.c("ui.tab") };
                                let p = ui.painter();
                                p.rect_filled(rect, Rounding::ZERO, bg);
                                p.vline(rect.right() - 0.5, rect.y_range(), Stroke::new(1.0_f32, pal.c("ui.line")));
                                if active {
                                    let y = if bottom { rect.bottom() - 1.0 } else { rect.top() + 1.0 };
                                    p.hline(rect.left()..=rect.right() - 1.0, y, Stroke::new(2.0_f32, pal.c("ui.tabAccent")));
                                }
                                let text_rect = Rect::from_min_max(Pos2::new(rect.left() + 10.0, rect.top()), Pos2::new(rect.right() - close_w - 2.0, rect.bottom()));
                                let color = if active { pal.c("ui.tabActiveText") } else { pal.c("ui.tabText") };
                                let galley = ui.fonts(|f| {
                                    let mut job = egui::text::LayoutJob::simple_singleline(title.clone(), font.clone(), color);
                                    job.wrap.max_width = text_rect.width();
                                    job.wrap.max_rows = 1;
                                    job.wrap.break_anywhere = true;
                                    job.wrap.overflow_character = Some('…');
                                    f.layout_job(job)
                                });
                                p.galley(Pos2::new(text_rect.left(), rect.center().y - galley.size().y / 2.0), galley, color);
                                // close button / unsaved dot
                                if self.s.theme.tab_close_buttons {
                                    let cr = Rect::from_center_size(Pos2::new(rect.right() - 14.0, rect.center().y), Vec2::splat(18.0));
                                    let cresp = ui.interact(cr, egui::Id::new(("tab_close", id)), Sense::click());
                                    let p = ui.painter();
                                    if cresp.hovered() {
                                        p.rect_filled(cr, Rounding::same(3.0), pal.c("ui.menuHover"));
                                    }
                                    if modified && !cresp.hovered() {
                                        p.circle_filled(cr.center(), 4.0, pal.c("ui.tabText"));
                                    } else if hovered || active || cresp.hovered() {
                                        let c = cr.center();
                                        let s = 3.5;
                                        let st = Stroke::new(1.3_f32, if cresp.hovered() { pal.c("ui.text") } else { pal.c("ui.tabText") });
                                        p.line_segment([c + Vec2::new(-s, -s), c + Vec2::new(s, s)], st);
                                        p.line_segment([c + Vec2::new(s, -s), c + Vec2::new(-s, s)], st);
                                    }
                                    if cresp.clicked() {
                                        cmds.push(format!("close:{id}"));
                                    }
                                } else if modified {
                                    ui.painter().circle_filled(Pos2::new(rect.right() - 8.0, rect.center().y), 3.0, pal.c("ui.tabText"));
                                }
                                if resp.clicked() || resp.drag_started() {
                                    cmds.push(format!("activate:{id}"));
                                }
                                if resp.drag_started() {
                                    self.tab_drag = Some(id);
                                }
                                if resp.middle_clicked() {
                                    cmds.push(format!("close:{id}"));
                                }
                                if let Some(d) = self.tab_drag {
                                    if d != id && ui.input(|i| i.pointer.primary_down()) && ui.rect_contains_pointer(rect) {
                                        if let Some(from) = self.index_of(d) {
                                            swap = Some((from, i));
                                        }
                                    }
                                }
                                let resp = resp.on_hover_text(path.clone().unwrap_or_else(|| format!("{title} (not saved yet)")));
                                resp.context_menu(|ui| {
                                    let items = [("Close", format!("close:{id}")), ("Close Others", format!("close_others:{id}")),
                                                 ("Close Tabs to the Right", format!("close_right:{id}")), ("Close All", "close_all".to_string())];
                                    for (l, c) in items {
                                        if crate::widgets::button(ui, l).clicked() {
                                            cmds.push(c);
                                            ui.close_menu();
                                        }
                                    }
                                    ui.separator();
                                    if let Some(p) = &path {
                                        if crate::widgets::button(ui, "Copy Full Path").clicked() {
                                            cmds.push(format!("copy_path:{p}"));
                                            ui.close_menu();
                                        }
                                    }
                                    if crate::widgets::button(ui, "Copy File Name").clicked() {
                                        cmds.push(format!("copy_path:{title}"));
                                        ui.close_menu();
                                    }
                                    if path.is_some() && crate::widgets::button(ui, "Open Containing Folder").clicked() {
                                        cmds.push(format!("activate:{id}"));
                                        cmds.push("open_folder".into());
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.add_enabled(!self.closed.is_empty(), crate::widgets::Button::new("Reopen Closed Tab")).clicked() {
                                        cmds.push("reopen_tab".into());
                                        ui.close_menu();
                                    }
                                });
                            }
                            if let Some((from, to)) = swap {
                                let t = self.tabs.remove(from);
                                self.tabs.insert(to, t);
                                let active_id = self.tab_drag.unwrap_or(0);
                                if let Some(i) = self.index_of(active_id) {
                                    self.active = i;
                                }
                            }
                            if !ui.input(|i| i.pointer.primary_down()) {
                                self.tab_drag = None;
                            }
                            // "+" new tab
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(30.0, 30.0), Sense::click());
                            if resp.hovered() {
                                ui.painter().rect_filled(r, Rounding::ZERO, pal.c("ui.tabHover"));
                            }
                            let c = r.center();
                            let st = Stroke::new(1.3_f32, pal.c("ui.tabText"));
                            ui.painter().line_segment([c - Vec2::new(5.0, 0.0), c + Vec2::new(5.0, 0.0)], st);
                            ui.painter().line_segment([c - Vec2::new(0.0, 5.0), c + Vec2::new(0.0, 5.0)], st);
                            if resp.on_hover_text("New tab (Ctrl+T)").clicked() {
                                cmds.push("new".into());
                            }
                        });
                    });
                // all tabs list
                let (r, resp) = ui.allocate_exact_size(Vec2::new(list_w, 30.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, Rounding::ZERO, pal.c("ui.tabHover"));
                }
                let c = r.center();
                let st = Stroke::new(1.4_f32, pal.c("ui.tabText"));
                ui.painter().line_segment([c + Vec2::new(-4.0, -2.0), c + Vec2::new(0.0, 2.0)], st);
                ui.painter().line_segment([c + Vec2::new(0.0, 2.0), c + Vec2::new(4.0, -2.0)], st);
                let pid = egui::Id::new("tab_list");
                if resp.on_hover_text("All tabs").clicked() {
                    ui.memory_mut(|m| m.toggle_popup(pid));
                }
                egui::popup::popup_below_widget(ui, pid, &r_resp_dummy(ui, r), egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.set_min_width(240.0);
                    for (i, t) in self.tabs.iter().enumerate() {
                        let label = format!("{}{}{}", if i < 9 { format!("{}  ", i + 1) } else { "    ".into() }, t.title(), if t.modified() { "  ●" } else { "" });
                        if crate::widgets::selectable(ui, i == self.active, label).clicked() {
                            cmds.push(format!("activate:{}", t.id));
                        }
                    }
                    ui.separator();
                    if ui.add_enabled(!self.closed.is_empty(), crate::widgets::Button::new("Reopen Closed Tab")).clicked() {
                        cmds.push("reopen_tab".into());
                    }
                    if crate::widgets::button(ui, "Close All Tabs").clicked() {
                        cmds.push("close_all".into());
                    }
                });
            });
        });
    }

    // ===================================================================== status bar
    fn status_bar(&mut self, ctx: &egui::Context, cmds: &mut Cmds) {
        let fields = self.status_texts(ctx);
        let pal = self.pal.clone();
        let hidden = self.s.theme.hidden_status.clone();
        egui::TopBottomPanel::bottom("status").exact_height(29.0).frame(egui::Frame::none().fill(pal.c("ui.status"))).show_separator_line(false)
            .show(ctx, |ui| {
                let full = ui.max_rect();
                let font = FontId::proportional(12.5);
                let fields: Vec<(&str, String, f32, Option<&str>)> = fields.into_iter().filter(|f| !hidden.iter().any(|h| h == f.0)).collect();
                // right side fields are sized to their text, the rest share the remaining width
                let mut fixed = 0.0;
                let mut stars = 0.0;
                let widths: Vec<f32> = fields.iter().map(|(_, t, star, _)| {
                    if *star > 0.0 {
                        stars += star;
                        0.0
                    } else {
                        let w = ui.fonts(|f| f.layout_no_wrap(t.clone(), font.clone(), Color32::WHITE).size().x) + 9.0;
                        fixed += w;
                        w
                    }
                }).collect();
                let rest = (full.width() - fixed).max(0.0);
                let mut x = full.left();
                for (i, (key, text, star, cmd)) in fields.iter().enumerate() {
                    let w = if *star > 0.0 { rest * star / stars.max(0.01) } else { widths[i] };
                    let r = Rect::from_min_size(Pos2::new(x, full.top()), Vec2::new(w, full.height()));
                    x += w;
                    let resp = ui.interact(r, egui::Id::new(("status", *key)), if cmd.is_some() { Sense::click() } else { Sense::hover() });
                    if cmd.is_some() && resp.hovered() {
                        ui.painter().rect_filled(r.shrink2(Vec2::new(0.0, 1.0)), Rounding::ZERO, theme::blend(pal.c("ui.status"), pal.c("ui.statusText"), 0.12));
                        ctx.set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if i + 1 < fields.len() {
                        ui.painter().vline(r.right() - 0.5, r.top() + 1.0..=r.bottom() - 1.0, Stroke::new(1.0_f32, pal.c("ui.statusSep")));
                    }
                    let tp = ui.painter().with_clip_rect(r.shrink2(Vec2::new(3.0, 0.0)));
                    tp.text(Pos2::new(r.left() + 3.0, r.center().y), Align2::LEFT_CENTER, text, font.clone(), pal.c("ui.statusText"));
                    if let Some(c) = cmd {
                        if resp.clicked() {
                            match *c {
                                "enc_menu" | "eol_menu" | "scheme_menu" => {
                                    self.status_popup = Some(c);
                                    ui.memory_mut(|m| m.open_popup(egui::Id::new(*c)));
                                }
                                _ => cmds.push(c.to_string()),
                            }
                        }
                        if self.status_popup == Some(*c) {
                            egui::popup::popup_above_or_below_widget(ui, egui::Id::new(*c), &resp, egui::AboveOrBelow::Above,
                                egui::PopupCloseBehavior::CloseOnClick, |ui| {
                                    ui.set_min_width(220.0);
                                    match *c {
                                        "enc_menu" => self.encoding_menu(ui, cmds),
                                        "eol_menu" => self.eol_menu(ui, cmds),
                                        _ => self.scheme_menu(ui, cmds),
                                    }
                                });
                        }
                    }
                }
            });
    }

    /// (key, text, star width (0 = fit text), click command)
    fn status_texts(&mut self, ctx: &egui::Context) -> Vec<(&'static str, String, f32, Option<&'static str>)> {
        let now = ctx.input(|i| i.time);
        let i = self.active;
        let info = self.info.as_ref().map(|(t, _)| t.clone());
        let mark = self.s.mark_occurrences;
        let t = &mut self.tabs[i];
        let d = &mut t.doc;
        let caret = d.sel.head;
        let line = d.line_of(caret);
        let col = caret - d.line_start(line);
        let n = |v: usize| {
            let s = v.to_string();
            let mut out = String::new();
            for (k, c) in s.chars().enumerate() {
                if k > 0 && (s.len() - k) % 3 == 0 {
                    out.push(',');
                }
                out.push(c);
            }
            out
        };
        let (sa, sb) = d.sel.range();
        let key = (d.version, t.id, sa, sb);
        if self.status.key != key {
            self.status.key = key;
            let sel = if sb > sa && sb - sa <= 1000 { Some(d.slice(sa, sb)) } else { None };
            self.status.eval = sel.as_ref().filter(|s| s.len() <= 256).and_then(|s| crate::eval::eval(s)).map(crate::eval::format);
            let term = sel.filter(|s| s.chars().count() <= 200 && !s.contains(['\n', '\r']) && !s.trim().is_empty());
            t.view.occ_term = if mark { term.clone() } else { None };
            self.status.occ = match term {
                Some(term) if d.len_chars() < 50_000_000 => {
                    let text = d.text();
                    let start_byte = d.rope.char_to_byte(sa);
                    let mut count = 0;
                    let mut index = 0;
                    for (b, _) in text.match_indices(term.as_str()) {
                        count += 1;
                        if b == start_byte {
                            index = count;
                        }
                        if count >= 100_000 {
                            break;
                        }
                    }
                    format!("Occ  {}/{}", if index > 0 { n(index) } else { "-".into() }, n(count))
                }
                _ => "Occ  -/-".into(),
            };
        }
        let bv = (d.version, t.id);
        if self.status.bytes_version != bv && now - self.status.bytes_time > 0.25 {
            self.status.bytes_version = bv;
            self.status.bytes_time = now;
            let text = d.text();
            self.status.bytes = textfile::byte_count(&text, t.enc, t.eol);
        } else if self.status.bytes_version != bv {
            ctx.request_repaint_after(std::time::Duration::from_millis(260));
        }
        let lines = d.len_lines();
        let line_len = d.line_len(line);
        let sel_lines = if sb > sa { d.line_of(sb) - d.line_of(sa) + 1 } else { 0 };
        let focused_view = t.view.filter.is_some();
        let scheme = t.hl.syntax().unwrap_or("Text Files").to_string();
        vec![
            ("ln", format!("Ln  {} / {}", n(line + 1), n(lines)), 3.0, Some("goto")),
            ("col", format!("Col  {} / {}", n(col + 1), n(line_len)), 2.0, Some("goto")),
            ("info", info.unwrap_or_else(|| if focused_view { "Focused view  (Esc to exit)".into() } else { String::new() }), 2.4, None),
            ("eval", format!("Eval  {}", self.status.eval.clone().unwrap_or("--".into())), 2.0, Some("eval_copy")),
            ("sel", if sb > sa { format!("Sel  {}", n(sb - sa)) } else { "Sel  --".into() }, 2.0, None),
            ("sln", if sb > sa { format!("SLn  {}", n(sel_lines)) } else { "SLn  --".into() }, 2.0, None),
            ("occ", self.status.occ.clone(), 2.0, None),
            ("bytes", if self.status.bytes == 1 { "1 byte".into() } else { format!("{} bytes", n(self.status.bytes)) }, 2.0, None),
            ("enc", textfile::encoding_name(t.enc), 0.0, Some("enc_menu")),
            ("eol", t.eol.name().into(), 0.0, Some("eol_menu")),
            ("ins", if t.doc.overwrite { "OVR".into() } else { "INS".into() }, 0.0, Some("ins")),
            ("std", if self.s.use_second_font { "2ND".into() } else { "STD".into() }, 0.0, Some("toggle_2nd")),
            ("scheme", scheme, 0.0, Some("scheme_menu")),
        ]
    }

    // ===================================================================== console / preview / editor
    fn console_panel(&mut self, ctx: &egui::Context, cmds: &mut Cmds) {
        let pal = self.pal.clone();
        let dir = self.tab().dir();
        let mut act = crate::console::Action::None;
        let r = egui::TopBottomPanel::bottom("console").resizable(true).default_height(self.console.height).height_range(80.0..=900.0)
            .frame(egui::Frame::none().fill(pal.c("ui.console"))).show(ctx, |ui| {
                act = self.console.show(ui, &pal, &mut self.icons, dir.as_deref());
            });
        self.console.height = r.response.rect.height();
        match act {
            crate::console::Action::Run => cmds.push("run".into()),
            crate::console::Action::Close => cmds.push("console".into()),
            crate::console::Action::None => {}
        }
    }

    fn preview_panel(&mut self, ctx: &egui::Context) {
        let pal = self.pal.clone();
        let i = self.active;
        let width = ctx.screen_rect().width() / 2.0;
        egui::SidePanel::right("md_preview_panel").resizable(true).default_width(width).width_range(160.0..=ctx.screen_rect().width() - 200.0)
            .frame(egui::Frame::none().fill(pal.c("ed.bg")).stroke(Stroke::new(1.0_f32, pal.c("ui.line"))))
            .show(ctx, |ui| {
                let t = &mut self.tabs[i];
                let text = t.doc.text();
                self.preview.update(&text, t.doc.version, t.id as usize);
                if self.s.md_sync_scroll {
                    let line = t.view.row_line(t.view.top);
                    self.preview.sync_to(line);
                }
                let base = t.dir();
                self.preview.show(ui, &pal, base.as_deref());
            });
        if let Some(url) = self.preview.open_link.take() {
            if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:") {
                ctx.open_url(egui::OpenUrl::new_tab(url));
            } else if let Some(dir) = self.tab().dir() {
                let p = dir.join(url.split('#').next().unwrap_or("").replace("%20", " "));
                if p.is_file() {
                    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                    if matches!(ext.as_str(), "md" | "markdown" | "txt") {
                        self.open_path(&p);
                    } else {
                        crate::win::shell("open", &p.display().to_string(), None);
                    }
                }
            }
        }
    }

    fn md_bar(&mut self, ui: &mut egui::Ui, cmds: &mut Cmds) {
        let pal = self.pal.clone();
        egui::Frame::none().fill(pal.c("ui.panel")).inner_margin(egui::Margin::symmetric(3.0, 1.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(1.0, 1.0);
                let groups: [&[(&str, &str)]; 4] = [
                    &[("md_bold", "Bold (Ctrl+B)"), ("md_italic", "Italic (Ctrl+I)"), ("md_strike", "Strikethrough"), ("md_heading", "Heading (Ctrl+Shift+H)")],
                    &[("md_code", "Inline code"), ("md_codeblock", "Code block")],
                    &[("md_link", "Link (Ctrl+K)"), ("md_image", "Insert image...")],
                    &[("md_ul", "Bullet list"), ("md_ol", "Numbered list"), ("md_task", "Task list"), ("md_quote", "Quote"), ("md_table", "Table"), ("md_hr", "Horizontal rule")],
                ];
                for (gi, g) in groups.iter().enumerate() {
                    if gi > 0 {
                        self.tool_sep(ui);
                    }
                    for (id, tip) in g.iter() {
                        if self.tool_button(ui, id, tip, true, false).clicked() {
                            cmds.push(id.to_string());
                            cmds.push("@refocus".into());
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let prev = self.s.md_preview;
                    if self.tool_button(ui, "preview", "Show / hide preview (Ctrl+Shift+V)", true, prev).clicked() {
                        cmds.push("md_preview".into());
                    }
                    let sync = self.s.md_sync_scroll;
                    let b = crate::widgets::Button::new(egui::RichText::new("Sync").size(12.0).color(if sync { Color32::BLACK } else { pal.c("ui.text") }))
                        .fill(if sync { pal.c("ui.toolHover") } else { Color32::TRANSPARENT });
                    if ui.add(b).on_hover_text("Scroll the preview together with the text").clicked() {
                        cmds.push("md_sync".into());
                    }
                });
            });
        });
    }

    fn central(&mut self, ctx: &egui::Context, cmds: &mut Cmds, md: bool) {
        let pal = self.pal.clone();
        egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::TRANSPARENT)).show(ctx, |ui| {
            if md {
                self.md_bar(ui, cmds);
            }
            let rect = ui.available_rect_before_wrap();
            ui.painter().rect_filled(rect, Rounding::ZERO, pal.c("ed.bg"));
            if self.s.theme.editor_image_active() {
                if let Some(tex) = self.bg_texture(ctx) {
                    paint_image(ui.painter(), &tex, rect, &self.s.theme);
                }
            }
            let i = self.active;
            let id = self.editor_id();
            let th = &self.s.theme;
            let font_family = egui::FontFamily::Name("editor".into());
            let pointer = match th.pointer.as_str() {
                "Arrow" => CursorIcon::Default,
                "Cross" => CursorIcon::Crosshair,
                "Hand" => CursorIcon::PointingHand,
                "Cell" => CursorIcon::Cell,
                "Move" => CursorIcon::Move,
                _ => CursorIcon::Text,
            };
            let t = &mut self.tabs[i];
            let binary = t.enc == textfile::FileEncoding::Binary;
            let eol = t.eol.as_str();
            let cfg = ViewCfg {
                font: FontId::new((self.s.font_size * self.s.zoom).clamp(4.0, 200.0), font_family),
                line_numbers: self.s.line_numbers,
                num_scale: th.line_number_scale.clamp(0.4, 1.5),
                wrap: self.s.word_wrap,
                show_ws: self.s.show_whitespace,
                show_eol: self.s.show_eol,
                ruler: if self.s.long_line_marker { Some(self.s.long_line_column.max(1)) } else { None },
                cur_line: self.s.highlight_current_line,
                tab_width: self.s.tab_width.clamp(1, 16),
                caret_shape: CaretShape::parse(&th.caret_style),
                caret_width: th.caret_width,
                caret_blink: th.caret_blink,
                blink_ms: th.caret_blink_ms,
                pointer,
                pal: &pal,
                paint_bg: false,
                eol,
                paste_eol: if binary { None } else { Some(eol) },
                auto_indent: self.s.auto_indent,
                indent_unit: if self.s.tabs_as_spaces { " ".repeat(self.s.tab_width.clamp(1, 16)) } else { "\t".into() },
                scroll_size: th.scrollbar_size,
                keyboard: self.modal.is_none(),
            };
            let out = view::show(ui, id, &mut t.doc, &mut t.hl, &mut t.view, &cfg);
            self.editor_focused = out.response.has_focus();
            if out.zoom != 1.0 {
                let z = self.s.zoom * out.zoom;
                self.zoom(z);
            }
            if out.edited {
                self.session_dirty = true;
            }
            if let Some(c) = out.typed {
                self.plugin_event(notepad9_plugin::EVENT_CHAR_TYPED, i, "", c as u32);
            }
            // completion follows typing
            if let Some(c) = out.typed {
                if crate::doc::Doc::is_word(c) {
                    if self.s.auto_complete || self.complete.open {
                        let syn = self.tabs[i].hl.syntax().map(|s| s.to_string());
                        self.complete.update(&mut self.tabs[i].doc, syn.as_deref(), !self.complete.open);
                    }
                } else {
                    self.complete.close();
                }
            } else if self.complete.open {
                let d = &self.tabs[i].doc;
                if d.sel.head < self.complete.start || !d.sel.is_empty() || d.line_of(d.sel.head) != d.line_of(self.complete.start) {
                    self.complete.close();
                } else if out.edited {
                    let syn = self.tabs[i].hl.syntax().map(|s| s.to_string());
                    self.complete.update(&mut self.tabs[i].doc, syn.as_deref(), false);
                }
            }
        });
        if cmds.iter().any(|c| c == "@refocus") {
            cmds.retain(|c| c != "@refocus");
            self.focus_editor();
        }
    }

    fn bg_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        let path = self.s.theme.bg_image.clone();
        if let Some((p, t)) = &self.bg {
            if *p == path {
                return Some(t.clone());
            }
        }
        let img = image::open(&path).ok()?;
        // decode no wider than the screen: big photos stay fast
        let maxw = (ctx.screen_rect().width() * ctx.pixels_per_point() * 1.2) as u32;
        let img = if img.width() > maxw && maxw > 0 { img.resize(maxw, u32::MAX, image::imageops::FilterType::Triangle) } else { img };
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let opts = egui::TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..egui::TextureOptions::LINEAR };
        let tex = ctx.load_texture("background", egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()), opts);
        self.bg = Some((path, tex.clone()));
        Some(tex)
    }
}

/// Paint the background picture in `area` with the chosen size / position / opacity.
fn paint_image(p: &egui::Painter, tex: &egui::TextureHandle, area: Rect, th: &theme::ThemeSettings) {
    let size = tex.size_vec2() / p.ctx().pixels_per_point();
    let tint = Color32::from_white_alpha((th.bg_opacity.clamp(0.0, 1.0) * 255.0) as u8);
    let p = p.with_clip_rect(area);
    let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    let place = |w: f32, h: f32| -> Rect {
        let (x, y) = match th.bg_align.as_str() {
            "TopLeft" => (area.left(), area.top()),
            "TopRight" => (area.right() - w, area.top()),
            "BottomLeft" => (area.left(), area.bottom() - h),
            "BottomRight" => (area.right() - w, area.bottom() - h),
            _ => (area.center().x - w / 2.0, area.center().y - h / 2.0),
        };
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
    };
    match th.bg_stretch.as_str() {
        "Fill" => {
            p.image(tex.id(), area, full_uv, tint);
        }
        "Tile" => {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(area.width() / size.x.max(1.0), area.height() / size.y.max(1.0)));
            p.image(tex.id(), area, uv, tint);
        }
        "None" => {
            p.image(tex.id(), place(size.x, size.y), full_uv, tint);
        }
        "Uniform" => {
            let k = (area.width() / size.x).min(area.height() / size.y);
            p.image(tex.id(), place(size.x * k, size.y * k), full_uv, tint);
        }
        _ => {
            let k = (area.width() / size.x).max(area.height() / size.y);
            p.image(tex.id(), place(size.x * k, size.y * k), full_uv, tint);
        }
    }
}

/// popup_below_widget needs a Response; re-create one for a painted rect.
fn r_resp_dummy(ui: &egui::Ui, r: Rect) -> egui::Response {
    ui.interact(r, egui::Id::new("tab_list_anchor"), Sense::hover())
}
