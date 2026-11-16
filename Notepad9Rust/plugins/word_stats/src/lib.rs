//! Word Stats — a Notepad9 plugin that shows live statistics of the current document
//! in a panel on the right: words, characters, lines, reading time, frequent words.

use notepad9_plugin::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};

const CMD_TOGGLE: u32 = 1;
const CMD_COPY: u32 = 2;
const WIN: u32 = 1;
/// bigger documents are not analysed word by word
const MAX_CHARS: usize = 8_000_000;

/// Large documents are recounted at most every 400 ms while the user types.
#[derive(Default)]
struct Throttle {
    last: Option<Instant>,
    waiting: bool,
}

impl Throttle {
    /// True when the work may run now; otherwise a timer brings us back.
    fn ready(&mut self, host: &Host) -> bool {
        let big = host.text_len() > 200_000;
        if big && self.last.map_or(false, |t| t.elapsed() < Duration::from_millis(400)) {
            if !self.waiting {
                self.waiting = true;
                host.set_timer(400);
            }
            return false;
        }
        self.last = Some(Instant::now());
        true
    }
    fn on_timer(&mut self, host: &Host) {
        self.waiting = false;
        host.set_timer(0);
        host.request_repaint();
    }
}

#[derive(Default)]
struct WordStats {
    throttle: Throttle,
    text_dirty: bool,
    sel_dirty: bool,
    s: Stats,
    sel_words: usize,
    sel_chars: usize,
    top: Vec<(String, usize)>,
    title: String,
}

#[derive(Default)]
struct Stats {
    chars: usize,
    chars_no_space: usize,
    words: usize,
    lines: usize,
    empty_lines: usize,
    sentences: usize,
    paragraphs: usize,
    longest: (usize, usize),
    too_big: bool,
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '\''))
        .map(|w| w.trim_matches('\''))
        .filter(|w| !w.is_empty())
}

fn group(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

fn minutes(words: usize, per_minute: usize) -> String {
    let secs = words * 60 / per_minute.max(1);
    if secs < 60 {
        format!("{secs} s")
    } else {
        format!("{} min {} s", secs / 60, secs % 60)
    }
}

impl WordStats {
    fn recount(&mut self, host: &Host) {
        self.title = host.tab_title(host.active_tab());
        let len = host.text_len();
        let mut s = Stats { lines: host.line_count(), ..Default::default() };
        self.top.clear();
        if len > MAX_CHARS {
            s.chars = len;
            s.too_big = true;
            self.s = s;
            return;
        }
        let text = host.text();
        let mut freq: HashMap<String, usize> = HashMap::new();
        for w in words(&text) {
            s.words += 1;
            if w.chars().count() >= 4 {
                *freq.entry(w.to_lowercase()).or_default() += 1;
            }
        }
        let mut in_para = false;
        for (i, line) in text.split('\n').enumerate() {
            let line = line.strip_suffix('\r').unwrap_or(line);
            let n = line.chars().count();
            if n > s.longest.1 {
                s.longest = (i + 1, n);
            }
            if line.trim().is_empty() {
                s.empty_lines += 1;
                in_para = false;
            } else if !in_para {
                s.paragraphs += 1;
                in_para = true;
            }
        }
        let mut prev_end = false;
        for c in text.chars() {
            if c != '\r' && c != '\n' {
                s.chars += 1;
                if !c.is_whitespace() {
                    s.chars_no_space += 1;
                }
            }
            let end = matches!(c, '.' | '!' | '?' | '…');
            if !end && prev_end && c.is_whitespace() {
                s.sentences += 1;
            }
            prev_end = end;
        }
        if prev_end {
            s.sentences += 1;
        }
        let mut top: Vec<(String, usize)> = freq.into_iter().filter(|(_, n)| *n > 1).collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        top.truncate(12);
        self.top = top;
        self.s = s;
    }

    fn recount_selection(&mut self, host: &Host) {
        let (a, b) = host.selection();
        if a == b || b - a > MAX_CHARS {
            self.sel_words = 0;
            self.sel_chars = b - a;
            return;
        }
        let t = host.selected_text();
        self.sel_chars = t.chars().filter(|c| *c != '\r' && *c != '\n').count();
        self.sel_words = words(&t).count();
    }

    fn summary(&self) -> String {
        let s = &self.s;
        format!(
            "{}\nWords: {}\nCharacters: {}\nCharacters (no spaces): {}\nLines: {}\nParagraphs: {}\nSentences: {}\nReading time: {}",
            self.title, s.words, s.chars, s.chars_no_space, s.lines, s.paragraphs, s.sentences, minutes(s.words, 230)
        )
    }

    fn row(ui: &mut Ui, label: &str, value: &str) {
        ui.monospace(&format!("{label:<18}{value:>12}"));
    }
}

impl Plugin for WordStats {
    fn info() -> Info {
        Info {
            name: "Word Stats",
            version: "1.0.0",
            author: "Notepad9",
            description: "Live word, character and line counts of the current document, reading time and the most frequent words.",
        }
    }

    fn init(&mut self, host: &Host) {
        host.add_menu_item("Word Stats/Show Statistics", "Ctrl+Alt+W", CMD_TOGGLE);
        host.add_menu_item("Word Stats/Copy Summary", "", CMD_COPY);
        host.add_toolbar_button("ac_word", "Word statistics (Ctrl+Alt+W)", CMD_TOGGLE);
        if host.setting("open") == "1" {
            host.open_window(WIN, "Word Stats", WindowKind::RightPanel, 270.0, 0.0);
        }
        self.text_dirty = true;
        self.sel_dirty = true;
    }

    fn on_command(&mut self, host: &Host, command: u32) {
        match command {
            CMD_TOGGLE => {
                host.toggle_window(WIN, "Word Stats", WindowKind::RightPanel, 270.0, 0.0);
                self.text_dirty = true;
                self.sel_dirty = true;
            }
            CMD_COPY => {
                self.recount(host);
                host.set_clipboard(&self.summary());
                host.show_info("Word statistics copied to the clipboard");
            }
            _ => {}
        }
    }

    fn on_event(&mut self, host: &Host, event: &Event) {
        // only remember that something changed: counting happens while the panel is drawn
        match event {
            Event::TextChanged { .. } | Event::TabActivated { .. } | Event::FileOpened { .. } | Event::Startup => {
                self.text_dirty = true;
                self.sel_dirty = true;
            }
            Event::SelectionChanged { .. } => self.sel_dirty = true,
            Event::Timer => self.throttle.on_timer(host),
            _ => {}
        }
    }

    fn ui(&mut self, host: &Host, _window: u32, ui: &mut Ui) {
        if self.text_dirty && self.throttle.ready(host) {
            self.text_dirty = false;
            self.recount(host);
        }
        if self.sel_dirty {
            self.sel_dirty = false;
            self.recount_selection(host);
        }
        let s = &self.s;
        ui.heading(&self.title);
        let lang = host.language();
        ui.weak(if lang.is_empty() { "Plain text" } else { &lang });
        ui.separator();
        if s.too_big {
            ui.label("The document is too large for a word count.");
            Self::row(ui, "Characters", &group(s.chars));
            Self::row(ui, "Lines", &group(s.lines));
            return;
        }
        Self::row(ui, "Words", &group(s.words));
        Self::row(ui, "Characters", &group(s.chars));
        Self::row(ui, "  without spaces", &group(s.chars_no_space));
        Self::row(ui, "Lines", &group(s.lines));
        Self::row(ui, "  empty", &group(s.empty_lines));
        Self::row(ui, "Paragraphs", &group(s.paragraphs));
        Self::row(ui, "Sentences", &group(s.sentences));
        if s.lines > 0 {
            Self::row(ui, "Longest line", &format!("{} ch", group(s.longest.1)));
        }
        ui.separator();
        Self::row(ui, "Reading time", &minutes(s.words, 230));
        Self::row(ui, "Speaking time", &minutes(s.words, 130));
        if self.sel_chars > 0 {
            ui.separator();
            ui.strong("Selection");
            Self::row(ui, "Words", &group(self.sel_words));
            Self::row(ui, "Characters", &group(self.sel_chars));
            let part = if s.chars > 0 { self.sel_chars as f32 / s.chars as f32 } else { 0.0 };
            ui.progress(part.min(1.0), &format!("{:.0}% of the document", part * 100.0));
        }
        if !self.top.is_empty() {
            ui.space(4.0);
            ui.collapsing("top", "Most frequent words", true, |ui| {
                for (w, n) in &self.top {
                    ui.monospace(&format!("{:<20}{:>8}", w.chars().take(20).collect::<String>(), n));
                }
            });
        }
        ui.space(6.0);
        let (mut copy, mut refresh) = (false, false);
        ui.horizontal(|ui| {
            copy = ui.button("copy", "Copy summary");
            refresh = ui.button("refresh", "Refresh");
        });
        if copy {
            host.set_clipboard(&self.summary());
            host.show_info("Word statistics copied to the clipboard");
        }
        if refresh {
            self.text_dirty = true;
            host.request_repaint();
        }
    }

    fn shutdown(&mut self, host: &Host) {
        host.set_setting("open", if host.is_window_open(WIN) { "1" } else { "0" });
    }
}

export_plugin!(WordStats);
