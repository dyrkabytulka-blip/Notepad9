//! Line and case operations (Edit → Lines / Convert).

use crate::doc::{Doc, Sel};

fn sel_lines(d: &Doc) -> (usize, usize) {
    let (a, b) = d.sel.range();
    let la = d.line_of(a);
    let mut lb = d.line_of(b);
    if lb > la && d.col_of(b) == 0 {
        lb -= 1;
    }
    (la, lb)
}

fn break_of(d: &Doc, l: usize) -> String {
    d.slice(d.line_end(l), d.line_next(l))
}

pub fn duplicate(d: &mut Doc) {
    if !d.sel.is_empty() {
        let (_, b) = d.sel.range();
        let t = d.selected_text();
        d.insert_at(b, &t);
        let n = t.chars().count();
        d.sel = Sel { anchor: b, head: b + n };
        return;
    }
    let l = d.line_of(d.sel.head);
    let col = d.col_of(d.sel.head);
    let text = d.line_string(l);
    let mut nl = break_of(d, l);
    if nl.is_empty() {
        nl = if l > 0 { break_of(d, l - 1) } else { "\r\n".into() };
    }
    let end = d.line_end(l);
    d.replace(end, end, &format!("{nl}{text}"));
    let s = d.line_start(l + 1);
    d.sel = Sel::caret(s + col.min(d.line_len(l + 1)));
}

pub fn delete_lines(d: &mut Doc) {
    let (la, lb) = sel_lines(d);
    let mut s = d.line_start(la);
    let e = d.line_next(lb);
    if e == d.line_end(lb) && la > 0 {
        s = d.line_end(la - 1);
    }
    d.replace(s, e, "");
}

pub fn move_lines(d: &mut Doc, up: bool) {
    let (la, lb) = sel_lines(d);
    let sel = d.sel;
    let start = d.line_start(la);
    let block = d.slice(start, d.line_end(lb));
    if up {
        if la == 0 {
            return;
        }
        let prev = la - 1;
        let ps = d.line_start(prev);
        let prev_text = d.line_string(prev);
        let nl = break_of(d, prev);
        let end = d.line_end(lb);
        d.replace(ps, end, &format!("{block}{nl}{prev_text}"));
        let shift = |p: usize| p - (start - ps);
        d.sel = Sel { anchor: shift(sel.anchor), head: shift(sel.head) };
    } else {
        if lb + 1 >= d.len_lines() {
            return;
        }
        let next = lb + 1;
        let next_text = d.line_string(next);
        let nl = break_of(d, lb);
        let end = d.line_end(next);
        d.replace(start, end, &format!("{next_text}{nl}{block}"));
        let delta = next_text.chars().count() + nl.chars().count();
        d.sel = Sel { anchor: sel.anchor + delta, head: sel.head + delta };
    }
}

fn tabs_to_spaces(s: &str, w: usize) -> String {
    let mut out = String::new();
    let mut col = 0;
    for c in s.chars() {
        if c == '\t' {
            let n = w - col % w;
            out.push_str(&" ".repeat(n));
            col += n;
        } else {
            out.push(c);
            col += 1;
        }
    }
    out
}

fn spaces_to_tabs(s: &str, w: usize) -> String {
    let mut col = 0;
    let mut i = 0;
    let chars: Vec<char> = s.chars().collect();
    while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
        col = if chars[i] == '\t' { (col / w + 1) * w } else { col + 1 };
        i += 1;
    }
    if i == 0 {
        return s.to_string();
    }
    let rest: String = chars[i..].iter().collect();
    format!("{}{}{}", "\t".repeat(col / w), " ".repeat(col % w), rest)
}

/// Sort / unique / remove empty / join / tabs ↔ spaces / trim, on the selected lines (or all).
pub fn transform_lines(d: &mut Doc, what: &str, tab_width: usize, nl: &str) {
    let had_sel = !d.sel.is_empty();
    let (la, lb) = if had_sel { sel_lines(d) } else { (0, d.len_lines().saturating_sub(1)) };
    let lines: Vec<String> = (la..=lb).map(|l| d.line_string(l)).collect();
    let w = tab_width.max(1);
    let out: Vec<String> = match what {
        "sort_asc" => {
            let mut v = lines.clone();
            v.sort_by_key(|a| a.to_lowercase());
            v
        }
        "sort_desc" => {
            let mut v = lines.clone();
            v.sort_by_key(|a| std::cmp::Reverse(a.to_lowercase()));
            v
        }
        "uniq" => {
            let mut seen = std::collections::HashSet::new();
            lines.iter().filter(|l| seen.insert((*l).clone())).cloned().collect()
        }
        "no_empty" => lines.iter().filter(|l| !l.trim().is_empty()).cloned().collect(),
        "join" => vec![lines.iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ")],
        "tabs2spaces" => lines.iter().map(|l| tabs_to_spaces(l, w)).collect(),
        "spaces2tabs" => lines.iter().map(|l| spaces_to_tabs(l, w)).collect(),
        "trim" => lines.iter().map(|l| l.trim_end_matches([' ', '\t']).to_string()).collect(),
        _ => return,
    };
    if out == lines {
        return;
    }
    let s = d.line_start(la);
    let e = d.line_end(lb);
    let text = out.join(nl);
    if had_sel { d.replace_select(s, e, &text) } else { d.replace(s, e, &text); d.sel = Sel::caret(s) }
}

pub fn transform_case(d: &mut Doc, what: &str) {
    if d.sel.is_empty() {
        let (s, e) = d.word_at(d.sel.head);
        if e <= s {
            return;
        }
        d.sel = Sel { anchor: s, head: e };
    }
    let (a, b) = d.sel.range();
    let t = d.selected_text();
    let r: String = match what {
        "upper" => t.to_uppercase(),
        "lower" => t.to_lowercase(),
        "title" => {
            let mut out = String::new();
            let mut start = true;
            for c in t.chars() {
                if c.is_alphanumeric() {
                    if start { out.extend(c.to_uppercase()) } else { out.extend(c.to_lowercase()) }
                    start = false;
                } else {
                    out.push(c);
                    start = true;
                }
            }
            out
        }
        "invert" => t
            .chars()
            .flat_map(|c| if c.is_uppercase() { c.to_lowercase().collect::<Vec<_>>() } else { c.to_uppercase().collect() })
            .collect(),
        _ => return,
    };
    if r != t {
        d.replace_select(a, b, &r);
    }
}

/// Local date and time, like "07.11.2026 14:05".
pub fn now_string() -> String {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    let mut st = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut st) };
    let st: windows_sys::Win32::Foundation::SYSTEMTIME = st;
    format!("{:02}.{:02}.{} {:02}:{:02}", st.wDay, st.wMonth, st.wYear, st.wHour, st.wMinute)
}
