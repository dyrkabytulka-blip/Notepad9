//! System fonts (Segoe UI, Consolas ...) loaded from C:\Windows\Fonts by family name.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::path::PathBuf;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

const KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts";

fn entries() -> &'static Vec<(String, PathBuf)> {
    static E: std::sync::OnceLock<Vec<(String, PathBuf)>> = std::sync::OnceLock::new();
    E.get_or_init(read_entries)
}

fn read_entries() -> Vec<(String, PathBuf)> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let fonts_dir = PathBuf::from(windir).join("Fonts");
    let mut v = Vec::new();
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        if let Ok(k) = RegKey::predef(root).open_subkey(KEY) {
            for (name, value) in k.enum_values().flatten() {
                let file = value.to_string();
                let file = file.trim_matches('"').to_string();
                let p = PathBuf::from(&file);
                let p = if p.is_absolute() { p } else { fonts_dir.join(p) };
                v.push((name, p));
            }
        }
    }
    v
}

fn clean(name: &str) -> String {
    let mut n = name.to_string();
    for s in [" (TrueType)", " (OpenType)", " (All res)"] {
        n = n.replace(s, "");
    }
    n.trim().to_string()
}

/// Family names that can be used for the editor / interface (regular styles only).
pub fn families() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for (name, p) in entries().iter() {
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
            continue;
        }
        for part in clean(&name).split(" & ") {
            let low = part.to_lowercase();
            if [" bold", " italic", " oblique", " light", " semibold", " black", " thin", " medium", " condensed", " heavy", " semilight", " extra"]
                .iter()
                .any(|s| low.contains(s))
            {
                continue;
            }
            v.push(part.trim().to_string());
        }
    }
    v.sort_by_key(|s| s.to_lowercase());
    v.dedup();
    v
}

/// Font file of a family ("Consolas" → consola.ttf).
pub fn file_of(family: &str) -> Option<PathBuf> {
    let fam = family.to_lowercase();
    let list = entries();
    let mut best: Option<PathBuf> = None;
    for (name, p) in list.iter() {
        let n = clean(name).to_lowercase();
        let parts: Vec<&str> = n.split(" & ").map(|s| s.trim()).collect();
        if parts.iter().any(|x| *x == fam || *x == format!("{fam} regular")) {
            return Some(p.clone());
        }
        if best.is_none() && n.starts_with(&fam) && !n.contains("bold") && !n.contains("italic") {
            best = Some(p.clone());
        }
    }
    best
}

fn load(defs: &mut FontDefinitions, key: &str, family: &str) -> bool {
    if let Some(p) = file_of(family) {
        if let Ok(bytes) = std::fs::read(&p) {
            defs.font_data.insert(key.to_string(), FontData::from_owned(bytes));
            return true;
        }
    }
    false
}

/// Install the interface font, the editor font and a symbol fallback.
pub fn install(ctx: &egui::Context, ui_family: &str, editor_family: &str) {
    install_with_preview(ctx, ui_family, editor_family, None);
}

/// Name of the extra family used by the font dialog to show a font before it is chosen.
pub const PREVIEW: &str = "preview";

/// Fixed-width fonts, by name (good enough for picking an editor font).
pub fn looks_monospace(family: &str) -> bool {
    let f = family.to_lowercase();
    ["mono", "code", "consol", "courier", "terminal", "fixed", "cascadia", "lucida console", "inconsolata", "hack",
     "menlo", "typewriter", "fira", "jetbrains", "iosevka", "hasklig", "sometype", "simsun", "ms gothic", "nsimsun"]
        .iter()
        .any(|k| f.contains(k))
}

/// Like `install`, plus `preview` loaded as the family "preview" (when given and found).
pub fn install_with_preview(ctx: &egui::Context, ui_family: &str, editor_family: &str, preview: Option<&str>) {
    let mut defs = FontDefinitions::default();
    let has_ui = load(&mut defs, "ui", ui_family) || load(&mut defs, "ui", "Segoe UI");
    let has_ed = load(&mut defs, "editor", editor_family) || load(&mut defs, "editor", "Consolas");
    let has_sym = load(&mut defs, "symbols", "Segoe UI Symbol");
    let proportional = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let monospace = defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();

    let mut p = Vec::new();
    if has_ui {
        p.push("ui".to_string());
    }
    p.extend(proportional.iter().cloned());
    if has_sym {
        p.push("symbols".into());
    }
    let mut m = Vec::new();
    if has_ed {
        m.push("editor".to_string());
    }
    m.extend(monospace.iter().cloned());
    if has_ui {
        m.push("ui".into());
    }
    if has_sym {
        m.push("symbols".into());
    }
    defs.families.insert(FontFamily::Proportional, p);
    defs.families.insert(FontFamily::Monospace, m.clone());
    defs.families.insert(FontFamily::Name("editor".into()), m);
    if let Some(f) = preview {
        if load(&mut defs, PREVIEW, f) {
            let mut v = vec![PREVIEW.to_string()];
            if has_ui {
                v.push("ui".into());
            }
            v.extend(proportional.iter().cloned());
            if has_sym {
                v.push("symbols".into());
            }
            defs.families.insert(FontFamily::Name(PREVIEW.into()), v);
        }
    }
    ctx.set_fonts(defs);
}
