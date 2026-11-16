//! Notepad9's own widgets: buttons, menu items, check boxes, radio buttons, sliders and
//! selectable rows. They replace the stock egui look everywhere in the app.

use eframe::egui::{
    self, emath::Numeric, pos2, vec2, Color32, Id, Image, Rect, Response, Sense, Shape, Stroke, TextStyle, TextWrapMode, Ui, Vec2,
    Widget, WidgetInfo, WidgetText, WidgetType,
};
use std::ops::RangeInclusive;
use std::sync::RwLock;

/// Colors the widgets are painted with (set from the theme).
#[derive(Clone, Copy)]
pub struct Colors {
    pub accent: Color32,
    pub on_accent: Color32,
    pub field: Color32,
    pub border: Color32,
    pub button: Color32,
    pub hover: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub window: Color32,
    pub light: bool,
}

static COLORS: RwLock<Option<Colors>> = RwLock::new(None);

pub fn set_colors(c: Colors) {
    if let Ok(mut g) = COLORS.write() {
        *g = Some(c);
    }
}

fn colors(ui: &Ui) -> Colors {
    if let Some(c) = COLORS.read().ok().and_then(|c| *c) {
        return c;
    }
    let v = ui.visuals();
    Colors {
        accent: v.selection.bg_fill,
        on_accent: Color32::WHITE,
        field: v.extreme_bg_color,
        border: v.widgets.inactive.bg_stroke.color,
        button: v.widgets.inactive.bg_fill,
        hover: v.widgets.hovered.bg_fill,
        text: v.text_color(),
        dim: v.weak_text_color(),
        window: v.panel_fill,
        light: !v.dark_mode,
    }
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()), f(a.a(), b.a()))
}

fn shade(c: Color32, light: bool, amount: f32) -> Color32 {
    // towards black on light themes, towards white on dark ones
    mix(c, if light { Color32::BLACK } else { Color32::WHITE }, amount)
}

fn anim(ui: &Ui, id: Id, on: bool) -> f32 {
    ui.ctx().animate_bool_with_time(id, on, 0.12)
}

fn focus_ring(ui: &Ui, rect: Rect, rounding: f32, c: &Colors) {
    ui.painter().rect_stroke(rect.expand(2.0), rounding + 2.0, Stroke::new(1.5_f32, c.accent));
}

/// Inside egui menus (popup or menu bar) the stock style makes buttons transparent.
fn in_menu(ui: &Ui) -> bool {
    ui.visuals().widgets.inactive.weak_bg_fill == Color32::TRANSPARENT
}

fn untrim(text: WidgetText) -> WidgetText {
    // menu labels were padded with spaces to line up with icons; the slot does that now
    if let WidgetText::RichText(rt) = &text {
        let s = rt.text();
        if s.starts_with(' ') {
            return WidgetText::from(s.trim_start().to_string());
        }
    }
    text
}

// ---------------------------------------------------------------- button

/// Drop-in for `egui::Button` with the Notepad9 look. In menus it draws a menu row with an
/// icon / check slot and a right-aligned shortcut.
pub struct Button<'a> {
    text: Option<WidgetText>,
    image: Option<Image<'a>>,
    shortcut: Option<WidgetText>,
    frame: Option<bool>,
    fill: Option<Color32>,
    min_size: Vec2,
    small: bool,
    checked: Option<bool>,
}

impl<'a> Button<'a> {
    pub fn new(text: impl Into<WidgetText>) -> Self {
        Self { text: Some(text.into()), image: None, shortcut: None, frame: None, fill: None, min_size: Vec2::ZERO, small: false, checked: None }
    }
    pub fn image(image: impl Into<Image<'a>>) -> Self {
        Self { text: None, image: Some(image.into()), ..Self::new("") }
    }
    pub fn image_and_text(image: impl Into<Image<'a>>, text: impl Into<WidgetText>) -> Self {
        Self { image: Some(image.into()), ..Self::new(text) }
    }
    pub fn frame(mut self, on: bool) -> Self {
        self.frame = Some(on);
        self
    }
    /// Filled ("primary") button; transparent means flat.
    pub fn fill(mut self, c: Color32) -> Self {
        if c == Color32::TRANSPARENT {
            self.frame = Some(false);
        } else {
            self.fill = Some(c);
        }
        self
    }
    pub fn min_size(mut self, s: Vec2) -> Self {
        self.min_size = s;
        self
    }
    pub fn shortcut_text(mut self, t: impl Into<WidgetText>) -> Self {
        self.shortcut = Some(t.into());
        self
    }
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
    /// Menu row with a check mark slot.
    pub fn checked(mut self, on: bool) -> Self {
        self.checked = Some(on);
        self
    }
}

impl Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        if in_menu(ui) && self.frame != Some(true) && self.fill.is_none() && (self.text.as_ref().is_some_and(|t| !t.is_empty())) {
            return menu_row(ui, self);
        }
        let c = colors(ui);
        let flat = self.frame == Some(false);
        let pad = if self.small { vec2(7.0, 1.0) } else if flat { vec2(4.0, 2.0) } else { vec2(12.0, 4.0) };
        let text = self.text.filter(|t| !t.is_empty());
        let galley = text.map(|t| t.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, TextStyle::Button));
        let img_size = self.image.as_ref().map(|i| i.load_and_calc_size(ui, Vec2::splat(16.0)).unwrap_or(Vec2::splat(16.0)));
        let gap = if galley.is_some() && img_size.is_some() { 6.0 } else { 0.0 };
        let content = vec2(
            img_size.map_or(0.0, |s| s.x) + gap + galley.as_ref().map_or(0.0, |g| g.size().x),
            img_size.map_or(0.0, |s| s.y).max(galley.as_ref().map_or(0.0, |g| g.size().y)),
        );
        let min_h = if self.small || flat { 0.0 } else { ui.spacing().interact_size.y };
        let mut size = content + pad * 2.0;
        size.y = size.y.max(min_h);
        if !flat && !self.small && galley.is_some() {
            size.x = size.x.max(64.0);
        }
        size = size.max(self.min_size);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let label = galley.as_ref().map(|g| g.text().to_string()).unwrap_or_default();
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        let enabled = ui.is_enabled();
        let hov = anim(ui, resp.id.with("hov"), resp.hovered() && enabled);
        let pressed = resp.is_pointer_button_down_on() && enabled;
        let r = if self.small { 4.0 } else { 6.0 };
        let p = ui.painter();
        let mut text_color = if enabled { c.text } else { c.dim };
        if flat {
            if hov > 0.0 {
                let f = if pressed { shade(c.hover, c.light, 0.06) } else { c.hover };
                p.rect_filled(rect, r, f.gamma_multiply(hov));
            }
        } else {
            let base = self.fill.unwrap_or(c.button);
            let primary = self.fill.is_some();
            if primary {
                text_color = if enabled { c.on_accent } else { mix(c.on_accent, base, 0.5) };
            }
            let mut fill = mix(base, if primary { shade(base, false, 0.14) } else { c.hover }, hov);
            if pressed {
                fill = shade(fill, true, 0.10);
            }
            if !enabled {
                fill = mix(fill, c.window, 0.45);
            }
            let edge = if primary { shade(base, true, 0.25) } else { mix(c.border, c.accent, hov * 0.55) };
            // darker bottom edge gives the button some depth (gone while pressed)
            if !pressed {
                p.rect_filled(rect.translate(vec2(0.0, 1.0)), r, shade(edge, true, 0.25).gamma_multiply(0.8));
            }
            let body = if pressed { rect.translate(vec2(0.0, 1.0)) } else { rect };
            p.rect(body, r, fill, Stroke::new(1.0_f32, edge));
            // soft top highlight
            let hl = Color32::from_white_alpha(if c.light { 150 } else if primary { 40 } else { 16 });
            p.line_segment([pos2(body.left() + r, body.top() + 1.5), pos2(body.right() - r, body.top() + 1.5)], Stroke::new(1.0_f32, hl));
            if resp.has_focus() {
                focus_ring(ui, rect, r, &c);
            }
        }
        let dy = if pressed && !flat { 1.0 } else { 0.0 };
        let mut x = rect.center().x - content.x / 2.0;
        if let (Some(img), Some(s)) = (self.image, img_size) {
            let ir = Rect::from_min_size(pos2(x, rect.center().y - s.y / 2.0 + dy), s);
            img.paint_at(ui, ir);
            x += s.x + gap;
        }
        if let Some(g) = galley {
            let pos = pos2(x, rect.center().y - g.size().y / 2.0 + dy);
            ui.painter().galley(pos, g, text_color);
        }
        resp
    }
}

const SLOT: f32 = 18.0;

fn menu_row(ui: &mut Ui, b: Button<'_>) -> Response {
    let c = colors(ui);
    let text = untrim(b.text.unwrap_or_else(|| "".into()));
    let galley = text.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, TextStyle::Button);
    let sc = b.shortcut.map(|s| s.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, TextStyle::Button));
    let pad = vec2(8.0, 4.0);
    let w = pad.x + SLOT + 8.0 + galley.size().x + sc.as_ref().map_or(0.0, |s| 28.0 + s.size().x) + pad.x;
    // same height as egui's sub-menu rows, so a menu looks even
    let h = (galley.size().y + 6.0).max(ui.spacing().interact_size.y);
    // menus use a justified layout: the row takes the whole menu width
    let resp = ui.allocate_response(vec2(w, h), Sense::click());
    let rect = resp.rect;
    let label = galley.text().to_string();
    resp.widget_info(|| match b.checked {
        Some(on) => WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), on, &label),
        None => WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label),
    });
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let enabled = ui.is_enabled();
    let hov = anim(ui, resp.id.with("hov"), resp.hovered() && enabled);
    let p = ui.painter();
    if hov > 0.0 {
        let fill = if resp.is_pointer_button_down_on() { shade(c.hover, c.light, 0.06) } else { c.hover };
        p.rect_filled(rect, 5.0, fill.gamma_multiply(hov));
        // accent marker that grows in from the middle
        let bh = (rect.height() - 12.0) * hov;
        let bar = Rect::from_center_size(pos2(rect.left() + 2.5, rect.center().y), vec2(3.0, bh.max(1.0)));
        p.rect_filled(bar, 1.5, c.accent.gamma_multiply(hov));
    }
    let slot = Rect::from_center_size(pos2(rect.left() + pad.x + SLOT / 2.0, rect.center().y), Vec2::splat(SLOT));
    if let Some(img) = b.image {
        let s = img.load_and_calc_size(ui, Vec2::splat(16.0)).unwrap_or(Vec2::splat(16.0));
        img.paint_at(ui, Rect::from_center_size(slot.center(), s));
    }
    if b.checked == Some(true) {
        let k = anim(ui, resp.id.with("chk"), true);
        check_mark(ui, Rect::from_center_size(slot.center(), Vec2::splat(15.0)), c.accent, k, 2.0);
    }
    let col = if enabled { c.text } else { c.dim };
    let tx = slot.right() + 8.0;
    ui.painter().galley(pos2(tx, rect.center().y - galley.size().y / 2.0), galley, col);
    if let Some(s) = sc {
        let pos = pos2(rect.right() - pad.x - s.size().x, rect.center().y - s.size().y / 2.0);
        ui.painter().galley(pos, s, if enabled { c.dim } else { mix(c.dim, c.window, 0.4) });
    }
    resp
}

/// Label for `ui.menu_button` inside a menu: starts where `menu_row` texts start.
pub fn sub(ui: &Ui, label: &str) -> egui::text::LayoutJob {
    let font = TextStyle::Button.resolve(ui.style());
    let lead = (8.0 + SLOT + 8.0 - ui.spacing().button_padding.x).max(0.0);
    let mut job = egui::text::LayoutJob::default();
    job.append(label.trim_start(), lead, egui::TextFormat { font_id: font, color: Color32::PLACEHOLDER, ..Default::default() });
    job
}

/// Like `ui.button(text)`.
pub fn button(ui: &mut Ui, text: impl Into<WidgetText>) -> Response {
    ui.add(Button::new(text))
}

pub fn small_button(ui: &mut Ui, text: impl Into<WidgetText>) -> Response {
    ui.add(Button::new(text).small())
}

fn check_mark(ui: &Ui, b: Rect, color: Color32, k: f32, width: f32) {
    // two strokes; the second one is drawn as the animation goes
    let p0 = b.lerp_inside(vec2(0.18, 0.53));
    let p1 = b.lerp_inside(vec2(0.42, 0.76));
    let p2 = b.lerp_inside(vec2(0.82, 0.28));
    let t1 = (k * 2.0).min(1.0);
    let t2 = ((k - 0.5) * 2.0).clamp(0.0, 1.0);
    let mut pts = vec![p0, p0 + (p1 - p0) * t1];
    if t2 > 0.0 {
        pts.push(p1 + (p2 - p1) * t2);
    }
    ui.painter().add(Shape::line(pts, Stroke::new(width, color)));
}

// ---------------------------------------------------------------- check box / radio

fn labeled(ui: &mut Ui, text: WidgetText, indicator: f32) -> (Rect, Rect, Response, Option<std::sync::Arc<egui::Galley>>) {
    let gap = 8.0;
    let galley = if text.is_empty() {
        None
    } else {
        Some(text.into_galley(ui, None, (ui.available_width() - indicator - gap).max(20.0), TextStyle::Button))
    };
    let w = indicator + galley.as_ref().map_or(0.0, |g| gap + g.size().x);
    let h = ui.spacing().interact_size.y.max(galley.as_ref().map_or(0.0, |g| g.size().y));
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let ind = Rect::from_center_size(pos2(rect.left() + indicator / 2.0, rect.center().y), Vec2::splat(indicator));
    (rect, ind, resp, galley)
}

/// Like `ui.checkbox`: accent-filled box with an animated check mark.
pub fn checkbox(ui: &mut Ui, checked: &mut bool, text: impl Into<WidgetText>) -> Response {
    let c = colors(ui);
    let (rect, b, mut resp, galley) = labeled(ui, text.into(), 18.0);
    if resp.clicked() {
        *checked = !*checked;
        resp.mark_changed();
    }
    let label = galley.as_ref().map(|g| g.text().to_string()).unwrap_or_default();
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), *checked, &label));
    if ui.is_rect_visible(rect) {
        let enabled = ui.is_enabled();
        let on = anim(ui, resp.id, *checked);
        let hov = anim(ui, resp.id.with("hov"), resp.hovered() && enabled);
        let accent = if enabled { mix(c.accent, shade(c.accent, false, 0.15), hov) } else { mix(c.accent, c.window, 0.5) };
        let off_border = mix(mix(c.border, c.text, 0.25), c.accent, hov * 0.6);
        let fill = mix(mix(c.field, c.hover, hov * 0.5), accent, on);
        let stroke = mix(off_border, accent, on);
        let p = ui.painter();
        p.rect(b, 5.0, fill, Stroke::new(1.3_f32, stroke));
        if on > 0.0 {
            check_mark(ui, b.shrink(2.0), c.on_accent, on, 2.2);
        }
        if resp.has_focus() {
            focus_ring(ui, b, 5.0, &c);
        }
        if let Some(g) = galley {
            ui.painter().galley(pos2(b.right() + 8.0, rect.center().y - g.size().y / 2.0), g, if enabled { c.text } else { c.dim });
        }
    }
    resp
}

/// Radio button: accent ring with a dot that pops in.
pub fn radio(ui: &mut Ui, selected: bool, text: impl Into<WidgetText>) -> Response {
    let c = colors(ui);
    let (rect, b, resp, galley) = labeled(ui, text.into(), 18.0);
    let label = galley.as_ref().map(|g| g.text().to_string()).unwrap_or_default();
    resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, ui.is_enabled(), selected, &label));
    if ui.is_rect_visible(rect) {
        let enabled = ui.is_enabled();
        let on = anim(ui, resp.id, selected);
        let hov = anim(ui, resp.id.with("hov"), resp.hovered() && enabled);
        let accent = if enabled { c.accent } else { mix(c.accent, c.window, 0.5) };
        let ctr = b.center();
        let off_border = mix(mix(c.border, c.text, 0.25), c.accent, hov * 0.6);
        let p = ui.painter();
        p.circle(ctr, 9.0, mix(mix(c.field, c.hover, hov * 0.5), accent, on), Stroke::new(1.3_f32, mix(off_border, accent, on)));
        if on > 0.0 {
            let rr = 3.6 * on + if resp.hovered() && selected { 0.8 } else { 0.0 };
            p.circle_filled(ctr, rr, c.on_accent);
        }
        if resp.has_focus() {
            focus_ring(ui, b, 9.0, &c);
        }
        if let Some(g) = galley {
            ui.painter().galley(pos2(b.right() + 8.0, rect.center().y - g.size().y / 2.0), g, if enabled { c.text } else { c.dim });
        }
    }
    resp
}

/// Like `ui.radio_value`.
pub fn radio_value<T: PartialEq>(ui: &mut Ui, current: &mut T, value: T, text: impl Into<WidgetText>) -> Response {
    let sel = *current == value;
    let mut r = radio(ui, sel, text);
    if r.clicked() && !sel {
        *current = value;
        r.mark_changed();
    }
    r
}

// ---------------------------------------------------------------- selectable row

/// Like `egui::SelectableLabel`: rounded row, the selected one tinted with the accent and marked.
pub struct Selectable {
    selected: bool,
    text: WidgetText,
}

impl Selectable {
    pub fn new(selected: bool, text: impl Into<WidgetText>) -> Self {
        Self { selected, text: text.into() }
    }
}

impl Widget for Selectable {
    fn ui(self, ui: &mut Ui) -> Response {
        let c = colors(ui);
        let pad = vec2(9.0, 3.0);
        let galley = self.text.into_galley(ui, None, (ui.available_width() - pad.x * 2.0).max(20.0), TextStyle::Button);
        let size = vec2(galley.size().x + pad.x * 2.0, (galley.size().y + pad.y * 2.0).max(22.0));
        let resp = ui.allocate_response(size, Sense::click());
        let rect = resp.rect;
        let label = galley.text().to_string();
        resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, ui.is_enabled(), self.selected, &label));
        if ui.is_rect_visible(rect) {
            let hov = anim(ui, resp.id.with("hov"), resp.hovered() && ui.is_enabled());
            let on = anim(ui, resp.id.with("sel"), self.selected);
            let p = ui.painter();
            let sel_fill = mix(c.window, c.accent, if c.light { 0.16 } else { 0.28 });
            let fill = mix(c.hover.gamma_multiply(hov), sel_fill, on);
            if fill.a() > 0 {
                p.rect_filled(rect, 5.0, fill);
            }
            if on > 0.0 {
                let bar = Rect::from_center_size(pos2(rect.left() + 2.5, rect.center().y), vec2(3.0, (rect.height() - 8.0) * on));
                p.rect_filled(bar, 1.5, c.accent);
            }
            let pos = pos2(rect.left() + pad.x, rect.center().y - galley.size().y / 2.0);
            ui.painter().galley(pos, galley, c.text);
        }
        resp
    }
}

pub fn selectable(ui: &mut Ui, selected: bool, text: impl Into<WidgetText>) -> Response {
    ui.add(Selectable::new(selected, text))
}

// ---------------------------------------------------------------- slider

/// Like `egui::Slider` (subset): thin track with an accent fill, round thumb with an accent
/// core, value shown in a small chip, label after it. Arrow keys work when focused.
pub struct Slider<'a, N: Numeric> {
    value: &'a mut N,
    range: RangeInclusive<N>,
    step: Option<f64>,
    text: Option<WidgetText>,
    show_value: bool,
}

impl<'a, N: Numeric> Slider<'a, N> {
    pub fn new(value: &'a mut N, range: RangeInclusive<N>) -> Self {
        Self { value, range, step: None, text: None, show_value: true }
    }
    pub fn step_by(mut self, s: f64) -> Self {
        self.step = (s > 0.0).then_some(s);
        self
    }
    pub fn text(mut self, t: impl Into<WidgetText>) -> Self {
        self.text = Some(t.into());
        self
    }
    pub fn show_value(mut self, on: bool) -> Self {
        self.show_value = on;
        self
    }
}

fn decimals(step: Option<f64>, integral: bool, span: f64) -> usize {
    if integral {
        return 0;
    }
    match step {
        Some(s) if s >= 1.0 => 0,
        Some(s) => ((-s.log10()).ceil().max(0.0) as usize).min(4),
        None if span <= 2.0 => 2,
        None if span <= 100.0 => 1,
        None => 0,
    }
}

impl<N: Numeric> Widget for Slider<'_, N> {
    fn ui(self, ui: &mut Ui) -> Response {
        let c = colors(ui);
        let (min, max) = (self.range.start().to_f64(), self.range.end().to_f64());
        let span = (max - min).abs().max(f64::EPSILON);
        let dec = decimals(self.step, N::INTEGRAL, span);
        let snap = |v: f64| {
            let mut v = v.clamp(min.min(max), max.max(min));
            if let Some(s) = self.step {
                v = min + ((v - min) / s).round() * s;
            }
            if N::INTEGRAL {
                v = v.round();
            }
            v.clamp(min.min(max), max.max(min))
        };
        let inner = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let h = ui.spacing().interact_size.y.max(20.0);
            let w = ui.spacing().slider_width.max(60.0);
            let (rect, mut resp) = ui.allocate_exact_size(vec2(w, h), Sense::click_and_drag());
            let thumb = 8.0;
            let (x0, x1) = (rect.left() + thumb, rect.right() - thumb);
            let old = self.value.to_f64();
            let mut v = old;
            if let Some(p) = resp.interact_pointer_pos() {
                if resp.is_pointer_button_down_on() || resp.clicked() || resp.dragged() {
                    let t = ((p.x - x0) / (x1 - x0)).clamp(0.0, 1.0) as f64;
                    v = snap(min + t * (max - min));
                }
            }
            if resp.has_focus() {
                let k = self.step.unwrap_or(if N::INTEGRAL { 1.0 } else { span / 100.0 });
                let (l, r) = ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowDown),
                                           i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowUp)));
                if l {
                    v = snap(v - k);
                }
                if r {
                    v = snap(v + k);
                }
            }
            if v != old {
                *self.value = N::from_f64(v);
                resp.mark_changed();
            }
            let cur = self.value.to_f64();
            resp.widget_info(|| WidgetInfo::slider(ui.is_enabled(), cur, ""));
            if ui.is_rect_visible(rect) {
                let enabled = ui.is_enabled();
                let t = (((cur - min) / (max - min)).clamp(0.0, 1.0)) as f32;
                let tx = x0 + (x1 - x0) * t;
                let cy = rect.center().y;
                let p = ui.painter();
                let accent = if enabled { c.accent } else { mix(c.accent, c.window, 0.55) };
                let track = Rect::from_min_max(pos2(x0, cy - 2.0), pos2(x1, cy + 2.0));
                p.rect_filled(track, 2.0, mix(c.border, c.window, 0.15));
                p.rect_filled(Rect::from_min_max(track.min, pos2(tx, track.max.y)), 2.0, accent);
                let hov = anim(ui, resp.id.with("hov"), (resp.hovered() || resp.dragged()) && enabled);
                let down = ui.ctx().animate_value_with_time(resp.id.with("down"), if resp.is_pointer_button_down_on() { 1.0 } else { 0.0 }, 0.08);
                let thumb_fill = if c.light { Color32::WHITE } else { mix(c.button, Color32::WHITE, 0.10) };
                // soft shadow, body, accent core (bigger on hover, smaller while dragging)
                p.circle_filled(pos2(tx, cy + 1.0), thumb + 0.5, Color32::from_black_alpha(if c.light { 40 } else { 90 }));
                p.circle(pos2(tx, cy), thumb, thumb_fill, Stroke::new(1.0_f32, mix(c.border, c.window, 0.1)));
                let core = 4.2 + 1.2 * hov - 1.4 * down;
                p.circle_filled(pos2(tx, cy), core, accent);
                if resp.has_focus() {
                    focus_ring(ui, Rect::from_center_size(pos2(tx, cy), Vec2::splat(thumb * 2.0)), thumb, &c);
                }
            }
            if self.show_value {
                let fmt = |x: f64| format!("{:.*}", dec, x);
                let font = TextStyle::Button.resolve(ui.style());
                let widest = [fmt(min), fmt(max), fmt(cur)].into_iter().map(|s| {
                    ui.fonts(|f| f.layout_no_wrap(s, font.clone(), c.text).size().x)
                }).fold(0.0_f32, f32::max);
                let chip_h = ui.spacing().interact_size.y - 4.0;
                let (cr, _) = ui.allocate_exact_size(vec2(widest + 14.0, chip_h), Sense::hover());
                if ui.is_rect_visible(cr) {
                    let p = ui.painter();
                    p.rect(cr, 5.0, c.field, Stroke::new(1.0_f32, mix(c.border, c.window, 0.2)));
                    let g = ui.fonts(|f| f.layout_no_wrap(fmt(cur), font.clone(), c.text));
                    p.galley(pos2(cr.center().x - g.size().x / 2.0, cr.center().y - g.size().y / 2.0), g, c.text);
                }
            }
            if let Some(t) = self.text {
                ui.label(t);
            }
            resp
        });
        inner.inner
    }
}
