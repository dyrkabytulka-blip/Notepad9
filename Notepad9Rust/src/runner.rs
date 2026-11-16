//! How to run a file (F5): a command for the console, the associated program, or the Markdown preview.

use std::path::Path;

pub enum RunKind {
    Console(String),
    Shell,
    Preview,
}

pub fn for_path(path: &Path) -> RunKind {
    let f = format!("\"{}\"", path.display());
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let tmp_exe = format!("\"{}\"", std::env::temp_dir().join(format!("notepad9_run_{stem}.exe")).display());
    let cmd = match ext.as_str() {
        "py" | "pyw" => format!("python -u {f}"),
        "js" | "mjs" | "cjs" => format!("node {f}"),
        "ts" | "mts" => format!("npx --yes tsx {f}"),
        "ps1" => format!("powershell -NoProfile -ExecutionPolicy Bypass -File {f}"),
        "bat" | "cmd" => format!("call {f}"),
        "cs" => format!("dotnet run {f}"),
        "rs" => format!("rustc {f} -o {tmp_exe} && {tmp_exe}"),
        "java" => format!("java {f}"),
        "kt" | "kts" => format!("kotlinc -script {f}"),
        "go" => format!("go run {f}"),
        "rb" => format!("ruby {f}"),
        "php" => format!("php {f}"),
        "lua" => format!("lua {f}"),
        "pl" => format!("perl {f}"),
        "r" => format!("Rscript {f}"),
        "jl" => format!("julia {f}"),
        "dart" => format!("dart run {f}"),
        "swift" => format!("swift {f}"),
        "sh" | "bash" => format!("bash {f}"),
        "vbs" => format!("cscript //nologo {f}"),
        "c" => format!("gcc {f} -o {tmp_exe} && {tmp_exe}"),
        "cpp" | "cc" | "cxx" => format!("g++ -std=c++20 {f} -o {tmp_exe} && {tmp_exe}"),
        "exe" | "com" => f,
        "md" | "markdown" => return RunKind::Preview,
        _ => return RunKind::Shell,
    };
    RunKind::Console(cmd)
}
