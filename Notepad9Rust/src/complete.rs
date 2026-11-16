//! Word completion: words of the document + keywords of the language.

use crate::doc::Doc;
use std::collections::BTreeSet;

pub struct Item {
    pub text: String,
    pub keyword: bool,
}

#[derive(Default)]
pub struct Completion {
    pub open: bool,
    pub start: usize,
    pub items: Vec<Item>,
    pub index: usize,
    words_version: u64,
    words: Vec<String>,
}

fn s(list: &str) -> Vec<&str> {
    list.split_whitespace().collect()
}

pub fn keywords(syntax: Option<&str>) -> Vec<&'static str> {
    let k: &'static str = match syntax.unwrap_or("") {
        "C#" => "abstract as async await base bool break byte case catch char checked class const continue decimal default delegate do double else enum event explicit extern false finally fixed float for foreach get goto if implicit in init int interface internal is lock long namespace new null object operator out override params private protected public readonly record ref required return sbyte sealed set short sizeof stackalloc static string struct switch this throw true try typeof uint ulong unchecked unsafe ushort using value var virtual void volatile when where while with yield Console WriteLine ReadLine List Dictionary Task nameof",
        "Python" => "False None True and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield print input len range enumerate zip open self super isinstance list dict set tuple str int float bool sorted reversed min max sum abs map filter __init__ __name__ __main__",
        "JavaScript" | "TypeScript" => "async await break case catch class const continue debugger default delete do else export extends false finally for function if import in instanceof let new null return super switch this throw true try typeof undefined var void while with yield console log document window JSON parse stringify Promise addEventListener querySelector getElementById setTimeout fetch Array Object Math",
        "Java" => "abstract assert boolean break byte case catch char class const continue default do double else enum extends final finally float for if implements import instanceof int interface long native new null package private protected public return short static strictfp super switch synchronized this throw throws transient true false try void volatile while String System out println ArrayList HashMap Override",
        "C++" | "C" => "alignas auto bool break case catch char class const constexpr const_cast continue decltype default delete do double dynamic_cast else enum explicit export extern false float for friend goto if inline int long mutable namespace new noexcept nullptr operator private protected public register reinterpret_cast return short signed sizeof static static_cast struct switch template this throw true try typedef typename union unsigned using virtual void volatile while include define ifdef ifndef endif std cout cin endl vector string map unique_ptr shared_ptr printf",
        "Rust" => "as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while String Vec Option Some None Result Ok Err Box println format vec HashMap",
        "Go" => "break case chan const continue default defer else fallthrough for func go goto if import interface map package range return select struct switch type var fmt Println Printf error nil string int bool",
        "PHP" => "abstract and array as break callable case catch class clone const continue declare default do echo else elseif empty extends final finally fn for foreach function global goto if implements include instanceof interface isset list match namespace new null or print private protected public readonly require require_once return static switch throw trait try unset use var while yield true false",
        "SQL" => "SELECT FROM WHERE INSERT INTO VALUES UPDATE SET DELETE CREATE TABLE ALTER DROP INDEX VIEW PROCEDURE FUNCTION JOIN INNER LEFT RIGHT OUTER FULL ON AND OR NOT NULL IS IN EXISTS BETWEEN LIKE GROUP BY ORDER HAVING DISTINCT TOP AS UNION ALL CASE WHEN THEN ELSE END BEGIN COMMIT ROLLBACK PRIMARY KEY FOREIGN REFERENCES DEFAULT COUNT SUM AVG MIN MAX CAST VARCHAR INT",
        "HTML" => "html head body title meta link script style div span section article header footer nav main aside h1 h2 h3 h4 h5 h6 p a img ul ol li table thead tbody tr th td form input button label select option textarea iframe canvas video audio source br hr strong em code pre blockquote class id href src alt type name value placeholder",
        "CSS" => "display flex grid block inline none position absolute relative fixed sticky top right bottom left width height min-width max-width margin padding border border-radius background background-color color font font-family font-size font-weight line-height text-align overflow opacity transform transition animation box-shadow z-index cursor justify-content align-items gap",
        "Bourne Again Shell (bash)" => "if then else elif fi for while do done case esac function return echo export local read cd ls grep sed awk cat",
        "Batch File" => "echo set if else goto call exit for in do rem pause cd dir copy del mkdir setlocal endlocal errorlevel",
        _ => "",
    };
    s(k)
}

impl Completion {
    pub fn close(&mut self) {
        self.open = false;
        self.items.clear();
    }

    pub fn word_before(doc: &Doc) -> (usize, String) {
        let c = doc.sel.head;
        let mut s = c;
        while s > 0 && Doc::is_word(doc.rope.char(s - 1)) {
            s -= 1;
        }
        (s, doc.slice(s, c))
    }

    fn refresh_words(&mut self, doc: &mut Doc) {
        if self.words_version == doc.version && !self.words.is_empty() {
            return;
        }
        self.words_version = doc.version;
        let mut set = BTreeSet::new();
        if doc.len_chars() <= 4_000_000 {
            let (start, _) = Self::word_before(doc);
            let text = doc.text();
            let mut word_start: Option<usize> = None; // byte
            let mut char_i = 0usize;
            let mut wstart_char = 0usize;
            let push = |a: usize, b: usize, ac: usize, set: &mut BTreeSet<String>| {
                let w = &text[a..b];
                let n = w.chars().count();
                if (3..=60).contains(&n) && ac != start && !w.chars().next().map_or(true, |c| c.is_ascii_digit()) {
                    set.insert(w.to_string());
                }
            };
            for (bi, ch) in text.char_indices() {
                if Doc::is_word(ch) {
                    if word_start.is_none() {
                        word_start = Some(bi);
                        wstart_char = char_i;
                    }
                } else if let Some(a) = word_start.take() {
                    push(a, bi, wstart_char, &mut set);
                }
                char_i += 1;
            }
            if let Some(a) = word_start {
                push(a, text.len(), wstart_char, &mut set);
            }
        }
        self.words = set.into_iter().collect();
    }

    /// (Re)build the list for the word before the caret. Returns false if nothing matches.
    pub fn update(&mut self, doc: &mut Doc, syntax: Option<&str>, automatic: bool) -> bool {
        let (start, prefix) = Self::word_before(doc);
        if automatic && prefix.chars().count() < 2 {
            self.close();
            return false;
        }
        self.refresh_words(doc);
        let low = prefix.to_lowercase();
        let mut items: Vec<Item> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for k in keywords(syntax) {
            if k != prefix && k.to_lowercase().contains(&low) && seen.insert(k.to_string()) {
                items.push(Item { text: k.to_string(), keyword: true });
            }
        }
        for w in &self.words {
            if *w != prefix && w.to_lowercase().contains(&low) && seen.insert(w.clone()) {
                items.push(Item { text: w.clone(), keyword: false });
            }
        }
        items.sort_by(|a, b| {
            let ap = a.text.to_lowercase().starts_with(&low);
            let bp = b.text.to_lowercase().starts_with(&low);
            bp.cmp(&ap).then_with(|| a.text.to_lowercase().cmp(&b.text.to_lowercase()))
        });
        items.truncate(300);
        if items.is_empty() {
            self.close();
            return false;
        }
        self.start = start;
        self.items = items;
        self.index = 0;
        self.open = true;
        true
    }

    pub fn accept(&mut self, doc: &mut Doc) {
        if let Some(it) = self.items.get(self.index) {
            let end = doc.sel.head;
            let text = it.text.clone();
            doc.replace(self.start, end, &text);
        }
        self.close();
    }
}
