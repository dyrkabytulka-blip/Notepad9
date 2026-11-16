//! Text Tools — a Notepad9 plugin with converters (Base64, URL, JSON, HTML) and line tools.
//! Shows a floating tool window (toolbar button with its own PNG icon) and menu commands;
//! its options are kept with the plugin settings.

use notepad9_plugin::*;

const WIN: u32 = 1;
const CMD_WINDOW: u32 = 1;
static ICON: &[u8] = include_bytes!("../icon.png");

#[derive(Clone, Copy, PartialEq)]
enum Op {
    B64Enc,
    B64Dec,
    UrlEnc,
    UrlDec,
    HtmlEsc,
    HtmlUnesc,
    JsonFormat,
    JsonMinify,
    Reverse,
    SortLen,
    Shuffle,
    Number,
    Unnumber,
    Wrap,
    Trim,
    Lorem,
}

struct OpDef {
    cmd: u32,
    name: &'static str,
    group: usize,
    op: Op,
}

const GROUPS: [&str; 4] = ["Encode / decode", "JSON", "Lines", "Insert"];

const OPS: &[OpDef] = &[
    OpDef { cmd: 10, name: "Base64 Encode", group: 0, op: Op::B64Enc },
    OpDef { cmd: 11, name: "Base64 Decode", group: 0, op: Op::B64Dec },
    OpDef { cmd: 12, name: "URL Encode", group: 0, op: Op::UrlEnc },
    OpDef { cmd: 13, name: "URL Decode", group: 0, op: Op::UrlDec },
    OpDef { cmd: 14, name: "HTML Escape", group: 0, op: Op::HtmlEsc },
    OpDef { cmd: 15, name: "HTML Unescape", group: 0, op: Op::HtmlUnesc },
    OpDef { cmd: 20, name: "JSON Format", group: 1, op: Op::JsonFormat },
    OpDef { cmd: 21, name: "JSON Minify", group: 1, op: Op::JsonMinify },
    OpDef { cmd: 30, name: "Reverse Lines", group: 2, op: Op::Reverse },
    OpDef { cmd: 31, name: "Sort Lines by Length", group: 2, op: Op::SortLen },
    OpDef { cmd: 32, name: "Shuffle Lines", group: 2, op: Op::Shuffle },
    OpDef { cmd: 33, name: "Number Lines", group: 2, op: Op::Number },
    OpDef { cmd: 34, name: "Remove Line Numbers", group: 2, op: Op::Unnumber },
    OpDef { cmd: 35, name: "Wrap Lines at Column", group: 2, op: Op::Wrap },
    OpDef { cmd: 36, name: "Trim Lines", group: 2, op: Op::Trim },
    OpDef { cmd: 40, name: "Lorem Ipsum", group: 3, op: Op::Lorem },
];

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.";

struct Options {
    wrap_col: f64,
    json_indent: f64,
    zero_pad: bool,
    separator: String,
}

impl Default for Options {
    fn default() -> Self {
        Self { wrap_col: 80.0, json_indent: 2.0, zero_pad: false, separator: ". ".into() }
    }
}

#[derive(Default)]
struct TextTools {
    o: Options,
    last: String,
    /// the window's inputs got their saved values
    filled: bool,
}

// ------------------------------------------------------------------ converters

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() / 3 * 4 + 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.chars().filter(|c| !c.is_whitespace()) {
        if c == '=' {
            break;
        }
        let v = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            _ => return Err(format!("\"{c}\" is not a Base64 character.")),
        };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    let hex = |c: u8| (c as char).to_digit(16).map(|v| v as u8);
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => match (hex(b[i + 1]), hex(b[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h << 4 | l);
                    i += 3;
                    continue;
                }
                _ => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn html_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        let end = rest[..rest.len().min(12)].find(';');
        let decoded = end.and_then(|e| {
            let name = &rest[1..e];
            let c = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16).ok().and_then(char::from_u32),
                n if n.starts_with('#') => n[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            c.map(|c| (c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Pretty-print or minify JSON without parsing it into values (keeps numbers and key order as written).
fn json(s: &str, indent: usize, minify: bool) -> Result<String, String> {
    let chars: Vec<char> = s.chars().collect();
    let first = chars.iter().find(|c| !c.is_whitespace());
    if !matches!(first, Some('{') | Some('[')) {
        return Err("This does not look like JSON (it should start with { or [).".into());
    }
    let pad = " ".repeat(indent);
    let mut out = String::with_capacity(s.len() * 2);
    let (mut depth, mut in_str, mut esc) = (0usize, false, false);
    let newline = |out: &mut String, depth: usize| {
        out.push('\n');
        for _ in 0..depth {
            out.push_str(&pad);
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                out.push(c);
            }
            '{' | '[' => {
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                    out.push(c);
                    out.push(chars[j]);
                    i = j + 1;
                    continue;
                }
                depth += 1;
                out.push(c);
                if !minify {
                    newline(&mut out, depth);
                }
            }
            '}' | ']' => {
                if depth == 0 {
                    return Err(format!("Unexpected \"{c}\": the brackets do not match."));
                }
                depth -= 1;
                if !minify {
                    newline(&mut out, depth);
                }
                out.push(c);
            }
            ',' => {
                out.push(c);
                if !minify {
                    newline(&mut out, depth);
                }
            }
            ':' => {
                out.push(':');
                if !minify {
                    out.push(' ');
                }
            }
            c if c.is_whitespace() => {}
            c => out.push(c),
        }
        i += 1;
    }
    if in_str {
        return Err("A string is not closed (missing \").".into());
    }
    if depth != 0 {
        return Err("Some brackets are not closed.".into());
    }
    Ok(out)
}

/// Lines of the text and whether it ended with a line break.
fn split_lines(s: &str) -> (Vec<String>, bool) {
    let norm = s.replace("\r\n", "\n").replace('\r', "\n");
    let trailing = norm.ends_with('\n');
    let body = if trailing { &norm[..norm.len() - 1] } else { &norm[..] };
    (body.split('\n').map(str::to_string).collect(), trailing)
}

fn join_lines(lines: Vec<String>, trailing: bool) -> String {
    let mut s = lines.join("\n");
    if trailing {
        s.push('\n');
    }
    s
}

fn wrap(line: &str, col: usize) -> Vec<String> {
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    if line.chars().count() <= col {
        return vec![line.to_string()];
    }
    let mut out = Vec::new();
    let mut cur = indent.clone();
    for w in line.split_whitespace() {
        let need = if cur.trim().is_empty() { w.chars().count() } else { w.chars().count() + 1 };
        if cur.trim().is_empty() {
            cur.push_str(w);
        } else if cur.chars().count() + need <= col {
            cur.push(' ');
            cur.push_str(w);
        } else {
            out.push(std::mem::replace(&mut cur, format!("{indent}{w}")));
        }
    }
    out.push(cur);
    out
}

struct Rng(u64);

impl Rng {
    fn new() -> Self {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        Rng(t | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

// ------------------------------------------------------------------ plugin

impl TextTools {
    fn load_options(&mut self, host: &Host) {
        let d = Options::default();
        let num = |k: &str, def: f64| host.setting(k).parse::<f64>().unwrap_or(def);
        // stored as "s:<text>" so that an empty separator can be told from "never set"
        let sep = host.setting("separator");
        self.o = Options {
            wrap_col: num("wrap_col", d.wrap_col),
            json_indent: num("json_indent", d.json_indent),
            zero_pad: host.setting("zero_pad") == "1",
            separator: sep.strip_prefix("s:").map(str::to_string).unwrap_or(d.separator),
        };
    }

    fn convert(&self, op: Op, input: &str) -> Result<String, String> {
        let o = &self.o;
        Ok(match op {
            Op::B64Enc => b64_encode(input.as_bytes()),
            Op::B64Dec => {
                let bytes = b64_decode(input)?;
                String::from_utf8(bytes).map_err(|_| "The decoded data is not UTF-8 text.".to_string())?
            }
            Op::UrlEnc => url_encode(input),
            Op::UrlDec => url_decode(input),
            Op::HtmlEsc => html_escape(input),
            Op::HtmlUnesc => html_unescape(input),
            Op::JsonFormat => json(input, o.json_indent.round() as usize, false)?,
            Op::JsonMinify => json(input, 0, true)?,
            Op::Lorem => LOREM.to_string(),
            _ => {
                let (mut lines, trailing) = split_lines(input);
                match op {
                    Op::Reverse => lines.reverse(),
                    Op::SortLen => lines.sort_by_key(|l| l.chars().count()),
                    Op::Shuffle => {
                        let mut r = Rng::new();
                        for i in (1..lines.len()).rev() {
                            lines.swap(i, (r.next() % (i as u64 + 1)) as usize);
                        }
                    }
                    Op::Number => {
                        let w = lines.len().to_string().len();
                        lines = lines.iter().enumerate().map(|(i, l)| {
                            if o.zero_pad { format!("{:0w$}{}{l}", i + 1, o.separator) } else { format!("{:>w$}{}{l}", i + 1, o.separator) }
                        }).collect();
                    }
                    Op::Unnumber => {
                        lines = lines.iter().map(|l| {
                            let t = l.trim_start();
                            let digits = t.len() - t.trim_start_matches(|c: char| c.is_ascii_digit()).len();
                            if digits == 0 {
                                return l.clone();
                            }
                            t[digits..].trim_start_matches(['.', ')', ':', '|', '-']).trim_start_matches([' ', '\t']).to_string()
                        }).collect();
                    }
                    Op::Wrap => {
                        let col = (o.wrap_col.round() as usize).max(10);
                        lines = lines.iter().flat_map(|l| wrap(l, col)).collect();
                    }
                    Op::Trim => lines = lines.iter().map(|l| l.trim().to_string()).collect(),
                    _ => {}
                }
                join_lines(lines, trailing)
            }
        })
    }

    fn run(&mut self, host: &Host, def: &OpDef) {
        self.last = def.name.to_string();
        if def.op == Op::Lorem {
            host.replace_selection(LOREM);
            return;
        }
        let (a, b) = host.selection();
        let whole = a == b;
        let input = if whole { host.text() } else { host.selected_text() };
        if input.is_empty() {
            host.show_info("Text Tools: the document is empty");
            return;
        }
        match self.convert(def.op, &input) {
            Ok(out) if out.replace("\r\n", "\n") == input.replace("\r\n", "\n") => {
                host.show_info(&format!("{}: nothing to change", def.name));
            }
            Ok(out) => {
                if whole {
                    host.set_text(&out);
                } else {
                    host.replace_selection(&out);
                }
                host.show_info(&format!("{}: done{}", def.name, if whole { " (whole document)" } else { "" }));
            }
            Err(e) => host.message(def.name, &e),
        }
    }
}

impl Plugin for TextTools {
    fn info() -> Info {
        Info {
            name: "Text Tools",
            version: "1.0.0",
            author: "Notepad9",
            description: "Base64, URL, HTML and JSON converters, line tools (reverse, shuffle, number, wrap ...) and Lorem Ipsum. Works on the selection or on the whole text.",
        }
    }

    fn init(&mut self, host: &Host) {
        self.load_options(host);
        host.add_toolbar_button_png(ICON, "Text Tools", CMD_WINDOW);
        host.add_menu_item("Text Tools/Text Tools Window...", "Ctrl+Alt+X", CMD_WINDOW);
        for d in OPS {
            let sc = match d.op {
                Op::JsonFormat => "Ctrl+Alt+J",
                _ => "",
            };
            host.add_menu_item(&format!("Text Tools/{}", d.name), sc, d.cmd);
        }
    }

    fn on_command(&mut self, host: &Host, command: u32) {
        if command == CMD_WINDOW {
            host.toggle_window(WIN, "Text Tools", WindowKind::Floating, 400.0, 0.0);
            return;
        }
        if let Some(d) = OPS.iter().find(|d| d.cmd == command) {
            self.run(host, d);
        }
    }

    fn ui(&mut self, host: &Host, _window: u32, ui: &mut Ui) {
        ui.weak("Works on the selection, or on the whole text when nothing is selected.");
        ui.space(4.0);
        let mut pick: Option<&'static OpDef> = None;
        for (gi, g) in GROUPS.iter().enumerate() {
            ui.collapsing(g, g, gi < 3, |ui| {
                ui.wrap(|ui| {
                    for d in OPS.iter().filter(|d| d.group == gi) {
                        if ui.button(d.name, d.name) {
                            pick = Some(d);
                        }
                    }
                });
            });
        }
        if !self.filled {
            self.filled = true;
            ui.set_input("separator", &self.o.separator);
        }
        ui.collapsing("options", "Options", false, |ui| {
            let wrap_col = ui.slider("wrap_col", "Wrap column", 20.0, 200.0, self.o.wrap_col).round();
            let indent = ui.slider("json_indent", "JSON indent", 1.0, 8.0, self.o.json_indent).round();
            let zero = ui.checkbox("zero_pad", "Number lines with leading zeros (001, 002 ...)", self.o.zero_pad);
            let mut sep = String::new();
            ui.horizontal(|ui| {
                ui.label("Line number separator:");
                sep = ui.text_input("separator", "(none)");
            });
            if wrap_col != self.o.wrap_col {
                self.o.wrap_col = wrap_col;
                host.set_setting("wrap_col", &wrap_col.to_string());
            }
            if indent != self.o.json_indent {
                self.o.json_indent = indent;
                host.set_setting("json_indent", &indent.to_string());
            }
            if zero != self.o.zero_pad {
                self.o.zero_pad = zero;
                host.set_setting("zero_pad", if zero { "1" } else { "0" });
            }
            if sep != self.o.separator {
                host.set_setting("separator", &format!("s:{sep}"));
                self.o.separator = sep;
            }
            if ui.button("reset", "Reset options") {
                self.o = Options::default();
                for k in ["wrap_col", "json_indent", "zero_pad", "separator"] {
                    host.set_setting(k, "");
                }
                ui.set_slider("wrap_col", self.o.wrap_col);
                ui.set_slider("json_indent", self.o.json_indent);
                ui.set_checkbox("zero_pad", self.o.zero_pad);
                ui.set_input("separator", &self.o.separator);
            }
        });
        if !self.last.is_empty() {
            ui.space(4.0);
            ui.small(&format!("Last used: {}", self.last));
        }
        if let Some(d) = pick {
            self.run(host, d);
        }
    }
}

export_plugin!(TextTools);
