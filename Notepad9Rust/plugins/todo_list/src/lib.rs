//! TODO List — a Notepad9 plugin: every TODO / FIXME / HACK / NOTE / BUG / XXX of the
//! current file in a panel under the editor. Click a row to jump to the line.

use notepad9_plugin::*;
use std::time::{Duration, Instant};

const CMD_TOGGLE: u32 = 1;
const CMD_INSERT: u32 = 2;
const CMD_NEXT: u32 = 3;
const WIN: u32 = 1;
const MAX_ITEMS: usize = 5000;

const TAGS: [(&str, (u8, u8, u8)); 6] = [
    ("TODO", (86, 156, 214)),
    ("FIXME", (240, 110, 90)),
    ("BUG", (240, 80, 80)),
    ("HACK", (220, 170, 70)),
    ("XXX", (200, 120, 220)),
    ("NOTE", (120, 190, 120)),
];

struct Item {
    line: usize,
    tag: usize,
    text: String,
}

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
struct TodoList {
    throttle: Throttle,
    dirty: bool,
    items: Vec<Item>,
    truncated: bool,
    file: String,
}

/// Lines of the text, whatever the line endings (CR+LF, LF or CR).
fn lines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let (mut start, mut i) = (0, 0);
    while i < b.len() {
        if b[i] == b'\n' || b[i] == b'\r' {
            out.push(&text[start..i]);
            if b[i] == b'\r' && i + 1 < b.len() && b[i + 1] == b'\n' {
                i += 1;
            }
            start = i + 1;
        }
        i += 1;
    }
    out.push(&text[start..]);
    out
}

/// Tag at a word boundary: "TODO", "TODO:", "TODO(name)" but not "TODOS" or "autodo".
fn find_tag(line: &str) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    for (ti, (tag, _)) in TAGS.iter().enumerate() {
        let mut from = 0;
        while let Some(p) = line[from..].find(tag) {
            let at = from + p;
            let before = line[..at].chars().next_back();
            let after = line[at + tag.len()..].chars().next();
            let word = |c: Option<char>| c.map_or(false, |c| c.is_alphanumeric() || c == '_');
            if !word(before) && !word(after) {
                if best.map_or(true, |(_, b)| at < b) {
                    best = Some((ti, at));
                }
                break;
            }
            from = at + tag.len();
        }
    }
    best
}

fn comment_for(lang: &str) -> (&'static str, &'static str) {
    match lang {
        "Python" | "Ruby" | "Perl" | "Shell Script (Bash)" | "Bourne Again Shell (bash)" | "YAML" | "TOML" | "Makefile" | "R" | "PowerShell" => ("# ", ""),
        "HTML" | "XML" | "Markdown" => ("<!-- ", " -->"),
        "SQL" | "Lua" | "Haskell" => ("-- ", ""),
        "Batch File" => ("REM ", ""),
        "LaTeX" | "TeX" | "Erlang" => ("% ", ""),
        "CSS" => ("/* ", " */"),
        _ => ("// ", ""),
    }
}

impl TodoList {
    fn scan(&mut self, host: &Host) {
        self.dirty = false;
        self.items.clear();
        self.truncated = false;
        self.file = host.tab_title(host.active_tab());
        let text = host.text();
        for (i, line) in lines(&text).into_iter().enumerate() {
            if let Some((tag, at)) = find_tag(line) {
                if self.items.len() >= MAX_ITEMS {
                    self.truncated = true;
                    break;
                }
                let rest = line[at + TAGS[tag].0.len()..].trim_start_matches([':', ' ', '\t', '-']).trim();
                let rest = rest.trim_end_matches("*/").trim_end_matches("-->").trim();
                self.items.push(Item { line: i, tag, text: rest.chars().take(200).collect() });
            }
        }
    }
}

impl Plugin for TodoList {
    fn info() -> Info {
        Info {
            name: "TODO List",
            version: "1.0.0",
            author: "Notepad9",
            description: "Lists TODO, FIXME, BUG, HACK, XXX and NOTE comments of the current file. Click a row to go to it.",
        }
    }

    fn init(&mut self, host: &Host) {
        host.add_menu_item("TODO List/Show TODO List", "Ctrl+Alt+T", CMD_TOGGLE);
        host.add_menu_item("TODO List/Next TODO", "Ctrl+Alt+N", CMD_NEXT);
        host.add_menu_item("TODO List/Insert TODO Comment", "Ctrl+Alt+I", CMD_INSERT);
        host.add_toolbar_button("mark", "TODO list (Ctrl+Alt+T)", CMD_TOGGLE);
        if host.setting("open") == "1" {
            host.open_window(WIN, "TODO List", WindowKind::BottomPanel, 0.0, 190.0);
        }
        self.dirty = true;
    }

    fn on_command(&mut self, host: &Host, command: u32) {
        match command {
            CMD_TOGGLE => {
                host.toggle_window(WIN, "TODO List", WindowKind::BottomPanel, 0.0, 190.0);
                self.dirty = true;
            }
            CMD_INSERT => {
                let (open, close) = comment_for(&host.language());
                let line = host.line_of(host.caret());
                let indent: String = host.line_text(line).chars().take_while(|c| *c == ' ' || *c == '\t').collect();
                let at = host.line_start(line);
                let text = format!("{indent}{open}TODO: {close}\n");
                host.insert_text(at, &text);
                // caret after "TODO: "
                let caret = host.line_start(line) + indent.chars().count() + open.chars().count() + 6;
                host.set_selection(caret, caret);
            }
            CMD_NEXT => {
                self.scan(host);
                let cur = host.line_of(host.caret());
                let next = self.items.iter().find(|i| i.line > cur).or_else(|| self.items.first());
                match next {
                    Some(i) => host.goto_line(i.line),
                    None => host.show_info("No TODO comments in this file"),
                }
            }
            _ => {}
        }
    }

    fn on_event(&mut self, host: &Host, event: &Event) {
        if matches!(event, Event::TextChanged { .. } | Event::TabActivated { .. } | Event::FileOpened { .. } | Event::Startup) {
            self.dirty = true;
        }
        if matches!(event, Event::Timer) {
            self.throttle.on_timer(host);
        }
    }

    fn ui(&mut self, host: &Host, _window: u32, ui: &mut Ui) {
        if self.dirty && self.throttle.ready(host) {
            self.scan(host);
        }
        let mut show = [true; TAGS.len()];
        let mut filter = String::new();
        let mut rescan = false;
        ui.horizontal(|ui| {
            for (i, (tag, _)) in TAGS.iter().enumerate() {
                show[i] = ui.checkbox(tag, tag, true);
            }
            ui.space(12.0);
            rescan = ui.button("rescan", "Rescan");
        });
        ui.horizontal(|ui| {
            ui.weak("Filter:");
            filter = ui.text_input("filter", "text to look for...").to_lowercase();
        });
        let visible: Vec<&Item> = self.items.iter()
            .filter(|i| show[i.tag] && (filter.is_empty() || i.text.to_lowercase().contains(&filter)))
            .collect();
        ui.weak(&format!(
            "{}: {} of {} item(s){}",
            if self.file.is_empty() { "Untitled" } else { self.file.as_str() },
            visible.len(),
            self.items.len(),
            if self.truncated { " (list cut at 5000)" } else { "" }
        ));
        ui.separator();
        if visible.is_empty() {
            ui.weak("Nothing found. Add comments like  // TODO: something  and they show up here.");
        }
        let cur = host.line_of(host.caret());
        let mut go: Option<usize> = None;
        for it in &visible {
            ui.horizontal(|ui| {
                let (tag, color) = TAGS[it.tag];
                ui.colored(&format!("{tag:<6}"), color);
                let label = format!("Ln {:<6} {}", it.line + 1, if it.text.is_empty() { "(no text)" } else { it.text.as_str() });
                if ui.selectable(&format!("row{}", it.line), &label, it.line == cur) {
                    go = Some(it.line);
                }
            });
        }
        if let Some(l) = go {
            host.goto_line(l);
        }
        if rescan {
            self.dirty = true;
            host.request_repaint();
        }
    }

    fn shutdown(&mut self, host: &Host) {
        host.set_setting("open", if host.is_window_open(WIN) { "1" } else { "0" });
    }
}

export_plugin!(TodoList);
