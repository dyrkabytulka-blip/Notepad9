//! settings.json in %APPDATA%\Notepad9 (same names as the C# version, so settings carry over).

use crate::theme::ThemeSettings;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AppSettings {
    pub left: Option<f32>,
    pub top: Option<f32>,
    pub width: f32,
    pub height: f32,
    pub maximized: bool,

    pub font_family: String,
    pub font_size: f32,
    pub second_font_family: String,
    pub use_second_font: bool,
    pub zoom: f32,

    pub word_wrap: bool,
    pub line_numbers: bool,
    pub show_whitespace: bool,
    pub show_eol: bool,
    #[serde(skip)]
    pub long_line_marker: bool,
    pub long_line_column: usize,
    pub highlight_current_line: bool,
    pub mark_occurrences: bool,
    pub show_toolbar: bool,
    pub show_status_bar: bool,
    #[serde(skip)]
    pub always_on_top: bool,

    pub console_height: f32,
    pub md_preview: bool,
    pub md_sync_scroll: bool,
    pub auto_complete: bool,
    pub restore_session: bool,
    pub hot_exit: bool,
    pub theme: ThemeSettings,

    pub tab_width: usize,
    pub tabs_as_spaces: bool,
    pub auto_indent: bool,
    pub auto_detect_scheme: bool,
    pub detect_file_changes: bool,
    pub remember_recent: bool,
    pub save_on_exit: bool,

    pub find_match_case: bool,
    pub find_whole_word: bool,
    pub find_regex: bool,
    pub find_wrap: bool,
    pub last_find: String,
    pub last_replace: String,
    pub last_find_folder: String,
    pub last_find_mask: String,

    pub recent: Vec<String>,
    pub favorites: Vec<String>,
    /// plugin file names (without .dll) that are not loaded
    pub disabled_plugins: Vec<String>,
    pub show_sidebar: bool,
    /// folder shown in the sidebar ("" = none)
    pub sidebar_root: String,
    pub sidebar_width: f32,
    /// commands run from the palette, newest first
    pub palette_recent: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            left: None,
            top: None,
            width: 1100.0,
            height: 700.0,
            maximized: true,
            font_family: "Consolas".into(),
            font_size: 14.667,
            second_font_family: "Segoe UI".into(),
            use_second_font: false,
            zoom: 1.0,
            word_wrap: false,
            line_numbers: true,
            show_whitespace: false,
            show_eol: false,
            long_line_marker: false,
            long_line_column: 80,
            highlight_current_line: true,
            mark_occurrences: true,
            show_toolbar: true,
            show_status_bar: true,
            always_on_top: false,
            console_height: 220.0,
            md_preview: true,
            md_sync_scroll: true,
            auto_complete: true,
            restore_session: true,
            hot_exit: true,
            theme: ThemeSettings::default(),
            tab_width: 4,
            tabs_as_spaces: false,
            auto_indent: true,
            auto_detect_scheme: true,
            detect_file_changes: true,
            remember_recent: true,
            save_on_exit: true,
            find_match_case: false,
            find_whole_word: false,
            find_regex: false,
            find_wrap: true,
            last_find: String::new(),
            last_replace: String::new(),
            last_find_folder: String::new(),
            last_find_mask: "*.*".into(),
            recent: Vec::new(),
            favorites: Vec::new(),
            disabled_plugins: Vec::new(),
            show_sidebar: false,
            sidebar_root: String::new(),
            sidebar_width: 250.0,
            palette_recent: Vec::new(),
        }
    }
}

pub fn folder() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("Notepad9")
}

fn file() -> PathBuf {
    folder().join("settings.json")
}

impl AppSettings {
    pub fn load() -> Self {
        std::fs::read_to_string(file())
            .ok()
            .and_then(|s| serde_json::from_str::<AppSettings>(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(folder());
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let tmp = file().with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, file());
            }
        }
    }

    pub fn reset() {
        let _ = std::fs::remove_file(file());
    }

    pub fn add_recent(&mut self, path: &str) {
        if !self.remember_recent || path.is_empty() {
            return;
        }
        self.recent.retain(|p| !p.eq_ignore_ascii_case(path));
        self.recent.insert(0, path.to_string());
        self.recent.truncate(16);
    }
}
