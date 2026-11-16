//! Markdown preview rendered with egui widgets (no browser), with scroll sync by source line.

use crate::theme::Palette;
use eframe::egui::{self, Color32, RichText, TextureHandle, Vec2};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Default)]
struct Inline {
    text: String,
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    link: Option<String>,
    hard_break: bool,
}

enum Kind {
    Heading(u8),
    Para,
    Code(String),
    Rule,
    Table(Vec<Vec<Vec<Inline>>>),
    Image(String, String),
}

struct Block {
    kind: Kind,
    inlines: Vec<Inline>,
    indent: usize,
    quote: usize,
    marker: Option<String>,
    line: usize,
}

#[derive(Default)]
pub struct Preview {
    version: Option<(u64, usize)>,
    blocks: Vec<Block>,
    tops: Vec<(usize, f32)>,
    want_line: Option<usize>,
    applied_line: Option<usize>,
    images: HashMap<PathBuf, Option<TextureHandle>>,
    pub open_link: Option<String>,
}

impl Preview {
    pub fn update(&mut self, text: &str, version: u64, tab_id: usize) {
        if self.version == Some((version, tab_id)) {
            return;
        }
        self.version = Some((version, tab_id));
        self.blocks = parse(text);
        self.applied_line = None;
    }

    pub fn sync_to(&mut self, line: usize) {
        self.want_line = Some(line);
    }

    pub fn show(&mut self, ui: &mut egui::Ui, pal: &Palette, base: Option<&Path>) {
        let fg = pal.c("ed.fg");
        let link_c = pal.c("ed.link");
        let code_bg = crate::theme::blend(pal.c("ed.bg"), fg, 0.08);
        let border = crate::theme::blend(pal.c("ed.bg"), fg, 0.18);
        let heading_c = crate::theme::blend(fg, if pal.is_light() { Color32::BLACK } else { Color32::WHITE }, 0.3);
        let dim = crate::theme::blend(fg, pal.c("ed.bg"), 0.35);
        let mut scroll = egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt("md_preview");
        if let Some(l) = self.want_line {
            if self.applied_line != Some(l) && !self.tops.is_empty() {
                let mut y = 0.0;
                for (i, (bl, by)) in self.tops.iter().enumerate() {
                    if *bl <= l {
                        y = *by;
                        if let Some((nl, ny)) = self.tops.get(i + 1) {
                            if *nl > *bl && *nl > l {
                                y += (ny - by) * (l - bl) as f32 / (nl - bl) as f32;
                            }
                        }
                    } else {
                        break;
                    }
                }
                scroll = scroll.vertical_scroll_offset((y - 12.0).max(0.0));
                self.applied_line = Some(l);
            }
        }
        let body = 15.0;
        let mut tops = Vec::with_capacity(self.blocks.len());
        let blocks = std::mem::take(&mut self.blocks);
        scroll.show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(920.0));
            ui.add_space(14.0);
            let origin = ui.min_rect().top();
            for b in &blocks {
                tops.push((b.line, ui.cursor().top() - origin));
                let indent = b.indent as f32 * 22.0 + b.quote as f32 * 16.0;
                ui.horizontal_top(|ui| {
                    ui.add_space(26.0 + indent);
                    if b.quote > 0 {
                        let r = ui.cursor();
                        ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(r.left() - 12.0, r.top()), Vec2::new(4.0, body * 1.5)), 1.0, link_c);
                    }
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width() - 20.0);
                        match &b.kind {
                            Kind::Heading(level) => {
                                let size = match level { 1 => 28.0, 2 => 23.0, 3 => 19.5, 4 => 17.0, _ => 15.5 };
                                ui.add_space(if *level <= 2 { 10.0 } else { 6.0 });
                                self.inlines(ui, &b.inlines, size, heading_c, link_c, code_bg, true);
                                if *level <= 2 {
                                    let r = ui.available_rect_before_wrap();
                                    ui.painter().hline(r.left()..=r.right(), r.top() + 2.0, egui::Stroke::new(1.0_f32, border));
                                    ui.add_space(8.0);
                                }
                            }
                            Kind::Para => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    if let Some(m) = &b.marker {
                                        ui.label(RichText::new(format!("{m}  ")).size(body).color(dim));
                                    }
                                    self.inline_row(ui, &b.inlines, body, if b.quote > 0 { dim } else { fg }, link_c, code_bg, false);
                                });
                            }
                            Kind::Code(text) => {
                                egui::Frame::none().fill(code_bg).stroke(egui::Stroke::new(1.0_f32, border)).rounding(6.0)
                                    .inner_margin(egui::Margin::symmetric(12.0, 10.0)).show(ui, |ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.label(RichText::new(text.trim_end_matches('\n')).monospace().size(13.5).color(fg));
                                    });
                            }
                            Kind::Rule => {
                                ui.add_space(6.0);
                                let r = ui.available_rect_before_wrap();
                                ui.painter().hline(r.left()..=r.right(), r.top(), egui::Stroke::new(1.0_f32, border));
                                ui.add_space(6.0);
                            }
                            Kind::Table(rows) => {
                                egui::Frame::none().stroke(egui::Stroke::new(1.0_f32, border)).inner_margin(6.0).show(ui, |ui| {
                                    egui::Grid::new(("md_table", b.line)).striped(true).spacing(Vec2::new(18.0, 6.0)).show(ui, |ui| {
                                        for (ri, row) in rows.iter().enumerate() {
                                            for cell in row {
                                                ui.horizontal_wrapped(|ui| {
                                                    ui.spacing_mut().item_spacing.x = 0.0;
                                                    self.inline_row(ui, cell, body, if ri == 0 { heading_c } else { fg }, link_c, code_bg, ri == 0);
                                                });
                                            }
                                            ui.end_row();
                                        }
                                    });
                                });
                            }
                            Kind::Image(url, alt) => self.image(ui, base, url, alt, dim),
                        }
                    });
                });
                ui.add_space(6.0);
            }
            ui.add_space(ui.available_height().max(300.0));
        });
        self.blocks = blocks;
        self.tops = tops;
    }

    #[allow(clippy::too_many_arguments)]
    fn inlines(&mut self, ui: &mut egui::Ui, inl: &[Inline], size: f32, color: Color32, link: Color32, code_bg: Color32, bold: bool) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            self.inline_row(ui, inl, size, color, link, code_bg, bold);
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn inline_row(&mut self, ui: &mut egui::Ui, inl: &[Inline], size: f32, color: Color32, link: Color32, code_bg: Color32, bold: bool) {
        for i in inl {
            if i.hard_break {
                ui.end_row();
                continue;
            }
            let mut t = RichText::new(&i.text).size(size).color(color);
            if i.bold || bold {
                t = t.strong();
            }
            if i.italic {
                t = t.italics();
            }
            if i.strike {
                t = t.strikethrough();
            }
            if i.code {
                t = t.monospace().size(size * 0.9).background_color(code_bg);
            }
            if let Some(url) = &i.link {
                let r = ui.add(egui::Link::new(t.color(link))).on_hover_text(url);
                if r.clicked() {
                    self.open_link = Some(url.clone());
                }
            } else {
                ui.label(t);
            }
        }
    }

    fn image(&mut self, ui: &mut egui::Ui, base: Option<&Path>, url: &str, alt: &str, dim: Color32) {
        let local = if url.starts_with("http://") || url.starts_with("https://") {
            None
        } else {
            let p = PathBuf::from(url.replace("%20", " "));
            Some(if p.is_absolute() { p } else { base.map(|b| b.join(&p)).unwrap_or(p) })
        };
        let tex = local.as_ref().and_then(|p| {
            self.images
                .entry(p.clone())
                .or_insert_with(|| {
                    let img = image::open(p).ok()?.to_rgba8();
                    let size = [img.width() as usize, img.height() as usize];
                    Some(ui.ctx().load_texture(p.display().to_string(), egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), Default::default()))
                })
                .clone()
        });
        match tex {
            Some(t) => {
                let s = t.size_vec2();
                let w = s.x.min(ui.available_width());
                ui.image((t.id(), Vec2::new(w, s.y * w / s.x.max(1.0))));
            }
            None => {
                ui.label(RichText::new(format!("🖼 {alt}  ({url})")).color(dim));
            }
        }
    }
}

fn parse(text: &str) -> Vec<Block> {
    let line_starts: Vec<usize> = std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
    let line_of = |b: usize| match line_starts.binary_search(&b) {
        Ok(i) => i,
        Err(i) => i.saturating_sub(1),
    };
    let mut out: Vec<Block> = Vec::new();
    let mut cur: Option<Block> = None;
    let (mut bold, mut italic, mut strike) = (0, 0, 0);
    let mut link: Option<String> = None;
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut quote = 0usize;
    let mut marker: Option<String> = None;
    let mut code: Option<(String, usize)> = None;
    let mut table: Option<(Vec<Vec<Vec<Inline>>>, usize)> = None;
    let mut cell: Option<Vec<Inline>> = None;
    let mut image: Option<(String, String, usize)> = None;

    let new_block = |kind: Kind, line: usize, lists: &Vec<Option<u64>>, quote: usize, marker: &mut Option<String>| Block {
        kind,
        inlines: Vec::new(),
        indent: lists.len().saturating_sub(1),
        quote,
        marker: marker.take(),
        line,
    };

    for (ev, range) in Parser::new_ext(text, Options::all()).into_offset_iter() {
        let line = line_of(range.start);
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                let n = match level {
                    HeadingLevel::H1 => 1, HeadingLevel::H2 => 2, HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4, HeadingLevel::H5 => 5, HeadingLevel::H6 => 6,
                };
                cur = Some(new_block(Kind::Heading(n), line, &lists, quote, &mut marker));
            }
            Event::Start(Tag::Paragraph) => {
                if table.is_none() {
                    cur = Some(new_block(Kind::Para, line, &lists, quote, &mut marker));
                }
            }
            Event::End(TagEnd::Heading(_)) | Event::End(TagEnd::Paragraph) => {
                if let Some(b) = cur.take() {
                    out.push(b);
                }
            }
            Event::Start(Tag::List(start)) => {
                if let Some(b) = cur.take() {
                    out.push(b);
                }
                lists.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                if let Some(b) = cur.take() {
                    out.push(b);
                }
                marker = Some(match lists.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}.");
                        *n += 1;
                        m
                    }
                    _ => ["•", "◦", "▪"][lists.len().saturating_sub(1) % 3].to_string(),
                });
            }
            Event::End(TagEnd::Item) => {
                if let Some(b) = cur.take() {
                    out.push(b);
                }
                marker = None;
            }
            Event::TaskListMarker(done) => {
                marker = Some(if done { "☑".into() } else { "☐".into() });
                if let Some(b) = cur.as_mut() {
                    b.marker = marker.take();
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                if let Some(b) = cur.take() {
                    out.push(b);
                }
                quote += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => quote = quote.saturating_sub(1),
            Event::Start(Tag::CodeBlock(_)) => {
                if let Some(b) = cur.take() {
                    out.push(b);
                }
                code = Some((String::new(), line));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((t, l)) = code.take() {
                    out.push(new_block(Kind::Code(t), l, &lists, quote, &mut marker));
                }
            }
            Event::Start(Tag::Emphasis) => italic += 1,
            Event::End(TagEnd::Emphasis) => italic -= 1,
            Event::Start(Tag::Strong) => bold += 1,
            Event::End(TagEnd::Strong) => bold -= 1,
            Event::Start(Tag::Strikethrough) => strike += 1,
            Event::End(TagEnd::Strikethrough) => strike -= 1,
            Event::Start(Tag::Link { dest_url, .. }) => link = Some(dest_url.to_string()),
            Event::End(TagEnd::Link) => link = None,
            Event::Start(Tag::Image { dest_url, .. }) => image = Some((dest_url.to_string(), String::new(), line)),
            Event::End(TagEnd::Image) => {
                if let Some((url, alt, l)) = image.take() {
                    if let Some(b) = cur.take() {
                        if !b.inlines.is_empty() {
                            out.push(b);
                        }
                    }
                    out.push(new_block(Kind::Image(url, alt), l, &lists, quote, &mut marker));
                    cur = Some(new_block(Kind::Para, l, &lists, quote, &mut None));
                }
            }
            Event::Start(Tag::Table(_)) => {
                if let Some(b) = cur.take() {
                    out.push(b);
                }
                table = Some((Vec::new(), line));
            }
            Event::End(TagEnd::Table) => {
                if let Some((rows, l)) = table.take() {
                    out.push(new_block(Kind::Table(rows), l, &lists, quote, &mut marker));
                }
            }
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => {
                if let Some((rows, _)) = table.as_mut() {
                    rows.push(Vec::new());
                }
            }
            Event::Start(Tag::TableCell) => cell = Some(Vec::new()),
            Event::End(TagEnd::TableCell) => {
                if let (Some(c), Some((rows, _))) = (cell.take(), table.as_mut()) {
                    if let Some(r) = rows.last_mut() {
                        r.push(c);
                    }
                }
            }
            Event::Rule => {
                if let Some(b) = cur.take() {
                    out.push(b);
                }
                out.push(new_block(Kind::Rule, line, &lists, quote, &mut marker));
            }
            Event::Text(t) | Event::Code(t) | Event::Html(t) | Event::InlineHtml(t) => {
                let is_code = matches!(ev_kind(&t, &range, text), true) && false;
                let _ = is_code;
                if let Some((c, _)) = code.as_mut() {
                    c.push_str(&t);
                    continue;
                }
                if let Some((_, alt, _)) = image.as_mut() {
                    alt.push_str(&t);
                    continue;
                }
                let inline_code = text.get(range.clone()).map_or(false, |s| s.starts_with('`'));
                let inl = Inline {
                    text: t.to_string(),
                    bold: bold > 0,
                    italic: italic > 0,
                    strike: strike > 0,
                    code: inline_code,
                    link: link.clone(),
                    hard_break: false,
                };
                if let Some(c) = cell.as_mut() {
                    c.push(inl);
                    continue;
                }
                if cur.is_none() {
                    cur = Some(new_block(Kind::Para, line, &lists, quote, &mut marker));
                }
                cur.as_mut().unwrap().inlines.push(inl);
            }
            Event::SoftBreak => {
                if let Some(b) = cur.as_mut() {
                    b.inlines.push(Inline { text: " ".into(), ..Default::default() });
                }
            }
            Event::HardBreak => {
                if let Some(b) = cur.as_mut() {
                    b.inlines.push(Inline { hard_break: true, ..Default::default() });
                }
            }
            _ => {}
        }
    }
    if let Some(b) = cur.take() {
        out.push(b);
    }
    out
}

fn ev_kind(_t: &str, _r: &std::ops::Range<usize>, _s: &str) -> bool {
    false
}
