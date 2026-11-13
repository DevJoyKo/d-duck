//! Skills, slash commands and workflows the user makes or edits in D.Duck (skill-authoring, D-086).
//!
//! - New items are created with `create_new` only (never overwriting), each folder from the base down checked with
//!   lstat so a link cannot redirect the write. User folders are 0700, project `.claude/...` folders 0755.
//! - Workflow files are never executed or imported: `meta` is read by a small literal parser with Claude Code's rules
//!   (workflow-meta-check, D-183).
//! - "Claude에게 부탁하기" jobs write into an app-data folder; the user reviews the result and installs it through
//!   the same create path.

use crate::platform::CanonExt;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::agents_catalog::parse_frontmatter;
use crate::install_link::{SkillRisk, display_marked, skill_risk};
use crate::text::display_safe;

pub const MAX_CONTENT: usize = 256 * 1024;
pub const GUIDE: &str = include_str!("../data/authoring_guide.md");
const TEMPLATES: &str = include_str!("../data/authoring_templates.json");
pub const DDUCK_NEW: &str = "dduck-new";

fn re(s: &str) -> Regex {
    Regex::new(s).unwrap_or_else(|_| unreachable!())
}
static NAME_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\A[A-Za-z0-9][A-Za-z0-9_-]{0,63}\z"));
static STR: &str = r#"(?:'([^'\n\\]*)'|"([^"\n\\]*)")"#;
static PHASE_CALL_RE: LazyLock<Regex> = LazyLock::new(|| re(&format!(r"(?:^|[^A-Za-z0-9_.$])phase\(\s*{STR}")));

pub fn valid_name(n: &str) -> bool {
    NAME_RE.is_match(n)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Skill,
    Command,
    Workflow,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Skill, Kind::Command, Kind::Workflow];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Skill => "skill",
            Kind::Command => "command",
            Kind::Workflow => "workflow",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Skill => "스킬",
            Kind::Command => "커맨드",
            Kind::Workflow => "워크플로",
        }
    }

    /// The task name for a make job in the current language ("스킬 만들기" / "Make a skill").
    pub fn task_name(self) -> String {
        use crate::i18n::{Code, Params, current, msg, text};
        let lang = current();
        let kind = text(lang, match self {
            Kind::Skill => Code::KindSkill,
            Kind::Command => Code::KindCommand,
            Kind::Workflow => Code::KindWorkflow,
        });
        msg(lang, Code::MakeKind, &Params::new().text("kind", kind, 20))
    }

    pub fn folder(self) -> &'static str {
        match self {
            Kind::Skill => "skills",
            Kind::Command => "commands",
            Kind::Workflow => "workflows",
        }
    }

    /// Path parts under the root (`<config>` or `<project>/.claude`).
    pub fn parts(self, name: &str) -> Vec<String> {
        match self {
            Kind::Skill => vec!["skills".into(), name.into(), "SKILL.md".into()],
            Kind::Command => vec!["commands".into(), format!("{name}.md")],
            Kind::Workflow => vec!["workflows".into(), format!("{name}.js")],
        }
    }
}

/// The bundled starting text for `kind` (placeholders `{{name}}` and `{{description}}`); the webview fills it.
pub fn template(kind: Kind) -> String {
    template_in(kind, crate::i18n::current())
}

/// [`template`] in `lang` (the bundled `{ko, en}` text).
pub fn template_in(kind: Kind, lang: crate::i18n::Lang) -> String {
    serde_json::from_str::<serde_json::Value>(TEMPLATES)
        .ok()
        .map(|v| crate::i18n::pick_lang(v, lang))
        .and_then(|v| v.get(kind.as_str())?.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Template with values (tests and `/dduck-new`); single-line, quote-free values only.
pub fn fill(kind: Kind, name: &str, description: &str) -> String {
    let d: String = description.chars().filter(|c| !c.is_control() && *c != '\'' && *c != '"' && *c != '\\').collect();
    template(kind).replace("{{name}}", name).replace("{{description}}", &d)
}

// ---------- workflow meta (literal parser; never executed, D-183) ----------
//
// Claude Code (2.1.319) parses the whole script and registers a workflow only when its first statement is
// `export const meta = {…}` holding pure literals. This reads just that statement with the same rules; the script
// body is not checked (the editor says so). Linear in the input, bounded in depth, tokens and items, and never panics.

/// Why Claude Code would skip a workflow file. A code only: no file text, token or value is carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MetaReason {
    NotFirstStatement,
    ParseError,
    NotLiteral,
    /// A template with `${…}`.
    TemplateInterpolation,
    UnsupportedUnary,
    SparseArray,
    ReservedKey,
    UnsupportedKey,
    MissingName,
    EmptyName,
    MissingDescription,
    EmptyDescription,
    NameMismatch,
    TooComplex,
}

impl MetaReason {
    pub const ALL: [MetaReason; 14] = [
        MetaReason::NotFirstStatement,
        MetaReason::ParseError,
        MetaReason::NotLiteral,
        MetaReason::TemplateInterpolation,
        MetaReason::UnsupportedUnary,
        MetaReason::SparseArray,
        MetaReason::ReservedKey,
        MetaReason::UnsupportedKey,
        MetaReason::MissingName,
        MetaReason::EmptyName,
        MetaReason::MissingDescription,
        MetaReason::EmptyDescription,
        MetaReason::NameMismatch,
        MetaReason::TooComplex,
    ];
}

/// A [`MetaReason`] with the 1-based line where it was found (a number only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaIssue {
    pub reason: MetaReason,
    pub line: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowMeta {
    /// The decoded value as written (no display cleaning): compared with the file name as is.
    pub name: String,
    pub description: String,
    pub phases: Vec<String>,
    pub calls: Vec<String>,
}

const META_MAX_DEPTH: usize = 32;
const META_MAX_TOKENS: usize = 10_000;
const META_MAX_ITEMS: usize = 1_000;
const RESERVED_KEYS: [&str; 3] = ["__proto__", "constructor", "prototype"];

/// A parsed literal; numbers, booleans and null are not needed beyond "a literal".
#[derive(Debug, Clone, PartialEq)]
enum Lit {
    Str(String),
    Other,
    Arr(Vec<Lit>),
    /// Keys are unique: a repeated key keeps its last value, as in JavaScript.
    Obj(Vec<(String, Lit)>),
}

impl Lit {
    fn get(&self, key: &str) -> Option<&Lit> {
        match self {
            Lit::Obj(v) => v.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// JavaScript white space (Rust's list has U+0085 but not U+FEFF).
fn is_js_space(c: char) -> bool {
    c == '\u{feff}' || (c != '\u{85}' && c.is_whitespace())
}

fn is_id_start(c: char) -> bool {
    c == '$' || c == '_' || c.is_alphabetic()
}

fn is_id_part(c: char) -> bool {
    is_id_start(c) || c.is_alphanumeric() || c == '\u{200c}' || c == '\u{200d}'
}

type MetaResult<T> = Result<T, MetaIssue>;

struct MetaParser<'a> {
    s: &'a str,
    /// Always on a char boundary: it only moves by whole chars.
    i: usize,
    line: u32,
    tokens: usize,
}

impl<'a> MetaParser<'a> {
    fn fail<T>(&self, reason: MetaReason) -> MetaResult<T> {
        Err(MetaIssue { reason, line: self.line })
    }

    fn rest(&self) -> &'a str {
        self.s.get(self.i..).unwrap_or("")
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn peek2(&self) -> Option<char> {
        self.rest().chars().nth(1)
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        if c == '\n' {
            self.line = self.line.saturating_add(1);
        }
        Some(c)
    }

    fn eat(&mut self, c: char) -> bool {
        let hit = self.peek() == Some(c);
        if hit {
            self.bump();
        }
        hit
    }

    fn token(&mut self) -> MetaResult<()> {
        self.tokens += 1;
        if self.tokens > META_MAX_TOKENS { self.fail(MetaReason::TooComplex) } else { Ok(()) }
    }

    /// Skips white space and comments; `true` when a line break was crossed.
    fn trivia(&mut self) -> MetaResult<bool> {
        let mut nl = false;
        loop {
            match self.peek() {
                Some(c) if is_line_break(c) => {
                    nl = true;
                    self.bump();
                }
                Some(c) if is_js_space(c) => {
                    self.bump();
                }
                Some('/') if self.peek2() == Some('/') => {
                    while self.peek().is_some_and(|c| !is_line_break(c)) {
                        self.bump();
                    }
                }
                Some('/') if self.peek2() == Some('*') => {
                    self.bump();
                    self.bump();
                    loop {
                        match self.bump() {
                            None => return self.fail(MetaReason::ParseError),
                            Some('*') if self.peek() == Some('/') => {
                                self.bump();
                                break;
                            }
                            Some(c) if is_line_break(c) => nl = true,
                            Some(_) => {}
                        }
                    }
                }
                _ => return Ok(nl),
            }
        }
    }

    fn word(&mut self) -> &'a str {
        let start = self.i;
        if self.peek().is_some_and(is_id_start) {
            while self.peek().is_some_and(is_id_part) {
                self.bump();
            }
        }
        self.s.get(start..self.i).unwrap_or("")
    }

    fn number_start(&self) -> bool {
        match self.peek() {
            Some(c) if c.is_ascii_digit() => true,
            Some('.') => self.peek2().is_some_and(|c| c.is_ascii_digit()),
            _ => false,
        }
    }

    /// Digits in `radix`; `_` only between two digits. Returns how many digits.
    fn digits(&mut self, radix: u32) -> MetaResult<usize> {
        let mut n = 0;
        while let Some(c) = self.peek() {
            if c.is_digit(radix) {
                n += 1;
                self.bump();
            } else if c == '_' && n > 0 && self.peek2().is_some_and(|d| d.is_digit(radix)) {
                self.bump();
            } else if c == '_' {
                return self.fail(MetaReason::ParseError);
            } else {
                break;
            }
        }
        Ok(n)
    }

    /// Decimal, `.5`, `1e3`, `0x`/`0o`/`0b`, `_` separators and BigInt (`1n`, a Literal to Claude Code's parser).
    /// Legacy octal (`017`) is a strict-mode error. `true` for a BigInt.
    fn number(&mut self) -> MetaResult<bool> {
        let r = self.rest();
        let radix = match r.get(..2) {
            Some("0x" | "0X") => 16,
            Some("0o" | "0O") => 8,
            Some("0b" | "0B") => 2,
            _ => 10,
        };
        let mut decimal_point_or_exponent = false;
        if radix != 10 {
            self.bump();
            self.bump();
            if self.digits(radix)? == 0 {
                return self.fail(MetaReason::ParseError);
            }
        } else {
            if r.starts_with('0') && r.get(1..).is_some_and(|t| t.starts_with(|c: char| c.is_ascii_digit() || c == '_')) {
                return self.fail(MetaReason::ParseError);
            }
            self.digits(10)?;
            if self.eat('.') {
                decimal_point_or_exponent = true;
                if self.peek() == Some('_') {
                    return self.fail(MetaReason::ParseError);
                }
            }
            self.digits(10)?;
            if matches!(self.peek(), Some('e' | 'E')) {
                decimal_point_or_exponent = true;
                self.bump();
                if !self.eat('+') {
                    self.eat('-');
                }
                if self.digits(10)? == 0 {
                    return self.fail(MetaReason::ParseError);
                }
            }
        }
        let big = self.peek() == Some('n');
        if big {
            if decimal_point_or_exponent {
                return self.fail(MetaReason::ParseError);
            }
            self.bump();
        }
        if self.peek().is_some_and(is_id_part) {
            return self.fail(MetaReason::ParseError);
        }
        Ok(big)
    }

    fn hex(&mut self, n: usize) -> MetaResult<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            let Some(d) = self.peek().and_then(|c| c.to_digit(16)) else { return self.fail(MetaReason::NotLiteral) };
            v = v * 16 + d;
            self.bump();
        }
        Ok(v)
    }

    /// The code point after `\u`: `XXXX` or `{X…}` up to 10FFFF.
    fn code_point(&mut self) -> MetaResult<u32> {
        if !self.eat('{') {
            return self.hex(4);
        }
        let (mut v, mut n) = (0u32, 0usize);
        loop {
            match self.peek() {
                Some('}') if n > 0 => {
                    self.bump();
                    return Ok(v);
                }
                Some(c) => {
                    let Some(d) = c.to_digit(16) else { return self.fail(MetaReason::NotLiteral) };
                    v = v * 16 + d;
                    n += 1;
                    if v > 0x10FFFF {
                        return self.fail(MetaReason::NotLiteral);
                    }
                    self.bump();
                }
                None => return self.fail(MetaReason::NotLiteral),
            }
        }
    }

    /// One escape after `\`. Legacy octal (`\1`, `\01`), bad or truncated escapes and lone surrogates → `notLiteral`.
    fn escape(&mut self, out: &mut String) -> MetaResult<()> {
        let Some(c) = self.bump() else { return self.fail(MetaReason::NotLiteral) };
        let ch = match c {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            'b' => '\u{8}',
            'f' => '\u{c}',
            'v' => '\u{b}',
            '0' if !self.peek().is_some_and(|d| d.is_ascii_digit()) => '\0',
            '0'..='9' => return self.fail(MetaReason::NotLiteral),
            'x' => {
                let v = self.hex(2)?;
                char::from_u32(v).unwrap_or('\u{fffd}')
            }
            'u' => {
                let mut v = self.code_point()?;
                if (0xD800..0xDC00).contains(&v) {
                    if !self.rest().starts_with("\\u") {
                        return self.fail(MetaReason::NotLiteral);
                    }
                    self.bump();
                    self.bump();
                    let lo = self.code_point()?;
                    if !(0xDC00..0xE000).contains(&lo) {
                        return self.fail(MetaReason::NotLiteral);
                    }
                    v = 0x10000 + ((v - 0xD800) << 10) + (lo - 0xDC00);
                }
                match char::from_u32(v) {
                    Some(c) => c,
                    None => return self.fail(MetaReason::NotLiteral),
                }
            }
            // Line continuation.
            '\r' => {
                self.eat('\n');
                return Ok(());
            }
            '\n' | '\u{2028}' | '\u{2029}' => return Ok(()),
            other => other,
        };
        out.push(ch);
        Ok(())
    }

    fn string(&mut self, quote: char) -> MetaResult<String> {
        self.bump();
        let mut out = String::new();
        loop {
            match self.bump() {
                None | Some('\n' | '\r') => return self.fail(MetaReason::ParseError),
                Some(c) if c == quote => return Ok(out),
                Some('\\') => self.escape(&mut out)?,
                Some(c) => out.push(c),
            }
        }
    }

    /// A template without `${…}`.
    fn template(&mut self) -> MetaResult<String> {
        self.bump();
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return self.fail(MetaReason::ParseError),
                Some('`') => return Ok(out),
                Some('$') if self.peek() == Some('{') => return self.fail(MetaReason::TemplateInterpolation),
                Some('\r') => {
                    self.eat('\n');
                    out.push('\n');
                }
                Some('\\') => self.escape(&mut out)?,
                Some(c) => out.push(c),
            }
        }
    }

    fn value(&mut self, depth: usize) -> MetaResult<Lit> {
        self.token()?;
        self.trivia()?;
        let Some(c) = self.peek() else { return self.fail(MetaReason::ParseError) };
        match c {
            '{' => self.object(depth + 1),
            '[' => self.array(depth + 1),
            '\'' | '"' => self.string(c).map(Lit::Str),
            '`' => self.template().map(Lit::Str),
            // Only a minus on a plain number (not a BigInt), as in Claude Code.
            '-' => {
                self.bump();
                self.trivia()?;
                if self.number_start() && !self.number()? { Ok(Lit::Other) } else { self.fail(MetaReason::UnsupportedUnary) }
            }
            '(' => self.paren(depth + 1),
            '+' | '!' | '~' => self.fail(MetaReason::UnsupportedUnary),
            '.' if self.rest().starts_with("...") => self.fail(MetaReason::NotLiteral),
            _ if self.number_start() => self.number().map(|_| Lit::Other),
            _ if is_id_start(c) => match self.word() {
                "true" | "false" | "null" => Ok(Lit::Other),
                "typeof" | "void" | "delete" => self.fail(MetaReason::UnsupportedUnary),
                _ => self.fail(MetaReason::NotLiteral),
            },
            // Regular expressions: a Literal to Claude Code's parser, but no evidence of how Claude Code treats the
            // value, so refused (design 6).
            '/' => self.fail(MetaReason::NotLiteral),
            _ => self.fail(MetaReason::ParseError),
        }
    }

    /// `( value )`: Claude Code's parser drops parentheses. `(a, b)` is a sequence, not a literal.
    fn paren(&mut self, depth: usize) -> MetaResult<Lit> {
        if depth > META_MAX_DEPTH {
            return self.fail(MetaReason::TooComplex);
        }
        self.bump();
        let v = self.value(depth)?;
        self.trivia()?;
        if self.eat(')') {
            return Ok(v);
        }
        match self.peek() {
            Some(c) if ",([.`?+-*/%<>=&|^!".contains(c) => self.fail(MetaReason::NotLiteral),
            _ => self.fail(MetaReason::ParseError),
        }
    }

    /// An identifier name with `\uXXXX`/`\u{…}` escapes decoded (`\u0064escription` is `description`).
    fn ident_name(&mut self) -> MetaResult<String> {
        let mut out = String::new();
        loop {
            let c = match self.peek() {
                Some('\\') => {
                    self.bump();
                    if !self.eat('u') {
                        return self.fail(MetaReason::ParseError);
                    }
                    let v = self.code_point()?;
                    let Some(c) = char::from_u32(v) else { return self.fail(MetaReason::ParseError) };
                    if !(if out.is_empty() { is_id_start(c) } else { is_id_part(c) }) {
                        return self.fail(MetaReason::ParseError);
                    }
                    c
                }
                Some(c) if (out.is_empty() && is_id_start(c)) || (!out.is_empty() && is_id_part(c)) => {
                    self.bump();
                    c
                }
                _ => return Ok(out),
            };
            out.push(c);
            if out.len() > MAX_CONTENT {
                return self.fail(MetaReason::TooComplex);
            }
        }
    }

    /// After a value: `,` (→ false) or the closing bracket (→ true). An operator, call or member access makes the
    /// value an expression.
    fn after_item(&mut self, close: char) -> MetaResult<bool> {
        self.trivia()?;
        if self.eat(',') {
            return Ok(false);
        }
        if self.eat(close) {
            return Ok(true);
        }
        match self.peek() {
            Some(c) if "([.`?+-*/%<>=&|^!".contains(c) => self.fail(MetaReason::NotLiteral),
            _ => self.fail(MetaReason::ParseError),
        }
    }

    fn key(&mut self) -> MetaResult<String> {
        let Some(c) = self.peek() else { return self.fail(MetaReason::ParseError) };
        let key = match c {
            '\'' | '"' => self.string(c)?,
            '[' | '`' => return self.fail(MetaReason::UnsupportedKey),
            '.' if self.rest().starts_with("...") => return self.fail(MetaReason::NotLiteral),
            '*' => return self.fail(MetaReason::NotLiteral),
            _ if self.number_start() => {
                let start = self.i;
                self.number()?;
                self.s.get(start..self.i).unwrap_or("").to_string()
            }
            _ if c == '\\' || is_id_start(c) => {
                let w = self.ident_name()?;
                // A shorthand, method or accessor: anything but `:` after a bare name.
                self.trivia()?;
                if self.peek() != Some(':') {
                    return self.fail(MetaReason::NotLiteral);
                }
                w
            }
            _ => return self.fail(MetaReason::ParseError),
        };
        if RESERVED_KEYS.contains(&key.as_str()) {
            return self.fail(MetaReason::ReservedKey);
        }
        Ok(key)
    }

    fn object(&mut self, depth: usize) -> MetaResult<Lit> {
        if depth > META_MAX_DEPTH {
            return self.fail(MetaReason::TooComplex);
        }
        self.bump();
        let (mut items, mut count) = (Vec::<(String, Lit)>::new(), 0usize);
        loop {
            self.trivia()?;
            if self.eat('}') {
                return Ok(Lit::Obj(items));
            }
            count += 1;
            if count > META_MAX_ITEMS {
                return self.fail(MetaReason::TooComplex);
            }
            self.token()?;
            let key = self.key()?;
            self.trivia()?;
            if !self.eat(':') {
                return self.fail(if self.peek() == Some('(') { MetaReason::NotLiteral } else { MetaReason::ParseError });
            }
            let v = self.value(depth)?;
            match items.iter_mut().find(|(k, _)| *k == key) {
                Some(e) => e.1 = v,
                None => items.push((key, v)),
            }
            if self.after_item('}')? {
                return Ok(Lit::Obj(items));
            }
        }
    }

    fn array(&mut self, depth: usize) -> MetaResult<Lit> {
        if depth > META_MAX_DEPTH {
            return self.fail(MetaReason::TooComplex);
        }
        self.bump();
        let mut items = Vec::new();
        loop {
            self.trivia()?;
            if self.eat(']') {
                return Ok(Lit::Arr(items));
            }
            if self.peek() == Some(',') {
                return self.fail(MetaReason::SparseArray);
            }
            if items.len() >= META_MAX_ITEMS {
                return self.fail(MetaReason::TooComplex);
            }
            items.push(self.value(depth)?);
            if self.after_item(']')? {
                return Ok(Lit::Arr(items));
            }
        }
    }

    /// What follows the meta object must end the declaration; `{…}.x`, `{…}(…)` or `{…} + 1` is no longer an object.
    fn statement_end(&mut self) -> MetaResult<()> {
        let nl = self.trivia()?;
        let r = self.rest();
        if r.is_empty() || r.starts_with(';') || r.starts_with(',') || (nl && (r.starts_with("++") || r.starts_with("--"))) {
            return Ok(());
        }
        let goes_on = r.starts_with("!=")
            || r.starts_with(|c: char| "([.`?+-*/%<>=&|^".contains(c))
            || ["in", "instanceof", "as", "satisfies"].iter().any(|w| r.strip_prefix(w).is_some_and(|t| !t.starts_with(is_id_part)));
        if goes_on {
            self.fail(MetaReason::NotFirstStatement)
        } else if nl {
            Ok(())
        } else {
            self.fail(MetaReason::ParseError)
        }
    }

    /// `export const meta = {…}` as the first statement → (the object, its end byte, the line it opens on).
    fn meta(&mut self) -> MetaResult<(Lit, usize, u32)> {
        self.eat('\u{feff}');
        if self.rest().starts_with("#!") {
            while self.peek().is_some_and(|c| !is_line_break(c)) {
                self.bump();
            }
        }
        self.trivia()?;
        for w in ["export", "const", "meta"] {
            if self.word() != w {
                return self.fail(MetaReason::NotFirstStatement);
            }
            self.trivia()?;
        }
        match self.peek() {
            // `meta: Meta = …` is TypeScript: Claude Code's parser stops there.
            Some(':') => return self.fail(MetaReason::ParseError),
            Some('=') if self.peek2() != Some('=') => {
                self.bump();
            }
            _ => return self.fail(MetaReason::NotFirstStatement),
        }
        self.trivia()?;
        // `({…})` is still the object to Claude Code's parser.
        let mut parens = 0usize;
        while self.eat('(') {
            parens += 1;
            if parens > META_MAX_DEPTH {
                return self.fail(MetaReason::TooComplex);
            }
            self.trivia()?;
        }
        if self.peek() != Some('{') {
            return self.fail(MetaReason::NotFirstStatement);
        }
        let line = self.line;
        let v = self.object(1)?;
        for _ in 0..parens {
            self.trivia()?;
            if self.eat(')') {
                continue;
            }
            return match self.peek() {
                Some(c) if ",([.`?+-*/%<>=&|^!".contains(c) => self.fail(MetaReason::NotFirstStatement),
                _ => self.fail(MetaReason::ParseError),
            };
        }
        let end = self.i;
        self.statement_end()?;
        Ok((v, end, line))
    }
}

fn cap(c: &regex::Captures) -> String {
    c.get(1).or_else(|| c.get(2)).map_or(String::new(), |m| m.as_str().to_string())
}

/// Reads the meta the way Claude Code does: `Err` when Claude Code would skip the file. `name` and `description`
/// must be non-empty strings (white space only counts as non-empty, like Claude Code). Never executes anything.
pub fn workflow_meta(text: &str) -> Result<WorkflowMeta, MetaIssue> {
    if text.len() > MAX_CONTENT {
        return Err(MetaIssue { reason: MetaReason::TooComplex, line: 1 });
    }
    let mut p = MetaParser { s: text, i: 0, line: 1, tokens: 0 };
    let (v, end, line) = p.meta()?;
    let issue = |reason| Err(MetaIssue { reason, line });
    let name = match v.get("name") {
        Some(Lit::Str(s)) if s.is_empty() => return issue(MetaReason::EmptyName),
        Some(Lit::Str(s)) => s.clone(),
        _ => return issue(MetaReason::MissingName),
    };
    let description = match v.get("description") {
        Some(Lit::Str(s)) if s.is_empty() => return issue(MetaReason::EmptyDescription),
        Some(Lit::Str(s)) => s.clone(),
        _ => return issue(MetaReason::MissingDescription),
    };
    // Claude Code enforces no shape on phases; the titles only feed the phase() warnings.
    let phases = match v.get("phases") {
        Some(Lit::Arr(a)) => a
            .iter()
            .filter_map(|p| match p.get("title") {
                Some(Lit::Str(t)) => Some(t.clone()),
                _ => None,
            })
            .take(50)
            .collect(),
        _ => Vec::new(),
    };
    let body = text.get(end..).unwrap_or("");
    Ok(WorkflowMeta { name, description, phases, calls: PHASE_CALL_RE.captures_iter(body).map(|c| cap(&c)).take(50).collect() })
}

/// The editor's live check (`check_workflow_meta`, D-183): Claude Code's meta rules, then the decoded `meta.name`
/// against the file name as is. Pure: no file is read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaCheck {
    pub ok: bool,
    pub reason: Option<MetaReason>,
    pub line: Option<u32>,
    /// The description is only white space: Claude Code accepts it, the editor warns.
    pub blank_description: bool,
    /// For `nameMismatch` only: the display-cleaned `meta.name` Claude Code registers (the editor's warning when an
    /// existing file is saved, design 8). The user's own text, like the list's `registeredName`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered_name: Option<String>,
}

pub fn check_workflow_meta(text: &str, name: &str) -> MetaCheck {
    match workflow_meta(text) {
        Err(e) => MetaCheck { ok: false, reason: Some(e.reason), line: Some(e.line), blank_description: false, registered_name: None },
        Ok(m) if m.name != name => MetaCheck {
            ok: false,
            reason: Some(MetaReason::NameMismatch),
            line: None,
            blank_description: false,
            registered_name: Some(display_safe(&m.name, 100)),
        },
        Ok(m) => MetaCheck { ok: true, reason: None, line: None, blank_description: m.description.trim().is_empty(), registered_name: None },
    }
}

/// The reason and how to fix it, one line: the same sentence the editor shows (`extensions.wfcheck.reason.*`).
pub fn meta_reason_text(reason: MetaReason, lang: crate::i18n::Lang) -> &'static str {
    use crate::i18n::Fail;
    match reason {
        MetaReason::NotFirstStatement => Fail::MetaFirst,
        MetaReason::ParseError => Fail::MetaParse,
        MetaReason::NotLiteral => Fail::MetaNotLiteral,
        MetaReason::TemplateInterpolation => Fail::MetaTemplate,
        MetaReason::UnsupportedUnary => Fail::MetaUnary,
        MetaReason::SparseArray => Fail::MetaSparse,
        MetaReason::ReservedKey => Fail::MetaReservedKey,
        MetaReason::UnsupportedKey => Fail::MetaKey,
        MetaReason::MissingName => Fail::MetaNoName,
        MetaReason::EmptyName => Fail::MetaEmptyName,
        MetaReason::MissingDescription => Fail::MetaNoDesc,
        MetaReason::EmptyDescription => Fail::MetaEmptyDesc,
        MetaReason::NameMismatch => Fail::MetaNameDiffers,
        MetaReason::TooComplex => Fail::MetaTooComplex,
    }
    .text(lang)
}

/// The save refusal for a meta problem, in the current language: the reason's own sentence and its line.
fn meta_fail(issue: MetaIssue) -> String {
    let lang = crate::i18n::current();
    format!("{} · {}", meta_reason_text(issue.reason, lang), crate::tr_fmt!(crate::i18n::Fail::MetaLine, line = issue.line))
}

/// "meta.name을 <name>(으)로 맞춰 주세요" with the right particle.
fn meta_name_fail(name: &str) -> String {
    crate::tr_fmt!(crate::i18n::Fail::MetaName, name = name, ro = crate::i18n::ro(crate::i18n::current(), name))
}

/// Validates content for `kind`/`name` (a new item). `Ok(warnings)`; `Err` blocks saving.
pub fn check_content(kind: Kind, name: &str, content: &str) -> Result<Vec<String>, String> {
    check_content_as(kind, name, content, false)
}

/// Like [`check_content`] for a file that already exists: a workflow whose `meta.name` differs from the file name
/// is saved with a warning naming what Claude Code registers (it registers by meta.name, design 8).
pub fn check_existing(kind: Kind, name: &str, content: &str) -> Result<Vec<String>, String> {
    check_content_as(kind, name, content, true)
}

fn check_content_as(kind: Kind, name: &str, content: &str, existing: bool) -> Result<Vec<String>, String> {
    if content.len() > MAX_CONTENT || content.contains('\0') {
        return Err(crate::i18n::Fail::Text256Only.str().into());
    }
    if content.trim().is_empty() {
        return Err(crate::i18n::Fail::ContentEmpty.str().into());
    }
    let mut warn = Vec::new();
    match kind {
        Kind::Skill | Kind::Command => {
            let fm = parse_frontmatter(content);
            if kind == Kind::Skill {
                let fm = fm.as_ref().ok_or(crate::i18n::Fail::SkillFront.str())?;
                if fm.get("name").is_some_and(|n| n != name) {
                    return Err(crate::tr_fmt!(crate::i18n::Fail::SkillNameMismatch, name = name));
                }
            }
            if fm.as_ref().and_then(|f| f.get("description")).is_none_or(|d| d.trim().is_empty()) {
                warn.push(crate::i18n::Fail::NoDescription.str().into());
            }
            let r = skill_risk(content);
            if r.allowed_tools.is_some() {
                warn.push(crate::i18n::Fail::HasAllowedTools.str().into());
            }
            if r.hooks {
                warn.push(crate::i18n::Fail::HasHooks.str().into());
            }
            if r.shell_lines > 0 {
                warn.push(crate::tr_fmt!(crate::i18n::Fail::ShellLines, a0 = r.shell_lines));
            }
        }
        Kind::Workflow => {
            let m = workflow_meta(content).map_err(meta_fail)?;
            if m.name != name {
                if !existing {
                    return Err(meta_name_fail(name));
                }
                let shown = display_safe(&m.name, 100);
                let ro = crate::i18n::ro(crate::i18n::current(), &shown);
                warn.push(crate::tr_fmt!(crate::i18n::Fail::MetaRegistered, name = shown, ro = ro));
            }
            if m.description.trim().is_empty() {
                warn.push(crate::i18n::Fail::NoDescription.str().into());
            }
            let missing: Vec<&String> = m.calls.iter().filter(|c| !m.phases.contains(c)).collect();
            if !missing.is_empty() {
                warn.push(crate::tr_fmt!(crate::i18n::Fail::PhaseUnknown, a0 = missing.iter().map(|s| display_safe(s, 40)).collect::<Vec<_>>().join(", ")));
            }
            if m.phases.iter().any(|p| !m.calls.contains(p)) {
                warn.push(crate::i18n::Fail::PhaseUnused.str().into());
            }
            if ["Date.now(", "Math.random(", "new Date()"].iter().any(|k| content.contains(k)) {
                warn.push(crate::i18n::Fail::NoNondeterminism.str().into());
            }
        }
    }
    Ok(warn)
}

// ---------- create ----------

/// Where a new item goes. `base` is the config dir (user) or the canonical project folder (project, under `.claude`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub base: PathBuf,
    pub project: bool,
}

impl Target {
    fn dir_mode(&self) -> u32 {
        if self.project { 0o755 } else { 0o700 }
    }

    fn prefix(&self) -> Vec<String> {
        if self.project { vec![".claude".into()] } else { Vec::new() }
    }

    /// The final path for `kind`/`name` (no checks).
    pub fn path_of(&self, kind: Kind, name: &str) -> PathBuf {
        let mut p = self.base.clone();
        p.extend(self.prefix().into_iter().chain(kind.parts(name)));
        p
    }
}

fn mkdir(p: &Path, mode: u32) -> std::io::Result<()> {
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut b = std::fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut b, mode);
    #[cfg(not(unix))]
    let _ = mode;
    b.create(p)
}

/// Walks `parts` below `base`, each one a real folder (created with `mode` when missing, never a link).
fn real_dirs(base: &Path, parts: &[String], mode: u32) -> Result<PathBuf, String> {
    if !std::fs::symlink_metadata(base).is_ok_and(|m| m.is_dir()) && !base.is_dir() {
        return Err(crate::i18n::Fail::FolderNotFound.str().into());
    }
    let mut p = base.to_path_buf();
    for part in parts {
        p.push(part);
        match std::fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_dir() => {}
            Ok(_) => return Err(crate::tr_fmt!(crate::i18n::Fail::PartDirBad, part = part)),
            Err(_) => mkdir(&p, mode).map_err(|_| crate::tr_fmt!(crate::i18n::Fail::PartDirFailed, part = part))?,
        }
    }
    Ok(p)
}

fn write_new(p: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o644);
    let mut f = o.open(p).map_err(|e| if e.kind() == std::io::ErrorKind::AlreadyExists { crate::i18n::Fail::NameExists.str().to_string() } else { crate::i18n::Fail::CreateFileFailed.str().to_string() })?;
    f.write_all(bytes).and_then(|_| f.sync_all()).map_err(|_| crate::i18n::Fail::WriteFileFailed.str().to_string())
}

/// `<root>/<skills|commands|workflows>`, ready to write into. User folders may be links (dotfile managers link
/// `~/.claude` or its sub-folders, QA MEDIUM) and are resolved; under a project every step must be a real folder.
fn kind_dir(target: &Target, kind: Kind) -> Result<PathBuf, String> {
    if target.project {
        let mut dirs = target.prefix();
        dirs.push(kind.folder().into());
        real_dirs(&target.base, &dirs, target.dir_mode()).map_err(|e| crate::tr_fmt!(crate::i18n::Fail::LinkedProjectDir, e = e))
    } else {
        crate::install_link::user_dir(&target.base.join(kind.folder()))
    }
}

/// Creates a new item; never overwrites. Returns the file path.
pub fn create_item(target: &Target, kind: Kind, name: &str, content: &str) -> Result<PathBuf, String> {
    if !valid_name(name) {
        return Err(crate::i18n::Fail::ItemNameFormat.str().into());
    }
    check_content(kind, name, content)?;
    let mode = target.dir_mode();
    let parent = kind_dir(target, kind)?;
    let file = match kind {
        Kind::Skill => {
            let dir = parent.join(name);
            mkdir(&dir, mode).map_err(|e| if e.kind() == std::io::ErrorKind::AlreadyExists { crate::i18n::Fail::NameExists.str().to_string() } else { crate::i18n::Fail::MakeDirFailed.str().to_string() })?;
            dir.join("SKILL.md")
        }
        _ => parent.join(kind.parts(name).pop().unwrap_or_default()),
    };
    write_new(&file, content.as_bytes())?;
    Ok(file)
}

// ---------- editing ----------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemText {
    pub content: String,
    pub token: String,
    pub kind: &'static str,
    pub name: String,
    pub location: String,
    /// Why the editor is read-only (plugin, synced, link…), decided here from the resolved path.
    pub readonly: Option<String>,
    pub warnings: Vec<String>,
    /// Other files next to a skill's SKILL.md (names only).
    pub extra_files: Vec<String>,
}

/// Link count of `p` (metadata `m`); Windows reads it from a handle and refuses when unknown (platform::ids).
fn nlink(p: &Path, m: &std::fs::Metadata) -> u64 {
    crate::platform::nlink(p, m)
}

/// Reads one catalog file for the editor.
pub fn read_item(path: &Path, kind: &'static str, name: &str, location: String, readonly: Option<&str>) -> Result<ItemText, String> {
    let m = std::fs::symlink_metadata(path).map_err(|_| crate::i18n::Fail::FileNotFound.str().to_string())?;
    if !m.file_type().is_file() || m.len() > MAX_CONTENT as u64 {
        return Err(crate::i18n::Fail::NotRegularOr256.str().into());
    }
    let mut buf = Vec::new();
    std::fs::File::open(path).and_then(|f| f.take(MAX_CONTENT as u64 + 1).read_to_end(&mut buf)).map_err(|_| crate::i18n::Fail::ReadFileFailed.str().to_string())?;
    let content = String::from_utf8(buf).map_err(|_| crate::i18n::Fail::NotUtf8Text.str().to_string())?;
    let readonly = readonly.map(str::to_string).or_else(|| (nlink(path, &m) != 1).then(|| crate::i18n::Fail::HardlinkNoEdit.str().to_string()));
    let k = match kind {
        "command" => Kind::Command,
        "workflow" => Kind::Workflow,
        _ => Kind::Skill,
    };
    let short = name.rsplit(':').next().unwrap_or(name);
    let warnings = check_existing(k, short, &content).unwrap_or_else(|e| vec![e]);
    let mut extra_files = Vec::new();
    if k == Kind::Skill {
        if let Some(dir) = path.parent() {
            for e in walkdir::WalkDir::new(dir).follow_links(false).max_depth(3).into_iter().filter_map(Result::ok).take(200) {
                if e.file_type().is_file() && e.path() != path {
                    if let Ok(rel) = e.path().strip_prefix(dir) {
                        extra_files.push(display_safe(&rel.to_string_lossy(), 200));
                    }
                }
            }
            extra_files.sort();
        }
    }
    Ok(ItemText { token: crate::agents_catalog::token_of(content.as_bytes()), content, kind, name: name.to_string(), location, readonly, warnings, extra_files })
}

// ---------- "Claude에게 부탁하기" jobs ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobMeta {
    pub kind: Kind,
    pub created_ms: i64,
    #[serde(default)]
    pub bg_id: Option<String>,
    /// First line of the request, for the list (display-safe, 80 chars).
    #[serde(default)]
    pub title: String,
}

/// `<root>/<token>/job.json` (outside the session's folder) and `<root>/<token>/work/{skills,commands,workflows}`.
pub fn create_job(root: &Path, token: &str, meta: &JobMeta) -> Result<PathBuf, String> {
    if !crate::install_link::valid_token(token) {
        return Err(crate::i18n::Fail::BadJob.str().into());
    }
    real_dirs(root.parent().ok_or(crate::i18n::Fail::NoAppData.str())?, &[root.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()], 0o700)?;
    let job = root.join(token);
    mkdir(&job, 0o700).map_err(|_| crate::i18n::Fail::JobFolderFailed.str().to_string())?;
    let work = job.join("work");
    mkdir(&work, 0o700).map_err(|_| crate::i18n::Fail::JobFolderFailed.str().to_string())?;
    for k in Kind::ALL {
        mkdir(&work.join(k.folder()), 0o700).map_err(|_| crate::i18n::Fail::JobFolderFailed.str().to_string())?;
    }
    save_job(root, token, meta)?;
    Ok(work)
}

pub fn save_job(root: &Path, token: &str, meta: &JobMeta) -> Result<(), String> {
    let path = root.join(token).join("job.json");
    let tmp = root.join(token).join(".job.json.tmp");
    let _ = std::fs::remove_file(&tmp);
    write_new(&tmp, &serde_json::to_vec(meta).map_err(|_| crate::i18n::Fail::JobSaveFailed.str().to_string())?)?;
    std::fs::rename(&tmp, &path).map_err(|_| crate::i18n::Fail::JobSaveFailed.str().to_string())
}

pub fn job_meta(root: &Path, token: &str) -> Option<JobMeta> {
    if !crate::install_link::valid_token(token) {
        return None;
    }
    let p = root.join(token).join("job.json");
    if !std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_file()) {
        return None;
    }
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Produced {
    pub kind: Kind,
    pub name: String,
    /// The main file, invisible characters marked.
    pub content: String,
    pub files: Vec<String>,
    /// Why it cannot be installed, if so.
    pub problem: Option<String>,
    pub warnings: Vec<String>,
    pub risk: SkillRisk,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub token: String,
    pub kind: Kind,
    pub created_ms: i64,
    pub bg_id: Option<String>,
    pub title: String,
    pub produced: Vec<Produced>,
    /// Files in the work folder that are not one of the three shapes (shown, never installed).
    pub stray: Vec<String>,
}

const MAX_JOB_FILES: usize = 200;
const MAX_JOB_BYTES: u64 = 5 * 1024 * 1024;

/// Every regular file under a produced skill folder (no links, valid names, ≤4 deep, size caps).
/// A relative path with `/` between its parts on every OS (Windows walks give `\`), for the part checks and the
/// `/`-split rebuild below (windows-support 1.4).
fn rel_slash(rel: &Path) -> String {
    rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

fn skill_files(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut out = Vec::new();
    let mut total = 0u64;
    let mut seen = HashSet::new();
    for e in walkdir::WalkDir::new(dir).follow_links(false).min_depth(1).max_depth(5).into_iter() {
        let e = e.map_err(|_| crate::i18n::Fail::ReadDirFailed.str().to_string())?;
        let rel = rel_slash(e.path().strip_prefix(dir).map_err(|_| crate::i18n::Fail::PathError.str().to_string())?);
        if e.file_type().is_symlink() {
            return Err(crate::tr_fmt!(crate::i18n::Fail::HasLink, a0 = display_safe(&rel, 100)));
        }
        if rel.split('/').any(|p| !crate::install_link::valid_part(p)) {
            return Err(crate::tr_fmt!(crate::i18n::Fail::NameChars, a0 = display_safe(&rel, 100)));
        }
        if e.depth() > 4 {
            return Err(crate::i18n::Fail::Depth4.str().into());
        }
        if !seen.insert(rel.to_ascii_lowercase()) {
            return Err(crate::i18n::Fail::CaseClashShort.str().into());
        }
        if e.file_type().is_file() {
            let m = e.metadata().map_err(|_| crate::i18n::Fail::ReadFileFailed.str().to_string())?;
            total += m.len();
            if m.len() > 1024 * 1024 || total > MAX_JOB_BYTES || out.len() >= MAX_JOB_FILES || nlink(e.path(), &m) != 1 {
                return Err(crate::i18n::Fail::TooBigOrMany.str().into());
            }
            out.push((rel, e.path().to_path_buf()));
        } else if !e.file_type().is_dir() {
            return Err(crate::i18n::Fail::HasNonRegular.str().into());
        }
    }
    out.sort();
    Ok(out)
}

/// Opens a regular, single-link file without following a final link and checks it on the descriptor.
fn open_regular(p: &Path, max: u64) -> Result<std::fs::File, String> {
    let f = crate::artifact_preview::open_flags(p).map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { crate::i18n::Fail::FileMissing.str().to_string() } else { crate::i18n::Fail::NotRegularFile.str().to_string() })?;
    let m = crate::artifact_preview::single_link_regular(&f).ok_or(crate::i18n::Fail::NotRegularFile.str())?;
    if m.len() > max {
        return Err(crate::i18n::Fail::FileTooBig.str().into());
    }
    Ok(f)
}

fn read_text(p: &Path) -> Result<String, String> {
    let f = open_regular(p, MAX_CONTENT as u64).map_err(|_| crate::i18n::Fail::NotRegularOr256.str().to_string())?;
    let mut buf = Vec::new();
    f.take(MAX_CONTENT as u64 + 1).read_to_end(&mut buf).map_err(|_| crate::i18n::Fail::ReadFileFailed.str().to_string())?;
    if buf.len() > MAX_CONTENT {
        return Err(crate::i18n::Fail::NotRegularOr256.str().into());
    }
    String::from_utf8(buf).map_err(|_| crate::i18n::Fail::NotUtf8Plain.str().into())
}

/// What a locked item's main file could run (allowed-tools, hooks, shell lines), for the copy confirmation.
pub fn copy_risk(src_main: &Path) -> Result<SkillRisk, String> {
    Ok(skill_risk(&read_text(src_main)?))
}

fn produced_one(kind: Kind, name: &str, main: &Path, files: Result<Vec<String>, String>) -> Produced {
    let text = read_text(main);
    let content = text.as_deref().map(|t| display_marked(t, 200_000)).unwrap_or_default();
    let mut problem = if valid_name(name) { None } else { Some(crate::i18n::Fail::ItemNameFormat.str().to_string()) };
    let mut warnings = Vec::new();
    let mut risk = SkillRisk::default();
    match &text {
        Ok(t) => {
            risk = skill_risk(t);
            match check_content(kind, name, t) {
                Ok(w) => warnings = w,
                Err(e) => problem = problem.or(Some(e)),
            }
        }
        Err(e) => problem = problem.or(Some(e.clone())),
    }
    let files = files.unwrap_or_else(|e| {
        problem = problem.clone().or(Some(e));
        Vec::new()
    });
    Produced { kind, name: display_safe(name, 100), content, files, problem, warnings, risk }
}

/// What the job produced so far (read-only scan of its work folder).
pub fn job(root: &Path, token: &str) -> Option<Job> {
    let meta = job_meta(root, token)?;
    let work = root.join(token).join("work");
    let mut produced = Vec::new();
    let mut stray = Vec::new();
    for k in Kind::ALL {
        let dir = work.join(k.folder());
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.filter_map(Result::ok).take(50) {
            let fname = e.file_name().to_string_lossy().to_string();
            let ft = std::fs::symlink_metadata(e.path()).map(|m| m.file_type());
            match (k, ft) {
                (Kind::Skill, Ok(t)) if t.is_dir() => {
                    let files = skill_files(&e.path()).map(|v| v.into_iter().map(|(r, _)| r).collect());
                    produced.push(produced_one(k, &fname, &e.path().join("SKILL.md"), files));
                }
                (Kind::Command, Ok(t)) if t.is_file() && fname.ends_with(".md") => {
                    produced.push(produced_one(k, fname.trim_end_matches(".md"), &e.path(), Ok(vec![fname.clone()])));
                }
                (Kind::Workflow, Ok(t)) if t.is_file() && fname.ends_with(".js") => {
                    produced.push(produced_one(k, fname.trim_end_matches(".js"), &e.path(), Ok(vec![fname.clone()])));
                }
                _ => stray.push(display_safe(&format!("{}/{fname}", k.folder()), 120)),
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(&work) {
        for e in entries.filter_map(Result::ok).take(50) {
            let n = e.file_name().to_string_lossy().to_string();
            if !Kind::ALL.iter().any(|k| k.folder() == n) {
                stray.push(display_safe(&n, 120));
            }
        }
    }
    produced.sort_by(|a, b| a.name.cmp(&b.name));
    Some(Job { token: token.into(), kind: meta.kind, created_ms: meta.created_ms, bg_id: meta.bg_id, title: meta.title, produced, stray })
}

/// Jobs under `root`, newest first.
pub fn jobs(root: &Path) -> Vec<Job> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut v: Vec<Job> = entries.filter_map(Result::ok).take(100).filter_map(|e| job(root, &e.file_name().to_string_lossy())).collect();
    v.sort_by_key(|j| std::cmp::Reverse(j.created_ms));
    v
}

/// Installs one produced item at `target` through the same checks as a direct create (never overwrites).
pub fn install_produced(root: &Path, token: &str, kind: Kind, name: &str, target: &Target) -> Result<PathBuf, String> {
    if !valid_name(name) {
        return Err(crate::i18n::Fail::NameBad.str().into());
    }
    job_meta(root, token).ok_or(crate::i18n::Fail::JobNotFound.str())?;
    let work = root.join(token).join("work").join(kind.folder());
    match kind {
        Kind::Skill => {
            let src = work.join(name);
            if !std::fs::symlink_metadata(&src).is_ok_and(|m| m.file_type().is_dir()) {
                return Err(crate::i18n::Fail::SkillMadeMissing.str().into());
            }
            let files = skill_files(&src)?;
            let main = read_text(&src.join("SKILL.md"))?;
            check_content(kind, name, &main)?;
            let skills = kind_dir(target, kind)?;
            if std::fs::symlink_metadata(skills.join(name)).is_ok() {
                return Err(crate::i18n::Fail::NameExists.str().into());
            }
            // Built next to the target, then renamed exclusively: no half-written skill is ever left (QA LOW).
            let tmp = skills.join(format!(".dduck-{}", crate::install_link::random_token()));
            let built = (|| -> Result<(), String> {
                mkdir(&tmp, target.dir_mode()).map_err(|_| crate::i18n::Fail::MakeDirFailed.str().to_string())?;
                for (rel, path) in &files {
                    let parts: Vec<String> = rel.split('/').map(str::to_string).collect();
                    let parent = real_dirs(&tmp, &parts[..parts.len() - 1], target.dir_mode())?;
                    let mut bytes = Vec::new();
                    std::fs::File::open(path).and_then(|f| f.take(1024 * 1024).read_to_end(&mut bytes)).map_err(|_| crate::i18n::Fail::ReadFileFailed.str().to_string())?;
                    write_new(&parent.join(&parts[parts.len() - 1]), &bytes)?;
                }
                crate::install_link::rename_exclusive(&tmp, &skills.join(name)).map_err(|e| if e == crate::i18n::Fail::SkillExists.str() { crate::i18n::Fail::NameExists.str().to_string() } else { e })
            })();
            if let Err(e) = built {
                let _ = std::fs::remove_dir_all(&tmp);
                return Err(e);
            }
            Ok(skills.join(name).join("SKILL.md"))
        }
        _ => {
            let file = work.join(kind.parts(name).pop().unwrap_or_default());
            let text = read_text(&file)?;
            create_item(target, kind, name, &text)
        }
    }
}

/// The fixed tool set for an authoring job: file tools only — no Bash, no web (security review).
pub const JOB_TOOLS: &str = "Read,Write,Edit,Glob,Grep";
pub const MAX_REQUEST: usize = 20_000;

/// The "Claude에게 부탁하기" preset (D-086 review): cwd = the job's empty work folder, acceptEdits, effort medium,
/// the tools above, the request as the user prompt after `--`, and the guide plus the target folder (validated,
/// no control characters) as the appended system prompt. The webview supplies only the kind and the request.
pub fn job_spec(work: &Path, kind: Kind, request: &str) -> Result<crate::claude_cli::TaskSpec, String> {
    use crate::claude_cli::{PermissionMode, TaskSpec};
    let req = request.trim();
    if req.is_empty() || req.chars().count() > MAX_REQUEST || req.contains('\0') {
        return Err(crate::i18n::ReasonCode::RequestLength.now());
    }
    let work = work.canon().map_err(|_| crate::i18n::Fail::TaskDirMissing.str().to_string())?;
    let shown = work.to_string_lossy().to_string();
    if shown.chars().any(|c| c.is_control() || crate::text::is_unsafe_char(c)) {
        return Err(crate::i18n::Fail::JobDirPath.str().into());
    }
    let shape = match kind {
        Kind::Skill => "skills/<이름>/SKILL.md (보조 파일은 같은 폴더 안)",
        Kind::Command => "commands/<이름>.md",
        Kind::Workflow => "workflows/<이름>.js",
    };
    let policy = format!(
        "{GUIDE}\n## 이번 작업\n- 만들 것: {label} 하나.\n- 결과는 현재 작업 폴더({shown}) 안의 {shape}에만 만든다. 이 폴더 밖에는 아무것도 만들거나 고치지 않는다.\n- 사용자는 D.Duck에서 결과를 리뷰한 뒤 직접 설치한다. 설치·복사·실행은 하지 않는다.\n- 끝나면 만든 파일 경로와 한 줄 요약만 답한다.",
        label = kind.label()
    );
    let mut spec = TaskSpec::basic(work, req.to_string(), Some(kind.task_name()), Some(PermissionMode::AcceptEdits));
    spec.effort = Some(crate::team::Effort::Medium);
    spec.tools = Some(JOB_TOOLS.to_string());
    spec.team_policy = true;
    spec.policy_text = Some(policy);
    // An app-owned job folder (not a repository): the default scope's deny list, no hook (delivery-guard).
    spec.guard_ctx = Some(crate::delivery_guard::GuardCtx::for_tests());
    spec.validate().map_err(|e| e.to_string())?;
    Ok(spec)
}

// ---------- "새로 만듦" record ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Made {
    pub kind: Kind,
    pub name: String,
    /// Canonical project folder, or `None` for the user's own folder.
    pub project: Option<String>,
    pub at_ms: i64,
}

pub fn made_list(file: &Path) -> Vec<Made> {
    std::fs::read(file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Keeps the newest 200 entries from the last 30 days.
pub fn record_made(file: &Path, m: Made) {
    let cutoff = m.at_ms - 30 * 86_400_000;
    let mut v: Vec<Made> = made_list(file).into_iter().filter(|x| x.at_ms >= cutoff && !(x.kind == m.kind && x.name == m.name && x.project == m.project)).collect();
    v.push(m);
    let skip = v.len().saturating_sub(200);
    let v: Vec<Made> = v.into_iter().skip(skip).collect();
    if let Ok(bytes) = serde_json::to_vec(&v) {
        let tmp = file.with_extension("tmp");
        let _ = std::fs::remove_file(&tmp);
        if write_new(&tmp, &bytes).is_ok() {
            let _ = std::fs::rename(&tmp, file);
        }
    }
}

/// Removes a finished job's folder. A job whose session is still running keeps its folder (QA LOW).
pub fn discard_job(root: &Path, token: &str, running: impl Fn(&str) -> bool) -> Result<(), String> {
    let meta = job_meta(root, token).ok_or(crate::i18n::Fail::JobNotFound.str())?;
    if meta.bg_id.as_deref().is_some_and(running) {
        return Err(crate::i18n::Fail::StillRunning.str().into());
    }
    std::fs::remove_dir_all(root.join(token)).map_err(|_| crate::i18n::Fail::JobDirRemove.str().to_string())
}

// ---------- /dduck-new terminal command ----------

/// `off` (no file), `ours` (exactly what D.Duck installed: hash recorded at install), `other`.
pub fn dduck_new_state(file: &Path, recorded: Option<&str>) -> &'static str {
    match std::fs::symlink_metadata(file) {
        Err(_) => "off",
        Ok(m) if !m.is_file() => "other",
        Ok(_) => {
            let now = std::fs::read(file).map(|b| crate::agents_catalog::token_of(&b)).unwrap_or_default();
            if recorded.map(str::trim).is_some_and(|r| !r.is_empty() && r == now) { "ours" } else { "other" }
        }
    }
}

/// The `/dduck-new` command: the same guide, no `allowed-tools`, no `!` shell lines.
pub fn dduck_new_command() -> String {
    format!(
        "---\ndescription: D.Duck 지침으로 스킬·커맨드·워크플로 만들기 (예: /dduck-new 스킬 PR 설명을 팀 형식으로 써 주는 스킬)\nargument-hint: [스킬|커맨드|워크플로] [만들고 싶은 것]\n---\n\n\
         사용자가 요청한 것: $ARGUMENTS\n\n\
         첫 단어가 종류(스킬·커맨드·워크플로)이에요. 종류가 없으면 먼저 물어보세요. 아래 지침을 따라 하나만 만들고, 만들 경로와 이름을 먼저 보여 준 뒤 사용자가 동의하면 만드세요.\n\
         기본 위치는 사용자 폴더(~/.claude/skills, ~/.claude/commands, ~/.claude/workflows)이고, 사용자가 프로젝트라고 하면 현재 프로젝트의 .claude/ 아래예요. 이미 있는 파일은 덮어쓰지 않아요.\n\n{GUIDE}"
    )
}

// ---------- "복사해서 내 것으로" (nav-ia, D-117) ----------

/// Files of a locked skill folder to copy: regular files only. Links, devices, hard-linked files and names with
/// `..` or odd characters are skipped (not copied, not followed); at most 200 files, 1 MB each, 5 MB in all.
fn copy_list(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut out = Vec::new();
    let mut total = 0u64;
    let mut seen = HashSet::new();
    let walk = walkdir::WalkDir::new(dir).follow_links(false).min_depth(1).max_depth(5).into_iter();
    for e in walk.filter_entry(|e| !e.file_type().is_symlink()) {
        let Ok(e) = e else { continue };
        let Ok(rel) = e.path().strip_prefix(dir) else { continue };
        let rel = rel_slash(rel);
        if !e.file_type().is_file() || rel.split('/').any(|p| !crate::install_link::valid_part(p)) || !seen.insert(rel.to_ascii_lowercase()) {
            continue;
        }
        let Ok(m) = std::fs::symlink_metadata(e.path()) else { continue };
        if !m.file_type().is_file() || nlink(e.path(), &m) != 1 {
            continue;
        }
        total += m.len();
        if m.len() > crate::install_link::MAX_FILE_BYTES || total > crate::install_link::MAX_TOTAL_BYTES || out.len() >= crate::install_link::MAX_FILES {
            return Err(crate::i18n::Fail::CopyTooBig.str().into());
        }
        out.push((rel, e.path().to_path_buf()));
    }
    out.sort();
    Ok(out)
}

/// `name:` in the frontmatter becomes `name`; one origin comment goes at the end (Claude drops HTML comments).
pub fn retitle(kind: Kind, text: &str, name: &str, origin: &str) -> String {
    let mut out = String::with_capacity(text.len() + 80);
    let fm_end = text.strip_prefix("---\n").and_then(|rest| rest.find("\n---").map(|i| i + 4));
    match (kind, fm_end) {
        (Kind::Skill, Some(end)) => {
            let (head, body) = text.split_at(end);
            let mut named = false;
            for line in head.split_inclusive('\n') {
                if !named && line.starts_with("name:") {
                    out.push_str(&format!("name: {name}\n"));
                    named = true;
                } else {
                    out.push_str(line);
                }
            }
            if !named {
                out.insert_str(4, &format!("name: {name}\n"));
            }
            out.push_str(body);
        }
        _ => out.push_str(text),
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    let origin: String = display_safe(origin, 120).chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '.' | '@' | '/') || (*c == '-')).collect();
    out.push_str(&format!("\n<!-- copied-from: {} -->\n", origin.replace("--", "-")));
    out
}

/// Copies a locked skill (whole folder, LICENSE and helper files included) or command into `target` as `name`.
/// The source is never changed and nothing is overwritten: a skill is built in a hidden folder next to the target
/// and renamed exclusively. Returns the new main file.
pub fn copy_item(src_main: &Path, kind: Kind, origin: &str, name: &str, target: &Target) -> Result<PathBuf, String> {
    if !valid_name(name) {
        return Err(crate::i18n::Fail::ItemNameFormat2.str().into());
    }
    if kind == Kind::Workflow {
        return Err(crate::i18n::Fail::WorkflowNoCopy.str().into());
    }
    // Windows-made files (CRLF) are copied with LF so the frontmatter and its `name:` are found.
    let main = retitle(kind, &read_text(src_main)?.replace("\r\n", "\n"), name, origin);
    check_content(kind, name, &main)?;
    if kind == Kind::Command {
        return create_item(target, kind, name, &main);
    }
    let src = src_main.parent().ok_or(crate::i18n::Fail::SourceDirMissing.str())?;
    let files = copy_list(src)?;
    let skills = kind_dir(target, kind)?;
    if std::fs::symlink_metadata(skills.join(name)).is_ok() {
        return Err(crate::i18n::Fail::NameExists.str().into());
    }
    let tmp = skills.join(format!(".dduck-{}", crate::install_link::random_token()));
    let built = (|| -> Result<(), String> {
        mkdir(&tmp, target.dir_mode()).map_err(|_| crate::i18n::Fail::MakeDirFailed.str().to_string())?;
        write_new(&tmp.join("SKILL.md"), main.as_bytes())?;
        for (rel, path) in files.iter().filter(|(r, _)| r != "SKILL.md") {
            let parts: Vec<String> = rel.split('/').map(str::to_string).collect();
            let parent = real_dirs(&tmp, &parts[..parts.len() - 1], target.dir_mode())?;
            let mut bytes = Vec::new();
            open_regular(path, crate::install_link::MAX_FILE_BYTES)?.take(crate::install_link::MAX_FILE_BYTES).read_to_end(&mut bytes).map_err(|_| crate::i18n::Fail::ReadFileFailed.str().to_string())?;
            write_new(&parent.join(&parts[parts.len() - 1]), &bytes)?;
        }
        crate::install_link::rename_exclusive(&tmp, &skills.join(name)).map_err(|e| if e == crate::i18n::Fail::SkillExists.str() { crate::i18n::Fail::NameExists.str().to_string() } else { e })
    })();
    if let Err(e) = built {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    Ok(skills.join(name).join("SKILL.md"))
}


#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn templates_pass_their_own_checks_and_carry_no_shell() {
        for k in Kind::ALL {
            let t = fill(k, "standup", "매일 아침 스탠드업 정리");
            assert!(check_content(k, "standup", &t).unwrap().is_empty(), "{k:?}: {:?}", check_content(k, "standup", &t));
            let r = skill_risk(&t);
            assert!(r.allowed_tools.is_none() && !r.hooks && r.shell_lines == 0, "{k:?}");
        }
        for k in Kind::ALL {
            let t = template_in(k, crate::i18n::Lang::En).replace("{{name}}", "standup").replace("{{description}}", "Daily standup notes");
            assert!(check_content(k, "standup", &t).unwrap().is_empty(), "en {k:?}");
            assert!(!t.chars().any(|c| ('가'..='힣').contains(&c)), "en {k:?} has Korean");
        }
        let cmd = dduck_new_command();
        let r = skill_risk(&cmd);
        assert!(r.allowed_tools.is_none() && !r.hooks && r.shell_lines == 0 && !cmd.contains("allowed-tools:") && !cmd.contains("!`"));
        assert!(cmd.contains("$ARGUMENTS") && cmd.contains("workflow-authoring"));
        assert!(!GUIDE.contains("!`") && !GUIDE.contains("allowed-tools:"));
    }

    #[test]
    fn job_preset_is_fixed_by_rust() {
        let tmp = tempfile::tempdir().unwrap();
        let tok = crate::install_link::random_token();
        let work = create_job(&tmp.path().join("authoring"), &tok, &JobMeta { kind: Kind::Command, created_ms: 1, bg_id: None, title: "x".into() }).unwrap();
        let spec = job_spec(&work, Kind::Command, "--dangerously-skip-permissions 스탠드업 커맨드\n만들어 줘").unwrap();
        let args: Vec<String> = spec.bg_args().iter().map(|a| a.to_string_lossy().to_string()).collect();
        let dash = args.iter().position(|a| a == "--").unwrap();
        assert_eq!(args[dash + 1], "--dangerously-skip-permissions 스탠드업 커맨드\n만들어 줘", "request only after --");
        assert!(args[..dash].contains(&"--permission-mode=acceptEdits".to_string()));
        assert!(args[..dash].contains(&"--tools=Read,Write,Edit,Glob,Grep".to_string()));
        assert!(args[..dash].contains(&"--effort=medium".to_string()));
        // Bash appears only as deny rules (delivery-guard); nothing grants it.
        assert!(!args.iter().filter(|a| !a.starts_with("--disallowedTools=")).any(|a| a.starts_with("--add-dir") || a.starts_with("--allowedTools") || a.contains("Bash")), "{args:?}");
        assert!(args[..dash].iter().any(|a| a.starts_with("--disallowedTools=Bash(gh pr merge *)")));
        let sys = args.iter().find(|a| a.starts_with("--append-system-prompt=")).unwrap();
        assert!(sys.contains("commands/<이름>.md") && sys.contains(&work.canon().unwrap().to_string_lossy().to_string()) && !sys.contains("스탠드업"));
        assert!(job_spec(&work, Kind::Skill, "  ").is_err());
        assert!(job_spec(&tmp.path().join("missing"), Kind::Skill, "x").is_err());
    }

    #[test]
    fn workflow_meta_rules() {
        let ok = "// 주석\n/* 블록 */\nexport const meta = {\n  name: 'triage',\n  description: \"이슈 분류\",\n  phases: [{ title: '수집' }, { title: '정리', detail: 'x' }],\n}\nphase('수집')\nawait agent('a', { phase: '수집' })\nphase(\"정리\")\n";
        let m = workflow_meta(ok).unwrap();
        assert_eq!((m.name.as_str(), m.description.as_str()), ("triage", "이슈 분류"));
        assert_eq!(m.phases, ["수집", "정리"]);
        assert_eq!(m.calls, ["수집", "정리"]);
        assert!(check_content(Kind::Workflow, "triage", ok).unwrap().is_empty());
        assert!(check_content(Kind::Workflow, "other", ok).unwrap_err().contains("meta.name을 other로"));
        assert_eq!(reason("const x = 1\nexport const meta = { name: 'a', description: 'b' }"), Some(MetaReason::NotFirstStatement), "meta must come first");
        let w = check_content(Kind::Workflow, "a", "export const meta = { name: 'a', description: 'b', phases: [{ title: 'A' }] }\nphase('B')\nconst t = Date.now()\n").unwrap();
        assert_eq!(w.len(), 3, "{w:?}");
        assert!(check_content(Kind::Workflow, "a", "export const meta = { name: 'a' }").is_err(), "description required");
    }

    // ---------- workflow-meta-check (D-183) ----------

    fn reason(text: &str) -> Option<MetaReason> {
        workflow_meta(text).err().map(|e| e.reason)
    }

    /// `export const meta = { name: 'a', description: 'd', <extra> }` and a body.
    fn with(extra: &str) -> String {
        format!("export const meta = {{ name: 'a', description: 'd', {extra} }}\nreturn 1\n")
    }

    #[test]
    fn qa_probe_cases_match_claude_code() {
        // Rejected by Claude Code 2.1.319 (scratchpad/wfqa/ccsim_result.txt).
        assert_eq!(reason("export const meta = { name: 'tmpl', description: 'd', whenToUse: `x ${1}` }\nreturn 1\n"), Some(MetaReason::TemplateInterpolation));
        assert_eq!(reason("export const meta = { name: 'spread', description: 'd', ...{} }\nreturn 1\n"), Some(MetaReason::NotLiteral));
        assert_eq!(reason("export const meta = { name: 'phvar', description: 'd', phases: [{ title: T }] }\nreturn 1\n"), Some(MetaReason::NotLiteral));
        assert_eq!(reason("export const meta = { name: 'fncall', description: 'd', whenToUse: String(1) }\nreturn 1\n"), Some(MetaReason::NotLiteral));
        // A type annotation on meta stops the parser; one in the body is out of scope (the editor says so).
        assert_eq!(reason("export const meta: Meta = { name: 'tsann', description: 'd' }\n"), Some(MetaReason::ParseError));
        assert!(workflow_meta("export const meta = { name: 'tsann', description: 'd' }\nconst x: string = 'a'\n").is_ok(), "body syntax is not checked");
        // Accepted: a correct escape used to be refused.
        let m = workflow_meta("export const meta = { name: 'esc', description: 'it\\'s' }\nreturn 1\n").unwrap();
        assert_eq!(m.description, "it's");
        assert!(check_content(Kind::Workflow, "esc", "export const meta = { name: 'esc', description: 'it\\'s' }\nreturn 1\n").is_ok());
        // The accepted QA files.
        assert!(check_workflow_meta("export const meta = {\n  name: 'triage',\n  description: '이슈를 분류해요',\n  phases: [{ title: '단계 1' }],\n}\n\nphase('단계 1')\n", "triage").ok);
    }

    #[test]
    fn values_follow_claude_code_rules() {
        for ok in ["n: -1", "n: - 2.5", "n: [1, 'b', \"c\", `d`, true, false, null, [], {}]", "n: { deep: { x: [1, { y: 2 }] } }", "n: [1, 2,]", "'quoted key': 1", "\"q\": 1", "12: 'num key'", "default: 1", "phases: 'not an array'"] {
            assert!(workflow_meta(&with(ok)).is_ok(), "{ok}");
        }
        for (bad, r) in [
            ("n: !0", MetaReason::UnsupportedUnary),
            ("n: +1", MetaReason::UnsupportedUnary),
            ("n: ~1", MetaReason::UnsupportedUnary),
            ("n: -'1'", MetaReason::UnsupportedUnary),
            ("n: --1", MetaReason::UnsupportedUnary),
            ("n: typeof 1", MetaReason::UnsupportedUnary),
            ("n: [1,,2]", MetaReason::SparseArray),
            ("n: [,]", MetaReason::SparseArray),
            ("__proto__: {}", MetaReason::ReservedKey),
            ("'constructor': 1", MetaReason::ReservedKey),
            ("prototype: 1", MetaReason::ReservedKey),
            ("[k]: 1", MetaReason::UnsupportedKey),
            ("`k`: 1", MetaReason::UnsupportedKey),
            ("n: /x/", MetaReason::NotLiteral),
            ("n: undefined", MetaReason::NotLiteral),
            ("n: Infinity", MetaReason::NotLiteral),
            ("n: 'a' + 'b'", MetaReason::NotLiteral),
            ("n: 'a'.length", MetaReason::NotLiteral),
            ("n: [...x]", MetaReason::NotLiteral),
            ("short", MetaReason::NotLiteral),
            ("f() { return 1 }", MetaReason::NotLiteral),
            ("get g() { return 1 }", MetaReason::NotLiteral),
            ("n: 1 2", MetaReason::ParseError),
        ] {
            assert_eq!(reason(&with(bad)), Some(r), "{bad}");
        }
    }

    #[test]
    fn numbers_follow_strict_mode() {
        for ok in ["0", "10", ".5", "5.", "1e3", "1E+3", "2.5e-3", "0x1F", "0o17", "0b101", "1_000", "0x1_F", "0.5"] {
            assert!(workflow_meta(&with(&format!("n: {ok}"))).is_ok(), "{ok}");
        }
        // BigInt is a Literal to Claude Code's parser (scratchpad/qa-evidence/workflow-meta-check/ccsim2.txt: big.js).
        for ok in ["1n", "0n", "0x1Fn", "0o7n", "0b1n", "1_000n"] {
            assert!(workflow_meta(&with(&format!("n: {ok}"))).is_ok(), "{ok}");
        }
        assert_eq!(reason(&with("n: -1n")), Some(MetaReason::UnsupportedUnary), "the minus takes numbers only");
        for bad in ["1.5n", "1e3n", ".5n", "017n", "01n", "1nn"] {
            assert_eq!(reason(&with(&format!("n: {bad}"))), Some(MetaReason::ParseError), "{bad}");
        }
        for bad in ["017", "08", "1__0", "1_", "0_1", "1._5", "0x", "1e", "3in", "1.5.5"] {
            assert!(reason(&with(&format!("n: {bad}"))).is_some(), "{bad}");
        }
        assert_eq!(reason(&with("n: 017")), Some(MetaReason::ParseError), "legacy octal");
    }

    #[test]
    fn string_escapes_decode_like_javascript() {
        let d = |lit: &str| workflow_meta(&format!("export const meta = {{ name: 'a', description: {lit} }}")).map(|m| m.description);
        assert_eq!(d(r#"'a\nb\tc\\d\'e\"f'"#).unwrap(), "a\nb\tc\\d'e\"f");
        assert_eq!(d(r"'\x41\u0042\u{43}\u{1F600}\uD83D\uDE00'").unwrap(), "ABC😀😀");
        assert_eq!(d("'line\\\ncontinued'").unwrap(), "linecontinued");
        assert_eq!(d("'cr\\\r\nlf'").unwrap(), "crlf");
        assert_eq!(d(r"'\0'").unwrap(), "\0");
        assert_eq!(d(r"'\q'").unwrap(), "q", "identity escape");
        assert_eq!(d("`multi\nline`").unwrap(), "multi\nline");
        assert_eq!(d("`a $ b {c}`").unwrap(), "a $ b {c}");
        for bad in [r"'\01'", r"'\1'", r"'\8'", r"'\x4'", r"'\u12'", r"'\u{110000}'", r"'\u{}'", r"'\uD800'", r"'\uDC00'", r"'\uD800\u0041'", "`\\07`"] {
            assert_eq!(d(bad).err().map(|e| e.reason), Some(MetaReason::NotLiteral), "{bad}");
        }
        assert_eq!(d("'bare\nnewline'").err().map(|e| e.reason), Some(MetaReason::ParseError));
        assert_eq!(d("'unterminated").err().map(|e| e.reason), Some(MetaReason::ParseError));
        assert_eq!(reason("export const meta = { n: 'truncated\\"), Some(MetaReason::NotLiteral), "escape cut at the end");
    }

    #[test]
    fn parentheses_are_stripped_like_claude_code() {
        // Claude Code's parser drops parentheses (ccsim2.txt: paren.js is accepted).
        let m = workflow_meta("export const meta = ({ name: ('paren'), description: (('d')) })\nreturn 1\n").unwrap();
        assert_eq!((m.name.as_str(), m.description.as_str()), ("paren", "d"));
        assert!(workflow_meta("export const meta = (( /* c */ { name: 'a', description: 'd' } ));").is_ok());
        for ok in ["n: ( [1, (2)] )", "n: ({ x: (1) })", "n: (-1)", "n: (`t`)", "n: (null)"] {
            assert!(workflow_meta(&with(ok)).is_ok(), "{ok}");
        }
        for (bad, r) in [
            ("n: (1, 2)", MetaReason::NotLiteral),
            ("n: ('a' + 'b')", MetaReason::NotLiteral),
            ("n: (a)", MetaReason::NotLiteral),
            ("n: ('a').x", MetaReason::NotLiteral),
            ("n: ()", MetaReason::ParseError),
            ("n: ('a'", MetaReason::ParseError),
            ("n: ('a'))", MetaReason::ParseError),
        ] {
            assert_eq!(reason(&with(bad)), Some(r), "{bad}");
        }
        for (bad, r) in [
            ("export const meta = ({ name: 'a', description: 'd' }", MetaReason::ParseError),
            ("export const meta = ({ name: 'a', description: 'd' }).x", MetaReason::NotFirstStatement),
            ("export const meta = ({ name: 'a', description: 'd' }, 1)", MetaReason::NotFirstStatement),
            ("export const meta = (make())", MetaReason::NotFirstStatement),
            ("export const meta = ('x')", MetaReason::NotFirstStatement),
        ] {
            assert_eq!(reason(bad), Some(r), "{bad}");
        }
        let deep = |n: usize| format!("export const meta = {{ name: 'a', description: 'd', x: {}1{} }}", "(".repeat(n), ")".repeat(n));
        assert!(workflow_meta(&deep(10)).is_ok());
        assert_eq!(reason(&deep(100_000)), Some(MetaReason::TooComplex), "no stack overflow");
        let outer = format!("export const meta = {}{{ name: 'a', description: 'd' }}{}", "(".repeat(100_000), ")".repeat(100_000));
        assert_eq!(reason(&outer), Some(MetaReason::TooComplex));
    }

    #[test]
    fn escaped_identifier_keys_are_decoded() {
        // `\u0064escription` is the key `description` to Claude Code (ccsim2.txt: idesc.js is accepted).
        let m = workflow_meta("export const meta = { name: 'a', \\u0064escription: 'd' }").unwrap();
        assert_eq!(m.description, "d");
        assert_eq!(workflow_meta("export const meta = { \\u{6E}ame: 'b', descr\\u0069ption: 'd' }").unwrap().name, "b");
        assert_eq!(reason(&with("\\u005f_proto__: {}")), Some(MetaReason::ReservedKey), "decoded before the reserved check");
        for bad in ["\\u0031x: 1", "a\\u0020b: 1", "\\x61: 1", "\\u00: 1", "\\u{110000}: 1", "\\u{}: 1", "\\: 1", "\\u0061"] {
            assert!(reason(&with(bad)).is_some(), "{bad}");
        }
    }

    #[test]
    fn edit_keeps_a_different_meta_name_as_a_warning() {
        let t = |n: &str| format!("export const meta = {{ name: '{n}', description: 'd' }}");
        // Creating still refuses; saving an existing file only warns which name Claude Code uses.
        assert!(check_content(Kind::Workflow, "triage", &t("sync")).is_err());
        let w = check_existing(Kind::Workflow, "triage", &t("sync")).unwrap();
        assert_eq!(w, ["Claude Code에는 sync로 등록돼요"]);
        assert_eq!(check_existing(Kind::Workflow, "triage", &t("plan")).unwrap(), ["Claude Code에는 plan으로 등록돼요"]);
        assert_eq!(check_existing(Kind::Workflow, "triage", &t("수집")).unwrap(), ["Claude Code에는 수집으로 등록돼요"]);
        assert_eq!(check_existing(Kind::Workflow, "triage", &t("메일")).unwrap(), ["Claude Code에는 메일로 등록돼요"]);
        assert!(check_existing(Kind::Workflow, "triage", &t("triage")).unwrap().is_empty());
        assert!(check_existing(Kind::Workflow, "triage", "export const meta = { name: `${x}`, description: 'd' }").is_err(), "other reasons still refuse");
        // The live check names the registered name (display-cleaned) only for a mismatch.
        let c = check_workflow_meta(&t("sy\u{202e}nc"), "triage");
        assert_eq!((c.ok, c.reason), (false, Some(MetaReason::NameMismatch)));
        assert!(c.registered_name.as_deref().is_some_and(|n| n.starts_with("sy") && !n.contains('\u{202e}')), "{c:?}");
        assert_eq!(check_workflow_meta(&t("triage"), "triage").registered_name, None);
    }

    #[test]
    fn save_refusals_name_the_reason_and_the_fix() {
        let e = |text: &str| check_content(Kind::Workflow, "a", text).unwrap_err();
        let m = |extra: &str| format!("export const meta = {{ name: 'a', description: 'd',\n {extra} }}");
        assert_eq!(e(&m("x: `${y}`")), "meta에 템플릿 보간을 쓸 수 없어요 · 보간을 빼고 따옴표 문자열로 써 주세요 · 2번째 줄");
        assert!(e(&m("x: [1,,2]")).starts_with("배열에 빈 칸을 둘 수 없어요 · "));
        assert!(e(&m("__proto__: 1")).starts_with("__proto__·constructor·prototype은 키로 쓸 수 없어요 · "));
        assert!(e(&format!("export const meta = {{ name: 'a', description: 'd', x: {}1{} }}", "[".repeat(40), "]".repeat(40))).starts_with("meta가 너무 크거나 깊어요 · "));
        assert_eq!(e("export const meta = { name: 'b', description: 'd' }"), "meta.name을 a로 맞춰 주세요");
        assert_eq!(check_content(Kind::Workflow, "plan", "export const meta = { name: 'b', description: 'd' }").unwrap_err(), "meta.name을 plan으로 맞춰 주세요");
        // Every reason has its own sentence, the same one the editor shows (src/i18n/ko|en/extensions.ts).
        let ui = |lang: &str| std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../src/i18n/{lang}/extensions.ts"))).ok();
        let mut seen = std::collections::HashSet::new();
        for r in MetaReason::ALL {
            for lang in [crate::i18n::Lang::Ko, crate::i18n::Lang::En] {
                let text = meta_reason_text(r, lang);
                assert!(text.contains(" · "), "{r:?} has a fix: {text}");
                assert!(seen.insert(text), "{r:?} is its own sentence");
                if let Some(cat) = ui(if lang == crate::i18n::Lang::Ko { "ko" } else { "en" }) {
                    if r != MetaReason::NameMismatch {
                        assert!(cat.contains(&format!("\"{}\"", meta_reason_text(r, lang))), "{r:?} {lang:?} matches the UI");
                    }
                }
            }
        }
    }

    #[test]
    fn keys_repeat_last_wins_and_name_description_rules() {
        let m = workflow_meta("export const meta = { name: 'x', description: 'first', description: 'last', name: 'a' }").unwrap();
        assert_eq!((m.name.as_str(), m.description.as_str()), ("a", "last"));
        assert_eq!(reason("export const meta = { name: '', description: 'd' }"), Some(MetaReason::EmptyName));
        assert_eq!(reason("export const meta = { description: 'd' }"), Some(MetaReason::MissingName));
        assert_eq!(reason("export const meta = { name: 1, description: 'd' }"), Some(MetaReason::MissingName));
        assert_eq!(reason("export const meta = { name: 'a', description: '' }"), Some(MetaReason::EmptyDescription));
        assert_eq!(reason("export const meta = { name: 'a' }"), Some(MetaReason::MissingDescription));
        // White space only is a non-empty string to Claude Code: saved, with a warning.
        let blank = "export const meta = { name: 'a', description: '   ' }";
        assert!(check_workflow_meta(blank, "a").ok && check_workflow_meta(blank, "a").blank_description);
        assert_eq!(check_content(Kind::Workflow, "a", blank).unwrap().len(), 1);
    }

    #[test]
    fn first_statement_and_what_follows() {
        assert!(workflow_meta("\u{feff}#!/usr/bin/env node\n// c\n/* b\n */ export  const\nmeta /* x */ = {name:'a',description:'d'};\nphase('x')").is_ok());
        assert!(workflow_meta("export const meta = { name: 'a', description: 'd' }\n!x").is_ok(), "ASI before !");
        for (bad, r) in [
            ("'use strict'\nexport const meta = { name: 'a', description: 'd' }", MetaReason::NotFirstStatement),
            ("export let meta = { name: 'a', description: 'd' }", MetaReason::NotFirstStatement),
            ("export const metadata = { name: 'a', description: 'd' }", MetaReason::NotFirstStatement),
            ("export const meta = make()", MetaReason::NotFirstStatement),
            ("export const meta = { name: 'a', description: 'd' }.x", MetaReason::NotFirstStatement),
            ("export const meta = { name: 'a', description: 'd' }\n(async () => 1)()", MetaReason::NotFirstStatement),
            ("export const meta = { name: 'a', description: 'd' } as const", MetaReason::NotFirstStatement),
            ("export const meta = { name: 'a', description: 'd' } return 1", MetaReason::ParseError),
            ("export const meta = { name: 'a', description: 'd'", MetaReason::ParseError),
            ("/* unclosed comment", MetaReason::ParseError),
            ("", MetaReason::NotFirstStatement),
        ] {
            assert_eq!(reason(bad), Some(r), "{bad:?}");
        }
    }

    #[test]
    fn issue_carries_a_line_number_only() {
        let e = workflow_meta("export const meta = {\n  name: 'a',\n  description: 'd',\n  x: String(1),\n}").unwrap_err();
        assert_eq!(e, MetaIssue { reason: MetaReason::NotLiteral, line: 4 });
        let json = serde_json::to_string(&check_workflow_meta("export const meta = { name: 'secret-value', description: f() }", "a")).unwrap();
        assert_eq!(json, r#"{"ok":false,"reason":"notLiteral","line":1,"blankDescription":false}"#);
    }

    #[test]
    fn name_is_compared_raw() {
        let t = |n: &str| format!("export const meta = {{ name: '{n}', description: 'd' }}");
        assert!(check_workflow_meta(&t("triage"), "triage").ok);
        assert_eq!(check_workflow_meta(&t("triage"), "triage-v2").reason, Some(MetaReason::NameMismatch));
        // An invisible character makes it another name (display_safe would have hidden it).
        assert_eq!(check_workflow_meta(&t("tri\u{200b}age"), "triage").reason, Some(MetaReason::NameMismatch));
        assert_eq!(check_workflow_meta(&t("tri\\u0061ge"), "triage").reason, None, "escapes are decoded first");
        assert!(check_content(Kind::Workflow, "triage", &t("tri\u{200b}age")).unwrap_err().contains("meta.name을 triage로"));
    }

    #[test]
    fn limits_answer_too_complex() {
        let deep = |n: usize| format!("export const meta = {{ name: 'a', description: 'd', x: {}1{} }}", "[".repeat(n), "]".repeat(n));
        assert!(workflow_meta(&deep(31)).is_ok(), "depth 32 with the meta object");
        assert_eq!(reason(&deep(32)), Some(MetaReason::TooComplex));
        assert_eq!(reason(&deep(100_000)), Some(MetaReason::TooComplex), "no stack overflow");
        let items = |n: usize| with(&format!("x: [{}]", vec!["1"; n].join(",")));
        assert!(workflow_meta(&items(1000)).is_ok());
        assert_eq!(reason(&items(1001)), Some(MetaReason::TooComplex));
        let tokens = with(&(0..12).map(|i| format!("k{i}: [{}]", vec!["1"; 900].join(","))).collect::<Vec<_>>().join(", "));
        assert_eq!(reason(&tokens), Some(MetaReason::TooComplex));
        assert_eq!(reason(&"x".repeat(MAX_CONTENT + 1)), Some(MetaReason::TooComplex));
    }

    #[test]
    fn hostile_inputs_never_panic() {
        for s in ["export const meta = {", "export const meta = { n: '\\u{", "export const meta = { n: '\\uD83D\\u", "export const meta = { n: 0x", "export const meta = { 'é", "export const meta = { n: `${", "export const meta = { n: -", "export const meta = { é: 'ü' } é", "\u{feff}", "#!", "export const meta = { n: '\\x\u{1F600}' }"] {
            let _ = workflow_meta(s);
            let _ = check_workflow_meta(s, "a");
        }
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(2000))]
        #[test]
        fn meta_parser_never_panics(s in "\\PC{0,200}") {
            let _ = check_workflow_meta(&s, "a");
            let _ = check_workflow_meta(&format!("export const meta = {{ name: 'a', description: 'd', x: {s}"), "a");
        }
        #[test]
        fn meta_parser_never_panics_on_js_alphabet(s in "[{}\\[\\]'\"`\\\\$:,./*+\\-!~()0-9a-fnux_ \n\r\u{2028}é😀]{0,120}") {
            let _ = check_workflow_meta(&format!("export const meta = {s}"), "a");
        }
    }

    #[test]
    fn create_never_overwrites_and_refuses_links() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        std::fs::create_dir_all(&cfg).unwrap();
        let user = Target { base: cfg.clone(), project: false };
        let cmd = fill(Kind::Command, "standup", "스탠드업");
        let p = create_item(&user, Kind::Command, "standup", &cmd).unwrap();
        assert_eq!(p, cfg.canon().unwrap().join("commands/standup.md"));
        assert_eq!(create_item(&user, Kind::Command, "standup", "x").unwrap_err(), "이미 있는 이름이에요");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), cmd);
        let skill = fill(Kind::Skill, "pr-writer", "PR 설명");
        create_item(&user, Kind::Skill, "pr-writer", &skill).unwrap();
        assert_eq!(create_item(&user, Kind::Skill, "pr-writer", &skill).unwrap_err(), "이미 있는 이름이에요");
        assert!(create_item(&user, Kind::Skill, "bad", &skill).unwrap_err().contains("name"), "frontmatter name must match");
        for bad in ["../x", "-x", "a b", "", &"a".repeat(65), "가"] {
            assert!(create_item(&user, Kind::Command, bad, &cmd).is_err(), "{bad}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(cfg.join("skills")).unwrap().permissions().mode() & 0o777, 0o700);
            let proj = tmp.path().join("proj");
            std::fs::create_dir_all(&proj).unwrap();
            let pt = Target { base: proj.clone(), project: true };
            create_item(&pt, Kind::Workflow, "triage", &fill(Kind::Workflow, "triage", "분류")).unwrap();
            assert_eq!(std::fs::metadata(proj.join(".claude/workflows")).unwrap().permissions().mode() & 0o777, 0o755);
            // A linked .claude or commands folder is refused.
            let proj2 = tmp.path().join("proj2");
            std::fs::create_dir_all(&proj2).unwrap();
            std::os::unix::fs::symlink(tmp.path().join("elsewhere"), proj2.join(".claude")).unwrap();
            std::fs::create_dir_all(tmp.path().join("elsewhere")).unwrap();
            assert!(create_item(&Target { base: proj2, project: true }, Kind::Command, "x", &cmd).unwrap_err().contains("링크"));
            // A linked user commands folder (dotfile manager) is followed.
            let dots = tmp.path().join("dots/commands");
            std::fs::create_dir_all(&dots).unwrap();
            let cfg2 = tmp.path().join("cfg2");
            std::fs::create_dir_all(&cfg2).unwrap();
            std::os::unix::fs::symlink(&dots, cfg2.join("commands")).unwrap();
            create_item(&Target { base: cfg2, project: false }, Kind::Command, "linked", &cmd).unwrap();
            assert!(dots.join("linked.md").is_file());
            assert!(!tmp.path().join("elsewhere/commands").exists());
        }
    }

    #[test]
    fn jobs_are_reviewed_then_installed_through_create() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("authoring");
        let tok = crate::install_link::random_token();
        let work = create_job(&root, &tok, &JobMeta { kind: Kind::Skill, created_ms: 5, bg_id: None, title: "PR".into() }).unwrap();
        std::fs::create_dir_all(work.join("skills/pr-writer/scripts")).unwrap();
        std::fs::write(work.join("skills/pr-writer/SKILL.md"), fill(Kind::Skill, "pr-writer", "PR")).unwrap();
        std::fs::write(work.join("skills/pr-writer/scripts/x.sh"), "echo").unwrap();
        std::fs::write(work.join("commands/bad name.md"), "x").unwrap();
        std::fs::write(work.join("notes.txt"), "x").unwrap();
        let j = job(&root, &tok).unwrap();
        assert_eq!(j.produced.len(), 2);
        let pr = j.produced.iter().find(|p| p.name == "pr-writer").unwrap();
        assert!(pr.problem.is_none() && pr.files == ["SKILL.md", "scripts/x.sh"], "{pr:?}");
        assert!(j.produced.iter().any(|p| p.name == "bad name" && p.problem.is_some()));
        assert_eq!(j.stray, ["notes.txt"]);
        let cfg = tmp.path().join("cfg");
        std::fs::create_dir_all(&cfg).unwrap();
        let t = Target { base: cfg.clone(), project: false };
        install_produced(&root, &tok, Kind::Skill, "pr-writer", &t).unwrap();
        assert!(cfg.join("skills/pr-writer/scripts/x.sh").is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(cfg.join("skills/pr-writer/scripts/x.sh")).unwrap().permissions().mode() & 0o111, 0);
        }
        assert_eq!(install_produced(&root, &tok, Kind::Skill, "pr-writer", &t).unwrap_err(), "이미 있는 이름이에요");
        assert!(std::fs::read_dir(cfg.join("skills")).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with(".dduck-")), "no temp left");
        assert!(install_produced(&root, &tok, Kind::Command, "bad name", &t).is_err());
        assert!(install_produced(&root, "nope", Kind::Skill, "pr-writer", &t).is_err());
        assert_eq!(jobs(&root).len(), 1);

        // Discard: refused while the session runs, allowed after.
        let mut m = job_meta(&root, &tok).unwrap();
        m.bg_id = Some("ab12".into());
        save_job(&root, &tok, &m).unwrap();
        assert!(discard_job(&root, &tok, |id| id == "ab12").unwrap_err().contains("작업 중"));
        discard_job(&root, &tok, |_| false).unwrap();
        assert!(jobs(&root).is_empty());

        // /dduck-new: only the exact installed bytes count as ours.
        let cmd = cfg.join("commands/dduck-new.md");
        assert_eq!(dduck_new_state(&cmd, None), "off");
        create_item(&t, Kind::Command, DDUCK_NEW, &dduck_new_command()).unwrap();
        let h = crate::agents_catalog::token_of(dduck_new_command().as_bytes());
        assert_eq!(dduck_new_state(&cmd, Some(&h)), "ours");
        assert_eq!(dduck_new_state(&cmd, None), "other");
        std::fs::write(&cmd, "changed").unwrap();
        assert_eq!(dduck_new_state(&cmd, Some(&h)), "other", "edited after install");

        let made = tmp.path().join("made.json");
        record_made(&made, Made { kind: Kind::Skill, name: "pr-writer".into(), project: None, at_ms: 100 });
        record_made(&made, Made { kind: Kind::Skill, name: "pr-writer".into(), project: None, at_ms: 200 });
        assert_eq!(made_list(&made).len(), 1);
    }

    #[test]
    fn copy_item_copies_the_folder_renames_and_never_overwrites() {
        let t = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let src = t.path().join("cache/frontend-design");
        std::fs::create_dir_all(src.join("refs")).ok();
        std::fs::write(src.join("SKILL.md"), "---\nname: frontend-design\ndescription: d\n---\nbody\n").ok();
        std::fs::write(src.join("LICENSE"), "MIT").ok();
        std::fs::write(src.join("refs/a.md"), "a").ok();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/hosts", src.join("hosts")).ok();
        let user = t.path().join("config");
        std::fs::create_dir_all(&user).ok();
        let target = Target { base: user.canon().unwrap_or_else(|_| unreachable!()), project: false };
        let before = std::fs::read_to_string(src.join("SKILL.md")).unwrap_or_default();
        let made = copy_item(&src.join("SKILL.md"), Kind::Skill, "example-plugin:frontend-design", "frontend-design-custom", &target).unwrap_or_else(|e| panic!("{e}"));
        let text = std::fs::read_to_string(&made).unwrap_or_default();
        assert!(text.starts_with("---\nname: frontend-design-custom\n"));
        assert!(text.trim_end().ends_with("<!-- copied-from: example-plugin:frontend-design -->"));
        let dir = made.parent().unwrap_or_else(|| unreachable!());
        assert_eq!(std::fs::read_to_string(dir.join("LICENSE")).unwrap_or_default(), "MIT");
        assert!(dir.join("refs/a.md").is_file());
        assert!(std::fs::symlink_metadata(dir.join("hosts")).is_err(), "links are not copied");
        assert_eq!(std::fs::read_to_string(src.join("SKILL.md")).unwrap_or_default(), before, "source unchanged");
        let again = copy_item(&src.join("SKILL.md"), Kind::Skill, "x", "frontend-design-custom", &target);
        assert_eq!(again.err().as_deref(), Some("이미 있는 이름이에요"));
        assert!(copy_item(&src.join("SKILL.md"), Kind::Skill, "x", "../evil", &target).is_err());
        assert!(retitle(Kind::Command, "hi", "n", "a --> b").ends_with("<!-- copied-from: a-b -->\n"));
        // A Windows-made SKILL.md (CRLF) copies with LF and the new name.
        let crlf = t.path().join("cache/crlf");
        std::fs::create_dir_all(&crlf).ok();
        std::fs::write(crlf.join("SKILL.md"), "---\r\nname: crlf\r\ndescription: d\r\n---\r\nbody\r\n").ok();
        let made = copy_item(&crlf.join("SKILL.md"), Kind::Skill, "p:crlf", "crlf-custom", &target).unwrap_or_else(|e| panic!("{e}"));
        assert!(std::fs::read_to_string(&made).unwrap_or_default().starts_with("---\nname: crlf-custom\n"));
        // A linked main file is refused, and copy_risk sees allowed-tools.
        #[cfg(unix)]
        {
            let linked = t.path().join("cache/linked");
            std::fs::create_dir_all(&linked).ok();
            std::os::unix::fs::symlink(src.join("SKILL.md"), linked.join("SKILL.md")).ok();
            assert!(copy_item(&linked.join("SKILL.md"), Kind::Skill, "x", "linked-custom", &target).is_err());
        }
        std::fs::write(crlf.join("SKILL.md"), "---\nname: crlf\ndescription: d\nallowed-tools: Bash\n---\nbody\n").ok();
        assert!(copy_risk(&crlf.join("SKILL.md")).map(|r| r.allowed_tools.is_some()).unwrap_or(false));
    }
}
