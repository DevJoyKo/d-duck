//! Claude Code rules (rules-editor, D-106): `.md` files under `~/.claude/rules/` (global) and
//! `<project>/.claude/rules/` (project), loaded every session or, with a `paths` frontmatter, only for matching files.
//!
//! - "Off" never moves a file: the rule's absolute path goes into `claudeMdExcludes` (project rules →
//!   `<project>/.claude/settings.local.json`, global rules → `~/.claude/settings.json`). "On" removes only that exact
//!   string; patterns the user wrote are never touched, and a rule they match shows as off by another setting.
//! - Only `paths` is edited, as text: other frontmatter lines and the body bytes stay. Frontmatter that is not of the
//!   simple form (block list or one-line value, no duplicates, closed) is read-only. Every write is re-parsed first.
//! - The webview sends an opaque id from the listing (or, for a new file, a name matching a fixed pattern), hash
//!   tokens and the new text. Directories are checked with lstat (no links below the config folder, which is
//!   canonicalized once), files must be regular with one link, saves go through the guarded temp + rename.
//! - Rule text is never logged, stored or put into an error. Git is never run (`memory::git_tracked`).

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::agents_catalog::{self, SaveError, token_of};
use crate::memory::{SecretHit, git_tracked, instruction_like, secrets};
use crate::text::{display_safe, is_unsafe_char};

pub const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_DEPTH: usize = 6;
const MAX_FILES: usize = 300;
const MAX_PATTERNS: usize = 50;
const MAX_PATTERN_CHARS: usize = 300;
const EXPANSION_LIMIT: u64 = 1_000;
const EXPANSION_BYTES: u64 = 4 * 1024 * 1024;
const KEY: &str = "claudeMdExcludes";
const TEMPLATES: &str = include_str!("../data/authoring_templates.json");

fn re(s: &str) -> Regex {
    Regex::new(s).unwrap_or_else(|_| unreachable!())
}
static NEW_NAME_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\A[A-Za-z0-9_-]{1,64}(/[A-Za-z0-9_-]{1,64})?\.md\z"));
static TOP_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\A([A-Za-z_][A-Za-z0-9_-]*):(?:[ \t]+(.*?))?[ \t]*\z"));
static ITEM_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\A( *)-(?:[ ]+(.*?))?[ \t]*\z"));

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    #[error("{}", crate::i18n::Fail::RuleNotListed.str())]
    NotFound,
    #[error("{0}")]
    Unsafe(&'static str),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("{}", crate::i18n::Fail::FileChangedShort.str())]
    Conflict,
    #[error("{}", crate::i18n::Fail::RuleExists.str())]
    Exists,
    #[error("{}", crate::i18n::Fail::SaveFileFailed.str())]
    Io,
}

impl From<SaveError> for RuleError {
    fn from(e: SaveError) -> Self {
        match e {
            SaveError::NotFound => RuleError::NotFound,
            SaveError::ReadOnly(s) => RuleError::Unsafe(s),
            SaveError::Invalid(s) => RuleError::Invalid(s),
            SaveError::Conflict => RuleError::Conflict,
            SaveError::Io => RuleError::Io,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    Global,
    Project,
}

/// Where rules live. `config_dir` is Claude Code's folder (`~/.claude`), `project` a folder the app already knows.
#[derive(Debug, Clone)]
pub struct Roots {
    pub config_dir: PathBuf,
    pub project: Option<PathBuf>,
    /// The user's home folder. A project that is the home folder (or when home is unknown) has no project scope:
    /// `~/.claude/rules` would be listed twice and its "local" settings would be the user's own.
    pub home: Option<PathBuf>,
}

pub const HOME_REASON: crate::i18n::Fail = crate::i18n::Fail::HomeIsGlobal;
const NO_HOME_REASON: crate::i18n::Fail = crate::i18n::Fail::NoHomeRules;

/// `Some(reason)` when `real` (canonical project folder) is the home folder or home cannot be resolved.
/// Compared by device + inode of the canonical folders, so links and case differences do not slip through.
fn home_refusal(real: &Path, home: Option<&Path>) -> Option<&'static str> {
    let Some(h) = home.filter(|h| !h.as_os_str().is_empty()).and_then(|h| h.canonicalize().ok()) else { return Some(NO_HOME_REASON.str()) };
    match (std::fs::symlink_metadata(real), std::fs::symlink_metadata(&h)) {
        (Ok(a), Ok(b)) => {
            let ((ad, ai, _), (bd, bi, _)) = (ids(&a), ids(&b));
            (cfg!(not(unix)) && real == h || cfg!(unix) && ad == bd && ai == bi).then_some(HOME_REASON.str())
        }
        _ => Some(NO_HOME_REASON.str()),
    }
}

/// One scope's folders: `shown` is the path Claude Code walks (used for `claudeMdExcludes` strings), `real` the
/// canonical one used for I/O. `rules` is `None` when the folder does not exist yet.
struct Base {
    scope: Scope,
    shown_rules: PathBuf,
    real_rules: Option<PathBuf>,
    /// The folder that holds `rules` (`<config>` or `<project>/.claude`), canonical, when it is a real folder.
    real_parent: Option<PathBuf>,
    project_real: Option<PathBuf>,
    /// Settings file the app writes its exclusions to.
    target: PathBuf,
    /// Every settings layer whose `claudeMdExcludes` applies.
    layers: Vec<PathBuf>,
    /// Project scope refused (home folder): listed empty and read-only, never read or written.
    refused: Option<&'static str>,
}

fn is_real_dir(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_dir())
}

#[cfg(unix)]
fn ids(m: &std::fs::Metadata) -> (u64, u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (m.dev(), m.ino(), m.nlink())
}

#[cfg(not(unix))]
fn ids(_m: &std::fs::Metadata) -> (u64, u64, u64) {
    (0, 0, 1)
}

fn base(roots: &Roots, scope: Scope) -> Option<Base> {
    let user_settings_real = roots.config_dir.canonicalize().ok().map(|c| c.join("settings.json"));
    match scope {
        Scope::Global => {
            let real = roots.config_dir.canonicalize().ok().filter(|c| is_real_dir(c));
            let target = real.as_ref().map_or_else(|| roots.config_dir.join("settings.json"), |c| c.join("settings.json"));
            let mut layers = vec![target.clone()];
            if let Some(p) = roots.project.as_ref().and_then(|p| p.canonicalize().ok()).filter(|p| home_refusal(p, roots.home.as_deref()).is_none()) {
                layers.push(p.join(".claude").join("settings.json"));
                layers.push(p.join(".claude").join("settings.local.json"));
            }
            Some(Base {
                scope,
                shown_rules: roots.config_dir.join("rules"),
                real_rules: real.as_ref().map(|c| c.join("rules")).filter(|r| is_real_dir(r)),
                real_parent: real,
                project_real: None,
                target,
                layers,
                refused: None,
            })
        }
        Scope::Project => {
            let shown = roots.project.as_ref()?;
            let real = shown.canonicalize().ok().filter(|c| is_real_dir(c))?;
            // Also refused when `<project>/.claude` is the Claude config folder itself (project-switcher).
            let env = crate::projects::Env { config_dir: &roots.config_dir, custom: None, home: roots.home.as_deref() };
            let refused = home_refusal(&real, roots.home.as_deref()).or_else(|| (!roots.config_dir.as_os_str().is_empty()).then(|| crate::projects::home_like(&real, &env)).flatten());
            let claude = real.join(".claude");
            let parent = is_real_dir(&claude).then(|| claude.clone());
            let target = claude.join("settings.local.json");
            let mut layers: Vec<PathBuf> = user_settings_real.into_iter().collect();
            layers.push(claude.join("settings.json"));
            layers.push(target.clone());
            Some(Base {
                scope,
                shown_rules: shown.join(".claude").join("rules"),
                real_rules: parent.as_ref().map(|c| c.join("rules")).filter(|r| is_real_dir(r)),
                real_parent: parent,
                project_real: Some(real),
                target,
                layers,
                refused,
            })
        }
    }
}

// ---------- frontmatter ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FmState {
    /// No frontmatter.
    None,
    /// Simple form: `paths` can be edited.
    Ok,
    /// Valid-looking but not the simple form: `paths` is read-only.
    Complex,
    /// Claude Code would ignore it (and load the rule always).
    Broken,
}

#[derive(Debug, Clone)]
struct Fm {
    state: FmState,
    bom: usize,
    /// Byte offset where the body starts (after the closing `---` line).
    body_start: usize,
    /// Byte offset of the closing marker line.
    close_at: usize,
    /// Byte range of the `paths` key line and its items.
    span: Option<(usize, usize)>,
    paths: Vec<String>,
    /// Inner lines outside the `paths` span (without line endings).
    others: Vec<String>,
}

fn strip_eol(l: &str) -> &str {
    l.strip_suffix("\r\n").or_else(|| l.strip_suffix('\n')).unwrap_or(l)
}

fn eol_of(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// Splits a comma-separated `paths` string, keeping commas inside braces.
fn split_commas(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut esc) = (Vec::new(), String::new(), 0usize, false);
    for c in s.chars() {
        if esc {
            cur.push(c);
            esc = false;
            continue;
        }
        match c {
            '\\' => {
                esc = true;
                cur.push(c);
            }
            '{' => {
                depth += 1;
                cur.push(c);
            }
            '}' => {
                depth = depth.saturating_sub(1);
                cur.push(c);
            }
            ',' if depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect()
}

enum Scalar {
    Value(String),
    Complex,
    Broken,
}

/// One YAML scalar in the simple subset: `"…"` (only `\"` and `\\` escapes), `'…'` or plain.
fn scalar(v: &str) -> Scalar {
    let v = v.trim();
    if let Some(rest) = v.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = rest.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some(_) => return Scalar::Complex,
                    None => return Scalar::Broken,
                },
                '"' => {
                    let tail = chars.as_str().trim();
                    return if tail.is_empty() || tail.starts_with('#') { Scalar::Value(out) } else { Scalar::Broken };
                }
                _ => out.push(c),
            }
        }
        return Scalar::Broken;
    }
    if let Some(rest) = v.strip_prefix('\'') {
        let mut out = String::new();
        let mut chars = rest.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                    out.push('\'');
                    continue;
                }
                let tail: String = chars.collect();
                let tail = tail.trim();
                return if tail.is_empty() || tail.starts_with('#') { Scalar::Value(out) } else { Scalar::Broken };
            }
            out.push(c);
        }
        return Scalar::Broken;
    }
    if v.starts_with(['*', '&', '!', '%', '@', '`']) {
        return Scalar::Broken;
    }
    if v.starts_with(['[', '{', '|', '>', '?', ':', '-']) || v.contains(": ") || v.contains(" #") {
        return Scalar::Complex;
    }
    Scalar::Value(v.to_string())
}

fn parse_fm(text: &str) -> Fm {
    let bom = if text.starts_with('\u{FEFF}') { '\u{FEFF}'.len_utf8() } else { 0 };
    let none = |state| Fm { state, bom, body_start: bom, close_at: bom, span: None, paths: Vec::new(), others: Vec::new() };
    let rest = &text[bom..];
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut at = bom;
    for l in rest.split_inclusive('\n') {
        lines.push((at, l));
        at += l.len();
    }
    if lines.first().map(|(_, l)| strip_eol(l)) != Some("---") {
        return none(FmState::None);
    }
    let Some(close) = lines.iter().skip(1).position(|(_, l)| strip_eol(l) == "---").map(|i| i + 1) else {
        return none(FmState::Broken);
    };
    let (close_at, close_line) = lines[close];
    let mut fm = Fm { state: FmState::Ok, bom, body_start: close_at + close_line.len(), close_at, span: None, paths: Vec::new(), others: Vec::new() };
    let inner = &lines[1..close];
    let mut worst = FmState::Ok;
    let bump = |s: FmState, worst: &mut FmState| {
        if s == FmState::Broken || (s == FmState::Complex && *worst == FmState::Ok) {
            *worst = s;
        }
    };
    let mut seen_keys: Vec<String> = Vec::new();
    let mut i = 0;
    while i < inner.len() {
        let l = strip_eol(inner[i].1);
        if l.trim().is_empty() || l.starts_with('#') {
            fm.others.push(l.to_string());
            i += 1;
            continue;
        }
        if l.starts_with('\t') {
            bump(FmState::Broken, &mut worst);
            fm.others.push(l.to_string());
            i += 1;
            continue;
        }
        let Some(c) = TOP_RE.captures(l) else {
            bump(FmState::Complex, &mut worst);
            fm.others.push(l.to_string());
            i += 1;
            continue;
        };
        let key = c.get(1).map_or("", |m| m.as_str()).to_string();
        let inline = c.get(2).map(|m| m.as_str()).unwrap_or("");
        let mut j = i + 1;
        while j < inner.len() {
            let n = strip_eol(inner[j].1);
            if n.starts_with([' ', '\t', '-']) && !n.trim().is_empty() {
                j += 1;
            } else {
                break;
            }
        }
        if seen_keys.contains(&key) {
            bump(FmState::Complex, &mut worst);
        }
        seen_keys.push(key.clone());
        if key != "paths" {
            fm.others.extend(inner[i..j].iter().map(|(_, l)| strip_eol(l).to_string()));
            i = j;
            continue;
        }
        if fm.span.is_some() {
            fm.others.extend(inner[i..j].iter().map(|(_, l)| strip_eol(l).to_string()));
            i = j;
            continue;
        }
        let end = inner.get(j).map_or(close_at, |(o, _)| *o);
        fm.span = Some((inner[i].0, end));
        let items = &inner[i + 1..j];
        if !inline.is_empty() {
            if !items.is_empty() {
                bump(FmState::Complex, &mut worst);
            }
            match scalar(inline) {
                Scalar::Value(v) => fm.paths = split_commas(&v),
                Scalar::Complex => bump(FmState::Complex, &mut worst),
                Scalar::Broken => bump(FmState::Broken, &mut worst),
            }
        } else {
            let mut indent: Option<usize> = None;
            for (_, raw) in items {
                let n = strip_eol(raw);
                if n.starts_with('\t') || n.trim_start_matches(' ').starts_with('\t') {
                    bump(FmState::Broken, &mut worst);
                    continue;
                }
                let Some(ic) = ITEM_RE.captures(n) else {
                    bump(FmState::Complex, &mut worst);
                    continue;
                };
                let ind = ic.get(1).map_or(0, |m| m.as_str().len());
                if *indent.get_or_insert(ind) != ind {
                    bump(FmState::Complex, &mut worst);
                }
                match ic.get(2).map(|m| m.as_str()).filter(|v| !v.is_empty()).map(scalar) {
                    Some(Scalar::Value(v)) if !v.is_empty() => fm.paths.push(v),
                    Some(Scalar::Broken) => bump(FmState::Broken, &mut worst),
                    _ => bump(FmState::Complex, &mut worst),
                }
            }
        }
        i = j;
    }
    fm.state = worst;
    fm
}

fn quote(p: &str) -> String {
    format!("\"{}\"", p.replace('\\', "\\\\").replace('"', "\\\""))
}

fn render_paths(paths: &[String], eol: &str) -> String {
    let mut s = format!("paths:{eol}");
    for p in paths {
        s.push_str(&format!("  - {}{eol}", quote(p)));
    }
    s
}

/// Checks the patterns the user typed. Refuses only what would break the file or the YAML.
pub fn check_patterns(paths: &[String]) -> Result<(), RuleError> {
    if paths.len() > MAX_PATTERNS {
        return Err(RuleError::Invalid(crate::i18n::Fail::PathsMax.str()));
    }
    for p in paths {
        if p.trim().is_empty() || p.trim() != p {
            return Err(RuleError::Invalid(crate::i18n::Fail::PathsSpaces.str()));
        }
        if p.chars().count() > MAX_PATTERN_CHARS {
            return Err(RuleError::Invalid(crate::i18n::Fail::PathsLong.str()));
        }
        if p.chars().any(|c| is_unsafe_char(c) || c == '\u{0085}') {
            return Err(RuleError::Invalid(crate::i18n::Fail::PathsInvisible.str()));
        }
    }
    Ok(())
}

/// The file text with `paths` set to `paths` (empty = key removed, and the frontmatter too when nothing else is
/// left). Every other byte stays; the result is re-parsed and refused unless it says exactly that.
fn with_paths(text: &str, paths: &[String]) -> Result<String, RuleError> {
    check_patterns(paths)?;
    let fm = parse_fm(text);
    let eol = eol_of(text);
    let out = match fm.state {
        FmState::Complex | FmState::Broken => return Err(RuleError::Unsafe(crate::i18n::Fail::FrontComplex.str())),
        FmState::None if paths.is_empty() => text.to_string(),
        FmState::None => format!("{}---{eol}{}---{eol}{}", &text[..fm.bom], render_paths(paths, eol), &text[fm.bom..]),
        FmState::Ok => {
            let block = if paths.is_empty() { String::new() } else { render_paths(paths, eol) };
            let (a, b) = fm.span.unwrap_or((fm.close_at, fm.close_at));
            let joined = format!("{}{block}{}", &text[..a], &text[b..]);
            let again = parse_fm(&joined);
            if paths.is_empty() && again.state == FmState::Ok && again.span.is_none() && again.others.iter().all(|l| l.trim().is_empty()) {
                format!("{}{}", &joined[..again.bom], &joined[again.body_start..])
            } else {
                joined
            }
        }
    };
    let (old, new) = (parse_fm(text), parse_fm(&out));
    let same_body = text[old.body_start..] == out[new.body_start..];
    let fm_ok = if paths.is_empty() { matches!(new.state, FmState::None | FmState::Ok) && new.paths.is_empty() } else { new.state == FmState::Ok && new.paths == paths };
    let others_ok = new.state == FmState::None || new.others == old.others;
    if !(same_body && fm_ok && others_ok) || out[..new.bom] != text[..old.bom] {
        return Err(RuleError::Unsafe(crate::i18n::Fail::FrontUnsafe.str()));
    }
    Ok(out)
}

/// `head` of `text` (BOM + frontmatter) followed by a new body.
fn with_body(text: &str, body: &str) -> String {
    let fm = parse_fm(text);
    format!("{}{body}", &text[..fm.body_start])
}

// ---------- glob warnings ----------

/// Number of patterns `p` expands to (`{a,b}` groups with a top-level comma), saturating.
fn expansions(p: &str) -> u64 {
    fn seq(s: &[char], i: &mut usize, stop_at_comma: bool) -> u64 {
        let mut n: u64 = 1;
        while *i < s.len() {
            match s[*i] {
                '\\' => *i += 2,
                ',' | '}' if stop_at_comma => return n,
                '{' => {
                    let save = *i;
                    *i += 1;
                    let mut alts: Vec<u64> = vec![seq(s, i, true)];
                    while *i < s.len() && s[*i] == ',' {
                        *i += 1;
                        alts.push(seq(s, i, true));
                    }
                    if *i < s.len() && s[*i] == '}' && alts.len() > 1 {
                        *i += 1;
                        n = n.saturating_mul(alts.iter().fold(0u64, |a, b| a.saturating_add(*b)));
                    } else if *i < s.len() && s[*i] == '}' {
                        *i += 1;
                        n = n.saturating_mul(alts[0]);
                    } else {
                        *i = save + 1;
                    }
                }
                _ => *i += 1,
            }
        }
        n
    }
    let chars: Vec<char> = p.chars().collect();
    let mut i = 0;
    seq(&chars, &mut i, false)
}

/// A `[` that cannot start a bracket expression (`photos [2024/**`).
fn bad_bracket(p: &str) -> bool {
    let c: Vec<char> = p.chars().collect();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '\\' => i += 2,
            '[' => {
                let mut j = i + 1;
                if j < c.len() && (c[j] == '!' || c[j] == '^') {
                    j += 1;
                }
                if j < c.len() && c[j] == ']' {
                    j += 1;
                }
                while j < c.len() && c[j] != ']' && c[j] != '/' {
                    j += 1;
                }
                if j >= c.len() || c[j] != ']' {
                    return true;
                }
                i = j + 1;
            }
            _ => i += 1,
        }
    }
    false
}

/// Warnings only (Claude Code keeps going with a bad pattern): one short line each.
pub fn glob_warnings(paths: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let (mut total, mut bytes) = (0u64, 0u64);
    for p in paths {
        let n = expansions(p);
        if n > 1 {
            total = total.saturating_add(n);
            bytes = bytes.saturating_add(n.saturating_mul(p.len() as u64));
        }
        if bad_bracket(p) {
            out.push(crate::tr_fmt!(crate::i18n::Fail::GlobBracket, a0 = display_safe(p, 80)));
        }
        let drive = p.len() >= 2 && p.as_bytes()[0].is_ascii_alphabetic() && p.as_bytes()[1] == b':';
        if p.starts_with('~') || p.split('/').any(|s| s == "..") || drive {
            out.push(crate::tr_fmt!(crate::i18n::Fail::GlobOutside, a0 = display_safe(p, 80)));
        }
    }
    if total > EXPANSION_LIMIT || bytes > EXPANSION_BYTES {
        out.push(crate::i18n::Fail::BraceMax.str().to_string());
    }
    out
}

// ---------- exclusion patterns ----------

fn seg_match(p: &[char], s: &[char]) -> bool {
    let (mut pi, mut si) = (0, 0);
    let (mut star, mut mark) = (None::<usize>, 0);
    while si < s.len() {
        if pi < p.len() {
            match p[pi] {
                '*' => {
                    star = Some(pi);
                    mark = si;
                    pi += 1;
                    continue;
                }
                '?' => {
                    pi += 1;
                    si += 1;
                    continue;
                }
                '[' => {
                    if let Some((ok, next)) = class(p, pi, s[si]) {
                        if ok {
                            pi = next;
                            si += 1;
                            continue;
                        }
                    } else if s[si] == '[' {
                        pi += 1;
                        si += 1;
                        continue;
                    }
                }
                '\\' if pi + 1 < p.len() => {
                    if p[pi + 1] == s[si] {
                        pi += 2;
                        si += 1;
                        continue;
                    }
                }
                c if c == s[si] => {
                    pi += 1;
                    si += 1;
                    continue;
                }
                _ => {}
            }
        }
        match star {
            Some(st) => {
                pi = st + 1;
                mark += 1;
                si = mark;
            }
            None => return false,
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

/// `[...]` at `p[i]`: (matches `c`, index after `]`), `None` when not a bracket expression.
fn class(p: &[char], i: usize, c: char) -> Option<(bool, usize)> {
    let mut j = i + 1;
    let neg = j < p.len() && (p[j] == '!' || p[j] == '^');
    if neg {
        j += 1;
    }
    let start = j;
    let mut hit = false;
    while j < p.len() && (p[j] != ']' || j == start) {
        if j + 2 < p.len() && p[j + 1] == '-' && p[j + 2] != ']' {
            if p[j] <= c && c <= p[j + 2] {
                hit = true;
            }
            j += 3;
        } else {
            if p[j] == c {
                hit = true;
            }
            j += 1;
        }
    }
    (j < p.len()).then_some((hit != neg, j + 1))
}

fn segs_match(p: &[&str], s: &[&str]) -> bool {
    match p.first() {
        None => s.is_empty(),
        Some(&"**") => (0..=s.len()).any(|k| segs_match(&p[1..], &s[k..])),
        Some(first) => {
            !s.is_empty() && seg_match(&first.chars().collect::<Vec<_>>(), &s[0].chars().collect::<Vec<_>>()) && segs_match(&p[1..], &s[1..])
        }
    }
}

/// Brace alternatives of `p` (at most 64; beyond that the pattern is used as written).
fn brace_expand(p: &str) -> Vec<String> {
    let c: Vec<char> = p.chars().collect();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '\\' => i += 2,
            '{' => {
                let (mut depth, mut j, mut cuts) = (0usize, i, vec![i]);
                while j < c.len() {
                    match c[j] {
                        '\\' => j += 1,
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        ',' if depth == 1 => cuts.push(j),
                        _ => {}
                    }
                    j += 1;
                }
                if j >= c.len() || cuts.len() < 2 {
                    i += 1;
                    continue;
                }
                cuts.push(j);
                let head: String = c[..i].iter().collect();
                let tail: String = c[j + 1..].iter().collect();
                let mut out = Vec::new();
                for w in cuts.windows(2) {
                    let alt: String = c[w[0] + 1..w[1]].iter().collect();
                    for e in brace_expand(&format!("{head}{alt}{tail}")) {
                        out.push(e);
                        if out.len() >= 64 {
                            return vec![p.to_string()];
                        }
                    }
                }
                return out;
            }
            _ => i += 1,
        }
    }
    vec![p.to_string()]
}

/// Glob match of an absolute path (`**` any number of segments, `*`, `?`, `[..]`, `{a,b}`, `\` escapes).
pub fn glob_match(pattern: &str, path: &str) -> bool {
    let s: Vec<&str> = path.split('/').collect();
    brace_expand(pattern).iter().any(|p| {
        let ps: Vec<&str> = p.split('/').collect();
        segs_match(&ps, &s)
    })
}

fn has_glob_chars(p: &str) -> bool {
    p.contains(['*', '?', '[', ']', '{', '}', '\\', '!'])
}

struct Settings {
    exists: bool,
    token: String,
    text: String,
    excludes: Vec<String>,
    readonly: Option<&'static str>,
}

fn read_settings(path: &Path) -> Settings {
    let empty = |readonly| Settings { exists: false, token: String::new(), text: String::new(), excludes: Vec::new(), readonly };
    let m = match std::fs::symlink_metadata(path) {
        Err(_) => return empty(None),
        Ok(m) => m,
    };
    if !m.file_type().is_file() || ids(&m).2 != 1 {
        return Settings { exists: true, ..empty(Some(crate::i18n::Fail::SettingsLinked.str())) };
    }
    if m.len() > MAX_FILE_BYTES {
        return Settings { exists: true, ..empty(Some(crate::i18n::Fail::SettingsTooBig.str())) };
    }
    let Ok(bytes) = std::fs::read(path) else { return Settings { exists: true, ..empty(Some(crate::i18n::Fail::SettingsUnreadable.str())) } };
    let token = token_of(&bytes);
    let Ok(text) = String::from_utf8(bytes) else { return Settings { exists: true, token, ..empty(Some(crate::i18n::Fail::SettingsUnparsable.str())) } };
    let v: Option<serde_json::Value> = serde_json::from_str(&text).ok();
    let (excludes, readonly) = match v.as_ref().map(|v| (v.is_object(), v.get(KEY))) {
        None | Some((false, _)) => (Vec::new(), Some(crate::i18n::Fail::SettingsUnparsable.str())),
        Some((true, None)) => (Vec::new(), None),
        Some((true, Some(serde_json::Value::Array(a)))) if a.iter().all(serde_json::Value::is_string) => {
            (a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(), None)
        }
        Some((true, Some(_))) => (Vec::new(), Some(crate::i18n::Fail::ExcludesFormat.str())),
    };
    Settings { exists: true, token, text, excludes, readonly }
}

// ---------- listing ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OffBy {
    /// The app's exact entry in the scope's settings file.
    App,
    /// Another `claudeMdExcludes` pattern: read-only here.
    Other,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleRow {
    pub id: String,
    pub scope: Scope,
    /// Path under `rules/`, e.g. `frontend/react.md`.
    pub rel: String,
    pub name: String,
    pub folder: Option<String>,
    pub paths: Vec<String>,
    pub fm: FmState,
    pub enabled: bool,
    pub off_by: Option<OffBy>,
    pub git_tracked: Option<bool>,
    pub link: bool,
    /// A linked folder: listed, not followed.
    pub dir_link: bool,
    /// A link whose target is outside the project: needs Claude Code's approval, path-scoped rules never load.
    pub link_outside: bool,
    /// Where a linked rule points (canonical), shown read-only.
    pub link_target: Option<String>,
    pub size: Option<u64>,
    pub token: String,
    pub readonly_reason: Option<&'static str>,
    pub has_secrets: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleGroup {
    pub scope: Scope,
    /// Folder shown to the user.
    pub dir: String,
    pub exists: bool,
    pub rules: Vec<RuleRow>,
    pub truncated: bool,
    pub settings_path: String,
    pub settings_token: String,
    pub settings_readonly: Option<&'static str>,
    /// Rules that load every session: count and total size.
    pub always_count: usize,
    pub always_bytes: u64,
    pub readonly_reason: Option<&'static str>,
}

fn rule_id(p: &Path) -> String {
    let h = Sha256::digest(p.as_os_str().as_encoded_bytes());
    format!("r-{}", h.iter().take(10).map(|b| format!("{b:02x}")).collect::<String>())
}

struct Found {
    rel: String,
    real: PathBuf,
    meta: std::fs::Metadata,
}

fn walk(dir: &Path, rel: &str, depth: usize, out: &mut Vec<Found>, truncated: &mut bool) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut names: Vec<String> = rd.filter_map(Result::ok).filter_map(|e| e.file_name().to_str().map(str::to_string)).collect();
    names.sort();
    for n in names {
        if n.starts_with('.') || n.chars().any(is_unsafe_char) {
            continue;
        }
        if out.len() >= MAX_FILES {
            *truncated = true;
            return;
        }
        let p = dir.join(&n);
        let Ok(m) = std::fs::symlink_metadata(&p) else { continue };
        let r = if rel.is_empty() { n.clone() } else { format!("{rel}/{n}") };
        if m.file_type().is_dir() {
            if depth < MAX_DEPTH {
                walk(&p, &r, depth + 1, out, truncated);
            } else {
                *truncated = true;
            }
        } else if m.file_type().is_symlink() {
            let to_dir = std::fs::metadata(&p).is_ok_and(|t| t.is_dir());
            if to_dir || n.ends_with(".md") {
                out.push(Found { rel: r, real: p, meta: m });
            }
        } else if m.file_type().is_file() && n.ends_with(".md") {
            out.push(Found { rel: r, real: p, meta: m });
        }
    }
}

/// `own`: the strings the app itself writes for this rule (the path Claude Code walks and its canonical form).
fn excluded_by(layers: &[(PathBuf, Settings)], target: &Path, own: &[String], candidates: &[String]) -> Option<OffBy> {
    let mut other = false;
    let mut app = false;
    for (path, s) in layers {
        for pat in &s.excludes {
            if path == target && own.contains(pat) {
                app = true;
            } else if candidates.iter().any(|c| glob_match(pat, c)) {
                other = true;
            }
        }
    }
    if other {
        Some(OffBy::Other)
    } else if app {
        Some(OffBy::App)
    } else {
        None
    }
}

/// The entry the app writes (first) and the canonical spelling it also accepts as its own.
fn own_entries(b: &Base, rel: &str) -> Vec<String> {
    let mut v = vec![b.shown_rules.join(rel).display().to_string()];
    if let Some(real) = &b.real_rules {
        let c = real.join(rel).display().to_string();
        if !v.contains(&c) {
            v.push(c);
        }
    }
    v
}

fn row(b: &Base, layers: &[(PathBuf, Settings)], f: &Found) -> RuleRow {
    let shown = b.shown_rules.join(&f.rel);
    let shown_s = shown.display().to_string();
    let link = f.meta.file_type().is_symlink();
    let target = if link { f.real.canonicalize().ok() } else { None };
    let dir_link = link && target.as_ref().is_some_and(|t| t.is_dir());
    let mut candidates = vec![shown_s.clone(), f.real.display().to_string()];
    if let Some(t) = &target {
        candidates.push(t.display().to_string());
    }
    let own = own_entries(b, &f.rel);
    let off_by = if dir_link {
        let globbed: Vec<String> = candidates.iter().map(|c| format!("{c}/x.md")).collect();
        excluded_by(layers, &b.target, &own, &globbed)
    } else {
        excluded_by(layers, &b.target, &own, &candidates)
    };
    let link_outside = link && b.project_real.as_ref().is_some_and(|p| !target.as_ref().is_some_and(|t| t.starts_with(p)));
    let (folder, file) = match f.rel.rsplit_once('/') {
        Some((d, n)) => (Some(display_safe(d, 200)), n),
        None => (None, f.rel.as_str()),
    };
    let mut r = RuleRow {
        id: rule_id(&f.real),
        scope: b.scope,
        rel: display_safe(&f.rel, 300),
        name: display_safe(file.trim_end_matches(".md"), 100),
        folder,
        paths: Vec::new(),
        fm: FmState::None,
        enabled: off_by.is_none(),
        off_by,
        git_tracked: None,
        link,
        dir_link,
        link_outside,
        link_target: target.as_ref().map(|t| display_safe(&t.display().to_string(), 300)),
        size: None,
        token: String::new(),
        readonly_reason: None,
        has_secrets: false,
        warnings: Vec::new(),
    };
    if link {
        r.readonly_reason = Some(crate::i18n::Fail::SharedLink.str());
        // The target is what Claude Code loads: its scope and size are shown, its text only in `read`.
        if let Some(text) = target.as_deref().filter(|_| !dir_link).and_then(link_text) {
            let fm = parse_fm(&text);
            r.size = Some(text.len() as u64);
            r.token = token_of(text.as_bytes());
            r.fm = fm.state;
            r.paths = if fm.state == FmState::Broken { Vec::new() } else { fm.paths.iter().map(|p| display_safe(p, MAX_PATTERN_CHARS)).collect() };
            r.warnings = glob_warnings(&r.paths);
            r.has_secrets = !secrets(&text).is_empty();
        }
        return r;
    }
    let (_, _, nlink) = ids(&f.meta);
    r.size = Some(f.meta.len());
    if b.scope == Scope::Project {
        r.git_tracked = git_tracked(&f.real);
    }
    if nlink != 1 {
        r.readonly_reason = Some(crate::i18n::Fail::HardlinkReadOnly.str());
        return r;
    }
    if f.meta.len() > MAX_FILE_BYTES {
        r.readonly_reason = Some(crate::i18n::Fail::Over256Unread.str());
        return r;
    }
    let Ok(bytes) = std::fs::read(&f.real) else { return r };
    r.token = token_of(&bytes);
    let Ok(text) = String::from_utf8(bytes) else {
        r.readonly_reason = Some(crate::i18n::Fail::NotUtf8ReadOnly.str());
        return r;
    };
    let fm = parse_fm(&text);
    r.fm = fm.state;
    r.paths = fm.paths.iter().map(|p| display_safe(p, MAX_PATTERN_CHARS)).collect();
    if fm.state == FmState::Broken {
        r.paths.clear();
    }
    r.warnings = glob_warnings(&r.paths);
    r.has_secrets = !secrets(&text).is_empty();
    r
}

/// Text of a linked rule's target: a regular file, at most 256KB, UTF-8. Read-only.
fn link_text(target: &Path) -> Option<String> {
    let m = std::fs::metadata(target).ok()?;
    if !m.is_file() || m.len() > MAX_FILE_BYTES {
        return None;
    }
    String::from_utf8(std::fs::read(target).ok()?).ok()
}

fn layers_of(b: &Base) -> Vec<(PathBuf, Settings)> {
    let mut v: Vec<(PathBuf, Settings)> = Vec::new();
    for l in &b.layers {
        if !v.iter().any(|(p, _)| p == l) {
            v.push((l.clone(), read_settings(l)));
        }
    }
    v
}

fn always(r: &RuleRow) -> bool {
    r.enabled && !r.dir_link && (r.fm == FmState::Broken || (r.paths.is_empty() && r.fm != FmState::Complex))
}

fn found_all(b: &Base) -> (Vec<Found>, bool) {
    let mut found = Vec::new();
    let mut truncated = false;
    if b.refused.is_some() {
        return (found, truncated);
    }
    if let Some(dir) = &b.real_rules {
        walk(dir, "", 1, &mut found, &mut truncated);
    }
    (found, truncated)
}

pub fn list_group(roots: &Roots, scope: Scope) -> Option<RuleGroup> {
    let b = base(roots, scope)?;
    let layers = if b.refused.is_some() { Vec::new() } else { layers_of(&b) };
    let (found, truncated) = found_all(&b);
    let rules: Vec<RuleRow> = found.iter().map(|f| row(&b, &layers, f)).collect();
    let target = layers.iter().find(|(p, _)| *p == b.target).map(|(_, s)| s);
    let exists_not_dir = |p: &Path| std::fs::symlink_metadata(p).is_ok() && !is_real_dir(p);
    let readonly_reason = match (&b.real_parent, &b.project_real) {
        _ if b.refused.is_some() => b.refused,
        (None, None) => Some(crate::i18n::Fail::ConfigDirNotFound.str()),
        (None, Some(p)) if exists_not_dir(&p.join(".claude")) => Some(crate::i18n::Fail::DotClaudeLinked.str()),
        (Some(parent), _) if exists_not_dir(&parent.join("rules")) => Some(crate::i18n::Fail::RulesDirLinked.str()),
        _ => None,
    };
    Some(RuleGroup {
        scope,
        dir: display_safe(&b.shown_rules.display().to_string(), 300),
        exists: b.real_rules.is_some() && b.refused.is_none(),
        always_count: rules.iter().filter(|r| always(r)).count(),
        always_bytes: rules.iter().filter(|r| always(r)).filter_map(|r| r.size).sum(),
        rules,
        truncated,
        settings_path: display_safe(&b.target.display().to_string(), 300),
        settings_token: target.map(|s| s.token.clone()).unwrap_or_default(),
        settings_readonly: target.and_then(|s| s.readonly),
        readonly_reason,
    })
}

/// Number of rules in `<project>/.claude/rules` (for listing projects that have rules but no memory).
/// 0 for the home folder and for folders that are links.
pub fn project_rule_count(project: &Path, home: Option<&Path>) -> usize {
    let roots = Roots { config_dir: PathBuf::new(), project: Some(project.to_path_buf()), home: home.map(Path::to_path_buf) };
    base(&roots, Scope::Project).map_or(0, |b| found_all(&b).0.len())
}

struct Resolved {
    b: Base,
    found: Found,
    row: RuleRow,
}

fn resolve(roots: &Roots, id: &str) -> Result<Resolved, RuleError> {
    if !id.starts_with("r-") || id.len() > 40 {
        return Err(RuleError::NotFound);
    }
    for scope in [Scope::Global, Scope::Project] {
        let Some(b) = base(roots, scope) else { continue };
        let (found, _) = found_all(&b);
        if let Some(f) = found.into_iter().find(|f| rule_id(&f.real) == id) {
            let layers = layers_of(&b);
            let row = row(&b, &layers, &f);
            return Ok(Resolved { b, found: f, row });
        }
    }
    Err(RuleError::NotFound)
}

// ---------- reading one ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleDoc {
    #[serde(flatten)]
    pub row: RuleRow,
    /// Text after the frontmatter. Shown in a textarea, never logged.
    pub body: String,
    pub paths_editable: bool,
    pub secrets: Vec<SecretHit>,
    pub instruction_like: Vec<usize>,
}

pub fn read(roots: &Roots, id: &str) -> Result<RuleDoc, RuleError> {
    let r = resolve(roots, id)?;
    if r.row.link && !r.row.dir_link {
        // Read-only view of the target. Nothing here can be saved.
        let text = std::fs::canonicalize(&r.found.real).ok().as_deref().and_then(link_text).unwrap_or_default();
        let fm = parse_fm(&text);
        return Ok(RuleDoc {
            body: text[fm.body_start.min(text.len())..].to_string(),
            paths_editable: false,
            secrets: secrets(&text),
            instruction_like: instruction_like(&text),
            row: r.row,
        });
    }
    if r.row.link || r.row.readonly_reason.is_some() || r.row.token.is_empty() {
        return Ok(RuleDoc { body: String::new(), paths_editable: false, secrets: Vec::new(), instruction_like: Vec::new(), row: r.row });
    }
    let bytes = std::fs::read(&r.found.real).map_err(|_| RuleError::Io)?;
    if token_of(&bytes) != r.row.token {
        return Err(RuleError::Conflict);
    }
    let text = String::from_utf8(bytes).map_err(|_| RuleError::Io)?;
    let fm = parse_fm(&text);
    Ok(RuleDoc {
        body: text[fm.body_start..].to_string(),
        paths_editable: matches!(fm.state, FmState::None | FmState::Ok),
        secrets: secrets(&text),
        instruction_like: instruction_like(&text),
        row: r.row,
    })
}

// ---------- writes ----------

fn check_body(body: &str) -> Result<(), RuleError> {
    if body.len() as u64 > MAX_FILE_BYTES || body.contains('\0') {
        return Err(RuleError::Invalid(crate::i18n::Fail::Text256Only.str()));
    }
    Ok(())
}

/// Saves the body and, when `paths` is given, the `paths` frontmatter of an existing rule.
pub fn save(roots: &Roots, id: &str, token: &str, body: &str, paths: Option<&[String]>) -> Result<(), RuleError> {
    check_body(body)?;
    let r = resolve(roots, id)?;
    if let Some(reason) = r.row.readonly_reason {
        return Err(RuleError::Unsafe(reason));
    }
    if r.row.token != token {
        return Err(RuleError::Conflict);
    }
    let old = std::fs::read_to_string(&r.found.real).map_err(|_| RuleError::Io)?;
    if token_of(old.as_bytes()) != token {
        return Err(RuleError::Conflict);
    }
    let head = match paths {
        Some(p) => with_paths(&old, p)?,
        None => old.clone(),
    };
    let content = with_body(&head, body);
    let plan = agents_catalog::plan_save_path(r.found.real.clone(), content, token)?;
    agents_catalog::commit_save(&plan)?;
    Ok(())
}

pub fn valid_new_name(n: &str) -> bool {
    NEW_NAME_RE.is_match(n)
}

fn make_dir(p: &Path) -> Result<(), RuleError> {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.file_type().is_dir() => Ok(()),
        Ok(_) => Err(RuleError::Unsafe(crate::i18n::Fail::RulesDirBad.str())),
        Err(_) => {
            let mut b = std::fs::DirBuilder::new();
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o755);
            b.create(p).map_err(|_| RuleError::Io)?;
            if is_real_dir(p) { Ok(()) } else { Err(RuleError::Io) }
        }
    }
}

/// Creates a new rule (`name` like `api.md` or `frontend/react.md`); never overwrites. Returns its id.
pub fn create(roots: &Roots, scope: Scope, name: &str, body: &str, paths: &[String]) -> Result<String, RuleError> {
    if !valid_new_name(name) {
        return Err(RuleError::Invalid(crate::i18n::Fail::RuleNameFormat.str()));
    }
    check_body(body)?;
    // A body that itself starts with `---` would be read as frontmatter: refuse instead of guessing.
    if parse_fm(body).state != FmState::None {
        return Err(RuleError::Invalid(crate::i18n::Fail::RuleBodyDashes.str()));
    }
    let content = with_paths(body, paths)?;
    let b = base(roots, scope).ok_or(RuleError::Unsafe(crate::i18n::Fail::ProjectDirNotFound.str()))?;
    if let Some(reason) = b.refused {
        return Err(RuleError::Unsafe(reason));
    }
    if scope == Scope::Project {
        let p = b.project_real.as_ref().ok_or(RuleError::Io)?;
        make_dir(&p.join(".claude"))?;
    }
    let parent = match scope {
        Scope::Global => b.real_parent.clone().ok_or(RuleError::Unsafe(crate::i18n::Fail::ConfigDirNotFound.str()))?,
        Scope::Project => b.project_real.as_ref().ok_or(RuleError::Io)?.join(".claude"),
    };
    let rules = parent.join("rules");
    make_dir(&rules)?;
    let mut dir = rules.clone();
    if let Some((sub, _)) = name.split_once('/') {
        dir = rules.join(sub);
        make_dir(&dir)?;
    }
    let file_name = name.rsplit('/').next().ok_or(RuleError::Io)?;
    let path = dir.join(file_name);
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o644);
    let mut f = opts.open(&path).map_err(|e| if e.kind() == std::io::ErrorKind::AlreadyExists { RuleError::Exists } else { RuleError::Io })?;
    if f.write_all(content.as_bytes()).and_then(|()| f.sync_all()).is_err() {
        drop(f);
        let _ = std::fs::remove_file(&path);
        return Err(RuleError::Io);
    }
    Ok(rule_id(&path))
}

pub struct TogglePlan {
    path: PathBuf,
    exists: bool,
    token: String,
    content: String,
    new_mode: u32,
    pub note: Option<&'static str>,
    pub changed: bool,
}

/// Plans turning a rule off (its absolute path added to `claudeMdExcludes`) or on (that exact entry removed).
/// Other members and other patterns stay byte for byte.
pub fn plan_enabled(roots: &Roots, id: &str, enabled: bool, settings_token: &str) -> Result<TogglePlan, RuleError> {
    let r = resolve(roots, id)?;
    if r.row.dir_link {
        return Err(RuleError::Unsafe(crate::i18n::Fail::LinkedDirToggle.str()));
    }
    if r.row.off_by == Some(OffBy::Other) {
        return Err(RuleError::Unsafe(crate::i18n::Fail::OffElsewhere.str()));
    }
    let owns = own_entries(&r.b, &r.found.rel);
    let own = owns[0].clone();
    if has_glob_chars(&own) || own.chars().any(is_unsafe_char) {
        return Err(RuleError::Unsafe(crate::i18n::Fail::PathSpecialChars.str()));
    }
    let dir = r.b.target.parent().ok_or(RuleError::Io)?;
    if !is_real_dir(dir) {
        return Err(RuleError::Unsafe(crate::i18n::Fail::ConfigDirLinked.str()));
    }
    let s = read_settings(&r.b.target);
    if let Some(reason) = s.readonly {
        return Err(RuleError::Unsafe(reason));
    }
    if s.token != settings_token {
        return Err(RuleError::Conflict);
    }
    let mut list = s.excludes.clone();
    let has = list.iter().any(|p| owns.contains(p));
    if enabled == !has {
        return Ok(TogglePlan { path: r.b.target, exists: s.exists, token: s.token, content: s.text, new_mode: 0o600, note: None, changed: false });
    }
    if enabled {
        list.retain(|p| !owns.contains(p));
    } else {
        list.push(own);
    }
    let content = excludes_text(&s.text, &list)?;
    let check: Option<serde_json::Value> = serde_json::from_str(&content).ok();
    let written: Option<Vec<String>> = match check.as_ref().map(|v| v.get(KEY)) {
        Some(None) if list.is_empty() => Some(Vec::new()),
        Some(Some(a)) => serde_json::from_value(a.clone()).ok(),
        _ => None,
    };
    if written.as_ref() != Some(&list) {
        return Err(RuleError::Invalid(crate::i18n::Fail::SaveJsonInvalid.str()));
    }
    Ok(TogglePlan { path: r.b.target, exists: s.exists, token: s.token, content, new_mode: 0o600, note: None, changed: true })
}

/// The settings text with `claudeMdExcludes` set to `list`, every other byte kept.
/// - An inline array (`["a", "b"]` on one line) stays inline; otherwise one entry per line.
/// - An empty list in a file that holds nothing else (what the app creates) leaves `{}`, not `{"claudeMdExcludes": []}`.
///   With other keys present the empty `[]` stays: removing a member byte-exactly needs the permissions scanner's
///   member spans, and `[]` means the same as no key.
fn excludes_text(text: &str, list: &[String]) -> Result<String, RuleError> {
    let bad = |_| RuleError::Unsafe(crate::i18n::Fail::SettingsShape.str());
    let eol = eol_of(text);
    let parsed: Option<serde_json::Value> = serde_json::from_str(text).ok();
    let only_key = parsed.as_ref().and_then(|v| v.as_object()).is_some_and(|o| o.len() == 1 && o.contains_key(KEY));
    if list.is_empty() && only_key {
        return Ok(format!("{{}}{eol}"));
    }
    // The current value's text: splice a marker in and read what it replaced.
    const MARK: &str = "\"\u{1}dduck-mark\u{1}\"";
    let marked = crate::permissions::splice_top_level(text, KEY, MARK).map_err(bad)?;
    let inline = match (marked.find(MARK), parsed.as_ref().and_then(|v| v.get(KEY))) {
        (Some(at), Some(_)) => {
            let tail = marked.len() - at - MARK.len();
            text.get(at..text.len() - tail).is_some_and(|old| !old.contains('\n'))
        }
        _ => false,
    };
    let items: Vec<String> = list.iter().map(|p| serde_json::to_string(p).unwrap_or_default()).collect();
    let value = if items.is_empty() {
        "[]".to_string()
    } else if inline {
        format!("[{}]", items.join(", "))
    } else {
        format!("[{eol}    {}{eol}  ]", items.join(&format!(",{eol}    ")))
    };
    crate::permissions::splice_top_level(text, KEY, &value).map_err(bad)
}

pub fn commit_enabled(plan: &TogglePlan) -> Result<(), RuleError> {
    if !plan.changed {
        return Ok(());
    }
    if plan.exists {
        let p = agents_catalog::plan_save_path(plan.path.clone(), plan.content.clone(), &plan.token)?;
        agents_catalog::commit_save(&p)?;
        return Ok(());
    }
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, plan.new_mode);
    let mut f = opts.open(&plan.path).map_err(|_| RuleError::Conflict)?;
    f.write_all(plan.content.as_bytes()).and_then(|()| f.sync_all()).map_err(|_| RuleError::Io)
}

pub fn set_enabled(roots: &Roots, id: &str, enabled: bool, settings_token: &str) -> Result<(), RuleError> {
    commit_enabled(&plan_enabled(roots, id, enabled, settings_token)?)
}

/// What the delete dialog showed; re-checked right before the file goes to the Trash.
#[derive(Debug, Clone)]
pub struct RuleTrash {
    pub path: PathBuf,
    pub rel: String,
    dev: u64,
    ino: u64,
    token: String,
}

pub fn plan_trash(roots: &Roots, id: &str, token: &str) -> Result<RuleTrash, RuleError> {
    let r = resolve(roots, id)?;
    if let Some(reason) = r.row.readonly_reason {
        return Err(RuleError::Unsafe(reason));
    }
    if r.row.token != token {
        return Err(RuleError::Conflict);
    }
    let (dev, ino, _) = ids(&r.found.meta);
    Ok(RuleTrash { path: r.found.real, rel: r.row.rel, dev, ino, token: token.to_string() })
}

/// Same file as when the dialog opened: regular, one link, same device/inode/content, parent a real folder.
pub fn recheck_trash(plan: &RuleTrash) -> Result<(), RuleError> {
    if !plan.path.parent().is_some_and(is_real_dir) {
        return Err(RuleError::Conflict);
    }
    let m = std::fs::symlink_metadata(&plan.path).map_err(|_| RuleError::Conflict)?;
    let (dev, ino, nlink) = ids(&m);
    if !m.file_type().is_file() || nlink != 1 || dev != plan.dev || ino != plan.ino {
        return Err(RuleError::Conflict);
    }
    let bytes = std::fs::read(&plan.path).map_err(|_| RuleError::Conflict)?;
    if token_of(&bytes) != plan.token {
        return Err(RuleError::Conflict);
    }
    Ok(())
}

// ---------- templates ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleTemplate {
    pub id: String,
    pub label: String,
    pub name: String,
    pub paths: Vec<String>,
    pub body: String,
}

pub fn templates() -> Vec<RuleTemplate> {
    serde_json::from_str::<serde_json::Value>(TEMPLATES)
        .ok()
        .map(|v| crate::i18n::pick_lang(v, crate::i18n::current()))
        .and_then(|v| v.get("rules").cloned())
        .and_then(|r| serde_json::from_value::<Vec<RuleTemplate>>(r).ok())
        .unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn reads_list_and_comma_forms() {
        let t = "---\npaths:\n  - \"src/api/**/*.ts\"\n  - 'lib/**'\n  - tests/*.rs\n---\n# body\n";
        let f = parse_fm(t);
        assert_eq!(f.state, FmState::Ok);
        assert_eq!(f.paths, s(&["src/api/**/*.ts", "lib/**", "tests/*.rs"]));
        assert_eq!(&t[f.body_start..], "# body\n");
        let c = parse_fm("---\npaths: \"src/**/*.{ts,tsx}, lib/*.ts\"\n---\nx");
        assert_eq!(c.paths, s(&["src/**/*.{ts,tsx}", "lib/*.ts"]));
        assert_eq!(parse_fm("---\npaths: src/*.ts,docs/**\n---\n").paths, s(&["src/*.ts", "docs/**"]));
        assert_eq!(parse_fm("# no fm\n").state, FmState::None);
        assert_eq!(parse_fm("---\npaths:\n---\n").paths, Vec::<String>::new());
    }

    #[test]
    fn complex_and_broken_forms() {
        assert_eq!(parse_fm("---\npaths: [\"a\", \"b\"]\n---\n").state, FmState::Complex);
        assert_eq!(parse_fm("---\npaths: |\n  a\n---\n").state, FmState::Complex);
        assert_eq!(parse_fm("---\npaths:\n  - a\npaths:\n  - b\n---\n").state, FmState::Complex);
        assert_eq!(parse_fm("---\npaths:\n  - a\n   - b\n---\n").state, FmState::Complex);
        assert_eq!(parse_fm("---\npaths:\n  - \"a\n---\n").state, FmState::Broken);
        assert_eq!(parse_fm("---\npaths:\n  - *.md\n---\n").state, FmState::Broken);
        assert_eq!(parse_fm("---\npaths:\n  - a\n").state, FmState::Broken);
        assert_eq!(parse_fm("---\n\tpaths: a\n---\n").state, FmState::Broken);
    }

    #[test]
    fn writes_paths_keeping_every_other_byte() {
        let t = "---\nowner: team-a # keep\npaths:\n  - old/**\ndescription: x\n---\n\n# Body\n  spaced  \n";
        let out = with_paths(t, &s(&["src/api/**/*.ts", "a\"b\\c"])).unwrap();
        assert_eq!(out, "---\nowner: team-a # keep\npaths:\n  - \"src/api/**/*.ts\"\n  - \"a\\\"b\\\\c\"\ndescription: x\n---\n\n# Body\n  spaced  \n");
        assert_eq!(parse_fm(&out).paths, s(&["src/api/**/*.ts", "a\"b\\c"]));
        let cleared = with_paths(&out, &[]).unwrap();
        assert_eq!(cleared, "---\nowner: team-a # keep\ndescription: x\n---\n\n# Body\n  spaced  \n");
        // Only paths in the frontmatter: removing it removes the frontmatter.
        let only = with_paths("---\npaths:\n  - a\n---\nbody\n", &[]).unwrap();
        assert_eq!(only, "body\n");
        // No frontmatter yet; BOM and CRLF kept.
        let crlf = with_paths("\u{FEFF}# T\r\nline\r\n", &s(&["*.md"])).unwrap();
        assert_eq!(crlf, "\u{FEFF}---\r\npaths:\r\n  - \"*.md\"\r\n---\r\n# T\r\nline\r\n");
        assert_eq!(with_paths(&crlf, &[]).unwrap(), "\u{FEFF}# T\r\nline\r\n");
        // Inserted before the closing marker when another key exists.
        assert_eq!(with_paths("---\nx: 1\n---\nb", &s(&["a"])).unwrap(), "---\nx: 1\npaths:\n  - \"a\"\n---\nb");
    }

    #[test]
    fn refuses_unsafe_patterns_and_complex_frontmatter() {
        for bad in ["a\nb", "a\u{2028}b", "a\u{0085}b", "a\u{202E}b", " lead", ""] {
            assert!(with_paths("x", &s(&[bad])).is_err(), "{bad:?}");
        }
        assert!(matches!(with_paths("---\npaths: [a]\n---\n", &s(&["b"])), Err(RuleError::Unsafe(_))));
        assert!(matches!(with_paths("---\npaths:\n  - \"a\n---\n", &[]), Err(RuleError::Unsafe(_))));
    }

    #[test]
    fn glob_warnings_follow_the_docs() {
        assert!(glob_warnings(&s(&["/*.md", "src/**/*.{ts,tsx}"])).is_empty(), "leading / is fine");
        assert_eq!(glob_warnings(&s(&["photos [2024/**"])).len(), 1);
        assert!(glob_warnings(&s(&["photos \\[2024/**", "[abc].md"])).is_empty());
        assert_eq!(glob_warnings(&s(&["~/x", "../y", "C:/z"])).len(), 3);
        assert_eq!(expansions("{a,b}/{c,d}/*.{ts,tsx}"), 8);
        assert_eq!(expansions("{a}"), 1);
        let big = "{0,1,2,3,4,5,6,7,8,9}{0,1,2,3,4,5,6,7,8,9}{0,1,2,3,4,5,6,7,8,9}{a,b}";
        assert!(glob_warnings(&s(&[big])).iter().any(|w| w.contains("1,000")));
    }

    #[test]
    fn exclusion_globs() {
        assert!(glob_match("/home/u/p/.claude/rules/**", "/home/u/p/.claude/rules/a/b.md"));
        assert!(glob_match("**/monorepo/CLAUDE.md", "/home/u/monorepo/CLAUDE.md"));
        assert!(glob_match("/p/.claude/rules/*.{md,txt}", "/p/.claude/rules/x.md"));
        assert!(!glob_match("/p/.claude/rules/*.md", "/p/.claude/rules/sub/x.md"));
        assert!(glob_match("/p/.claude/rules/t[ae]st.md", "/p/.claude/rules/test.md"));
        assert!(glob_match("/p/x.md", "/p/x.md"));
    }

    #[test]
    fn templates_load_and_are_short() {
        let t = templates();
        assert_eq!(t.len(), 3);
        for x in &t {
            assert!(valid_new_name(&format!("{}.md", x.name)));
            assert!(x.body.lines().count() <= 5, "{}", x.id);
            check_patterns(&x.paths).unwrap();
            let content = if x.paths.is_empty() { x.body.clone() } else { with_paths(&x.body, &x.paths).unwrap() };
            assert_eq!(parse_fm(&content).paths, x.paths);
        }
        assert!(t.iter().any(|x| x.paths == s(&["src/api/**/*.ts"])));
        // Skill/command/workflow templates still load as strings.
        assert!(!crate::authoring::template(crate::authoring::Kind::Skill).is_empty());
    }

    #[test]
    fn names_for_new_files() {
        for ok in ["api.md", "front_end/react-hooks.md", "A1.md"] {
            assert!(valid_new_name(ok), "{ok}");
        }
        for bad in ["../x.md", "a/b/c.md", ".hidden.md", "x.txt", "a b.md", "/abs.md", "가.md", "a\\b.md", "x.MD"] {
            assert!(!valid_new_name(bad), "{bad}");
        }
    }

    struct Env {
        _tmp: tempfile::TempDir,
        roots: Roots,
        project: PathBuf,
        config: PathBuf,
    }

    fn env() -> Env {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().canonicalize().unwrap();
        let config = base.join("cfg");
        let project = base.join("proj");
        std::fs::create_dir_all(config.join("rules")).unwrap();
        std::fs::create_dir_all(project.join(".claude").join("rules").join("frontend")).unwrap();
        Env { roots: Roots { config_dir: config.clone(), project: Some(project.clone()), home: Some(base.clone()) }, project, config, _tmp: tmp }
    }

    fn find<'a>(g: &'a RuleGroup, rel: &str) -> &'a RuleRow {
        g.rules.iter().find(|r| r.rel == rel).unwrap()
    }

    #[test]
    fn lists_recursively_with_scope_chips() {
        let e = env();
        let rules = e.project.join(".claude/rules");
        std::fs::write(rules.join("testing.md"), "# Testing\n").unwrap();
        std::fs::write(rules.join("frontend/react.md"), "---\npaths:\n  - \"src/**/*.tsx\"\n---\n# R\n").unwrap();
        std::fs::write(rules.join("broken.md"), "---\npaths:\n  - \"x\n---\n").unwrap();
        std::fs::write(rules.join("notes.txt"), "no").unwrap();
        std::fs::write(rules.join(".hidden.md"), "no").unwrap();
        std::fs::write(e.config.join("rules/global.md"), "g").unwrap();
        let g = list_group(&e.roots, Scope::Project).unwrap();
        assert_eq!(g.rules.len(), 3);
        let r = find(&g, "frontend/react.md");
        assert_eq!((r.name.as_str(), r.folder.as_deref(), r.paths.clone()), ("react", Some("frontend"), s(&["src/**/*.tsx"])));
        assert_eq!(find(&g, "broken.md").fm, FmState::Broken);
        assert!(find(&g, "testing.md").enabled);
        assert_eq!(g.always_count, 2);
        let gl = list_group(&e.roots, Scope::Global).unwrap();
        assert_eq!(gl.rules.len(), 1);
        // No project: global still lists, project group absent.
        let only = Roots { config_dir: e.config.clone(), project: None, home: e.roots.home.clone() };
        assert!(list_group(&only, Scope::Project).is_none());
        assert_eq!(list_group(&only, Scope::Global).unwrap().rules.len(), 1);
    }

    #[test]
    fn list_caps_files() {
        let e = env();
        for i in 0..(MAX_FILES + 5) {
            std::fs::write(e.config.join(format!("rules/r{i:04}.md")), "x").unwrap();
        }
        let g = list_group(&e.roots, Scope::Global).unwrap();
        assert_eq!(g.rules.len(), MAX_FILES);
        assert!(g.truncated);
    }

    #[test]
    fn create_edit_and_conflict() {
        let e = env();
        let id = create(&e.roots, Scope::Project, "api.md", "# API 규칙\n", &s(&["src/api/**/*.ts"])).unwrap();
        let path = e.project.join(".claude/rules/api.md");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "---\npaths:\n  - \"src/api/**/*.ts\"\n---\n# API 규칙\n");
        assert_eq!(create(&e.roots, Scope::Project, "api.md", "x", &[]), Err(RuleError::Exists));
        assert!(create(&e.roots, Scope::Project, "../x.md", "x", &[]).is_err());
        assert!(create(&e.roots, Scope::Project, "a.md", "---\nx: 1\n---\n", &[]).is_err());
        let sub = create(&e.roots, Scope::Global, "team/style.md", "s\n", &[]).unwrap();
        assert!(e.config.join("rules/team/style.md").is_file());
        let doc = read(&e.roots, &id).unwrap();
        assert_eq!(doc.body, "# API 규칙\n");
        assert!(doc.paths_editable);
        save(&e.roots, &id, &doc.row.token, "# API 규칙\n- 검증\n", Some(&s(&["src/api/**/*.ts", "lib/**"]))).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "---\npaths:\n  - \"src/api/**/*.ts\"\n  - \"lib/**\"\n---\n# API 규칙\n- 검증\n");
        // Stale token → conflict, file unchanged.
        assert_eq!(save(&e.roots, &id, &doc.row.token, "y", None), Err(RuleError::Conflict));
        let d2 = read(&e.roots, &sub).unwrap();
        save(&e.roots, &sub, &d2.row.token, "t\n", Some(&[])).unwrap();
        assert_eq!(std::fs::read_to_string(e.config.join("rules/team/style.md")).unwrap(), "t\n");
        assert_eq!(read(&e.roots, "r-nope").unwrap_err(), RuleError::NotFound);
    }

    #[test]
    fn complex_frontmatter_allows_body_only() {
        let e = env();
        let p = e.project.join(".claude/rules/c.md");
        std::fs::write(&p, "---\npaths: [\"a\"]\nx: 1\n---\nold\n").unwrap();
        let g = list_group(&e.roots, Scope::Project).unwrap();
        let r = find(&g, "c.md");
        assert_eq!(r.fm, FmState::Complex);
        assert!(save(&e.roots, &r.id, &r.token, "new\n", Some(&s(&["b"]))).is_err());
        save(&e.roots, &r.id, &r.token, "new\n", None).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "---\npaths: [\"a\"]\nx: 1\n---\nnew\n");
    }

    #[cfg(unix)]
    #[test]
    fn links_are_read_only_but_can_be_turned_off() {
        let e = env();
        let shared = e._tmp.path().canonicalize().unwrap().join("shared.md");
        std::fs::write(&shared, "---\npaths:\n  - a\n---\nsecret body\n").unwrap();
        let rules = e.project.join(".claude/rules");
        std::os::unix::fs::symlink(&shared, rules.join("security.md")).unwrap();
        let outside_dir = e._tmp.path().canonicalize().unwrap().join("shared-dir");
        std::fs::create_dir(&outside_dir).unwrap();
        std::fs::write(outside_dir.join("in.md"), "x").unwrap();
        std::os::unix::fs::symlink(&outside_dir, rules.join("shared")).unwrap();
        std::fs::write(rules.join("hard.md"), "h").unwrap();
        std::fs::hard_link(rules.join("hard.md"), e._tmp.path().join("hard2.md")).unwrap();
        let g = list_group(&e.roots, Scope::Project).unwrap();
        let l = find(&g, "security.md");
        assert!(l.link && l.link_outside && l.readonly_reason.is_some());
        assert_eq!(l.paths, s(&["a"]), "the target's scope is shown");
        assert_eq!(l.link_target.as_deref(), Some(shared.display().to_string().as_str()));
        assert!(find(&g, "shared").dir_link);
        assert!(!g.rules.iter().any(|r| r.rel == "shared/in.md"), "linked folders are not followed");
        assert!(find(&g, "hard.md").readonly_reason.is_some());
        let doc = read(&e.roots, &l.id).unwrap();
        assert_eq!(doc.body, "secret body\n", "read-only view of the target");
        assert!(!doc.paths_editable);
        assert!(save(&e.roots, &l.id, "", "x", None).is_err());
        assert!(plan_trash(&e.roots, &l.id, "").is_err());
        assert!(save(&e.roots, &find(&g, "hard.md").id, &find(&g, "hard.md").token, "x", None).is_err());
        set_enabled(&e.roots, &l.id, false, "").unwrap();
        let g = list_group(&e.roots, Scope::Project).unwrap();
        assert!(!find(&g, "security.md").enabled);
        assert_eq!(std::fs::read_to_string(&shared).unwrap(), "---\npaths:\n  - a\n---\nsecret body\n");
    }

    #[cfg(unix)]
    #[test]
    fn refuses_linked_rules_folder() {
        let e = env();
        let other = e._tmp.path().join("elsewhere");
        std::fs::create_dir(&other).unwrap();
        let claude = e.project.join(".claude");
        std::fs::remove_dir_all(claude.join("rules")).unwrap();
        std::os::unix::fs::symlink(&other, claude.join("rules")).unwrap();
        let g = list_group(&e.roots, Scope::Project).unwrap();
        assert!(!g.exists && g.readonly_reason.is_some());
        assert!(create(&e.roots, Scope::Project, "x.md", "x", &[]).is_err());
        assert!(!other.join("x.md").exists());
    }

    #[test]
    fn turning_off_and_on_edits_only_the_app_entry() {
        let e = env();
        let rules = e.project.join(".claude/rules");
        std::fs::write(rules.join("testing.md"), "# T\n").unwrap();
        std::fs::write(rules.join("frontend/a.md"), "# A\n").unwrap();
        let local = e.project.join(".claude/settings.local.json");
        let original = "{\n  \"permissions\": { \"allow\": [\"Bash(ls)\"] },\n  \"claudeMdExcludes\": [\"**/frontend/**\"]\n}\n";
        std::fs::write(&local, original).unwrap();
        let g = list_group(&e.roots, Scope::Project).unwrap();
        assert_eq!(find(&g, "frontend/a.md").off_by, Some(OffBy::Other));
        let t = find(&g, "testing.md");
        assert!(t.enabled);
        // Stale settings token.
        assert_eq!(set_enabled(&e.roots, &t.id, false, "stale"), Err(RuleError::Conflict));
        set_enabled(&e.roots, &t.id, false, &g.settings_token).unwrap();
        let abs = rules.join("testing.md").display().to_string();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&local).unwrap()).unwrap();
        assert_eq!(v["claudeMdExcludes"], serde_json::json!(["**/frontend/**", abs]));
        assert_eq!(v["permissions"]["allow"][0], "Bash(ls)");
        assert_eq!(std::fs::read_to_string(rules.join("testing.md")).unwrap(), "# T\n", "file untouched");
        let g = list_group(&e.roots, Scope::Project).unwrap();
        let t = find(&g, "testing.md");
        assert_eq!((t.enabled, t.off_by), (false, Some(OffBy::App)));
        assert!(set_enabled(&e.roots, &find(&g, "frontend/a.md").id, true, &g.settings_token).is_err(), "other pattern: read-only");
        set_enabled(&e.roots, &t.id, true, &g.settings_token).unwrap();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&local).unwrap()).unwrap();
        assert_eq!(v["claudeMdExcludes"], serde_json::json!(["**/frontend/**"]));
        let after = std::fs::read_to_string(&local).unwrap();
        assert!(after.starts_with("{\n  \"permissions\": { \"allow\": [\"Bash(ls)\"] },\n  \"claudeMdExcludes\": "), "other bytes kept");
    }

    #[test]
    fn global_rule_goes_to_user_settings_and_creates_file() {
        let e = env();
        std::fs::write(e.config.join("rules/style.md"), "s").unwrap();
        let g = list_group(&e.roots, Scope::Global).unwrap();
        assert_eq!(g.settings_token, "");
        set_enabled(&e.roots, &find(&g, "style.md").id, false, "").unwrap();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(e.config.join("settings.json")).unwrap()).unwrap();
        assert_eq!(v["claudeMdExcludes"][0], e.config.join("rules/style.md").display().to_string());
        assert!(!e.project.join(".claude/settings.local.json").exists());
        assert!(!list_group(&e.roots, Scope::Global).unwrap().rules[0].enabled);
    }

    #[test]
    fn bad_settings_are_not_rewritten() {
        let e = env();
        std::fs::write(e.config.join("rules/x.md"), "x").unwrap();
        std::fs::write(e.config.join("settings.json"), "{\"claudeMdExcludes\": \"oops\"}").unwrap();
        let g = list_group(&e.roots, Scope::Global).unwrap();
        assert!(g.settings_readonly.is_some());
        assert!(matches!(set_enabled(&e.roots, &g.rules[0].id, false, &g.settings_token), Err(RuleError::Unsafe(_))));
        std::fs::write(e.config.join("settings.json"), "{\"claudeMdExcludes\": [], \"claudeMdExcludes\": []}").unwrap();
        let g = list_group(&e.roots, Scope::Global).unwrap();
        assert!(set_enabled(&e.roots, &g.rules[0].id, false, &g.settings_token).is_err());
    }

    #[test]
    fn trash_plan_rechecks() {
        let e = env();
        let p = e.config.join("rules/x.md");
        std::fs::write(&p, "x").unwrap();
        let g = list_group(&e.roots, Scope::Global).unwrap();
        let plan = plan_trash(&e.roots, &g.rules[0].id, &g.rules[0].token).unwrap();
        recheck_trash(&plan).unwrap();
        std::fs::write(&p, "changed").unwrap();
        assert_eq!(recheck_trash(&plan), Err(RuleError::Conflict));
    }

    #[test]
    fn home_folder_has_no_project_scope() {
        let e = env();
        std::fs::write(e.config.join("rules/g.md"), "g").unwrap();
        std::fs::write(e.project.join(".claude/rules/p.md"), "p").unwrap();
        // The project is the home folder (reached through a link, so the paths differ but dev+ino match).
        let base = e._tmp.path().canonicalize().unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&e.project, base.join("home-link")).unwrap();
        for home in [Some(e.project.clone()), cfg!(unix).then(|| base.join("home-link"))].into_iter().flatten() {
            let roots = Roots { home: Some(home), ..e.roots.clone() };
            let g = list_group(&roots, Scope::Project).unwrap();
            assert_eq!((g.rules.len(), g.readonly_reason, g.exists), (0, Some(HOME_REASON.str()), false));
            assert_eq!(create(&roots, Scope::Project, "x.md", "x", &[]), Err(RuleError::Unsafe(HOME_REASON.str())));
            assert!(!e.project.join(".claude/rules/x.md").exists());
            let gl = list_group(&roots, Scope::Global).unwrap();
            assert_eq!(gl.rules.len(), 1, "global still lists");
            assert_eq!(project_rule_count(&e.project, roots.home.as_deref()), 0);
        }
        // Home unknown: refused too.
        for home in [None, Some(PathBuf::new()), Some(base.join("missing"))] {
            let roots = Roots { home, ..e.roots.clone() };
            let g = list_group(&roots, Scope::Project).unwrap();
            assert!(g.readonly_reason.is_some() && g.rules.is_empty());
            assert!(create(&roots, Scope::Project, "y.md", "y", &[]).is_err());
        }
        // A real project next to home is fine.
        assert_eq!(list_group(&e.roots, Scope::Project).unwrap().rules.len(), 1);
        assert_eq!(project_rule_count(&e.project, e.roots.home.as_deref()), 1);
    }

    #[test]
    fn turning_back_on_cleans_up_excludes() {
        let e = env();
        let rules = e.project.join(".claude/rules");
        std::fs::write(rules.join("t.md"), "t").unwrap();
        let local = e.project.join(".claude/settings.local.json");
        // A file the app created: back to `{}` after on.
        let g = list_group(&e.roots, Scope::Project).unwrap();
        set_enabled(&e.roots, &find(&g, "t.md").id, false, "").unwrap();
        assert!(std::fs::read_to_string(&local).unwrap().contains("claudeMdExcludes"));
        let g = list_group(&e.roots, Scope::Project).unwrap();
        set_enabled(&e.roots, &find(&g, "t.md").id, true, &g.settings_token).unwrap();
        assert_eq!(std::fs::read_to_string(&local).unwrap(), "{}\n");
        // Inline arrays stay inline, other keys keep their bytes.
        let original = "{ \"model\": \"x\", \"claudeMdExcludes\": [\"**/old/**\"] }\n";
        std::fs::write(&local, original).unwrap();
        let g = list_group(&e.roots, Scope::Project).unwrap();
        set_enabled(&e.roots, &find(&g, "t.md").id, false, &g.settings_token).unwrap();
        let abs = rules.join("t.md").display().to_string();
        assert_eq!(std::fs::read_to_string(&local).unwrap(), format!("{{ \"model\": \"x\", \"claudeMdExcludes\": [\"**/old/**\", {}] }}\n", serde_json::to_string(&abs).unwrap()));
        let g = list_group(&e.roots, Scope::Project).unwrap();
        set_enabled(&e.roots, &find(&g, "t.md").id, true, &g.settings_token).unwrap();
        assert_eq!(std::fs::read_to_string(&local).unwrap(), original);
        // Other keys present and the list empties: `[]` stays (same meaning as no key).
        std::fs::write(&local, "{\"model\": \"x\"}").unwrap();
        let g = list_group(&e.roots, Scope::Project).unwrap();
        set_enabled(&e.roots, &find(&g, "t.md").id, false, &g.settings_token).unwrap();
        let g = list_group(&e.roots, Scope::Project).unwrap();
        set_enabled(&e.roots, &find(&g, "t.md").id, true, &g.settings_token).unwrap();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&local).unwrap()).unwrap();
        assert_eq!(v, serde_json::json!({"model": "x", "claudeMdExcludes": []}));
    }

    #[cfg(unix)]
    #[test]
    fn excludes_match_the_canonical_path() {
        let e = env();
        let base = e._tmp.path().canonicalize().unwrap();
        std::os::unix::fs::symlink(&e.project, base.join("plink")).unwrap();
        std::fs::write(e.project.join(".claude/rules/t.md"), "t").unwrap();
        let canonical = e.project.join(".claude/rules/t.md").display().to_string();
        let local = e.project.join(".claude/settings.local.json");
        std::fs::write(&local, format!("{{\"claudeMdExcludes\": [{}]}}", serde_json::to_string(&canonical).unwrap())).unwrap();
        let roots = Roots { project: Some(base.join("plink")), ..e.roots.clone() };
        let g = list_group(&roots, Scope::Project).unwrap();
        let t = find(&g, "t.md");
        assert_eq!(t.off_by, Some(OffBy::App), "the canonical spelling counts as the app's own entry");
        set_enabled(&roots, &t.id, true, &g.settings_token).unwrap();
        assert!(list_group(&roots, Scope::Project).unwrap().rules[0].enabled);
        assert_eq!(std::fs::read_to_string(&local).unwrap(), "{}\n");
    }

    #[test]
    fn conflict_message_is_one_short_line() {
        let e = env();
        std::fs::write(e.config.join("rules/x.md"), "x").unwrap();
        let g = list_group(&e.roots, Scope::Global).unwrap();
        let err = save(&e.roots, &g.rules[0].id, "stale", "y", None).unwrap_err();
        assert_eq!(err, RuleError::Conflict);
        assert_eq!(err.to_string(), "파일이 바뀌었어요");
    }

    #[test]
    fn errors_never_carry_rule_text() {
        let e = env();
        let p = e.config.join("rules/x.md");
        std::fs::write(&p, "---\npaths: [\"SECRETPATTERN\"]\n---\nSECRETBODY\n").unwrap();
        let g = list_group(&e.roots, Scope::Global).unwrap();
        let err = save(&e.roots, &g.rules[0].id, &g.rules[0].token, "SECRETBODY2", Some(&s(&["SECRETPAT2"]))).unwrap_err().to_string();
        assert!(!err.contains("SECRET"), "{err}");
        let src = include_str!("rules.rs");
        let logging = ["log", "::", "info!"].concat();
        assert!(!src.contains(&logging) && !src.contains(&["eprint", "ln!"].concat()));
    }
}
