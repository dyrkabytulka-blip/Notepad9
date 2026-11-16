//! Markdown editing helpers: formatting buttons, list continuation on Enter, list indent on Tab.

use crate::doc::{Doc, Sel};
use regex::Regex;
use std::sync::OnceLock;

fn list_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(?P<indent>[ \t]*)(?P<marker>[-*+]|\d+[.)])(?P<space>[ \t]+)(?P<task>\[[ xX]\][ \t]+)?").unwrap())
}

fn quote_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(?P<indent>[ \t]*)(?P<marker>(>[ \t]?)+)").unwrap())
}

fn heading_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(#{1,6})[ \t]+").unwrap())
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Toggle **bold**, *italic*, ~~strike~~ or `code` around the selection (or the word at the caret).
pub fn wrap(doc: &mut Doc, marker: &str, placeholder: &str) {
    if doc.sel.is_empty() {
        let (s, e) = doc.word_at(doc.sel.head);
        if e > s {
            doc.sel = Sel { anchor: s, head: e };
        }
    }
    let (a, b) = doc.sel.range();
    let m = char_len(marker);
    if a == b {
        doc.replace(a, a, &format!("{marker}{placeholder}{marker}"));
        doc.sel = Sel { anchor: a + m, head: a + m + char_len(placeholder) };
        return;
    }
    let sel = doc.selected_text();
    if char_len(&sel) >= 2 * m && sel.starts_with(marker) && sel.ends_with(marker) {
        let inner: String = sel.chars().skip(m).take(char_len(&sel) - 2 * m).collect();
        doc.replace_select(a, b, &inner);
        return;
    }
    if a >= m && b + m <= doc.len_chars() && doc.slice(a - m, a) == marker && doc.slice(b, b + m) == marker {
        doc.replace(a - m, b + m, &sel);
        doc.sel = Sel { anchor: a - m, head: b - m };
        return;
    }
    doc.replace(a, b, &format!("{marker}{sel}{marker}"));
    doc.sel = Sel { anchor: a + m, head: b + m };
}

pub fn link(doc: &mut Doc) {
    let (a, b) = doc.sel.range();
    let sel = doc.selected_text();
    let t = sel.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        doc.replace(a, b, &format!("[text]({t})"));
        doc.sel = Sel { anchor: a + 1, head: a + 5 };
        return;
    }
    let label = if sel.is_empty() { "text".to_string() } else { sel.clone() };
    doc.replace(a, b, &format!("[{label}](https://)"));
    let ll = char_len(&label);
    if sel.is_empty() {
        doc.sel = Sel { anchor: a + 1, head: a + 1 + ll };
    } else {
        doc.sel = Sel { anchor: a + ll + 3, head: a + ll + 3 + 8 };
    }
}

pub fn image(doc: &mut Doc, path: &str) {
    let (a, b) = doc.sel.range();
    let alt = if a == b { "image".to_string() } else { doc.selected_text() };
    doc.replace(a, b, &format!("![{alt}]({})", path.replace(' ', "%20")));
    doc.sel = Sel { anchor: a + 2, head: a + 2 + char_len(&alt) };
}

fn sel_lines(doc: &Doc) -> (usize, usize) {
    let (a, b) = doc.sel.range();
    let la = doc.line_of(a);
    let mut lb = doc.line_of(b);
    if lb > la && doc.col_of(b) == 0 {
        lb -= 1;
    }
    (la, lb)
}

/// Cycle the heading level of the current line: none → # → ## → ### → none.
pub fn heading(doc: &mut Doc) {
    let l = doc.line_of(doc.sel.head);
    let text = doc.line_string(l);
    let (level, body) = match heading_re().captures(&text) {
        Some(c) => (c[1].len(), text[c.get(0).unwrap().end()..].to_string()),
        None => (0, text.trim_start().to_string()),
    };
    let next = if level >= 3 { 0 } else { level + 1 };
    let new = if next > 0 { format!("{} {}", "#".repeat(next), body) } else { body };
    let s = doc.line_start(l);
    let e = doc.line_end(l);
    doc.replace(s, e, &new);
}

#[derive(Clone, Copy, PartialEq)]
pub enum ListKind {
    Bullet,
    Numbered,
    Task,
    Quote,
}

pub fn toggle_list(doc: &mut Doc, kind: ListKind) {
    let (la, lb) = sel_lines(doc);
    let lines: Vec<String> = (la..=lb).map(|l| doc.line_string(l)).collect();
    let has = |t: &str| -> bool {
        if kind == ListKind::Quote {
            return quote_re().is_match(t);
        }
        match list_re().captures(t) {
            None => false,
            Some(c) => {
                let numbered = c["marker"].chars().next().map_or(false, |ch| ch.is_ascii_digit());
                let task = c.name("task").is_some();
                match kind {
                    ListKind::Task => task,
                    ListKind::Numbered => numbered && !task,
                    _ => !numbered && !task,
                }
            }
        }
    };
    let nonempty: Vec<&String> = lines.iter().filter(|t| !t.trim().is_empty()).collect();
    let remove = !nonempty.is_empty() && nonempty.iter().all(|t| has(t));
    let mut n = 1;
    let mut out: Vec<String> = Vec::new();
    for t in &lines {
        if t.trim().is_empty() && lines.len() > 1 {
            out.push(t.clone());
            continue;
        }
        let (indent, stripped) = if kind == ListKind::Quote {
            match quote_re().captures(t) {
                Some(c) => (c["indent"].to_string(), t[c.get(0).unwrap().end()..].to_string()),
                None => split_indent(t),
            }
        } else {
            match list_re().captures(t) {
                Some(c) => (c["indent"].to_string(), t[c.get(0).unwrap().end()..].to_string()),
                None => split_indent(t),
            }
        };
        let prefix = if remove {
            String::new()
        } else {
            match kind {
                ListKind::Bullet => "- ".into(),
                ListKind::Numbered => {
                    let p = format!("{n}. ");
                    n += 1;
                    p
                }
                ListKind::Task => "- [ ] ".into(),
                ListKind::Quote => "> ".into(),
            }
        };
        out.push(format!("{indent}{prefix}{stripped}"));
    }
    let nl = line_break(doc, la);
    let s = doc.line_start(la);
    let e = doc.line_end(lb);
    let text = out.join(nl);
    if la == lb {
        doc.replace(s, e, &text);
    } else {
        doc.replace_select(s, e, &text);
    }
}

fn split_indent(t: &str) -> (String, String) {
    let n = t.len() - t.trim_start().len();
    (t[..n].to_string(), t[n..].to_string())
}

fn line_break(doc: &Doc, l: usize) -> &'static str {
    let e = doc.line_end(l);
    let n = doc.line_next(l);
    match doc.slice(e, n).as_str() {
        "\n" => "\n",
        "\r" => "\r",
        "\r\n" => "\r\n",
        _ => "\r\n",
    }
}

pub fn code_block(doc: &mut Doc, nl: &str) {
    if doc.sel.is_empty() {
        let l = doc.line_of(doc.sel.head);
        let empty = doc.line_len(l) == 0;
        let at = if empty { doc.line_start(l) } else { doc.line_end(l) };
        let text = format!("{}```{nl}{nl}```{nl}", if empty { "" } else { nl });
        doc.replace(at, at, &text);
        let caret = at + if empty { 0 } else { char_len(nl) } + 3;
        doc.sel = Sel::caret(caret);
        return;
    }
    let (la, lb) = sel_lines(doc);
    let s = doc.line_start(la);
    let e = doc.line_end(lb);
    let block = doc.slice(s, e);
    doc.replace(s, e, &format!("```{nl}{block}{nl}```"));
    doc.sel = Sel::caret(s + 3);
}

pub fn table(doc: &mut Doc, nl: &str) {
    let l = doc.line_of(doc.sel.head);
    let empty = doc.line_len(l) == 0;
    let at = if empty { doc.line_start(l) } else { doc.line_end(l) };
    let t = format!(
        "| Column 1 | Column 2 | Column 3 |{nl}| -------- | -------- | -------- |{nl}|          |          |          |{nl}|          |          |          |{nl}"
    );
    let lead = if empty { String::new() } else { format!("{nl}{nl}") };
    doc.replace(at, at, &format!("{lead}{t}"));
    let start = at + char_len(&lead) + 2;
    doc.sel = Sel { anchor: start, head: start + 8 };
}

pub fn rule(doc: &mut Doc, nl: &str) {
    let l = doc.line_of(doc.sel.head);
    let empty = doc.line_len(l) == 0;
    let at = if empty { doc.line_start(l) } else { doc.line_end(l) };
    let text = format!("{}---{nl}{nl}", if empty { String::new() } else { format!("{nl}{nl}") });
    doc.replace(at, at, &text);
}

/// Enter inside a list item continues the list; Enter on an empty item ends it.
pub fn handle_enter(doc: &mut Doc, nl: &str) -> bool {
    if !doc.sel.is_empty() {
        return false;
    }
    let l = doc.line_of(doc.sel.head);
    let col = doc.col_of(doc.sel.head);
    let text = doc.line_string(l);
    let before: String = text.chars().take(col).collect();
    if let Some(c) = list_re().captures(&text) {
        let mlen = char_len(c.get(0).unwrap().as_str());
        if col >= mlen {
            let rest = &text[c.get(0).unwrap().end()..];
            let s = doc.line_start(l);
            if rest.trim().is_empty() {
                let indent = c["indent"].to_string();
                doc.replace(s, doc.line_end(l), &indent);
                return true;
            }
            let mut marker = c["marker"].to_string();
            if marker.chars().next().map_or(false, |ch| ch.is_ascii_digit()) {
                let num: String = marker.chars().take_while(|ch| ch.is_ascii_digit()).collect();
                let last = marker.chars().last().unwrap();
                marker = format!("{}{}", num.parse::<u64>().unwrap_or(0) + 1, last);
            }
            let task = if c.name("task").is_some() { "[ ] " } else { "" };
            let ins = format!("{nl}{}{}{}{}", &c["indent"], marker, &c["space"], task);
            doc.type_text(&ins, false);
            return true;
        }
    }
    if let Some(q) = quote_re().captures(&before) {
        let rest = &text[q.get(0).unwrap().end()..];
        if rest.trim().is_empty() {
            let s = doc.line_start(l);
            doc.replace(s, doc.line_end(l), "");
            return true;
        }
        let ins = format!("{nl}{}", q.get(0).unwrap().as_str());
        doc.type_text(&ins, false);
        return true;
    }
    false
}

/// Tab / Shift+Tab on list items indents / outdents the items.
pub fn handle_tab(doc: &mut Doc, outdent: bool, unit: &str) -> bool {
    let (la, lb) = sel_lines(doc);
    if !(la..=lb).all(|l| list_re().is_match(&doc.line_string(l))) {
        return false;
    }
    if la == lb && !outdent {
        let s = doc.line_start(la);
        doc.insert_at(s, unit);
    } else {
        doc.tab(outdent, unit);
    }
    true
}
