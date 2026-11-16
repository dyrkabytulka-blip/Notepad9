//! Incremental syntax highlighting with syntect. Grammar loading happens on a background
//! thread at start-up, and lines are only parsed up to what is (about to be) shown.

use crate::doc::Doc;
use std::collections::HashMap;
use std::sync::OnceLock;
use syntect::parsing::{ParseState, Scope, ScopeStack, ScopeStackOp, SyntaxSet};

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();

/// Start loading the grammars in the background (≈50 ms) so the window is not delayed.
pub fn preload() {
    std::thread::spawn(|| {
        let _ = syntaxes();
    });
}

pub fn syntaxes() -> &'static SyntaxSet {
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

pub fn ready() -> Option<&'static SyntaxSet> {
    SYNTAXES.get()
}

/// Syntax categories, mapped to the theme's "syn.*" colors.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cat {
    Plain,
    Comment,
    Str,
    Keyword,
    Number,
    Type,
    Method,
    Preprocessor,
    Tag,
    Attribute,
    Entity,
    Operator,
    Link,
    Heading,
    Emphasis,
    Strong,
}

impl Cat {
    pub fn color_id(self) -> &'static str {
        match self {
            Cat::Plain => "ed.fg",
            Cat::Comment => "syn.comment",
            Cat::Str | Cat::Emphasis => "syn.string",
            Cat::Keyword => "syn.keyword",
            Cat::Number => "syn.number",
            Cat::Type => "syn.type",
            Cat::Method | Cat::Strong => "syn.method",
            Cat::Preprocessor => "syn.preprocessor",
            Cat::Tag | Cat::Heading => "syn.tag",
            Cat::Attribute => "syn.attribute",
            Cat::Entity => "syn.entity",
            Cat::Operator => "syn.operator",
            Cat::Link => "ed.link",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub start: usize, // char column
    pub end: usize,
    pub cat: Cat,
}

fn classify_name(n: &str) -> Option<Cat> {
    let starts = |p: &str| n == p || n.starts_with(&format!("{p}."));
    Some(if starts("comment") || starts("punctuation.definition.comment") {
        Cat::Comment
    } else if starts("markup.heading") || starts("entity.name.section") {
        Cat::Heading
    } else if starts("markup.underline.link") || starts("markup.link") || starts("string.other.link") {
        Cat::Link
    } else if starts("markup.bold") {
        Cat::Strong
    } else if starts("markup.italic") {
        Cat::Emphasis
    } else if starts("markup.raw") || starts("string") || starts("constant.character.escape") {
        Cat::Str
    } else if starts("constant.numeric") {
        Cat::Number
    } else if starts("constant.character.entity") || starts("entity.other.attribute-name.class")
        || starts("entity.other.attribute-name.id") || starts("entity.name.tag.css") {
        Cat::Entity
    } else if starts("entity.name.tag") || starts("punctuation.definition.tag") {
        Cat::Tag
    } else if starts("entity.other.attribute-name") || starts("support.type.property-name") || starts("variable.other.member")
        || starts("meta.object-literal.key") {
        Cat::Attribute
    } else if starts("meta.preprocessor") || starts("keyword.control.import") || starts("keyword.control.directive")
        || starts("keyword.other.preprocessor") || starts("meta.attribute") || starts("keyword.control.cs.preprocessor") {
        Cat::Preprocessor
    } else if starts("entity.name.function") || starts("support.function") || starts("variable.function")
        || starts("meta.function-call") {
        Cat::Method
    } else if starts("entity.name.type") || starts("entity.name.class") || starts("entity.name.struct")
        || starts("entity.name.enum") || starts("support.type") || starts("support.class") || starts("entity.other.inherited-class")
        || starts("storage.type.primitive") || starts("storage.type.numeric") {
        Cat::Type
    } else if starts("keyword.operator") || starts("punctuation.separator") || starts("punctuation.accessor")
        || starts("punctuation.terminator") {
        Cat::Operator
    } else if starts("keyword") || starts("storage") || starts("constant.language") || starts("variable.language") {
        Cat::Keyword
    } else {
        return None;
    })
}

pub struct Highlighter {
    syntax: Option<String>,
    states: Vec<(ParseState, ScopeStack)>,
    spans: Vec<Vec<Span>>,
    cache: HashMap<Scope, Option<Cat>>,
    pub enabled: bool,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self { syntax: None, states: Vec::new(), spans: Vec::new(), cache: HashMap::new(), enabled: true }
    }
}

const MAX_LINE: usize = 20_000;
const MAX_DOC: usize = 20_000_000;

impl Highlighter {
    pub fn syntax(&self) -> Option<&str> {
        self.syntax.as_deref()
    }

    pub fn set_syntax(&mut self, name: Option<String>) {
        self.syntax = name;
        self.reset();
    }

    pub fn reset(&mut self) {
        self.states.clear();
        self.spans.clear();
    }

    pub fn invalidate(&mut self, line: usize) {
        self.spans.truncate(line);
        self.states.truncate(line + 1);
    }

    pub fn spans(&self, line: usize) -> Option<&[Span]> {
        self.spans.get(line).map(|v| v.as_slice())
    }

    pub fn active(&self) -> bool {
        self.enabled && self.syntax.is_some()
    }

    /// Parse lines until `upto` (inclusive), at most `budget` lines. Returns true when done.
    pub fn ensure(&mut self, doc: &Doc, upto: usize, budget: usize) -> bool {
        if !self.active() || doc.len_chars() > MAX_DOC {
            return true;
        }
        let Some(ss) = ready() else { return false };
        let Some(syntax) = self.syntax.as_deref().and_then(|n| ss.find_syntax_by_name(n)) else {
            self.syntax = None;
            return true;
        };
        if self.states.is_empty() {
            self.states.push((ParseState::new(syntax), ScopeStack::new()));
        }
        let upto = upto.min(doc.len_lines().saturating_sub(1));
        let mut done = 0;
        while self.spans.len() <= upto {
            if done >= budget {
                return false;
            }
            let l = self.spans.len();
            let (mut ps, mut stack) = self.states[l].clone();
            let mut line = doc.line_string(l);
            let mut spans = Vec::new();
            if line.chars().count() <= MAX_LINE {
                line.push('\n');
                if let Ok(ops) = ps.parse_line(&line, ss) {
                    let mut last = 0usize; // byte
                    let mut col = 0usize; // char
                    let mut apply_and_emit = |stack: &mut ScopeStack, upto_byte: usize, col: &mut usize, last: &mut usize, cache: &mut HashMap<Scope, Option<Cat>>| {
                        if upto_byte > *last {
                            let text = &line[*last..upto_byte];
                            let n = text.chars().count();
                            let cat = classify(stack, cache);
                            if cat != Cat::Plain {
                                spans.push(Span { start: *col, end: *col + n, cat });
                            }
                            *col += n;
                            *last = upto_byte;
                        }
                    };
                    for (pos, op) in &ops {
                        apply_and_emit(&mut stack, *pos, &mut col, &mut last, &mut self.cache);
                        let _ = stack.apply(op);
                    }
                    let end = line.len() - 1;
                    apply_and_emit(&mut stack, end, &mut col, &mut last, &mut self.cache);
                }
            }
            self.spans.push(spans);
            self.states.push((ps, stack));
            done += 1;
        }
        true
    }
}

fn classify(stack: &ScopeStack, cache: &mut HashMap<Scope, Option<Cat>>) -> Cat {
    for s in stack.as_slice().iter().rev() {
        let c = *cache.entry(*s).or_insert_with(|| classify_name(&s.build_string()));
        if let Some(c) = c {
            return c;
        }
    }
    Cat::Plain
}

/// Syntax for a file name (by extension), or None for plain text.
pub fn syntax_for_path(path: &str) -> Option<String> {
    let ss = syntaxes();
    let p = std::path::Path::new(path);
    let ext = p.extension()?.to_string_lossy().to_lowercase();
    let name = match ext.as_str() {
        "txt" | "log" | "text" => return None,
        "csx" => "C#",
        "xaml" | "csproj" | "props" | "targets" | "resx" | "svg" | "config" | "manifest" | "vcxproj" => "XML",
        "jsonc" => "JSON",
        "ts" | "mts" | "tsx" | "mjs" | "cjs" => "JavaScript",
        "h" | "hpp" | "hh" | "cc" | "cxx" | "ino" => "C++",
        "kt" | "kts" => "Java",
        "ps1" | "psm1" => "Bourne Again Shell (bash)",
        "toml" | "ini" | "cfg" | "conf" => "YAML",
        "cmd" => "Batch File",
        _ => {
            return ss.find_syntax_by_extension(&ext).map(|s| s.name.clone());
        }
    };
    ss.find_syntax_by_name(name).map(|s| s.name.clone())
}

pub fn all_syntax_names() -> Vec<String> {
    let mut v: Vec<String> = syntaxes()
        .syntaxes()
        .iter()
        .filter(|s| !s.hidden && s.name != "Plain Text")
        .map(|s| s.name.clone())
        .collect();
    v.sort_by_key(|s| s.to_lowercase());
    v.dedup();
    v
}

/// Ignore the ScopeStackOp import warning on some syntect versions.
#[allow(dead_code)]
fn _unused(_: ScopeStackOp) {}
