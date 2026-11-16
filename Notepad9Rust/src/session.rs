//! Open tabs kept between runs, including unsaved text ("hot exit").

use crate::textfile::{Eol, FileEncoding};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct SessionTab {
    pub path: Option<String>,
    pub untitled_number: u32,
    /// file with the unsaved text, relative to the session folder
    pub backup: Option<String>,
    pub modified: bool,
    pub caret: usize,
    /// first visible line
    pub top_line: Option<usize>,
    pub encoding: FileEncoding,
    pub eol: Eol,
    pub scheme: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct SessionData {
    pub tabs: Vec<SessionTab>,
    pub active: usize,
}

pub fn folder() -> PathBuf {
    crate::settings::folder().join("session")
}

fn file() -> PathBuf {
    folder().join("session.json")
}

pub fn load() -> SessionData {
    std::fs::read_to_string(file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn read_backup(name: &str) -> Option<String> {
    std::fs::read_to_string(folder().join(name)).ok()
}

pub fn backup_exists(name: &str) -> bool {
    folder().join(name).exists()
}

/// `backups` maps tab index → unsaved text.
pub fn save(mut data: SessionData, backups: Vec<(usize, String)>) {
    let dir = folder();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let mut keep: HashSet<String> = HashSet::new();
    keep.insert("session.json".into());
    for (i, text) in backups {
        let name = format!("backup_{}.txt", i + 1);
        if std::fs::write(dir.join(&name), text.as_bytes()).is_ok() {
            if let Some(t) = data.tabs.get_mut(i) {
                t.backup = Some(name.clone());
            }
            keep.insert(name);
        }
    }
    if let Ok(s) = serde_json::to_string_pretty(&data) {
        let tmp = dir.join("session.json.tmp");
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, file());
        }
    }
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !keep.contains(&name) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

pub fn clear() {
    let _ = std::fs::remove_dir_all(folder());
}
