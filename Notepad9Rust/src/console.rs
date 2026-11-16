//! Console panel: cmd.exe with redirected input/output, run-file support.

use crate::theme::Palette;
use eframe::egui::{self, text::LayoutJob, Color32, FontId, Key, RichText, TextFormat};
use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const MAX: usize = 200_000;

pub enum Action {
    None,
    Run,
    Close,
}

pub struct Console {
    pub visible: bool,
    pub height: f32,
    output: String,
    input: String,
    history: Vec<String>,
    hist: Option<usize>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    gen: u64,
    tx: Sender<(u64, Vec<u8>)>,
    rx: Receiver<(u64, Vec<u8>)>,
    pending: Vec<u8>,
    state: String,
    pub focus_input: bool,
}

impl Default for Console {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            visible: false,
            height: 220.0,
            output: String::new(),
            input: String::new(),
            history: Vec::new(),
            hist: None,
            child: None,
            stdin: None,
            gen: 0,
            tx,
            rx,
            pending: Vec::new(),
            state: String::new(),
            focus_input: false,
        }
    }
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\"))
}

impl Console {
    pub fn alive(&mut self) -> bool {
        match &mut self.child {
            Some(c) => matches!(c.try_wait(), Ok(None)),
            None => false,
        }
    }

    fn spawn(&mut self, args: &str, dir: &Path, ctx: &egui::Context) {
        self.kill();
        let dir = if dir.is_dir() { dir.to_path_buf() } else { home() };
        let child = Command::new("cmd.exe")
            .raw_arg(args)
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PYTHONIOENCODING", "utf-8")
            .env("PYTHONUNBUFFERED", "1")
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
        match child {
            Ok(mut c) => {
                self.gen += 1;
                let gen = self.gen;
                for stream in [c.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>), c.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>)]
                    .into_iter()
                    .flatten()
                {
                    let tx = self.tx.clone();
                    let ctx = ctx.clone();
                    std::thread::spawn(move || {
                        let mut s = stream;
                        let mut buf = [0u8; 4096];
                        while let Ok(n) = s.read(&mut buf) {
                            if n == 0 || tx.send((gen, buf[..n].to_vec())).is_err() {
                                break;
                            }
                            ctx.request_repaint();
                        }
                    });
                }
                self.stdin = c.stdin.take();
                self.child = Some(c);
                self.state = format!("cmd · {}", dir.display());
            }
            Err(e) => self.append(&format!("\nCould not start cmd.exe: {e}\n")),
        }
    }

    pub fn ensure_shell(&mut self, dir: Option<&Path>, ctx: &egui::Context) {
        if !self.alive() {
            let d = dir.map(|d| d.to_path_buf()).unwrap_or_else(home);
            self.spawn("/K chcp 65001>nul", &d, ctx);
        }
    }

    /// Run a command line in a fresh shell; the shell stays open afterwards.
    pub fn run(&mut self, cmdline: &str, dir: &Path, ctx: &egui::Context) {
        let head = if self.output.is_empty() { "" } else { "\n" };
        self.append(&format!("{head}\u{25B6} {cmdline}\n"));
        let d = dir.display().to_string();
        let delayed = !cmdline.contains('!') && !d.contains('!');
        let tail = if delayed { " & echo. & echo [exit code !errorlevel!]" } else { "" };
        let inner = format!("chcp 65001>nul & cd /d \"{d}\" & {cmdline}{tail}");
        let args = format!("{}/S /K \"{inner}\"", if delayed { "/V:ON " } else { "" });
        self.spawn(&args, dir, ctx);
        self.focus_input = true;
    }

    pub fn send(&mut self, line: &str, dir: Option<&Path>, ctx: &egui::Context) {
        self.ensure_shell(dir, ctx);
        let failed = match &mut self.stdin {
            Some(s) => {
                let r = s.write_all(format!("{line}\r\n").as_bytes()).is_err();
                let _ = s.flush();
                r
            }
            None => false,
        };
        if failed {
            self.append("\n[could not write to the shell]\n");
        }
    }

    pub fn kill(&mut self) {
        if let Some(mut c) = self.child.take() {
            let pid = c.id();
            let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &pid.to_string()]).creation_flags(CREATE_NO_WINDOW).status();
            let _ = c.kill();
        }
        self.stdin = None;
        self.gen += 1;
        self.pending.clear();
    }

    pub fn stop(&mut self, dir: Option<&Path>, ctx: &egui::Context) {
        let was = self.alive();
        self.kill();
        if was {
            self.append("\n\u{25A0} stopped\n");
        }
        self.ensure_shell(dir, ctx);
    }

    pub fn clear(&mut self) {
        self.output.clear();
    }

    fn append(&mut self, s: &str) {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        self.output.push_str(&strip_ansi(&s));
        if self.output.len() > MAX {
            let cut = self.output.len() - MAX * 3 / 4;
            let cut = (cut..self.output.len()).find(|&i| self.output.is_char_boundary(i)).unwrap_or(cut);
            self.output.drain(..cut);
        }
    }

    pub fn poll(&mut self) {
        let mut got = false;
        while let Ok((gen, bytes)) = self.rx.try_recv() {
            if gen != self.gen {
                continue;
            }
            self.pending.extend_from_slice(&bytes);
            got = true;
        }
        if got {
            // keep an incomplete UTF-8 sequence for the next chunk
            let valid = match std::str::from_utf8(&self.pending) {
                Ok(_) => self.pending.len(),
                Err(e) if e.error_len().is_none() => e.valid_up_to(),
                Err(_) => self.pending.len(),
            };
            let chunk: Vec<u8> = self.pending.drain(..valid).collect();
            let text = String::from_utf8_lossy(&chunk).to_string();
            self.append(&text);
        }
        if self.child.is_some() && !self.alive() {
            self.child = None;
            self.stdin = None;
            self.append("\n[shell exited]\n");
            self.state = "not running".into();
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, pal: &Palette, icons: &mut crate::icons::Icons, dir: Option<&Path>) -> Action {
        let mut action = Action::None;
        let ctx = ui.ctx().clone();
        // header
        egui::Frame::none().fill(pal.c("ui.panel")).inner_margin(egui::Margin::symmetric(8.0, 3.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("CONSOLE").size(11.0).strong().color(pal.c("ui.textDim")));
                ui.label(RichText::new(&self.state).size(11.0).color(pal.c("ui.textDisabled")));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(crate::widgets::Button::new("✕").frame(false)).on_hover_text("Hide console (Ctrl+`)").clicked() {
                        action = Action::Close;
                    }
                    for (icon, tip, what) in [("clear", "Clear (Ctrl+L)", 3), ("open", "Go to the folder of the current file", 2),
                                              ("stop", "Stop: kill the running program", 1), ("run", "Run current file (F5)", 0)] {
                        let img = icons.image(&ctx, icon, 16.0, true);
                        let b = match img {
                            Some(i) => ui.add(crate::widgets::Button::image(i).frame(false)),
                            None => crate::widgets::button(ui, icon),
                        };
                        if b.on_hover_text(tip).clicked() {
                            match what {
                                0 => action = Action::Run,
                                1 => self.stop(dir, &ctx),
                                2 => {
                                    if let Some(d) = dir {
                                        let line = format!("cd /d \"{}\"", d.display());
                                        self.send(&line, dir, &ctx);
                                    }
                                }
                                _ => self.clear(),
                            }
                        }
                    }
                });
            });
        });
        let font = FontId::new(13.0, egui::FontFamily::Name("editor".into()));
        // input line (bottom)
        let input_id = egui::Id::new("console_input");
        egui::TopBottomPanel::bottom("console_input_panel")
            .frame(egui::Frame::none().fill(pal.c("ui.panel")).inner_margin(egui::Margin::symmetric(8.0, 4.0)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("\u{276F}").font(font.clone()).color(Color32::from_rgb(0x6E, 0xE0, 0x5A)));
                    let te = egui::TextEdit::singleline(&mut self.input)
                        .id(input_id)
                        .font(font.clone())
                        .frame(false)
                        .desired_width(f32::INFINITY)
                        .text_color(pal.c("ui.consoleText"))
                        .hint_text("Type a command and press Enter");
                    let r = ui.add(te);
                    if self.focus_input {
                        r.request_focus();
                        self.focus_input = false;
                    }
                    if r.has_focus() {
                        let (up, down, ctrl_l, ctrl_c, esc) = ui.input(|i| {
                            (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.modifiers.ctrl && i.key_pressed(Key::L),
                             i.modifiers.ctrl && i.key_pressed(Key::C), i.key_pressed(Key::Escape))
                        });
                        if up && !self.history.is_empty() {
                            let i = self.hist.map_or(self.history.len() - 1, |i| i.saturating_sub(1));
                            self.hist = Some(i);
                            self.input = self.history[i].clone();
                        }
                        if down {
                            if let Some(i) = self.hist {
                                if i + 1 < self.history.len() {
                                    self.hist = Some(i + 1);
                                    self.input = self.history[i + 1].clone();
                                } else {
                                    self.hist = None;
                                    self.input.clear();
                                }
                            }
                        }
                        if ctrl_l {
                            self.clear();
                        }
                        if ctrl_c {
                            self.stop(dir, &ctx);
                        }
                        if esc {
                            action = Action::Close;
                        }
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        let line = std::mem::take(&mut self.input);
                        if line.trim().eq_ignore_ascii_case("cls") {
                            self.clear();
                        }
                        if !line.is_empty() && self.history.last() != Some(&line) {
                            self.history.push(line.clone());
                        }
                        self.hist = None;
                        self.send(&line, dir, &ctx);
                        r.request_focus();
                    }
                });
            });
        // output
        egui::Frame::none().fill(pal.c("ui.console")).inner_margin(egui::Margin { left: 8.0, right: 2.0, top: 4.0, bottom: 2.0 }).show(ui, |ui| {
            let fg = pal.c("ui.consoleText");
            let mut layouter = |ui: &egui::Ui, text: &str, wrap: f32| {
                let mut job = LayoutJob::default();
                for line in text.split_inclusive('\n') {
                    let color = if line.starts_with('\u{25B6}') {
                        Color32::from_rgb(0x6E, 0xE0, 0x5A)
                    } else if line.starts_with('\u{25A0}') {
                        Color32::from_rgb(0xE0, 0xA0, 0x40)
                    } else if line.starts_with("[exit code 0]") || line.starts_with("[shell exited]") {
                        Color32::from_gray(0x7A)
                    } else if line.starts_with("[exit code") {
                        Color32::from_rgb(0xF0, 0x6A, 0x5A)
                    } else {
                        fg
                    };
                    job.append(line, 0.0, TextFormat { font_id: font.clone(), color, ..Default::default() });
                }
                job.wrap.max_width = wrap;
                ui.fonts(|f| f.layout_job(job))
            };
            egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                let mut text = self.output.as_str();
                ui.add(egui::TextEdit::multiline(&mut text).frame(false).desired_width(f32::INFINITY).layouter(&mut layouter));
            });
        });
        action
    }
}

fn strip_ansi(s: &str) -> String {
    if !s.contains('\u{1b}') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            if it.peek() == Some(&'[') {
                it.next();
                for d in it.by_ref() {
                    if ('@'..='~').contains(&d) {
                        break;
                    }
                }
            } else if it.peek() == Some(&']') {
                for d in it.by_ref() {
                    if d == '\u{7}' {
                        break;
                    }
                }
            } else {
                it.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}
