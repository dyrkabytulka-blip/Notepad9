//! Text buffer (rope), selection and undo history. Positions are char indices.

use ropey::Rope;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Sel {
    pub anchor: usize,
    pub head: usize,
}

impl Sel {
    pub fn caret(p: usize) -> Self {
        Self { anchor: p, head: p }
    }
    pub fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Kind {
    Typing,
    BackDelete,
    FwdDelete,
    Other,
}

struct Edit {
    id: u64,
    start: usize,
    removed: String,
    inserted: String,
    before: Sel,
    after: Sel,
    kind: Kind,
    time: Instant,
}

pub struct Doc {
    pub rope: Rope,
    pub sel: Sel,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    next_id: u64,
    saved_id: u64,
    /// forced "modified" (encoding changed, text restored from the session ...)
    pub force_modified: bool,
    pub version: u64,
    /// first line touched since the highlighter last looked (taken by the owner)
    pub changed_from_line: Option<usize>,
    /// column (in pixels) kept while moving up / down
    pub preferred_x: Option<f32>,
    pub overwrite: bool,
    text_cache: Option<(u64, Arc<String>)>,
}

impl Default for Doc {
    fn default() -> Self {
        Self::new("")
    }
}

impl Doc {
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            sel: Sel::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            next_id: 1,
            saved_id: 0,
            force_modified: false,
            version: 0,
            changed_from_line: Some(0),
            preferred_x: None,
            overwrite: false,
            text_cache: None,
        }
    }

    /// Replace the whole text (file load). Clears undo history.
    pub fn set_text(&mut self, text: &str) {
        self.rope = Rope::from_str(text);
        self.sel = Sel::default();
        self.undo.clear();
        self.redo.clear();
        self.saved_id = 0;
        self.force_modified = false;
        self.bump(0);
    }

    fn bump(&mut self, line: usize) {
        self.version += 1;
        self.text_cache = None;
        self.changed_from_line = Some(self.changed_from_line.map_or(line, |l| l.min(line)));
    }

    // ------------------------------------------------------------------ state
    fn state_id(&self) -> u64 {
        self.undo.last().map_or(0, |e| e.id)
    }
    pub fn is_modified(&self) -> bool {
        self.force_modified || self.state_id() != self.saved_id
    }
    pub fn mark_saved(&mut self) {
        self.saved_id = self.state_id();
        self.force_modified = false;
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Full text, cached until the next edit.
    pub fn text(&mut self) -> Arc<String> {
        if let Some((v, t)) = &self.text_cache {
            if *v == self.version {
                return t.clone();
            }
        }
        let t = Arc::new(self.rope.to_string());
        self.text_cache = Some((self.version, t.clone()));
        t
    }

    // ------------------------------------------------------------------ lines
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }
    pub fn line_of(&self, c: usize) -> usize {
        self.rope.char_to_line(c.min(self.len_chars()))
    }
    pub fn line_start(&self, l: usize) -> usize {
        self.rope.line_to_char(l.min(self.len_lines().saturating_sub(1)))
    }
    /// Line length without its line break.
    pub fn line_len(&self, l: usize) -> usize {
        if l >= self.len_lines() {
            return 0;
        }
        let s = self.rope.line(l);
        let mut n = s.len_chars();
        if n > 0 && s.char(n - 1) == '\n' {
            n -= 1;
            if n > 0 && s.char(n - 1) == '\r' {
                n -= 1;
            }
        } else if n > 0 && s.char(n - 1) == '\r' {
            n -= 1;
        }
        n
    }
    pub fn line_end(&self, l: usize) -> usize {
        self.line_start(l) + self.line_len(l)
    }
    /// Start of the next line (after the line break).
    pub fn line_next(&self, l: usize) -> usize {
        if l + 1 >= self.len_lines() { self.len_chars() } else { self.rope.line_to_char(l + 1) }
    }
    pub fn line_string(&self, l: usize) -> String {
        let s = self.line_start(l);
        self.rope.slice(s..s + self.line_len(l)).to_string()
    }
    pub fn col_of(&self, c: usize) -> usize {
        c - self.line_start(self.line_of(c))
    }
    pub fn char_at(&self, c: usize) -> Option<char> {
        if c < self.len_chars() { Some(self.rope.char(c)) } else { None }
    }
    pub fn slice(&self, a: usize, b: usize) -> String {
        let n = self.len_chars();
        self.rope.slice(a.min(n)..b.min(n)).to_string()
    }
    pub fn selected_text(&self) -> String {
        let (a, b) = self.sel.range();
        self.slice(a, b)
    }

    pub fn is_word(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    pub fn word_at(&self, c: usize) -> (usize, usize) {
        let n = self.len_chars();
        let mut s = c.min(n);
        let mut e = s;
        while s > 0 && Self::is_word(self.rope.char(s - 1)) {
            s -= 1;
        }
        while e < n && Self::is_word(self.rope.char(e)) {
            e += 1;
        }
        (s, e)
    }

    fn word_left(&self, c: usize) -> usize {
        let mut i = c;
        while i > 0 && self.rope.char(i - 1).is_whitespace() && self.rope.char(i - 1) != '\n' {
            i -= 1;
        }
        if i > 0 && (self.rope.char(i - 1) == '\n' || self.rope.char(i - 1) == '\r') {
            return if i == c { i - 1 } else { i };
        }
        if i > 0 && Self::is_word(self.rope.char(i - 1)) {
            while i > 0 && Self::is_word(self.rope.char(i - 1)) {
                i -= 1;
            }
        } else if i > 0 {
            let k = self.rope.char(i - 1);
            while i > 0 && self.rope.char(i - 1) == k {
                i -= 1;
            }
        }
        i
    }

    fn word_right(&self, c: usize) -> usize {
        let n = self.len_chars();
        let mut i = c;
        if i < n && (self.rope.char(i) == '\n' || self.rope.char(i) == '\r') {
            return self.line_next(self.line_of(i));
        }
        if i < n && Self::is_word(self.rope.char(i)) {
            while i < n && Self::is_word(self.rope.char(i)) {
                i += 1;
            }
        } else if i < n && !self.rope.char(i).is_whitespace() {
            let k = self.rope.char(i);
            while i < n && self.rope.char(i) == k {
                i += 1;
            }
        }
        while i < n && self.rope.char(i).is_whitespace() && self.rope.char(i) != '\n' && self.rope.char(i) != '\r' {
            i += 1;
        }
        i
    }

    // ------------------------------------------------------------------ editing
    fn apply(&mut self, start: usize, end: usize, text: &str, kind: Kind, before: Sel, after: Sel) {
        let n = self.len_chars();
        let (start, end) = (start.min(n), end.min(n));
        let removed = self.rope.slice(start..end).to_string();
        if removed.is_empty() && text.is_empty() {
            self.sel = after;
            return;
        }
        let line = self.line_of(start);
        self.rope.remove(start..end);
        self.rope.insert(start, text);
        self.redo.clear();
        let now = Instant::now();
        // merge with the previous step: typing a word, holding backspace ...
        if let Some(last) = self.undo.last_mut() {
            let recent = now.duration_since(last.time).as_secs_f32() < 1.2 && last.id != self.saved_id;
            if recent && kind == last.kind {
                match kind {
                    Kind::Typing
                        if removed.is_empty() && last.removed.is_empty()
                            && last.start + last.inserted.chars().count() == start
                            && !text.contains('\n') =>
                    {
                        last.inserted.push_str(text);
                        last.after = after;
                        last.time = now;
                        self.sel = after;
                        self.bump(line);
                        return;
                    }
                    Kind::BackDelete if text.is_empty() && last.inserted.is_empty() && end == last.start => {
                        last.removed.insert_str(0, &removed);
                        last.start = start;
                        last.after = after;
                        last.time = now;
                        self.sel = after;
                        self.bump(line);
                        return;
                    }
                    Kind::FwdDelete if text.is_empty() && last.inserted.is_empty() && start == last.start => {
                        last.removed.push_str(&removed);
                        last.after = after;
                        last.time = now;
                        self.sel = after;
                        self.bump(line);
                        return;
                    }
                    _ => {}
                }
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        self.undo.push(Edit { id, start, removed, inserted: text.to_string(), before, after, kind, time: now });
        if self.undo.len() > 5000 {
            self.undo.remove(0);
        }
        self.sel = after;
        self.bump(line);
    }

    /// Replace a range; the caret ends after the new text.
    pub fn replace(&mut self, start: usize, end: usize, text: &str) {
        let p = start + text.chars().count();
        let before = self.sel;
        self.apply(start, end, text, Kind::Other, before, Sel::caret(p));
    }

    /// Replace a range and select the new text.
    pub fn replace_select(&mut self, start: usize, end: usize, text: &str) {
        let n = text.chars().count();
        let before = self.sel;
        self.apply(start, end, text, Kind::Other, before, Sel { anchor: start, head: start + n });
    }

    pub fn insert_at(&mut self, at: usize, text: &str) {
        let before = self.sel;
        // keep the selection where it was (shifted if after the insertion point)
        let n = text.chars().count();
        let shift = |p: usize| if p >= at { p + n } else { p };
        let after = Sel { anchor: shift(before.anchor), head: shift(before.head) };
        self.apply(at, at, text, Kind::Other, before, after);
    }

    /// Typed text (or pasted text): replaces the selection, honours overwrite mode.
    pub fn type_text(&mut self, text: &str, typing: bool) {
        let (a, mut b) = self.sel.range();
        if self.overwrite && a == b && typing {
            let l = self.line_of(a);
            let end = self.line_end(l);
            b = (a + text.chars().count()).min(end);
        }
        let p = a + text.chars().count();
        let before = self.sel;
        self.apply(a, b, text, if typing && a == b { Kind::Typing } else { Kind::Other }, before, Sel::caret(p));
        self.preferred_x = None;
    }

    pub fn backspace(&mut self, word: bool) {
        let (a, b) = self.sel.range();
        let before = self.sel;
        if a != b {
            self.apply(a, b, "", Kind::Other, before, Sel::caret(a));
            return;
        }
        if a == 0 {
            return;
        }
        let start = if word {
            self.word_left(a)
        } else if a >= 2 && self.rope.char(a - 1) == '\n' && self.rope.char(a - 2) == '\r' {
            a - 2
        } else {
            a - 1
        };
        self.apply(start, a, "", Kind::BackDelete, before, Sel::caret(start));
        self.preferred_x = None;
    }

    pub fn delete(&mut self, word: bool) {
        let (a, b) = self.sel.range();
        let before = self.sel;
        if a != b {
            self.apply(a, b, "", Kind::Other, before, Sel::caret(a));
            return;
        }
        let n = self.len_chars();
        if a >= n {
            return;
        }
        let end = if word {
            self.word_right(a).max(a + 1)
        } else if self.rope.char(a) == '\r' && a + 1 < n && self.rope.char(a + 1) == '\n' {
            a + 2
        } else {
            a + 1
        };
        self.apply(a, end, "", Kind::FwdDelete, before, Sel::caret(a));
        self.preferred_x = None;
    }

    /// Enter: new line keeping the indentation of the current line.
    pub fn newline(&mut self, eol: &str, auto_indent: bool) {
        let (a, _) = self.sel.range();
        let l = self.line_of(a);
        let mut indent = String::new();
        if auto_indent {
            let s = self.line_start(l);
            for c in self.rope.slice(s..a).chars() {
                if c == ' ' || c == '\t' { indent.push(c) } else { break }
            }
        }
        let text = format!("{eol}{indent}");
        self.type_text(&text, false);
    }

    /// Tab / Shift+Tab: insert indentation, or (re)indent all selected lines.
    pub fn tab(&mut self, outdent: bool, unit: &str) {
        let (a, b) = self.sel.range();
        let la = self.line_of(a);
        let lb = self.line_of(b);
        if !outdent && la == lb {
            self.type_text(unit, false);
            return;
        }
        let last = if lb > la && self.col_of(b) == 0 { lb - 1 } else { lb };
        let start = self.line_start(la);
        let end = self.line_end(last);
        let mut out = String::new();
        for l in la..=last {
            let s = self.line_string(l);
            let line = if outdent {
                if let Some(r) = s.strip_prefix('\t') {
                    r.to_string()
                } else {
                    let n = s.chars().take(unit.chars().count().max(1)).take_while(|c| *c == ' ').count();
                    s.chars().skip(n).collect()
                }
            } else if s.is_empty() {
                s
            } else {
                format!("{unit}{s}")
            };
            out.push_str(&line);
            if l < last {
                let nl_start = self.line_end(l);
                out.push_str(&self.slice(nl_start, self.line_next(l)));
            }
        }
        self.replace_select(start, end, &out);
    }

    pub fn undo(&mut self) {
        if let Some(e) = self.undo.pop() {
            let ins = e.inserted.chars().count();
            let line = self.line_of(e.start);
            self.rope.remove(e.start..e.start + ins);
            self.rope.insert(e.start, &e.removed);
            self.sel = e.before;
            self.redo.push(e);
            self.bump(line);
        }
    }

    pub fn redo(&mut self) {
        if let Some(e) = self.redo.pop() {
            let rem = e.removed.chars().count();
            let line = self.line_of(e.start);
            self.rope.remove(e.start..e.start + rem);
            self.rope.insert(e.start, &e.inserted);
            self.sel = e.after;
            self.undo.push(e);
            self.bump(line);
        }
    }

    // ------------------------------------------------------------------ caret movement
    pub fn set_caret(&mut self, p: usize, extend: bool) {
        let p = p.min(self.len_chars());
        if extend { self.sel.head = p } else { self.sel = Sel::caret(p) }
    }

    pub fn move_left(&mut self, word: bool, extend: bool) {
        if !extend && !self.sel.is_empty() {
            let (a, _) = self.sel.range();
            self.sel = Sel::caret(a);
            return;
        }
        let h = self.sel.head;
        let p = if word {
            self.word_left(h)
        } else if h >= 2 && self.rope.char(h - 1) == '\n' && self.rope.char(h - 2) == '\r' {
            h - 2
        } else {
            h.saturating_sub(1)
        };
        self.set_caret(p, extend);
        self.preferred_x = None;
    }

    pub fn move_right(&mut self, word: bool, extend: bool) {
        if !extend && !self.sel.is_empty() {
            let (_, b) = self.sel.range();
            self.sel = Sel::caret(b);
            return;
        }
        let h = self.sel.head;
        let n = self.len_chars();
        let p = if word {
            self.word_right(h)
        } else if h + 1 < n && self.rope.char(h) == '\r' && self.rope.char(h + 1) == '\n' {
            h + 2
        } else {
            (h + 1).min(n)
        };
        self.set_caret(p, extend);
        self.preferred_x = None;
    }

    /// Home: first non-blank character, then column 0.
    pub fn home(&mut self, extend: bool) {
        let l = self.line_of(self.sel.head);
        let s = self.line_start(l);
        let mut first = s;
        let e = self.line_end(l);
        while first < e && matches!(self.rope.char(first), ' ' | '\t') {
            first += 1;
        }
        let p = if self.sel.head == first { s } else { first };
        self.set_caret(p, extend);
        self.preferred_x = None;
    }

    pub fn end(&mut self, extend: bool) {
        let l = self.line_of(self.sel.head);
        let p = self.line_end(l);
        self.set_caret(p, extend);
        self.preferred_x = None;
    }

    pub fn select_all(&mut self) {
        self.sel = Sel { anchor: 0, head: self.len_chars() };
    }

    pub fn select_word_at(&mut self, p: usize) {
        let (s, e) = self.word_at(p);
        self.sel = if s == e { Sel { anchor: p, head: (p + 1).min(self.len_chars()) } } else { Sel { anchor: s, head: e } };
    }

    pub fn select_lines(&mut self, a: usize, b: usize) {
        let (la, lb) = (a.min(b), a.max(b));
        let s = self.line_start(la);
        let e = self.line_next(lb);
        self.sel = if b >= a { Sel { anchor: s, head: e } } else { Sel { anchor: e, head: s } };
    }
}
