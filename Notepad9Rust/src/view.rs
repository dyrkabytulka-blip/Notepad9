//! The editor widget: virtualized line rendering, caret, selection, mouse and keyboard.

use crate::doc::{Doc, Sel};
use crate::highlight::{Highlighter, Span};
use crate::theme::Palette;
use eframe::egui::{
    self, text::LayoutJob, Color32, CursorIcon, FontId, Galley, Key, Pos2, Rect, Response, Rounding, Sense, Stroke,
    TextFormat, Ui, Vec2,
};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CaretShape {
    Line,
    Block,
    Underline,
    Box,
}

impl CaretShape {
    pub fn parse(s: &str) -> Self {
        match s {
            "Block" => Self::Block,
            "Underline" => Self::Underline,
            "Box" => Self::Box,
            _ => Self::Line,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    Text,
    Words(usize, usize),
    Lines(usize),
}

#[derive(Default)]
pub struct ViewState {
    pub top: usize,
    pub top_off: f32,
    pub scroll_x: f32,
    max_w: f32,
    /// focused view: the only document lines shown
    pub filter: Option<Vec<usize>>,
    pub reveal: bool,
    pub center: Option<usize>,
    drag: Option<Drag>,
    pub occ_term: Option<String>,
    blink_start: f64,
    pub visible_rows: usize,
    pub caret_rect: Option<Rect>,
    vdrag: Option<(f32, usize)>,
    hdrag: Option<(f32, f32)>,
    last_press: (f64, Pos2, u32),
    pub has_focus: bool,
}

pub struct ViewCfg<'a> {
    pub font: FontId,
    pub line_numbers: bool,
    pub num_scale: f32,
    pub wrap: bool,
    pub show_ws: bool,
    pub show_eol: bool,
    pub ruler: Option<usize>,
    pub cur_line: bool,
    pub tab_width: usize,
    pub caret_shape: CaretShape,
    pub caret_width: f32,
    pub caret_blink: bool,
    pub blink_ms: u32,
    pub pointer: CursorIcon,
    pub pal: &'a Palette,
    pub paint_bg: bool,
    pub eol: &'a str,
    /// line breaks of pasted text are converted to this (None = keep, binary files)
    pub paste_eol: Option<&'a str>,
    pub auto_indent: bool,
    pub indent_unit: String,
    pub scroll_size: f32,
    pub keyboard: bool,
}

pub struct ViewOut {
    pub response: Response,
    pub typed: Option<char>,
    pub zoom: f32,
    pub edited: bool,
}

const LONG: usize = 4000;

struct Lay {
    galley: Arc<Galley>,
    /// doc column → display char (only when tabs were expanded)
    map: Option<Vec<u32>>,
    /// long lines are shown in a window starting at this column (monospace math)
    chunk: Option<usize>,
    x_off: f32,
    h: f32,
    len: usize,
}

impl ViewState {
    pub fn rows(&self, doc: &Doc) -> usize {
        self.filter.as_ref().map_or(doc.len_lines(), |f| f.len().max(1))
    }
    pub fn row_line(&self, r: usize) -> usize {
        match &self.filter {
            Some(f) if !f.is_empty() => f[r.min(f.len() - 1)],
            _ => r,
        }
    }
    pub fn line_row(&self, l: usize) -> usize {
        match &self.filter {
            Some(f) => match f.binary_search(&l) {
                Ok(i) => i,
                Err(i) => i.min(f.len().saturating_sub(1)),
            },
            None => l,
        }
    }
    pub fn restart_blink(&mut self, t: f64) {
        self.blink_start = t;
    }
    pub fn reset_layout(&mut self) {
        self.max_w = 0.0;
    }
}

fn control_picture(c: char) -> char {
    let u = c as u32;
    if u < 0x20 {
        char::from_u32(0x2400 + u).unwrap_or('?')
    } else if u == 0x7F {
        '\u{2421}'
    } else {
        c
    }
}

/// Builds the display text of a line (tabs expanded, control chars made visible) with colors.
fn build_job(text: &str, spans: Option<&[Span]>, cfg: &ViewCfg, chunk: Option<usize>, wrap_w: f32) -> (LayoutJob, Option<Vec<u32>>) {
    let pal = cfg.pal;
    let fg = pal.c("ed.fg");
    let mut job = LayoutJob::default();
    let mut map: Vec<u32> = Vec::new();
    let mut need_map = false;
    let mut disp = String::with_capacity(text.len() + 8);
    let mut disp_n: u32 = 0;
    let mut span_i = 0usize;
    let spans = spans.unwrap_or(&[]);
    let base = chunk.unwrap_or(0);
    let mut cur_color = fg;
    let mut sec_start = 0usize;
    let tw = cfg.tab_width.max(1) as u32;
    for (i, c) in text.chars().enumerate() {
        let col = base + i;
        while span_i < spans.len() && spans[span_i].end <= col {
            span_i += 1;
        }
        let color = if span_i < spans.len() && spans[span_i].start <= col { pal.c(spans[span_i].cat.color_id()) } else { fg };
        if color != cur_color {
            if disp.len() > sec_start {
                job.sections.push(egui::text::LayoutSection {
                    leading_space: 0.0,
                    byte_range: sec_start..disp.len(),
                    format: TextFormat { font_id: cfg.font.clone(), color: cur_color, ..Default::default() },
                });
            }
            sec_start = disp.len();
            cur_color = color;
        }
        map.push(disp_n);
        if c == '\t' {
            if chunk.is_some() {
                disp.push(' ');
                disp_n += 1;
            } else {
                let n = tw - (disp_n % tw);
                for _ in 0..n {
                    disp.push(' ');
                }
                disp_n += n;
                need_map = true;
            }
        } else {
            disp.push(control_picture(c));
            disp_n += 1;
        }
    }
    map.push(disp_n);
    if disp.len() > sec_start || job.sections.is_empty() {
        job.sections.push(egui::text::LayoutSection {
            leading_space: 0.0,
            byte_range: sec_start..disp.len(),
            format: TextFormat { font_id: cfg.font.clone(), color: cur_color, ..Default::default() },
        });
    }
    job.text = disp;
    job.wrap.max_width = if cfg.wrap && chunk.is_none() { wrap_w.max(20.0) } else { f32::INFINITY };
    job.break_on_newline = false;
    (job, if need_map { Some(map) } else { None })
}

struct Geo {
    line_h: f32,
    char_w: f32,
    text: Rect,
    wrap_w: f32,
}

fn layout(ui: &Ui, doc: &Doc, hl: &Highlighter, l: usize, cfg: &ViewCfg, g: &Geo, scroll_x: f32) -> Lay {
    let len = doc.line_len(l);
    let spans = hl.spans(l);
    if len > LONG {
        let start = ((scroll_x / g.char_w) as usize).saturating_sub(200).min(len);
        let end = (start + (g.text.width() / g.char_w) as usize + 400).min(len);
        let ls = doc.line_start(l);
        let text = doc.slice(ls + start, ls + end);
        let (job, _) = build_job(&text, spans, cfg, Some(start), g.wrap_w);
        let galley = ui.fonts(|f| f.layout_job(job));
        return Lay { h: g.line_h, galley, map: None, chunk: Some(start), x_off: start as f32 * g.char_w, len };
    }
    let text = doc.line_string(l);
    let (job, map) = build_job(&text, spans, cfg, None, g.wrap_w);
    let galley = ui.fonts(|f| f.layout_job(job));
    let h = galley.rect.height().max(g.line_h);
    Lay { galley, map, chunk: None, x_off: 0.0, h, len }
}

impl Lay {
    fn disp_of(&self, col: usize) -> usize {
        let col = col.min(self.len);
        if let Some(s) = self.chunk {
            return col.saturating_sub(s).min(self.galley.job.text.chars().count());
        }
        match &self.map {
            Some(m) => m[col.min(m.len() - 1)] as usize,
            None => col,
        }
    }
    fn col_of(&self, d: usize) -> usize {
        if let Some(s) = self.chunk {
            return (s + d).min(self.len);
        }
        match &self.map {
            None => d.min(self.len),
            Some(m) => {
                let i = match m.binary_search(&(d as u32)) {
                    Ok(i) => i,
                    Err(i) => {
                        let i = i.max(1) - 1;
                        // inside an expanded tab: nearest boundary
                        if i + 1 < m.len() && (m[i + 1] as usize - d) < (d - m[i] as usize) { i + 1 } else { i }
                    }
                };
                i.min(self.len)
            }
        }
    }
    /// (row index, x, row top, row height) of a display position, relative to the galley.
    fn pos(&self, d: usize) -> (usize, f32, f32, f32) {
        let rows = &self.galley.rows;
        let mut start = 0usize;
        for (ri, row) in rows.iter().enumerate() {
            let n = row.glyphs.len();
            let last = ri + 1 == rows.len();
            if d < start + n || last {
                let k = d.saturating_sub(start);
                let x = if k < n { row.glyphs[k].pos.x } else { row_end(row) };
                return (ri, x + self.x_off, row.rect.min.y, row.rect.height());
            }
            start += n;
        }
        (0, self.x_off, 0.0, self.h)
    }
    fn row_start(&self, ri: usize) -> usize {
        self.galley.rows.iter().take(ri).map(|r| r.glyphs.len()).sum()
    }
    fn hit_row_x(&self, ri: usize, x: f32) -> usize {
        let rows = &self.galley.rows;
        if rows.is_empty() {
            return 0;
        }
        let ri = ri.min(rows.len() - 1);
        let row = &rows[ri];
        let start = self.row_start(ri);
        let x = x - self.x_off;
        for (i, g) in row.glyphs.iter().enumerate() {
            if x < g.pos.x + g.advance_width / 2.0 {
                return start + i;
            }
        }
        // past the end of a wrapped row: stay before its last char so the caret stays on this row
        if ri + 1 < rows.len() && !row.glyphs.is_empty() { start + row.glyphs.len() - 1 } else { start + row.glyphs.len() }
    }
    fn hit(&self, local: Vec2) -> usize {
        let rows = &self.galley.rows;
        let mut ri = rows.len().saturating_sub(1);
        for (i, r) in rows.iter().enumerate() {
            if local.y < r.rect.max.y {
                ri = i;
                break;
            }
        }
        self.hit_row_x(ri, local.x)
    }
    fn width(&self) -> f32 {
        self.galley.rect.width() + self.x_off
    }
}

fn row_end(row: &egui::epaint::text::Row) -> f32 {
    row.glyphs.last().map_or(row.rect.min.x, |g| g.pos.x + g.advance_width)
}

/// Scroll so `line` is about a third from the top.
pub fn center_on(vs: &mut ViewState, line: usize) {
    let r = vs.line_row(line);
    let back = (vs.visible_rows / 3).max(1);
    vs.top = r.saturating_sub(back);
    vs.top_off = 0.0;
}

pub fn show(ui: &mut Ui, id: egui::Id, doc: &mut Doc, hl: &mut Highlighter, vs: &mut ViewState, cfg: &ViewCfg) -> ViewOut {
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let pal = cfg.pal;
    let mut out = ViewOut { response: resp.clone(), typed: None, zoom: 1.0, edited: false };

    if resp.clicked() || resp.drag_started() || resp.secondary_clicked() {
        resp.request_focus();
    }
    let focused = resp.has_focus();
    if focused {
        ctx.memory_mut(|m| {
            m.set_focus_lock_filter(id, egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: false })
        });
    }
    if focused != vs.has_focus {
        vs.has_focus = focused;
        vs.blink_start = now;
    }

    // ---------------------------------------------------------------- geometry
    let (line_h, char_w) = ui.fonts(|f| (f.row_height(&cfg.font), f.glyph_width(&cfg.font, '0')));
    let num_font = FontId::new((cfg.font.size * cfg.num_scale).max(6.0), egui::FontFamily::Name("editor".into()));
    let digits = doc.len_lines().to_string().len().max(2) as f32;
    let gutter_w = if cfg.line_numbers {
        (ui.fonts(|f| f.glyph_width(&num_font, '9')) * digits + 14.0).max(46.0)
    } else {
        6.0
    };
    let sb = cfg.scroll_size.clamp(4.0, 30.0);
    let text_left = rect.left() + gutter_w + 1.0;
    let mut text_rect = Rect::from_min_max(Pos2::new(text_left, rect.top()), Pos2::new(rect.right() - sb, rect.bottom()));
    let show_hbar = !cfg.wrap && vs.max_w > text_rect.width() - 4.0;
    if show_hbar {
        text_rect.max.y -= sb;
    }
    let geo = Geo { line_h, char_w, text: text_rect, wrap_w: text_rect.width() - 6.0 };
    let view_h = text_rect.height().max(line_h);
    vs.visible_rows = ((view_h / line_h) as usize).max(1);

    // ---------------------------------------------------------------- keyboard
    let mut edited = false;
    if focused && cfg.keyboard {
        let events = ctx.input(|i| i.events.clone());
        let mods = ctx.input(|i| i.modifiers);
        for ev in events {
            match ev {
                egui::Event::Text(t) => {
                    if mods.ctrl && !mods.alt || t.chars().all(|c| c.is_control()) {
                        continue;
                    }
                    let t: String = t.chars().filter(|c| !c.is_control()).collect();
                    if t.is_empty() {
                        continue;
                    }
                    doc.type_text(&t, true);
                    out.typed = t.chars().last();
                    edited = true;
                    vs.reveal = true;
                }
                egui::Event::Ime(egui::ImeEvent::Commit(t)) => {
                    if !t.is_empty() {
                        doc.type_text(&t, true);
                        edited = true;
                        vs.reveal = true;
                    }
                }
                egui::Event::Copy => {
                    let t = copy_text(doc, cfg.eol);
                    if !t.is_empty() {
                        ctx.copy_text(t);
                    }
                }
                egui::Event::Cut => {
                    let t = copy_text(doc, cfg.eol);
                    if !t.is_empty() {
                        ctx.copy_text(t);
                        if doc.sel.is_empty() {
                            let l = doc.line_of(doc.sel.head);
                            let (s, e) = (doc.line_start(l), doc.line_next(l));
                            doc.replace(s, e, "");
                        } else {
                            doc.backspace(false);
                        }
                        edited = true;
                        vs.reveal = true;
                    }
                }
                egui::Event::Paste(t) => {
                    let t = match cfg.paste_eol {
                        Some(nl) => crate::textfile::normalize_eol(&t, eol_of(nl)),
                        None => t,
                    };
                    doc.type_text(&t, false);
                    edited = true;
                    vs.reveal = true;
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    let (ctrl, shift, alt) = (modifiers.ctrl, modifiers.shift, modifiers.alt);
                    let mut moved = true;
                    match key {
                        Key::ArrowLeft if !alt => doc.move_left(ctrl, shift),
                        Key::ArrowRight if !alt => doc.move_right(ctrl, shift),
                        Key::ArrowUp if !alt => {
                            if ctrl { scroll_rows(vs, doc, -1) } else { vertical(ui, doc, hl, vs, cfg, &geo, -1, shift) }
                        }
                        Key::ArrowDown if !alt => {
                            if ctrl { scroll_rows(vs, doc, 1) } else { vertical(ui, doc, hl, vs, cfg, &geo, 1, shift) }
                        }
                        Key::PageUp if !alt => vertical(ui, doc, hl, vs, cfg, &geo, -(vs.visible_rows.max(2) as i32 - 1), shift),
                        Key::PageDown if !alt => vertical(ui, doc, hl, vs, cfg, &geo, vs.visible_rows.max(2) as i32 - 1, shift),
                        Key::Home if !alt => {
                            if ctrl { doc.set_caret(0, shift) } else { doc.home(shift) }
                            doc.preferred_x = None;
                        }
                        Key::End if !alt => {
                            if ctrl { doc.set_caret(doc.len_chars(), shift) } else { doc.end(shift) }
                            doc.preferred_x = None;
                        }
                        Key::Backspace if !alt => {
                            doc.backspace(ctrl);
                            edited = true;
                        }
                        Key::Delete if !alt && !shift => {
                            doc.delete(ctrl);
                            edited = true;
                        }
                        Key::Enter if !ctrl && !alt => {
                            doc.newline(cfg.eol, cfg.auto_indent);
                            edited = true;
                        }
                        Key::Tab if !ctrl && !alt => {
                            doc.tab(shift, &cfg.indent_unit);
                            edited = true;
                        }
                        Key::Insert if !ctrl && !shift && !alt => doc.overwrite = !doc.overwrite,
                        Key::Escape => {
                            if !doc.sel.is_empty() {
                                doc.sel = Sel::caret(doc.sel.head);
                            }
                        }
                        _ => moved = false,
                    }
                    if moved {
                        vs.reveal = true;
                        vs.blink_start = now;
                    }
                }
                _ => {}
            }
        }
    }
    if edited {
        out.edited = true;
        vs.blink_start = now;
        if vs.filter.is_some() {
            vs.filter = None;
        }
    }
    if let Some(l) = doc.changed_from_line.take() {
        hl.invalidate(l);
    }

    // ---------------------------------------------------------------- wheel / zoom
    if resp.hovered() {
        let (delta, zoom) = ctx.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
        if zoom != 1.0 {
            out.zoom = zoom;
        } else {
            if delta.y != 0.0 {
                vs.top_off -= delta.y;
            }
            if delta.x != 0.0 && !cfg.wrap {
                vs.scroll_x = (vs.scroll_x - delta.x).max(0.0);
            }
        }
    }

    let rows = vs.rows(doc);
    if vs.top >= rows {
        vs.top = rows.saturating_sub(1);
        vs.top_off = 0.0;
    }
    if let Some(line) = vs.center.take() {
        center_on(vs, line);
        vs.reveal = true;
    }
    normalize(ui, doc, hl, vs, cfg, &geo);
    if vs.reveal {
        vs.reveal = false;
        reveal(ui, doc, hl, vs, cfg, &geo, view_h);
    }

    // ---------------------------------------------------------------- highlighting (progressive)
    let last_line = vs.row_line((vs.top + vs.visible_rows + 2).min(rows.saturating_sub(1)));
    if !hl.ensure(doc, last_line + 30, 3000) {
        ctx.request_repaint();
    }

    // ---------------------------------------------------------------- layout of visible lines
    let mut vis: Vec<(usize, f32, Lay)> = Vec::new(); // (line, y top, layout)
    let mut y = text_rect.top() - vs.top_off;
    let mut r = vs.top;
    while r < rows && y < text_rect.bottom() {
        let l = vs.row_line(r);
        let lay = layout(ui, doc, hl, l, cfg, &geo, vs.scroll_x);
        let h = lay.h;
        vs.max_w = vs.max_w.max(lay.width() + 20.0);
        vis.push((l, y, lay));
        y += h;
        r += 1;
    }
    let x0 = text_rect.left() + 2.0 - if cfg.wrap { 0.0 } else { vs.scroll_x };

    // ---------------------------------------------------------------- mouse
    let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
    let (pressed, down) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down()));
    let hit = |p: Pos2, vis: &Vec<(usize, f32, Lay)>, doc: &Doc| -> usize {
        if vis.is_empty() {
            return doc.len_chars();
        }
        let mut idx = vis.len() - 1;
        for (i, (_, ly, lay)) in vis.iter().enumerate() {
            if p.y < ly + lay.h {
                idx = i;
                break;
            }
        }
        let (l, ly, lay) = &vis[idx];
        if p.y < vis[0].1 {
            let (l0, ly0, lay0) = &vis[0];
            let d = lay0.hit(Vec2::new(p.x - x0, (p.y - ly0).max(0.0)));
            return doc.line_start(*l0) + lay0.col_of(d);
        }
        let d = lay.hit(Vec2::new(p.x - x0, p.y - ly));
        doc.line_start(*l) + lay.col_of(d)
    };
    if let Some(p) = pointer {
        let in_gutter = p.x < text_left && p.x >= rect.left();
        if pressed && resp.is_pointer_button_down_on() && rect.contains(p) && p.x < rect.right() - sb {
            let shift = ctx.input(|i| i.modifiers.shift);
            let pos = hit(p, &vis, doc);
            let (lt, lp, lc) = vs.last_press;
            let count = if now - lt < 0.45 && lp.distance(p) < 6.0 { (lc % 3) + 1 } else { 1 };
            vs.last_press = (now, p, count);
            if in_gutter {
                let l = doc.line_of(pos);
                let anchor = if shift { doc.line_of(doc.sel.anchor) } else { l };
                doc.select_lines(anchor, l);
                vs.drag = Some(Drag::Lines(anchor));
            } else {
                match count {
                    2 => {
                        doc.select_word_at(pos);
                        let (a, b) = doc.sel.range();
                        vs.drag = Some(Drag::Words(a, b));
                    }
                    3 => {
                        let l = doc.line_of(pos);
                        doc.select_lines(l, l);
                        vs.drag = Some(Drag::Lines(l));
                    }
                    _ => {
                        doc.set_caret(pos, shift);
                        vs.drag = Some(Drag::Text);
                    }
                }
            }
            doc.preferred_x = None;
            vs.blink_start = now;
        } else if down && vs.drag.is_some() {
            let pos = hit(Pos2::new(p.x.max(text_left), p.y), &vis, doc);
            match vs.drag {
                Some(Drag::Text) => doc.sel.head = pos,
                Some(Drag::Words(a, b)) => {
                    let (ws, we) = doc.word_at(pos);
                    if pos < a { doc.sel = Sel { anchor: b, head: ws } } else { doc.sel = Sel { anchor: a, head: we.max(b) } }
                }
                Some(Drag::Lines(anchor)) => doc.select_lines(anchor, doc.line_of(pos)),
                None => {}
            }
            // auto-scroll while dragging outside the text
            if p.y < text_rect.top() {
                scroll_rows(vs, doc, -1);
                ctx.request_repaint();
            } else if p.y > text_rect.bottom() {
                scroll_rows(vs, doc, 1);
                ctx.request_repaint();
            }
            if !cfg.wrap && p.x > text_rect.right() {
                vs.scroll_x += 12.0;
                ctx.request_repaint();
            } else if !cfg.wrap && p.x < text_left && vs.scroll_x > 0.0 {
                vs.scroll_x = (vs.scroll_x - 12.0).max(0.0);
                ctx.request_repaint();
            }
        }
        if resp.hovered() && !in_gutter && p.x < rect.right() - sb && p.y < text_rect.bottom() {
            ctx.set_cursor_icon(cfg.pointer);
        }
    }
    if !down {
        vs.drag = None;
    }

    // ---------------------------------------------------------------- paint
    let painter = ui.painter_at(rect);
    if cfg.paint_bg {
        painter.rect_filled(rect, Rounding::ZERO, pal.c("ed.bg"));
    }
    let gutter = Rect::from_min_max(rect.min, Pos2::new(rect.left() + gutter_w, rect.bottom()));
    painter.rect_filled(gutter, Rounding::ZERO, pal.c("ed.gutter"));
    let tp = painter.with_clip_rect(text_rect);
    let caret = doc.sel.head;
    let caret_line = doc.line_of(caret);
    let (sa, sb_) = doc.sel.range();
    let text_w = text_rect.width();
    let ws_color = pal.c("ed.whitespace");

    for (l, ly, lay) in &vis {
        let ls = doc.line_start(*l);
        let le = ls + lay.len;
        let lnext = doc.line_next(*l);
        // current line
        if cfg.cur_line && *l == caret_line {
            tp.rect_filled(Rect::from_min_size(Pos2::new(text_rect.left(), *ly), Vec2::new(text_w, lay.h)), Rounding::ZERO, pal.c("ed.curLine"));
        }
        // selection
        if sa < lnext && sb_ > ls && sa != sb_ {
            let ca = sa.max(ls) - ls;
            let cb = sb_.min(le).max(sa.max(ls)) - ls;
            paint_range(&tp, lay, ca, cb, x0, *ly, pal.c("ed.selection"));
            if sb_ > le && le < lnext {
                let (_, x, ry, rh) = lay.pos(lay.disp_of(lay.len));
                tp.rect_filled(Rect::from_min_size(Pos2::new(x0 + x, ly + ry), Vec2::new(char_w * 0.6, rh)), Rounding::ZERO, pal.c("ed.selection"));
            }
        }
        // marked occurrences of the selected word
        if let Some(term) = &vs.occ_term {
            if !term.is_empty() && lay.chunk.is_none() {
                let s = doc.line_string(*l);
                let tl = term.chars().count();
                let fill = pal.c("ed.occurrence");
                for (bi, _) in s.match_indices(term.as_str()) {
                    let c = s[..bi].chars().count();
                    if ls + c == sa && sa != sb_ {
                        continue;
                    }
                    paint_range(&tp, lay, c, c + tl, x0, *ly, fill);
                }
            }
        }
        tp.galley(Pos2::new(x0 + lay.x_off, *ly), lay.galley.clone(), pal.c("ed.fg"));
        // whitespace
        if cfg.show_ws && lay.chunk.is_none() {
            let s = doc.line_string(*l);
            for (c, ch) in s.chars().enumerate() {
                if ch == ' ' || ch == '\t' {
                    let d = lay.disp_of(c);
                    let (_, x, ry, rh) = lay.pos(d);
                    let (_, x2, _, _) = lay.pos(d + if ch == '\t' { lay.disp_of(c + 1) - d } else { 1 });
                    let cy = ly + ry + rh / 2.0;
                    if ch == ' ' {
                        tp.circle_filled(Pos2::new(x0 + (x + x2) / 2.0, cy), 1.3, ws_color);
                    } else {
                        let (a, b) = (x0 + x + 2.0, x0 + x2.max(x + char_w) - 3.0);
                        tp.line_segment([Pos2::new(a, cy), Pos2::new(b, cy)], Stroke::new(1.0_f32, ws_color));
                        tp.line_segment([Pos2::new(b - 3.0, cy - 3.0), Pos2::new(b, cy)], Stroke::new(1.0_f32, ws_color));
                        tp.line_segment([Pos2::new(b - 3.0, cy + 3.0), Pos2::new(b, cy)], Stroke::new(1.0_f32, ws_color));
                    }
                }
            }
        }
        if cfg.show_eol && lnext > le {
            let brk = doc.slice(le, lnext);
            let mark = match brk.as_str() { "\r\n" => "CRLF", "\r" => "CR", _ => "LF" };
            let (_, x, ry, rh) = lay.pos(lay.disp_of(lay.len));
            tp.text(Pos2::new(x0 + x + 4.0, ly + ry + rh / 2.0), egui::Align2::LEFT_CENTER, mark,
                    FontId::new(cfg.font.size * 0.6, egui::FontFamily::Proportional), ws_color);
        }
        // line number
        if cfg.line_numbers {
            let n = (l + 1).to_string();
            let color = if *l == caret_line { pal.c("ed.lineNumCur") } else { pal.c("ed.lineNum") };
            let gp = painter.with_clip_rect(gutter);
            let base = ly + lay.galley.rows.first().map_or(line_h * 0.8, |r| r.glyphs.first().map_or(line_h * 0.78, |g| g.pos.y));
            let ascent = ui.fonts(|f| f.row_height(&num_font)) * 0.78;
            gp.text(Pos2::new(rect.left() + 6.0, base - ascent), egui::Align2::LEFT_TOP, n, num_font.clone(), color);
        }
    }

    // long line marker
    if let Some(col) = cfg.ruler {
        let x = x0 + col as f32 * char_w;
        if x > text_rect.left() && x < text_rect.right() {
            tp.line_segment([Pos2::new(x, text_rect.top()), Pos2::new(x, text_rect.bottom())], Stroke::new(1.0_f32, pal.c("ed.ruler")));
        }
    }

    // caret
    vs.caret_rect = None;
    if let Some((_, ly, lay)) = vis.iter().find(|(l, _, _)| *l == caret_line) {
        let col = caret - doc.line_start(caret_line);
        let d = lay.disp_of(col);
        let (_, x, ry, rh) = lay.pos(d);
        let next_x = if col < lay.len { lay.pos(lay.disp_of(col + 1)).1 } else { x + char_w };
        let cw = (next_x - x).max(2.0);
        let r = Rect::from_min_size(Pos2::new((x0 + x).round(), (ly + ry).round()), Vec2::new(cw, rh));
        vs.caret_rect = Some(r);
        if focused {
            let period = (cfg.blink_ms.max(100) as f64) / 1000.0;
            let elapsed = now - vs.blink_start;
            let on = !cfg.caret_blink || ((elapsed / period) as i64) % 2 == 0;
            if cfg.caret_blink {
                let wait = period - (elapsed % period);
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait.max(0.02)));
            }
            if on {
                let c = pal.c("ed.caret");
                let shape = if doc.overwrite { CaretShape::Block } else { cfg.caret_shape };
                let w = cfg.caret_width.clamp(1.0, 12.0);
                match shape {
                    CaretShape::Line => tp.rect_filled(Rect::from_min_size(r.min, Vec2::new(w, r.height())), Rounding::ZERO, c),
                    CaretShape::Block => tp.rect_filled(r, Rounding::ZERO, crate::theme::with_alpha(c, 0.6)),
                    CaretShape::Underline => tp.rect_filled(Rect::from_min_max(Pos2::new(r.left(), r.bottom() - w), r.max), Rounding::ZERO, c),
                    CaretShape::Box => tp.rect_stroke(r.shrink(w.min(3.0) / 2.0), Rounding::ZERO, Stroke::new(w.min(3.0), c)),
                };
            }
            ctx.output_mut(|o| o.ime = Some(egui::output::IMEOutput { rect: text_rect, cursor_rect: r }));
        }
    }

    // ---------------------------------------------------------------- scrollbars
    let track = Rect::from_min_max(Pos2::new(rect.right() - sb, rect.top()), Pos2::new(rect.right(), text_rect.bottom()));
    painter.rect_filled(track, Rounding::ZERO, pal.c("ui.scrollTrack"));
    let total = rows.max(1) as f32;
    let visible = (vs.visible_rows as f32).min(total);
    let thumb_h = (track.height() * (visible / (total + visible - 1.0).max(1.0))).clamp(24.0_f32.min(track.height()), track.height());
    let maxtop = (total - 1.0).max(1.0);
    let frac = ((vs.top as f32 + vs.top_off / line_h) / maxtop).clamp(0.0, 1.0);
    let thumb = Rect::from_min_size(Pos2::new(track.left() + 3.0, track.top() + (track.height() - thumb_h) * frac),
                                    Vec2::new(sb - 6.0, thumb_h));
    let tr = ui.interact(track, id.with("vbar"), Sense::click_and_drag());
    if tr.drag_started() || tr.clicked() {
        if let Some(p) = tr.interact_pointer_pos() {
            if thumb.contains(p) {
                vs.vdrag = Some((p.y, vs.top));
            } else if tr.clicked() || tr.drag_started() {
                let f = ((p.y - track.top() - thumb_h / 2.0) / (track.height() - thumb_h).max(1.0)).clamp(0.0, 1.0);
                vs.top = (f * maxtop) as usize;
                vs.top_off = 0.0;
                vs.vdrag = Some((p.y, vs.top));
            }
        }
    }
    if tr.dragged() {
        if let (Some((y0, t0)), Some(p)) = (vs.vdrag, tr.interact_pointer_pos()) {
            let per_px = maxtop / (track.height() - thumb_h).max(1.0);
            let t = (t0 as f32 + (p.y - y0) * per_px).clamp(0.0, maxtop);
            vs.top = t as usize;
            vs.top_off = 0.0;
        }
    }
    if tr.drag_stopped() {
        vs.vdrag = None;
    }
    let thumb_color = if tr.dragged() {
        crate::theme::lighten(pal.c("ui.scrollThumb"), 0.3)
    } else if tr.hovered() {
        crate::theme::lighten(pal.c("ui.scrollThumb"), 0.15)
    } else {
        pal.c("ui.scrollThumb")
    };
    painter.rect_filled(thumb, Rounding::same(3.0), thumb_color);

    if show_hbar {
        let ht = Rect::from_min_max(Pos2::new(text_left, text_rect.bottom()), Pos2::new(rect.right() - sb, rect.bottom()));
        painter.rect_filled(Rect::from_min_max(Pos2::new(rect.left(), ht.top()), rect.max), Rounding::ZERO, pal.c("ui.scrollTrack"));
        let tw = ht.width();
        let maxx = (vs.max_w - text_w).max(1.0);
        let th_w = (tw * (text_w / vs.max_w)).clamp(24.0_f32.min(tw), tw);
        let fx = (vs.scroll_x / maxx).clamp(0.0, 1.0);
        let hthumb = Rect::from_min_size(Pos2::new(ht.left() + (tw - th_w) * fx, ht.top() + 3.0), Vec2::new(th_w, sb - 6.0));
        let hr = ui.interact(ht, id.with("hbar"), Sense::click_and_drag());
        if hr.drag_started() || hr.clicked() {
            if let Some(p) = hr.interact_pointer_pos() {
                if !hthumb.contains(p) {
                    let f = ((p.x - ht.left() - th_w / 2.0) / (tw - th_w).max(1.0)).clamp(0.0, 1.0);
                    vs.scroll_x = f * maxx;
                }
                vs.hdrag = Some((p.x, vs.scroll_x));
            }
        }
        if hr.dragged() {
            if let (Some((x_start, s0)), Some(p)) = (vs.hdrag, hr.interact_pointer_pos()) {
                vs.scroll_x = (s0 + (p.x - x_start) * maxx / (tw - th_w).max(1.0)).clamp(0.0, maxx);
            }
        }
        if hr.drag_stopped() {
            vs.hdrag = None;
        }
        painter.rect_filled(hthumb, Rounding::same(3.0), pal.c("ui.scrollThumb"));
    } else if cfg.wrap {
        vs.scroll_x = 0.0;
    }
    out
}

fn eol_of(nl: &str) -> crate::textfile::Eol {
    match nl {
        "\n" => crate::textfile::Eol::Lf,
        "\r" => crate::textfile::Eol::Cr,
        _ => crate::textfile::Eol::CrLf,
    }
}

/// Selected text, or the whole line when nothing is selected.
fn copy_text(doc: &Doc, eol: &str) -> String {
    if doc.sel.is_empty() {
        let l = doc.line_of(doc.sel.head);
        let mut s = doc.line_string(l);
        s.push_str(eol);
        s
    } else {
        doc.selected_text()
    }
}

fn paint_range(p: &egui::Painter, lay: &Lay, ca: usize, cb: usize, x0: f32, ly: f32, color: Color32) {
    if cb <= ca {
        return;
    }
    let (da, db) = (lay.disp_of(ca), lay.disp_of(cb));
    let (ra, xa, _, _) = lay.pos(da);
    let (rb, xb, _, _) = lay.pos(db);
    for ri in ra..=rb {
        let row = &lay.galley.rows[ri.min(lay.galley.rows.len() - 1)];
        let left = if ri == ra { xa } else { row.rect.min.x + lay.x_off };
        let right = if ri == rb { xb } else { row_end(row) + lay.x_off };
        if right > left {
            p.rect_filled(Rect::from_min_max(Pos2::new(x0 + left, ly + row.rect.min.y), Pos2::new(x0 + right, ly + row.rect.max.y)),
                          Rounding::ZERO, color);
        }
    }
}

fn line_height(ui: &Ui, doc: &Doc, hl: &Highlighter, vs: &ViewState, cfg: &ViewCfg, g: &Geo, r: usize) -> f32 {
    if !cfg.wrap {
        return g.line_h;
    }
    layout(ui, doc, hl, vs.row_line(r), cfg, g, 0.0).h
}

fn scroll_rows(vs: &mut ViewState, doc: &Doc, d: i32) {
    let rows = vs.rows(doc);
    if d < 0 {
        vs.top = vs.top.saturating_sub((-d) as usize);
    } else {
        vs.top = (vs.top + d as usize).min(rows.saturating_sub(1));
    }
    vs.top_off = 0.0;
}

fn normalize(ui: &Ui, doc: &Doc, hl: &Highlighter, vs: &mut ViewState, cfg: &ViewCfg, g: &Geo) {
    let rows = vs.rows(doc);
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 10_000 {
            break;
        }
        if vs.top_off < 0.0 {
            if vs.top == 0 {
                vs.top_off = 0.0;
                break;
            }
            vs.top -= 1;
            vs.top_off += line_height(ui, doc, hl, vs, cfg, g, vs.top);
        } else {
            let h = line_height(ui, doc, hl, vs, cfg, g, vs.top);
            if vs.top_off >= h && vs.top + 1 < rows {
                vs.top_off -= h;
                vs.top += 1;
            } else {
                if vs.top + 1 >= rows {
                    vs.top_off = vs.top_off.min((h - g.line_h).max(0.0));
                }
                break;
            }
        }
    }
}

/// Scroll so the caret is visible.
fn reveal(ui: &Ui, doc: &Doc, hl: &Highlighter, vs: &mut ViewState, cfg: &ViewCfg, g: &Geo, view_h: f32) {
    let caret = doc.sel.head;
    let line = doc.line_of(caret);
    if let Some(f) = &vs.filter {
        if f.binary_search(&line).is_err() {
            return;
        }
    }
    let rc = vs.line_row(line);
    let lay = layout(ui, doc, hl, line, cfg, g, vs.scroll_x);
    let (_, x, ry, rh) = lay.pos(lay.disp_of(caret - doc.line_start(line)));
    if rc < vs.top || (rc == vs.top && ry < vs.top_off) {
        vs.top = rc;
        vs.top_off = ry;
    } else {
        // distance from the top of the view to the caret row
        let mut y = -vs.top_off;
        let mut r = vs.top;
        let mut far = false;
        while r < rc {
            y += line_height(ui, doc, hl, vs, cfg, g, r);
            r += 1;
            if y > view_h * 3.0 {
                far = true;
                break;
            }
        }
        if far {
            // put the caret near the bottom
            let mut acc = ry + rh;
            let mut t = rc;
            while t > 0 {
                let h = line_height(ui, doc, hl, vs, cfg, g, t - 1);
                if acc + h > view_h {
                    break;
                }
                acc += h;
                t -= 1;
            }
            vs.top = t;
            vs.top_off = 0.0;
        } else {
            let bottom = y + ry + rh;
            if bottom > view_h {
                vs.top_off += bottom - view_h;
                normalize(ui, doc, hl, vs, cfg, g);
            }
        }
    }
    if !cfg.wrap {
        let w = g.text.width();
        if x < vs.scroll_x {
            vs.scroll_x = (x - 40.0).max(0.0);
        } else if x > vs.scroll_x + w - 12.0 {
            vs.scroll_x = x - w + 60.0;
        }
    }
}

/// Up / down by `d` visual rows, keeping the column.
#[allow(clippy::too_many_arguments)]
fn vertical(ui: &Ui, doc: &mut Doc, hl: &Highlighter, vs: &mut ViewState, cfg: &ViewCfg, g: &Geo, d: i32, extend: bool) {
    let head = doc.sel.head;
    let mut line = doc.line_of(head);
    let mut lay = layout(ui, doc, hl, line, cfg, g, vs.scroll_x);
    let (mut ri, x, _, _) = lay.pos(lay.disp_of(head - doc.line_start(line)));
    let want_x = doc.preferred_x.unwrap_or(x);
    let mut steps = d.unsigned_abs();
    let rows = vs.rows(doc);
    let mut at_edge = false;
    while steps > 0 {
        if d < 0 {
            if ri > 0 {
                ri -= 1;
            } else {
                let r = vs.line_row(line);
                if r == 0 {
                    at_edge = true;
                    break;
                }
                line = vs.row_line(r - 1);
                lay = layout(ui, doc, hl, line, cfg, g, vs.scroll_x);
                ri = lay.galley.rows.len().saturating_sub(1);
            }
        } else if ri + 1 < lay.galley.rows.len() {
            ri += 1;
        } else {
            let r = vs.line_row(line);
            if r + 1 >= rows {
                at_edge = true;
                break;
            }
            line = vs.row_line(r + 1);
            lay = layout(ui, doc, hl, line, cfg, g, vs.scroll_x);
            ri = 0;
        }
        steps -= 1;
    }
    let p = if at_edge && steps == d.unsigned_abs() {
        if d < 0 { doc.line_start(line) } else { doc.line_end(line) }
    } else {
        let disp = lay.hit_row_x(ri, want_x);
        doc.line_start(line) + lay.col_of(disp)
    };
    doc.set_caret(p, extend);
    doc.preferred_x = Some(want_x);
}
