#![windows_subsystem = "windows"]
#![allow(dead_code)]
//! Notepad9 — a dark, fast text editor (Rust + egui).

mod app;
mod complete;
mod console;
mod dialogs;
mod doc;
mod eval;
mod fonts;
mod highlight;
mod icons;
mod ipc;
mod lineops;
mod mdtools;
mod mdview;
mod palette;
mod plugins;
mod runner;
mod search;
mod session;
mod shellicons;
mod sidebar;
mod settings;
mod textfile;
mod theme;
mod ui;
mod view;
mod widgets;
mod win;

use std::time::Instant;

fn main() -> eframe::Result<()> {
    let t0 = Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let force_new = args.iter().any(|a| a.eq_ignore_ascii_case("/new"));
    let files: Vec<String> = args
        .iter()
        .filter(|a| !a.starts_with('/') && !a.starts_with('-'))
        .map(|a| std::path::absolute(a).map(|p| p.display().to_string()).unwrap_or_else(|_| a.clone()))
        .collect();

    // already running: hand the files over before any window or GPU work
    let primary = ipc::acquire_instance();
    if !primary && !force_new && ipc::send_to_primary(&files) {
        return Ok(());
    }

    highlight::preload();
    let s = settings::AppSettings::load();
    let mut vp = eframe::egui::ViewportBuilder::default()
        .with_title("Notepad9")
        .with_inner_size([s.width.max(420.0), s.height.max(240.0)])
        .with_min_inner_size([420.0, 240.0])
        .with_drag_and_drop(true);
    // maximizing is done by App on the first frame, while the window is still hidden
    if let (Some(x), Some(y)) = (s.left, s.top) {
        if x > -2000.0 && y > -200.0 && x < 10000.0 && y < 10000.0 {
            vp = vp.with_position([x, y]);
        }
    }
    if let Some(icon) = icons::app_icon() {
        vp = vp.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport: vp,
        vsync: true,
        centered: s.left.is_none(),
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native("Notepad9", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, s, files, primary, t0)))))
}
