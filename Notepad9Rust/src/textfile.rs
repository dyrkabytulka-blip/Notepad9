//! Reading / writing text files: encoding and line-ending detection, byte-exact binary mode.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(from = "u8", into = "u8")]
pub enum FileEncoding {
    Ansi,
    #[default]
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    /// every byte is one character (Latin-1), written back byte for byte
    Binary,
}

impl From<u8> for FileEncoding {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::Ansi,
            2 => Self::Utf8Bom,
            3 => Self::Utf16Le,
            4 => Self::Utf16Be,
            5 => Self::Binary,
            _ => Self::Utf8,
        }
    }
}

impl From<FileEncoding> for u8 {
    fn from(e: FileEncoding) -> u8 {
        match e {
            FileEncoding::Ansi => 0,
            FileEncoding::Utf8 => 1,
            FileEncoding::Utf8Bom => 2,
            FileEncoding::Utf16Le => 3,
            FileEncoding::Utf16Be => 4,
            FileEncoding::Binary => 5,
        }
    }
}

pub const ALL_ENCODINGS: [FileEncoding; 6] = [
    FileEncoding::Ansi, FileEncoding::Utf8, FileEncoding::Utf8Bom, FileEncoding::Utf16Le, FileEncoding::Utf16Be, FileEncoding::Binary,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(from = "u8", into = "u8")]
pub enum Eol {
    #[default]
    CrLf,
    Lf,
    Cr,
}

impl From<u8> for Eol {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::Lf,
            2 => Self::Cr,
            _ => Self::CrLf,
        }
    }
}

impl From<Eol> for u8 {
    fn from(e: Eol) -> u8 {
        match e {
            Eol::CrLf => 0,
            Eol::Lf => 1,
            Eol::Cr => 2,
        }
    }
}

impl Eol {
    pub fn as_str(self) -> &'static str {
        match self {
            Eol::CrLf => "\r\n",
            Eol::Lf => "\n",
            Eol::Cr => "\r",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Eol::CrLf => "CR+LF",
            Eol::Lf => "LF",
            Eol::Cr => "CR",
        }
    }
}

/// The system ANSI code page (e.g. 1251 for Russian Windows).
pub fn ansi() -> &'static encoding_rs::Encoding {
    use std::sync::OnceLock;
    static ENC: OnceLock<&'static encoding_rs::Encoding> = OnceLock::new();
    ENC.get_or_init(|| {
        let cp = unsafe { windows_sys::Win32::Globalization::GetACP() };
        let label = match cp {
            932 => "shift_jis".to_string(),
            936 => "gbk".to_string(),
            949 => "euc-kr".to_string(),
            950 => "big5".to_string(),
            874 => "windows-874".to_string(),
            c => format!("windows-{c}"),
        };
        encoding_rs::Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::WINDOWS_1252)
    })
}

pub fn ansi_code_page() -> u32 {
    unsafe { windows_sys::Win32::Globalization::GetACP() }
}

pub fn encoding_name(e: FileEncoding) -> String {
    match e {
        FileEncoding::Ansi => format!("ANSI ({})", ansi_code_page()),
        FileEncoding::Utf8 => "Unicode (UTF-8)".into(),
        FileEncoding::Utf8Bom => "Unicode (UTF-8 BOM)".into(),
        FileEncoding::Utf16Le => "Unicode (UTF-16 LE BOM)".into(),
        FileEncoding::Utf16Be => "Unicode (UTF-16 BE BOM)".into(),
        FileEncoding::Binary => "Binary (byte-exact)".into(),
    }
}

/// NUL bytes or lots of control characters in the first 64 KB.
pub fn looks_binary(b: &[u8]) -> bool {
    let n = b.len().min(65536);
    let mut control = 0usize;
    for &c in &b[..n] {
        if c == 0 {
            return true;
        }
        if c < 0x09 || (c > 0x0D && c < 0x20 && c != 0x1B) {
            control += 1;
        }
    }
    n > 0 && control * 20 > n
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

fn utf16(b: &[u8], big_endian: bool) -> String {
    let units: Vec<u16> = b
        .chunks_exact(2)
        .map(|p| if big_endian { u16::from_be_bytes([p[0], p[1]]) } else { u16::from_le_bytes([p[0], p[1]]) })
        .collect();
    String::from_utf16_lossy(&units)
}

pub fn decode(b: &[u8]) -> (String, FileEncoding, Eol) {
    if b.len() >= 3 && b[..3] == [0xEF, 0xBB, 0xBF] {
        let t = String::from_utf8_lossy(&b[3..]).into_owned();
        let eol = detect_eol(&t);
        return (t, FileEncoding::Utf8Bom, eol);
    }
    if b.len() >= 2 && b[..2] == [0xFF, 0xFE] {
        let t = utf16(&b[2..], false);
        let eol = detect_eol(&t);
        return (t, FileEncoding::Utf16Le, eol);
    }
    if b.len() >= 2 && b[..2] == [0xFE, 0xFF] {
        let t = utf16(&b[2..], true);
        let eol = detect_eol(&t);
        return (t, FileEncoding::Utf16Be, eol);
    }
    if looks_binary(b) {
        return (latin1(b), FileEncoding::Binary, Eol::CrLf);
    }
    match std::str::from_utf8(b) {
        Ok(s) => {
            let eol = detect_eol(s);
            (s.to_string(), FileEncoding::Utf8, eol)
        }
        Err(_) => {
            let (t, _, _) = ansi().decode(b);
            let t = t.into_owned();
            let eol = detect_eol(&t);
            (t, FileEncoding::Ansi, eol)
        }
    }
}

/// Decode with a forced encoding ("Reload as").
pub fn decode_as(b: &[u8], enc: FileEncoding) -> (String, FileEncoding) {
    match enc {
        FileEncoding::Binary => (latin1(b), enc),
        FileEncoding::Ansi => (ansi().decode(b).0.into_owned(), enc),
        FileEncoding::Utf16Le => {
            let s = if b.starts_with(&[0xFF, 0xFE]) { &b[2..] } else { b };
            (utf16(s, false), enc)
        }
        FileEncoding::Utf16Be => {
            let s = if b.starts_with(&[0xFE, 0xFF]) { &b[2..] } else { b };
            (utf16(s, true), enc)
        }
        FileEncoding::Utf8 | FileEncoding::Utf8Bom => {
            if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
                (String::from_utf8_lossy(&b[3..]).into_owned(), FileEncoding::Utf8Bom)
            } else {
                (String::from_utf8_lossy(b).into_owned(), FileEncoding::Utf8)
            }
        }
    }
}

pub fn detect_eol(t: &str) -> Eol {
    let (mut crlf, mut lf, mut cr) = (0usize, 0usize, 0usize);
    let b = t.as_bytes();
    let n = b.len().min(1_000_000);
    let mut i = 0;
    while i < n {
        match b[i] {
            b'\r' => {
                if i + 1 < b.len() && b[i + 1] == b'\n' {
                    crlf += 1;
                    i += 1;
                } else {
                    cr += 1;
                }
            }
            b'\n' => lf += 1,
            _ => {}
        }
        i += 1;
    }
    if lf > crlf && lf >= cr {
        Eol::Lf
    } else if cr > crlf && cr > lf {
        Eol::Cr
    } else {
        Eol::CrLf
    }
}

pub fn normalize_eol(t: &str, eol: Eol) -> String {
    let nl = eol.as_str();
    let mut out = String::with_capacity(t.len() + t.len() / 40);
    let b = t.as_bytes();
    let mut last = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\r' || b[i] == b'\n' {
            out.push_str(&t[last..i]);
            out.push_str(nl);
            if b[i] == b'\r' && i + 1 < b.len() && b[i + 1] == b'\n' {
                i += 1;
            }
            last = i + 1;
        }
        i += 1;
    }
    out.push_str(&t[last..]);
    out
}

pub fn encode(t: &str, enc: FileEncoding, eol: Eol) -> Vec<u8> {
    if enc == FileEncoding::Binary {
        // no line-ending changes: every char is one byte
        return t.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
    }
    let t = normalize_eol(t, eol);
    match enc {
        FileEncoding::Utf8 => t.into_bytes(),
        FileEncoding::Utf8Bom => {
            let mut v = vec![0xEF, 0xBB, 0xBF];
            v.extend_from_slice(t.as_bytes());
            v
        }
        FileEncoding::Utf16Le => {
            let mut v = vec![0xFF, 0xFE];
            for u in t.encode_utf16() {
                v.extend_from_slice(&u.to_le_bytes());
            }
            v
        }
        FileEncoding::Utf16Be => {
            let mut v = vec![0xFE, 0xFF];
            for u in t.encode_utf16() {
                v.extend_from_slice(&u.to_be_bytes());
            }
            v
        }
        FileEncoding::Ansi => ansi().encode(&t).0.into_owned(),
        FileEncoding::Binary => unreachable!(),
    }
}

/// Size in bytes the text will have on disk.
pub fn byte_count(t: &str, enc: FileEncoding, eol: Eol) -> usize {
    match enc {
        FileEncoding::Binary => t.chars().count(),
        FileEncoding::Utf8 | FileEncoding::Utf8Bom => {
            let breaks = count_breaks(t);
            let raw_break_bytes = raw_break_bytes(t);
            let bom = if enc == FileEncoding::Utf8Bom { 3 } else { 0 };
            t.len() - raw_break_bytes + breaks * eol.as_str().len() + bom
        }
        _ => encode(t, enc, eol).len(),
    }
}

fn count_breaks(t: &str) -> usize {
    let b = t.as_bytes();
    let mut n = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\r' {
            n += 1;
            if i + 1 < b.len() && b[i + 1] == b'\n' {
                i += 1;
            }
        } else if b[i] == b'\n' {
            n += 1;
        }
        i += 1;
    }
    n
}

fn raw_break_bytes(t: &str) -> usize {
    t.bytes().filter(|&c| c == b'\r' || c == b'\n').count()
}

/// Write through a temp file so a crash never leaves a half-written file.
pub fn write(path: &Path, t: &str, enc: FileEncoding, eol: Eol) -> std::io::Result<()> {
    let data = encode(t, enc, eol);
    let tmp = path.with_extension(format!(
        "{}.n9tmp",
        path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default()
    ));
    std::fs::write(&tmp, &data)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // rename over a read-only or locked file failed: write directly
            let _ = std::fs::remove_file(&tmp);
            std::fs::write(path, &data)
        }
    }
}
