//! Find / replace on the document text (byte offsets in, char offsets out).

use crate::doc::Doc;
use regex::Regex;

#[derive(Clone, Default)]
pub struct Opts {
    pub find: String,
    pub replace: String,
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
    pub wrap: bool,
}

pub fn build(o: &Opts) -> Result<Option<Regex>, String> {
    if o.find.is_empty() {
        return Ok(None);
    }
    let mut p = if o.regex { o.find.clone() } else { regex::escape(&o.find) };
    if o.whole_word {
        p = format!(r"\b(?:{p})\b");
    }
    let flags = if o.match_case { "(?m)" } else { "(?mi)" };
    Regex::new(&format!("{flags}{p}")).map(Some).map_err(|e| e.to_string())
}

fn byte_to_char(t: &str, b: usize) -> usize {
    t[..b].chars().count()
}

/// Select the next (or previous) match. Returns a status message.
pub fn find_next(doc: &mut Doc, o: &Opts, backwards: bool) -> Result<Option<String>, String> {
    let re = match build(o)? {
        Some(r) => r,
        None => return Err("Nothing to find".into()),
    };
    let text = doc.text();
    let t = text.as_str();
    let (sa, sb) = doc.sel.range();
    let from = doc.rope.char_to_byte(sb.min(doc.len_chars()));
    let to = doc.rope.char_to_byte(sa.min(doc.len_chars()));
    let mut note = None;
    let m = if !backwards {
        let mut m = re.find_at(t, from);
        if let Some(x) = m {
            if x.start() == x.end() && x.start() == from && from < t.len() {
                m = re.find_at(t, from + 1);
            }
        }
        if m.is_none() && o.wrap {
            note = Some("Search wrapped to the top".to_string());
            m = re.find(t);
        }
        m
    } else {
        let mut last = None;
        for x in re.find_iter(&t[..to]) {
            last = Some(x);
        }
        if last.is_none() && o.wrap {
            note = Some("Search wrapped to the bottom".to_string());
            for x in re.find_iter(t) {
                last = Some(x);
            }
        }
        last
    };
    match m {
        Some(x) => {
            let a = byte_to_char(t, x.start());
            let b = a + t[x.start()..x.end()].chars().count();
            doc.sel = crate::doc::Sel { anchor: a, head: b };
            Ok(note)
        }
        None => Err(format!("\"{}\" not found", o.find)),
    }
}

pub fn replace_current(doc: &mut Doc, o: &Opts) -> Result<Option<String>, String> {
    let re = build(o)?.ok_or("Nothing to find")?;
    let (a, b) = doc.sel.range();
    if a != b {
        let sel = doc.selected_text();
        if let Some(m) = re.find(&sel) {
            if m.start() == 0 && m.end() == sel.len() {
                let repl = if o.regex { re.replace(&sel, o.replace.as_str()).to_string() } else { o.replace.clone() };
                doc.replace(a, b, &repl);
            }
        }
    }
    find_next(doc, o, false)
}

/// Replace every match (inside the selection if `in_sel`). One undo step.
pub fn replace_all(doc: &mut Doc, o: &Opts, in_sel: bool) -> Result<usize, String> {
    let re = build(o)?.ok_or("Nothing to find")?;
    let text = doc.text();
    let (sa, sb) = if in_sel && !doc.sel.is_empty() { doc.sel.range() } else { (0, doc.len_chars()) };
    let (ba, bb) = (doc.rope.char_to_byte(sa), doc.rope.char_to_byte(sb));
    let part = &text[ba..bb];
    let count = re.find_iter(part).count();
    if count == 0 {
        return Ok(0);
    }
    let new = if o.regex { re.replace_all(part, o.replace.as_str()).to_string() } else { re.replace_all(part, regex::NoExpand(&o.replace)).to_string() };
    doc.replace_select(sa, sb, &new);
    if !in_sel {
        doc.sel = crate::doc::Sel::caret(sa);
    }
    Ok(count)
}

pub fn count(text: &str, o: &Opts) -> usize {
    match build(o) {
        Ok(Some(re)) => re.find_iter(text).count(),
        _ => 0,
    }
}
