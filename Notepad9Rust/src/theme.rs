//! Colors, presets and the egui look (same ids and presets as the C# version).

use eframe::egui::{self, Color32, Rounding, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ThemeSettings {
    pub preset: String,
    /// colors changed by hand on top of the preset (id -> #AARRGGBB)
    pub colors: HashMap<String, String>,
    pub bg_target: String, // Off | Editor | Window
    pub bg_image: String,
    pub bg_opacity: f32,
    pub bg_stretch: String, // Fill | Uniform | UniformToFill | None | Tile
    pub bg_align: String,   // Center | TopLeft | TopRight | BottomLeft | BottomRight
    pub panel_opacity: f32,
    pub caret_style: String, // Line | Block | Underline | Box
    pub caret_width: f32,
    pub caret_blink: bool,
    pub caret_blink_ms: u32,
    pub pointer: String, // IBeam | Arrow | Cross | Hand | Cell | Move
    pub ui_font: String,
    pub ui_font_size: f32,
    pub line_number_scale: f32,
    pub toolbar_icon_size: f32,
    pub scrollbar_size: f32,
    pub corners: String, // Round | Small | Square
    pub tab_bar_mode: String, // Always | Multiple | Never
    pub tab_position: String, // Top | Bottom
    pub tab_close_buttons: bool,
    pub tab_max_width: f32,
    pub hidden_status: Vec<String>,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            preset: DEFAULT_PRESET.into(),
            colors: HashMap::new(),
            bg_target: "Off".into(),
            bg_image: String::new(),
            bg_opacity: 0.25,
            bg_stretch: "UniformToFill".into(),
            bg_align: "Center".into(),
            panel_opacity: 0.7,
            caret_style: "Line".into(),
            caret_width: 1.5,
            caret_blink: true,
            caret_blink_ms: 530,
            pointer: "IBeam".into(),
            ui_font: "Segoe UI".into(),
            ui_font_size: 12.0,
            line_number_scale: 0.75,
            toolbar_icon_size: 16.0,
            scrollbar_size: 13.0,
            corners: "Round".into(),
            tab_bar_mode: "Always".into(),
            tab_position: "Top".into(),
            tab_close_buttons: true,
            tab_max_width: 220.0,
            hidden_status: Vec::new(),
        }
    }
}

impl ThemeSettings {
    pub fn window_image_active(&self) -> bool {
        self.bg_target == "Window" && !self.bg_image.trim().is_empty() && std::path::Path::new(&self.bg_image).exists()
    }
    pub fn editor_image_active(&self) -> bool {
        self.bg_target == "Editor" && !self.bg_image.trim().is_empty() && std::path::Path::new(&self.bg_image).exists()
    }
}

pub const DEFAULT_PRESET: &str = "Notepad9 Dark";
pub const GROUP_UI: &str = "Interface";
pub const GROUP_EDITOR: &str = "Editor";
pub const GROUP_SYNTAX: &str = "Syntax";

pub struct ColorDef {
    pub id: &'static str,
    pub group: &'static str,
    pub label: &'static str,
}

macro_rules! defs {
    ($($id:literal, $g:ident, $l:literal;)*) => { &[$(ColorDef { id: $id, group: $g, label: $l }),*] };
}

pub static DEFS: &[ColorDef] = defs! {
    "ui.window", GROUP_UI, "Window background";
    "ui.title", GROUP_UI, "Title bar";
    "ui.titleText", GROUP_UI, "Title text";
    "ui.border", GROUP_UI, "Window border";
    "ui.menuBar", GROUP_UI, "Menu bar";
    "ui.text", GROUP_UI, "Menu and UI text";
    "ui.textDim", GROUP_UI, "Secondary text";
    "ui.textDisabled", GROUP_UI, "Disabled text";
    "ui.line", GROUP_UI, "Separator lines";
    "ui.toolbar", GROUP_UI, "Toolbar background";
    "ui.toolHover", GROUP_UI, "Toolbar button hover";
    "ui.toolHoverBorder", GROUP_UI, "Toolbar button hover border";
    "ui.toolPressed", GROUP_UI, "Toolbar button pressed";
    "ui.popup", GROUP_UI, "Menu popup background";
    "ui.popupBorder", GROUP_UI, "Menu popup border";
    "ui.menuHover", GROUP_UI, "Menu item highlight";
    "ui.tabBar", GROUP_UI, "Tab bar";
    "ui.tab", GROUP_UI, "Inactive tab";
    "ui.tabHover", GROUP_UI, "Tab under mouse";
    "ui.tabActive", GROUP_UI, "Active tab";
    "ui.tabText", GROUP_UI, "Tab text";
    "ui.tabActiveText", GROUP_UI, "Active tab text";
    "ui.tabAccent", GROUP_UI, "Active tab accent line";
    "ui.panel", GROUP_UI, "Panels (console and Markdown bars)";
    "ui.status", GROUP_UI, "Status bar";
    "ui.statusText", GROUP_UI, "Status bar text";
    "ui.statusSep", GROUP_UI, "Status bar separators";
    "ui.accent", GROUP_UI, "Accent (buttons, checkboxes)";
    "ui.field", GROUP_UI, "Input fields";
    "ui.fieldBorder", GROUP_UI, "Input field border";
    "ui.button", GROUP_UI, "Dialog buttons";
    "ui.scrollTrack", GROUP_UI, "Scrollbar track";
    "ui.scrollThumb", GROUP_UI, "Scrollbar thumb";
    "ui.console", GROUP_UI, "Console background";
    "ui.consoleText", GROUP_UI, "Console text";
    "ed.bg", GROUP_EDITOR, "Text background";
    "ed.fg", GROUP_EDITOR, "Text";
    "ed.curLine", GROUP_EDITOR, "Current line";
    "ed.selection", GROUP_EDITOR, "Selection";
    "ed.caret", GROUP_EDITOR, "Text cursor (caret)";
    "ed.gutter", GROUP_EDITOR, "Line number area";
    "ed.lineNum", GROUP_EDITOR, "Line numbers";
    "ed.lineNumCur", GROUP_EDITOR, "Current line number";
    "ed.whitespace", GROUP_EDITOR, "Whitespace symbols";
    "ed.ruler", GROUP_EDITOR, "Long line marker";
    "ed.occurrence", GROUP_EDITOR, "Marked occurrences";
    "ed.link", GROUP_EDITOR, "Links";
    "ed.fold", GROUP_EDITOR, "Folding markers";
    "syn.comment", GROUP_SYNTAX, "Comments";
    "syn.string", GROUP_SYNTAX, "Strings";
    "syn.keyword", GROUP_SYNTAX, "Keywords";
    "syn.number", GROUP_SYNTAX, "Numbers";
    "syn.type", GROUP_SYNTAX, "Types and classes";
    "syn.method", GROUP_SYNTAX, "Functions and methods";
    "syn.preprocessor", GROUP_SYNTAX, "Preprocessor";
    "syn.tag", GROUP_SYNTAX, "Markup tags";
    "syn.attribute", GROUP_SYNTAX, "Attributes and properties";
    "syn.entity", GROUP_SYNTAX, "Entities and selectors";
    "syn.operator", GROUP_SYNTAX, "Operators and punctuation";
};

const DARK: &[(&str, &str)] = &[
    ("ui.window", "#141414"), ("ui.title", "#141414"), ("ui.titleText", "#B4B4B4"), ("ui.border", "#3A3A3A"),
    ("ui.menuBar", "#141414"), ("ui.text", "#DEDEDE"), ("ui.textDim", "#9A9A9A"), ("ui.textDisabled", "#6A6A6A"),
    ("ui.line", "#414141"), ("ui.toolbar", "#141414"), ("ui.toolHover", "#CCE8FF"), ("ui.toolHoverBorder", "#99D1FF"),
    ("ui.toolPressed", "#99D1FF"), ("ui.popup", "#2B2B2B"), ("ui.popupBorder", "#4F4F4F"), ("ui.menuHover", "#414141"),
    ("ui.tabBar", "#1A1A1A"), ("ui.tab", "#1A1A1A"), ("ui.tabHover", "#242424"), ("ui.tabActive", "#141414"),
    ("ui.tabText", "#8E8E8E"), ("ui.tabActiveText", "#F0F0F0"), ("ui.tabAccent", "#C8A82E"),
    ("ui.panel", "#1B1B1B"), ("ui.status", "#414141"), ("ui.statusText", "#C7C7C7"), ("ui.statusSep", "#696969"),
    ("ui.accent", "#2F6FB5"), ("ui.field", "#1E1E1E"), ("ui.fieldBorder", "#4F4F4F"), ("ui.button", "#2F2F2F"),
    ("ui.scrollTrack", "#1A1A1A"), ("ui.scrollThumb", "#4A4A4A"), ("ui.console", "#101010"), ("ui.consoleText", "#CCCCCC"),
    ("ed.bg", "#141414"), ("ed.fg", "#DEDEDE"), ("ed.curLine", "#404013"), ("ed.selection", "#264F78"),
    ("ed.caret", "#B5B5B5"), ("ed.gutter", "#454545"), ("ed.lineNum", "#A8A8A8"), ("ed.lineNumCur", "#DEDEDE"),
    ("ed.whitespace", "#5E5E5E"), ("ed.ruler", "#3C3C3C"), ("ed.occurrence", "#553E8EC8"), ("ed.link", "#4FA3F7"),
    ("ed.fold", "#A0A0A0"),
    ("syn.comment", "#6A9955"), ("syn.string", "#CE9178"), ("syn.keyword", "#569CD6"), ("syn.number", "#B5CEA8"),
    ("syn.type", "#4EC9B0"), ("syn.method", "#DCDCAA"), ("syn.preprocessor", "#C586C0"), ("syn.tag", "#569CD6"),
    ("syn.attribute", "#9CDCFE"), ("syn.entity", "#D7BA7D"), ("syn.operator", "#D4D4D4"),
];

pub fn parse(s: &str) -> Option<Color32> {
    let h = s.trim().trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).ok()?;
    match h.len() {
        6 => Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)),
        8 => Some(Color32::from_rgba_unmultiplied((v >> 16) as u8, (v >> 8) as u8, v as u8, (v >> 24) as u8)),
        _ => None,
    }
}

pub fn hex(c: Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    if a == 255 { format!("#{r:02X}{g:02X}{b:02X}") } else { format!("#{a:02X}{r:02X}{g:02X}{b:02X}") }
}

pub fn blend(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let [ar, ag, ab, aa] = a.to_srgba_unmultiplied();
    let [br, bg, bb, ba] = b.to_srgba_unmultiplied();
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(m(ar, br), m(ag, bg), m(ab, bb), m(aa, ba))
}

pub fn lighten(c: Color32, t: f32) -> Color32 {
    let a = c.to_srgba_unmultiplied()[3];
    blend(c, Color32::from_rgba_unmultiplied(255, 255, 255, a), t)
}

pub fn darken(c: Color32, t: f32) -> Color32 {
    let a = c.to_srgba_unmultiplied()[3];
    blend(c, Color32::from_rgba_unmultiplied(0, 0, 0, a), t)
}

pub fn luminance(c: Color32) -> f32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0
}

pub fn with_alpha(c: Color32, a: f32) -> Color32 {
    let [r, g, b, ca] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (ca as f32 * a.clamp(0.0, 1.0)) as u8)
}

fn h(s: &str) -> Color32 {
    parse(s).unwrap_or(Color32::from_rgb(255, 0, 255))
}

/// Builds a full palette from a handful of base colors.
#[allow(clippy::too_many_arguments)]
fn make(bg: &str, fg: &str, panel: &str, line: &str, accent: &str, cur_line: &str, selection: &str, gutter: &str,
        status: &str, status_text: &str, tab_accent: &str, light: bool, syn: [&str; 11]) -> HashMap<&'static str, Color32> {
    let (bg, fg, panel, line, accent) = (h(bg), h(fg), h(panel), h(line), h(accent));
    let (gutter, status, status_text) = (h(gutter), h(status), h(status_text));
    let white = Color32::WHITE;
    let black = Color32::BLACK;
    let mut d: HashMap<&'static str, Color32> = HashMap::new();
    let mut s = |k: &'static str, c: Color32| { d.insert(k, c); };
    s("ui.window", bg); s("ui.title", panel); s("ui.titleText", blend(fg, panel, 0.25)); s("ui.border", line);
    s("ui.menuBar", panel); s("ui.text", fg); s("ui.textDim", blend(fg, bg, 0.35)); s("ui.textDisabled", blend(fg, bg, 0.6));
    s("ui.line", line); s("ui.toolbar", panel);
    s("ui.toolHover", blend(accent, if light { white } else { bg }, 0.55));
    s("ui.toolHoverBorder", blend(accent, if light { white } else { bg }, 0.25));
    s("ui.toolPressed", blend(accent, bg, 0.35));
    s("ui.popup", if light { white } else { blend(panel, white, 0.06) }); s("ui.popupBorder", line);
    s("ui.menuHover", if light { blend(accent, white, 0.8) } else { blend(panel, white, 0.14) });
    s("ui.tabBar", blend(panel, black, if light { 0.04 } else { 0.15 }));
    s("ui.tab", blend(panel, black, if light { 0.04 } else { 0.15 }));
    s("ui.tabHover", blend(panel, if light { black } else { white }, 0.06)); s("ui.tabActive", bg);
    s("ui.tabText", blend(fg, bg, 0.4)); s("ui.tabActiveText", fg); s("ui.tabAccent", h(tab_accent));
    s("ui.panel", panel); s("ui.status", status); s("ui.statusText", status_text); s("ui.statusSep", blend(status_text, status, 0.55));
    s("ui.accent", accent); s("ui.field", if light { white } else { blend(bg, white, 0.04) }); s("ui.fieldBorder", line);
    s("ui.button", if light { h("#F2F2F2") } else { blend(panel, white, 0.1) });
    s("ui.scrollTrack", blend(bg, if light { black } else { white }, 0.03)); s("ui.scrollThumb", blend(bg, fg, 0.25));
    s("ui.console", blend(bg, black, if light { 0.02 } else { 0.25 })); s("ui.consoleText", blend(fg, bg, 0.1));
    s("ed.bg", bg); s("ed.fg", fg); s("ed.curLine", h(cur_line)); s("ed.selection", h(selection));
    s("ed.caret", fg); s("ed.gutter", gutter); s("ed.lineNum", blend(fg, gutter, 0.4)); s("ed.lineNumCur", fg);
    s("ed.whitespace", blend(fg, bg, 0.7)); s("ed.ruler", blend(fg, bg, 0.82));
    s("ed.occurrence", with_alpha(accent, 0.33)); s("ed.link", accent); s("ed.fold", blend(fg, gutter, 0.35));
    let ids = ["syn.comment", "syn.string", "syn.keyword", "syn.number", "syn.type", "syn.method", "syn.preprocessor",
               "syn.tag", "syn.attribute", "syn.entity", "syn.operator"];
    for (i, id) in ids.iter().enumerate() { s(id, h(syn[i])); }
    d
}

pub const PRESETS: &[&str] = &[
    DEFAULT_PRESET, "Classic Light", "Midnight Blue", "Monokai", "Solarized Dark", "Nord", "Dracula", "Gruvbox", "High Contrast",
];

fn preset(name: &str) -> HashMap<&'static str, Color32> {
    match name {
        "Classic Light" => make("#FFFFFF", "#1E1E1E", "#F0F0F0", "#C8C8C8", "#0067C0", "#FFFFD2", "#ADD6FF", "#EDEDED",
            "#F0F0F0", "#333333", "#0067C0", true,
            ["#008000", "#A31515", "#0000FF", "#098658", "#267F99", "#795E26", "#AF00DB", "#800000", "#E50000", "#811F3F", "#333333"]),
        "Midnight Blue" => make("#0F1626", "#D6DEEB", "#111B2E", "#24324D", "#4A8FE7", "#16233A", "#1D3B66", "#131E33",
            "#0B1220", "#9FB2D0", "#4A8FE7", false,
            ["#637777", "#ECC48D", "#C792EA", "#F78C6C", "#FFCB8B", "#82AAFF", "#C792EA", "#7FDBCA", "#ADDB67", "#FFCB8B", "#7FDBCA"]),
        "Monokai" => make("#272822", "#F8F8F2", "#1E1F1C", "#3E3D32", "#A6E22E", "#3E3D32", "#49483E", "#2F302A",
            "#1E1F1C", "#CFCFC2", "#F92672", false,
            ["#75715E", "#E6DB74", "#F92672", "#AE81FF", "#66D9EF", "#A6E22E", "#F92672", "#F92672", "#A6E22E", "#FD971F", "#F8F8F2"]),
        "Solarized Dark" => make("#002B36", "#93A1A1", "#073642", "#0E4552", "#268BD2", "#073642", "#0F4B5A", "#05313D",
            "#073642", "#93A1A1", "#B58900", false,
            ["#586E75", "#2AA198", "#859900", "#D33682", "#B58900", "#268BD2", "#CB4B16", "#268BD2", "#93A1A1", "#CB4B16", "#839496"]),
        "Nord" => make("#2E3440", "#D8DEE9", "#3B4252", "#4C566A", "#88C0D0", "#3B4252", "#434C5E", "#353C4A",
            "#3B4252", "#D8DEE9", "#88C0D0", false,
            ["#616E88", "#A3BE8C", "#81A1C1", "#B48EAD", "#8FBCBB", "#88C0D0", "#5E81AC", "#81A1C1", "#8FBCBB", "#D08770", "#ECEFF4"]),
        "Dracula" => make("#282A36", "#F8F8F2", "#21222C", "#44475A", "#BD93F9", "#343746", "#44475A", "#2B2D3A",
            "#191A21", "#F8F8F2", "#FF79C6", false,
            ["#6272A4", "#F1FA8C", "#FF79C6", "#BD93F9", "#8BE9FD", "#50FA7B", "#FF79C6", "#FF79C6", "#50FA7B", "#FFB86C", "#F8F8F2"]),
        "Gruvbox" => make("#282828", "#EBDBB2", "#1D2021", "#3C3836", "#D79921", "#32302F", "#504945", "#32302F",
            "#3C3836", "#D5C4A1", "#FE8019", false,
            ["#928374", "#B8BB26", "#FB4934", "#D3869B", "#FABD2F", "#8EC07C", "#FE8019", "#83A598", "#8EC07C", "#FE8019", "#EBDBB2"]),
        "High Contrast" => make("#000000", "#FFFFFF", "#000000", "#FFFFFF", "#1AEBFF", "#202020", "#1A4B8C", "#000000",
            "#000000", "#FFFFFF", "#FFFF00", false,
            ["#7CFC00", "#FFD700", "#00FFFF", "#FF80FF", "#80FF80", "#FFFF80", "#FF8080", "#00FFFF", "#FFC0CB", "#FFA500", "#FFFFFF"]),
        _ => DARK.iter().map(|(k, v)| (*k, h(v))).collect(),
    }
}

pub fn preset_color(name: &str, id: &str) -> Color32 {
    preset(name).get(id).copied().unwrap_or(Color32::from_rgb(255, 0, 255))
}

/// Resolved colors: default dark → preset → user overrides.
#[derive(Clone)]
pub struct Palette {
    map: HashMap<String, Color32>,
}

impl Palette {
    pub fn resolve(t: &ThemeSettings) -> Self {
        let mut map: HashMap<String, Color32> = DARK.iter().map(|(k, v)| (k.to_string(), h(v))).collect();
        for (k, v) in preset(&t.preset) { map.insert(k.to_string(), v); }
        for (k, v) in &t.colors {
            if let Some(c) = parse(v) { map.insert(k.clone(), c); }
        }
        // a whole-window picture shows through the panels
        if t.window_image_active() {
            for k in ["ui.menuBar", "ui.toolbar", "ui.tabBar", "ui.tab", "ui.tabActive", "ui.tabHover", "ui.status",
                      "ui.panel", "ed.bg", "ed.gutter", "ui.console"] {
                if let Some(c) = map.get_mut(k) { *c = with_alpha(*c, t.panel_opacity); }
            }
        }
        Self { map }
    }

    pub fn c(&self, id: &str) -> Color32 {
        self.map.get(id).copied().unwrap_or(Color32::from_rgb(255, 0, 255))
    }

    pub fn is_light(&self) -> bool {
        luminance(self.c("ed.bg")) > 0.6
    }
}

/// egui visuals built from the palette (menus, buttons, windows, scrollbars ...).
pub fn apply(ctx: &egui::Context, pal: &Palette, t: &ThemeSettings) {
    let light = luminance(pal.c("ui.window")) > 0.6;
    let mut v = if light { egui::Visuals::light() } else { egui::Visuals::dark() };
    let text = pal.c("ui.text");
    let r = Rounding::same(5.0);
    v.override_text_color = Some(text);
    v.panel_fill = pal.c("ui.window");
    v.window_fill = pal.c("ui.popup");
    v.window_stroke = Stroke::new(1.0_f32, pal.c("ui.popupBorder"));
    v.window_rounding = Rounding::same(10.0);
    v.menu_rounding = Rounding::same(8.0);
    v.extreme_bg_color = pal.c("ui.field");
    v.faint_bg_color = blend(pal.c("ui.popup"), text, 0.04);
    v.code_bg_color = blend(pal.c("ed.bg"), pal.c("ed.fg"), 0.08);
    v.hyperlink_color = pal.c("ed.link");
    v.selection.bg_fill = pal.c("ed.selection");
    // focused text fields get an accent outline (selected rows are drawn by crate::widgets)
    v.selection.stroke = Stroke::new(1.5_f32, pal.c("ui.accent"));
    v.text_cursor.stroke = Stroke::new(2.0_f32, pal.c("ui.accent"));
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    v.collapsing_header_frame = false;
    v.indent_has_left_vline = true;
    v.popup_shadow = egui::epaint::Shadow { offset: Vec2::new(0.0, 6.0), blur: 18.0, spread: 0.0, color: Color32::from_black_alpha(if light { 50 } else { 120 }) };
    v.window_shadow = egui::epaint::Shadow { offset: Vec2::new(0.0, 10.0), blur: 32.0, spread: 0.0, color: Color32::from_black_alpha(if light { 60 } else { 140 }) };

    let btn = pal.c("ui.button");
    let border = pal.c("ui.fieldBorder");
    let hover = pal.c("ui.menuHover");
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = pal.c("ui.popup");
    w.noninteractive.weak_bg_fill = pal.c("ui.popup");
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, pal.c("ui.line"));
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, text);
    w.noninteractive.rounding = r;
    w.inactive.bg_fill = btn;
    w.inactive.weak_bg_fill = btn;
    w.inactive.bg_stroke = Stroke::new(1.0_f32, border);
    w.inactive.fg_stroke = Stroke::new(1.0_f32, text);
    w.inactive.rounding = r;
    w.hovered.bg_fill = hover;
    w.hovered.weak_bg_fill = hover;
    w.hovered.bg_stroke = Stroke::new(1.0_f32, blend(border, text, 0.3));
    w.hovered.fg_stroke = Stroke::new(1.5_f32, text);
    w.hovered.rounding = r;
    w.hovered.expansion = 0.0;
    w.active.bg_fill = darken(hover, 0.15);
    w.active.weak_bg_fill = darken(hover, 0.15);
    w.active.bg_stroke = Stroke::new(1.0_f32, pal.c("ui.accent"));
    w.active.fg_stroke = Stroke::new(1.5_f32, text);
    w.active.rounding = r;
    w.active.expansion = 0.0;
    w.open.bg_fill = hover;
    w.open.weak_bg_fill = hover;
    w.open.bg_stroke = Stroke::new(1.0_f32, border);
    w.open.fg_stroke = Stroke::new(1.0_f32, text);
    w.open.rounding = r;

    let accent = pal.c("ui.accent");
    crate::widgets::set_colors(crate::widgets::Colors {
        accent,
        on_accent: if luminance(accent) > 0.6 { Color32::from_rgb(20, 20, 20) } else { Color32::WHITE },
        field: pal.c("ui.field"),
        border,
        button: btn,
        hover,
        text,
        dim: pal.c("ui.textDim"),
        window: pal.c("ui.window"),
        light,
    });

    let mut style = (*ctx.style()).clone();
    style.visuals = v;
    let size = t.ui_font_size.clamp(8.0, 24.0) + 0.5; // egui points ≈ WPF device-independent pixels
    use egui::{FontFamily, FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Small, FontId::new(size * 0.85, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(size, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(size, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(size * 1.6, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(size, FontFamily::Monospace)),
    ]
    .into();
    style.spacing.item_spacing = Vec2::new(6.0, 4.0);
    style.spacing.button_padding = Vec2::new(10.0, 4.0);
    style.spacing.menu_margin = egui::Margin::same(6.0);
    style.spacing.slider_width = 180.0;
    style.spacing.icon_width = 18.0;
    style.spacing.icon_spacing = 8.0;
    style.spacing.combo_height = 300.0;
    style.spacing.window_margin = egui::Margin::same(12.0);
    style.spacing.interact_size.y = size + 8.0;
    style.spacing.scroll.bar_width = (t.scrollbar_size - 3.0).clamp(4.0, 24.0);
    style.spacing.scroll.floating = false;
    style.spacing.scroll.bar_inner_margin = 2.0;
    style.spacing.scroll.foreground_color = false;
    style.interaction.tooltip_delay = 0.4;
    ctx.set_style(style);
}
