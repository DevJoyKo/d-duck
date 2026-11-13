//! 첨부 보고서 (report-evidence, D-156): free-form HTML or Markdown documents an agent leaves in
//! `<report dir>/attachments/` and names in report.json. Read with the evidence rules (no link, one hard link, regular
//! file, real path re-checked), refused over the size limit (never cut), strict UTF-8 only. HTML is cleaned here
//! (ammonia allowlist, one CSS filter for `<style>` and `style`, links turned into text + host, images only as
//! checked `data:` URIs) and wrapped in a CSP meta prefix: the webview puts the string into a `sandbox=""` srcdoc as is.

use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::evidence::{self, Status, Sub};

pub const DIR: &str = "attachments";
pub const MAX_ATTACHMENTS: usize = 10;
pub const MAX_TITLE: usize = 80;
pub const MAX_HTML: u64 = 512 * 1024;
pub const MAX_MARKDOWN: u64 = 64 * 1024;
pub const MAX_ENTRIES: usize = 50;
/// Images per attachment, and their budget counted per reference.
pub const MAX_IMAGES: usize = 20;
pub const MAX_IMAGE_PIXELS: u64 = 80_000_000;
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
/// The card's first line reads only this much of the file.
pub const FIRST_LINE_BYTES: usize = 16 * 1024;
pub const FIRST_LINE_CHARS: usize = 120;
const MAX_CSS: usize = 64 * 1024;
const CACHE_ENTRIES: usize = 64;

/// The preview CSP (same as the artifact preview, `artifactCards.ts`): no script, no network, no fonts.
pub const PREVIEW_CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AttachmentKind {
    Html,
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub file: String,
    #[serde(default)]
    pub title: String,
    pub kind: AttachmentKind,
}

impl Attachment {
    /// The file name rule of evidence, and the extension matching the kind.
    pub fn valid(&self) -> bool {
        let ext = self.file.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        let kind_ok = match self.kind {
            AttachmentKind::Html => ext == "html",
            AttachmentKind::Markdown => ext == "md",
        };
        evidence::valid_name(&self.file) && kind_ok && self.title.chars().count() <= MAX_TITLE && !self.title.contains('\0')
    }

    pub fn clean(&mut self) {
        self.title = crate::text::display_safe(&self.title, MAX_TITLE);
    }

    fn max(&self) -> u64 {
        match self.kind {
            AttachmentKind::Html => MAX_HTML,
            AttachmentKind::Markdown => MAX_MARKDOWN,
        }
    }
}

/// One attachment as the report screen lists it (a tab): no file name or path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentView {
    pub title: String,
    pub kind: AttachmentKind,
    pub status: Status,
    /// Valid, but nothing to show after cleaning ("빈 문서").
    pub empty: bool,
}

fn folder(report_dir: &Path, size: evidence::Measured) -> Sub {
    evidence::sub_dir(report_dir, DIR, MAX_ENTRIES, size)
}

/// Reads an attachment: the evidence open rules, the size limit refused as a whole, strict UTF-8 without NUL.
fn read_text(dir: &Sub, a: &Attachment) -> Result<(String, evidence::Sig), Status> {
    let dir = match dir {
        Sub::Ok(d) => d,
        Sub::Bad(s) => return Err(*s),
    };
    let (bytes, sig) = evidence::read_in(dir, &a.file, a.max())?;
    let text = String::from_utf8(bytes).map_err(|_| Status::BadType)?;
    if text.contains('\0') {
        return Err(Status::BadType);
    }
    Ok((text, sig))
}

/// Nothing visible after cleaning: no text and no image element (HTML), only white space (Markdown).
fn empty_text(kind: AttachmentKind, text: &str) -> bool {
    match kind {
        AttachmentKind::Markdown => text.trim().is_empty(),
        AttachmentKind::Html => {
            let srcs = Arc::new(Mutex::new(Vec::new()));
            let html = builder(srcs.clone()).clean(text).to_string();
            srcs.lock().map(|v| v.is_empty()).unwrap_or(true) && visible_text(&html).trim().is_empty()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Checked {
    status: Status,
    empty: bool,
}

type CheckKey = (String, String, evidence::Sig);
/// Attachment checks by (report id, file, signature): a poll of the 리뷰 탭 only stats the files (code review).
static CHECKS: LazyLock<Mutex<evidence::Lru<CheckKey, Checked>>> = LazyLock::new(|| Mutex::new(evidence::Lru::new(1024)));

fn check(report_id: &str, dir: &Sub, a: &Attachment) -> Checked {
    let stat = match dir {
        Sub::Ok(d) => std::fs::symlink_metadata(d.join(&a.file)).ok().filter(|m| m.file_type().is_file()).map(|m| evidence::sig(&m)),
        Sub::Bad(_) => None,
    };
    let key_of = |s: evidence::Sig| (report_id.to_string(), a.file.clone(), s);
    if let Some(hit) = stat.and_then(|s| CHECKS.lock().ok()?.get(&key_of(s))) {
        return hit;
    }
    match read_text(dir, a) {
        Err(status) => Checked { status, empty: false },
        Ok((text, sig)) => {
            let c = Checked { status: Status::Valid, empty: empty_text(a.kind, &text) };
            if let Ok(mut cache) = CHECKS.lock() {
                cache.put(key_of(sig), c);
            }
            c
        }
    }
}

/// The tab list: what each attachment is, checked like [`open`] and kept by signature.
pub fn views(report_id: &str, report_dir: &Path, list: &[Attachment], size: evidence::Measured) -> Vec<AttachmentView> {
    let dir = folder(report_dir, size);
    list.iter()
        .map(|a| {
            let c = check(report_id, &dir, a);
            AttachmentView { title: a.title.clone(), kind: a.kind, status: c.status, empty: c.empty }
        })
        .collect()
}

/// What the webview gets on open. Paths never.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Opened {
    /// The whole srcdoc, CSP prefix included. The webview does not change it.
    Html { srcdoc: String },
    /// Markdown text for SafeMarkdown, and each same-folder image line as (name, data); the webview matches them by
    /// name and occurrence, so a different line split cannot shift pictures (security LOW).
    Markdown { text: String, images: Vec<MdImage> },
    /// Nothing to show after cleaning ("보여 드릴 내용이 없어요").
    Empty,
    Invalid { code: Status },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MdImage {
    pub name: String,
    /// Checked `data:` URI, or `None` (left out).
    pub data: Option<String>,
}

pub fn open(report_dir: &Path, a: &Attachment) -> Opened {
    let sub = folder(report_dir, evidence::measure(report_dir));
    let text = match read_text(&sub, a) {
        Ok((t, _)) => t,
        Err(code) => return Opened::Invalid { code },
    };
    let dir = match sub {
        Sub::Ok(d) => Some(d),
        Sub::Bad(_) => None,
    };
    match a.kind {
        AttachmentKind::Html => {
            let c = clean_html(&text, dir.as_deref());
            if c.is_empty() { Opened::Empty } else { Opened::Html { srcdoc: srcdoc(&c) } }
        }
        AttachmentKind::Markdown => {
            if text.trim().is_empty() {
                return Opened::Empty;
            }
            let images = markdown_images(&text, dir.as_deref());
            Opened::Markdown { text, images }
        }
    }
}

/// The original bytes for "저장" (report-copy-save, D-174): read again now with the same rules as [`open`] (no link,
/// regular file, real path re-checked, size limit, strict UTF-8 without NUL). Not cleaned: the agent's own file.
pub fn read_original(report_dir: &Path, a: &Attachment) -> Result<Vec<u8>, Status> {
    let sub = folder(report_dir, evidence::measure(report_dir));
    read_text(&sub, a).map(|(t, _)| t.into_bytes())
}

/// Why a save failed. Fixed codes only: no io message or path reaches the webview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveError {
    /// The chosen place is not a file path inside an existing folder.
    BadPlace,
    Write,
}

/// Writes `bytes` to `dest` (a path the user chose in the native save panel) through a temporary file in the same
/// folder and one rename, so a failed write leaves neither a half file nor the temporary one. `quarantine` marks the
/// saved HTML like a download, so a browser opens it with the macOS warning (security LOW).
pub fn save_copy(dest: &Path, bytes: &[u8], quarantine: bool) -> Result<(), SaveError> {
    use std::io::Write;
    let parent = dest.parent().filter(|p| !p.as_os_str().is_empty()).ok_or(SaveError::BadPlace)?;
    if dest.file_name().is_none() || !parent.is_dir() || std::fs::symlink_metadata(dest).is_ok_and(|m| m.is_dir()) {
        return Err(SaveError::BadPlace);
    }
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|_| SaveError::Write)?;
    tmp.write_all(bytes).and_then(|_| tmp.as_file().sync_all()).map_err(|_| SaveError::Write)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o644)).map_err(|_| SaveError::Write)?;
    }
    if quarantine {
        mark_quarantine(tmp.path());
    }
    // On error the temporary file comes back inside the error and is removed when dropped.
    tmp.persist(dest).map(|_| ()).map_err(|_| SaveError::Write)
}

/// `com.apple.quarantine` like a downloaded file. Best effort: a failure does not stop the save.
#[cfg(target_os = "macos")]
fn mark_quarantine(path: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let value = format!("0081;{secs:x};D.Duck;");
    let (Ok(p), Ok(name)) = (std::ffi::CString::new(path.as_os_str().as_bytes()), std::ffi::CString::new("com.apple.quarantine")) else { return };
    // SAFETY: both strings are NUL-terminated and live across the call; the value pointer and length match.
    let _ = unsafe { libc::setxattr(p.as_ptr(), name.as_ptr(), value.as_ptr().cast(), value.len(), 0, libc::XATTR_NOFOLLOW) };
}

#[cfg(not(target_os = "macos"))]
fn mark_quarantine(_path: &Path) {}

// ---------- images ----------

#[derive(Default)]
struct Budget {
    count: usize,
    pixels: u64,
    bytes: usize,
}

impl Budget {
    /// Counts one more reference; `false` (left out) once a limit would be passed.
    fn take(&mut self, img: &evidence::Image, bytes: usize) -> bool {
        let px = u64::from(img.width) * u64::from(img.height);
        if self.count + 1 > MAX_IMAGES || self.pixels + px > MAX_IMAGE_PIXELS || self.bytes + bytes > MAX_IMAGE_BYTES {
            return false;
        }
        self.count += 1;
        self.pixels += px;
        self.bytes += bytes;
        true
    }
}

/// A checked `data:` URI for an `<img src>` or Markdown image: a same-folder name (no percent decoding) or a
/// `data:image/png|jpeg;base64,` URI, both structure-checked, within the budget.
fn image_src(src: &str, dir: Option<&Path>, budget: &mut Budget) -> Option<String> {
    use base64::Engine;
    let bytes = if let Some(rest) = src.strip_prefix("data:") {
        let lower = rest.get(..20).unwrap_or(rest).to_ascii_lowercase();
        let body = if lower.starts_with("image/png;base64,") {
            &rest[17..]
        } else if lower.starts_with("image/jpeg;base64,") {
            &rest[18..]
        } else {
            return None;
        };
        let compact: String = body.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        if compact.len() > MAX_IMAGE_BYTES * 4 / 3 + 8 {
            return None;
        }
        base64::engine::general_purpose::STANDARD.decode(compact).ok()?
    } else {
        if !evidence::valid_name(src) {
            return None;
        }
        evidence::read_in(dir?, src, MAX_IMAGE_BYTES as u64).ok()?.0
    };
    let img = evidence::image_info(&bytes)?;
    budget.take(&img, bytes.len()).then(|| format!("data:{};base64,{}", img.mime, base64::engine::general_purpose::STANDARD.encode(&bytes)))
}

static MD_IMAGE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A\s*!\[([^\]\n]{0,200})\]\(([A-Za-z0-9._-]{1,80})\)\s*\z").unwrap_or_else(|_| unreachable!()));
static FENCE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A\s*```").unwrap_or_else(|_| unreachable!()));

/// Image lines outside code fences, in order (the same rule as `safeMarkdown.ts`, after the same unsafe characters
/// are dropped: control characters but `\n`/`\t`, bidi, zero-width; a lone `\r` is not a line break there either).
fn markdown_images(text: &str, dir: Option<&Path>) -> Vec<MdImage> {
    let mut budget = Budget::default();
    let mut fenced = false;
    let mut out = Vec::new();
    let clean: String = text.chars().filter(|c| matches!(c, '\n' | '\t') || !crate::text::is_unsafe_char(*c)).collect();
    for line in clean.split('\n') {
        if FENCE_RE.is_match(line) {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(name) = MD_IMAGE_RE.captures(line).and_then(|c| c.get(2)) {
            out.push(MdImage { name: name.as_str().to_string(), data: image_src(name.as_str(), dir, &mut budget) });
        }
    }
    out
}

// ---------- CSS: one filter for <style> and style="" ----------

const CSS_BANNED: &[&str] = &[
    "url(", "image-set", "image(", "cross-fade", "element(", "@import", "@font-face", "@namespace", "expression", "behavior", "-moz-binding", "src:", "javascript:", "attr(",
];

/// Comments removed and escapes decoded (`\XX…` hex, `\<newline>`, `\c`), so a banned word cannot hide behind them.
/// Backslashes, `<` and control characters never survive.
fn css_normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '/' && cs.get(i + 1) == Some(&'*') {
            match (i + 2..cs.len().saturating_sub(1)).find(|&k| cs[k] == '*' && cs[k + 1] == '/') {
                Some(k) => i = k + 2,
                None => i = cs.len(),
            }
            continue;
        }
        if c == '\\' {
            let hex: String = cs[i + 1..].iter().take(6).take_while(|h| h.is_ascii_hexdigit()).collect();
            if !hex.is_empty() {
                i += 1 + hex.len();
                if cs.get(i).is_some_and(|w| w.is_whitespace()) {
                    i += 1;
                }
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    push_css_char(&mut out, ch);
                }
                continue;
            }
            match cs.get(i + 1) {
                Some('\n') | None => i += 2,
                Some(&ch) => {
                    push_css_char(&mut out, ch);
                    i += 2;
                }
            }
            continue;
        }
        push_css_char(&mut out, c);
        i += 1;
    }
    out
}

fn push_css_char(out: &mut String, c: char) {
    if c == '\\' || c == '<' || c == '\0' || (c.is_control() && !matches!(c, '\n' | '\t')) || crate::text::is_unsafe_char(c) && !matches!(c, '\n' | '\t') {
        return;
    }
    out.push(c);
}

fn banned(s: &str) -> bool {
    let flat: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
    CSS_BANNED.iter().any(|b| flat.contains(b))
}

/// Splits at `sep` outside strings and parentheses.
fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut depth = 0i32;
    for c in s.chars() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == q {
                    quote = None;
                }
                continue;
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '(' => depth += 1,
            None if c == ')' => depth -= 1,
            None if c == sep && depth <= 0 => {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
            None => {}
        }
        cur.push(c);
    }
    parts.push(cur);
    parts
}

static PROP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A-?[a-zA-Z][a-zA-Z0-9-]{0,60}\z").unwrap_or_else(|_| unreachable!()));

/// Declarations (`a: b; c: d`): each one kept only with a plain property name and nothing banned.
fn filter_decls(s: &str) -> String {
    let mut out = Vec::new();
    for d in split_top(s, ';') {
        let Some((p, v)) = d.split_once(':') else { continue };
        let (p, v) = (p.trim(), v.trim());
        if v.is_empty() || !PROP_RE.is_match(p) || banned(&d) || v.contains(['{', '}']) {
            continue;
        }
        out.push(format!("{}:{}", p.to_ascii_lowercase(), v));
    }
    out.join(";")
}

/// Rules of a stylesheet. Only `@media` and `@supports` at-rules survive (their contents filtered the same way);
/// every other at-rule, and any rule or declaration with a banned word, is removed as a whole.
fn filter_rules(s: &str, depth: usize) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        // Prelude up to `{` or `;` (outside strings).
        let start = i;
        let mut quote: Option<char> = None;
        while i < cs.len() {
            let c = cs[i];
            match quote {
                Some(q) if c == q => quote = None,
                Some(_) => {}
                None if c == '"' || c == '\'' => quote = Some(c),
                None if c == '{' || c == ';' || c == '}' => break,
                None => {}
            }
            i += 1;
        }
        let prelude: String = cs[start..i].iter().collect::<String>().trim().to_string();
        if i >= cs.len() {
            break;
        }
        if cs[i] == ';' || cs[i] == '}' {
            i += 1;
            continue;
        }
        // Block up to the matching `}`.
        let body_start = i + 1;
        let mut level = 0i32;
        let mut quote: Option<char> = None;
        while i < cs.len() {
            let c = cs[i];
            match quote {
                Some(q) if c == q => quote = None,
                Some(_) => {}
                None if c == '"' || c == '\'' => quote = Some(c),
                None if c == '{' => level += 1,
                None if c == '}' => {
                    level -= 1;
                    if level == 0 {
                        break;
                    }
                }
                None => {}
            }
            i += 1;
        }
        let body: String = cs[body_start.min(cs.len())..i.min(cs.len())].iter().collect();
        i += 1;
        if prelude.is_empty() || banned(&prelude) {
            continue;
        }
        let lower = prelude.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix('@') {
            let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
            if matches!(name.as_str(), "media" | "supports") && depth < 4 {
                let inner = filter_rules(&body, depth + 1);
                if !inner.is_empty() {
                    out.push_str(&format!("{prelude}{{{inner}}}"));
                }
            }
            continue;
        }
        if body.contains('{') {
            continue;
        }
        let decls = filter_decls(&body);
        if !decls.is_empty() {
            out.push_str(&format!("{prelude}{{{decls}}}"));
        }
    }
    out
}

/// The one CSS filter (design 11): a stylesheet (`<style>` body).
pub fn filter_css(css: &str) -> String {
    filter_rules(&css_normalize(css), 0)
}

/// The same filter for a `style` attribute (declarations only).
pub fn filter_style_attr(style: &str) -> String {
    filter_decls(&css_normalize(style))
}

/// Properties a `style` attribute may keep (layout, text, colour, border, spacing).
const STYLE_PROPS: &[&str] = &[
    "color", "background-color", "font-weight", "font-style", "font-size", "font-family", "text-align", "text-decoration", "line-height", "letter-spacing", "white-space",
    "margin", "margin-top", "margin-right", "margin-bottom", "margin-left", "padding", "padding-top", "padding-right", "padding-bottom", "padding-left",
    "border", "border-top", "border-right", "border-bottom", "border-left", "border-color", "border-width", "border-style", "border-radius", "border-collapse",
    "width", "max-width", "min-width", "height", "max-height", "min-height", "display", "vertical-align", "gap", "flex", "flex-direction", "flex-wrap",
    "align-items", "justify-content", "grid-template-columns", "list-style-type", "opacity",
];

// ---------- HTML ----------

/// The cleaned document: stylesheet and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cleaned {
    pub css: String,
    pub body: String,
}

impl Cleaned {
    /// No visible text and no image.
    pub fn is_empty(&self) -> bool {
        !self.body.contains("<img") && visible_text(&self.body).trim().is_empty()
    }

    /// The same document as one HTML string (for the idempotency check: cleaning it again gives the same).
    pub fn as_html(&self) -> String {
        if self.css.is_empty() { self.body.clone() } else { format!("<style>{}</style>{}", self.css, self.body) }
    }
}

static STYLE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<style(?:[\s/][^>]*)?>(.*?)</style[\s/>]").unwrap_or_else(|_| unreachable!()));
static A_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?s)<a\b([^>]*)>|</a>"#).unwrap_or_else(|_| unreachable!()));
static IMG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<img\b[^>]*>"#).unwrap_or_else(|_| unreachable!()));
static HREF_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\bhref="([^"]*)""#).unwrap_or_else(|_| unreachable!()));
static SRC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r##"\bsrc="#dduckimg-(\d{1,4})""##).unwrap_or_else(|_| unreachable!()));
static HOST_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A[A-Za-z0-9.:\[\]-]{1,253}\z").unwrap_or_else(|_| unreachable!()));

const HOST_MARK: &str = "#dduckhost-";
const IMG_MARK: &str = "#dduckimg-";

/// The host of a web link (`http`/`https`), display-safe; empty for anything else.
fn link_host(href: &str) -> String {
    match url::Url::parse(href) {
        Ok(u) if matches!(u.scheme(), "http" | "https") => u.host_str().filter(|h| HOST_RE.is_match(h)).map(str::to_string).unwrap_or_default(),
        _ => String::new(),
    }
}

fn fragment_only(u: &str) -> Option<std::borrow::Cow<'_, str>> {
    u.starts_with('#').then(|| u.into())
}

/// ammonia with an allowlist (no svg, script, iframe, object, embed, form, input, meta, base, link, event attributes),
/// relative URLs only as `#fragment`, `href` only `#fragment` (other links become text + " (host)" afterwards) and
/// `img src` resolved afterwards in document order.
fn builder(srcs: Arc<Mutex<Vec<String>>>) -> ammonia::Builder<'static> {
    use std::collections::HashSet;
    let mut b = ammonia::Builder::default();
    b.rm_tags(["area", "map"]);
    b.add_clean_content_tags(["svg", "math", "title", "template", "noscript", "iframe", "object", "embed", "textarea", "select", "head"]);
    b.add_generic_attributes(["style", "class", "id"]);
    b.filter_style_properties(STYLE_PROPS.iter().copied().collect::<HashSet<&'static str>>());
    b.url_schemes(["http", "https", "mailto", "data"].into_iter().collect());
    b.url_relative(ammonia::UrlRelative::Custom(Box::new(fragment_only)));
    b.link_rel(None);
    b.attribute_filter(move |element, attribute, value| match (element, attribute) {
        // Our markers are never accepted from the document itself.
        (_, _) if value.starts_with("#dduck") => None,
        ("a", "href") if value.starts_with('#') => Some(value.into()),
        ("a", "href") => Some(format!("{HOST_MARK}{}", link_host(value)).into()),
        ("img", "src") => {
            let mut list = srcs.lock().ok()?;
            list.push(value.to_string());
            Some(format!("{IMG_MARK}{}", list.len() - 1).into())
        }
        (_, "style") => Some(filter_style_attr(value).into()),
        _ => Some(value.into()),
    });
    b
}

/// Cleans an HTML attachment. `dir` = the `attachments/` folder for same-folder images.
pub fn clean_html(src: &str, dir: Option<&Path>) -> Cleaned {
    let mut css = String::new();
    for c in STYLE_RE.captures_iter(src) {
        if let Some(m) = c.get(1) {
            if css.len() + m.as_str().len() > MAX_CSS {
                break;
            }
            css.push_str(m.as_str());
            css.push('\n');
        }
    }
    let srcs = Arc::new(Mutex::new(Vec::new()));
    let html = builder(srcs.clone()).clean(src).to_string();
    let srcs = srcs.lock().map(|v| v.clone()).unwrap_or_default();
    // Links: fragments stay; anything else loses the element, keeps the text, then " (host)".
    let mut body = String::with_capacity(html.len());
    let mut at = 0;
    let mut suffix: Option<String> = None;
    let mut kept = false;
    for m in A_RE.captures_iter(&html) {
        let Some(whole) = m.get(0) else { continue };
        body.push_str(&html[at..whole.start()]);
        at = whole.end();
        if whole.as_str() == "</a>" {
            if kept {
                body.push_str("</a>");
            }
            if let Some(h) = suffix.take().filter(|h| !h.is_empty()) {
                body.push_str(&format!(" ({h})"));
            }
            kept = false;
            continue;
        }
        let attrs = m.get(1).map_or("", |a| a.as_str());
        let href = HREF_RE.captures(attrs).and_then(|c| c.get(1)).map(|h| h.as_str().to_string());
        match href {
            Some(h) if h.starts_with(HOST_MARK) => {
                kept = false;
                suffix = Some(h[HOST_MARK.len()..].to_string());
            }
            Some(h) if h.starts_with('#') => {
                kept = true;
                suffix = None;
                body.push_str(whole.as_str());
            }
            _ => {
                kept = false;
                suffix = None;
            }
        }
    }
    body.push_str(&html[at..]);
    // Images in document order: a checked data: URI within the budget, or the element is removed.
    let mut budget = Budget::default();
    let body = IMG_RE
        .replace_all(&body, |c: &regex::Captures| {
            let tag = &c[0];
            let Some(s) = SRC_RE.captures(tag) else { return String::new() };
            let Some(orig) = s.get(1).and_then(|n| n.as_str().parse::<usize>().ok()).and_then(|n| srcs.get(n)) else { return String::new() };
            match image_src(orig, dir, &mut budget) {
                Some(data) => tag.replacen(&s[0], &format!("src=\"{data}\""), 1),
                None => String::new(),
            }
        })
        .into_owned();
    Cleaned { css: filter_css(&css), body }
}

const BASE_CSS: &str = "html{color-scheme:light}body{margin:16px 20px;font:14px/1.6 -apple-system,BlinkMacSystemFont,system-ui,sans-serif;color:#1f2328;background:#fff;overflow-wrap:anywhere}\
table{border-collapse:collapse;margin:8px 0}th,td{border:1px solid #d0d7de;padding:4px 8px;text-align:left;vertical-align:top}th{background:#f6f8fa}\
img{max-width:100%;height:auto}pre{overflow:auto;background:#f6f8fa;padding:8px;border-radius:4px}code{font-family:ui-monospace,Menlo,monospace;font-size:12px}";

/// The whole srcdoc: CSP and DNS-prefetch metas first, a base style, the filtered style, the body.
pub fn srcdoc(c: &Cleaned) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"{PREVIEW_CSP}\"><meta http-equiv=\"x-dns-prefetch-control\" content=\"off\"><style>{BASE_CSS}</style><style>{}</style></head><body>{}</body></html>",
        c.css, c.body
    )
}

static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap_or_else(|_| unreachable!()));
static BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)</?(p|div|h[1-6]|li|ul|ol|tr|table|blockquote|pre|section|article|header|footer|figure|figcaption|details|summary|br|hr)\b[^>]*>").unwrap_or_else(|_| unreachable!()));

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", " ").replace("&amp;", "&")
}

/// Text of cleaned HTML, block elements as line breaks.
fn visible_text(html: &str) -> String {
    unescape(&TAG_RE.replace_all(&BLOCK_RE.replace_all(html, "\n"), ""))
}

// ---------- the review card's first line ----------

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// The first paragraph that is not the title, at most 120 characters.
fn first_paragraph<'a>(paras: impl Iterator<Item = &'a str>, title: &str) -> Option<String> {
    let t = norm(title);
    paras.map(str::trim).filter(|p| !p.is_empty()).find(|p| norm(p) != t).map(|p| crate::text::display_safe(&p.split_whitespace().collect::<Vec<_>>().join(" "), FIRST_LINE_CHARS))
}

/// (report id, file, title, signature): a changed title picks a different first paragraph.
type LineKey = (String, String, String, evidence::Sig);
static LINE_CACHE: LazyLock<Mutex<evidence::Lru<LineKey, Option<String>>>> = LazyLock::new(|| Mutex::new(evidence::Lru::new(CACHE_ENTRIES)));

/// The card line of an attachment: only the first 16 KB are read (cut at a character boundary), cleaned once and
/// kept by (report id, file name, signature).
pub fn first_line(report_id: &str, report_dir: &Path, a: &Attachment, size: evidence::Measured) -> Option<String> {
    use crate::artifact_preview::{fd_path, open_flags, single_link_regular};
    use std::io::Read;
    let Sub::Ok(dir) = folder(report_dir, size) else { return None };
    if !evidence::valid_name(&a.file) {
        return None;
    }
    let path = dir.join(&a.file);
    let file = open_flags(&path).ok()?;
    let meta = single_link_regular(&file)?;
    if fd_path(&file).map_or(cfg!(windows), |real| real != path) || meta.len() > a.max() {
        return None;
    }
    let key = (report_id.to_string(), a.file.clone(), a.title.clone(), evidence::sig(&meta));
    if let Some(hit) = LINE_CACHE.lock().ok().and_then(|mut c| c.get(&key)) {
        return hit;
    }
    let mut buf = Vec::new();
    file.take(FIRST_LINE_BYTES as u64).read_to_end(&mut buf).ok()?;
    let text = match std::str::from_utf8(&buf) {
        Ok(t) => t.to_string(),
        // Cut inside a character: keep the valid part.
        Err(e) if e.error_len().is_none() && buf.len() == FIRST_LINE_BYTES => String::from_utf8_lossy(&buf[..e.valid_up_to()]).into_owned(),
        Err(_) => return None,
    };
    let line = match a.kind {
        AttachmentKind::Html => {
            let c = clean_html(&text, None);
            let t = visible_text(&c.body);
            first_paragraph(t.split('\n'), &a.title)
        }
        AttachmentKind::Markdown => {
            let lines = text.lines().filter(|l| !l.trim_start().starts_with('|') && !l.trim_start().starts_with("![") && !l.trim_start().starts_with("```"));
            let stripped: Vec<String> = lines.map(|l| l.trim_start().trim_start_matches(['#', '>', '-', '*', '+']).trim().to_string()).collect();
            first_paragraph(stripped.iter().map(String::as_str), &a.title)
        }
    };
    if let Ok(mut c) = LINE_CACHE.lock() {
        c.put(key, line.clone());
    }
    line
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::evidence::tests::{jpeg_bytes, png_bytes};

    fn clean(s: &str) -> Cleaned {
        clean_html(s, None)
    }

    #[test]
    fn scripts_frames_forms_and_events_are_removed() {
        let c = clean(
            r##"<h1>비교</h1><script>alert(1)</script><img src=x onerror="alert(1)"><meta http-equiv="refresh" content="0;url=https://evil.example">
            <base href="https://evil.example/"><link rel=stylesheet href="https://evil.example/x.css"><iframe src="https://evil.example"></iframe>
            <form action="https://evil.example"><input name=a><button>보내기</button></form><object data=x></object><embed src=x>
            <svg><a xlink:href="javascript:alert(1)"><text>svg글</text></a><use href="#x"/></svg><p onclick="x()" style="color:red">본문</p>"##,
        );
        let out = c.body.to_lowercase();
        for bad in ["<script", "alert", "onerror", "onclick", "<meta", "<base", "<link", "<iframe", "<form", "<input", "<object", "<embed", "<svg", "svg글", "evil.example", "<img"] {
            assert!(!out.contains(bad), "{bad} in {out}");
        }
        assert!(c.body.contains("<h1>비교</h1>") && c.body.contains("본문") && c.body.contains("color:red"));
    }

    #[test]
    fn links_become_text_with_the_host() {
        let c = clean(r##"<p><a href="https://docs.example.com/x">공식 문서</a>, <a href="#s2">아래</a>, <a href="//evil.example/x">상대</a>, <a href="javascript:alert(1)">js</a>, <a href="mailto:a@b.c">메일</a>, <a href="#dduckhost-fake.example">가짜</a></p><h2 id="s2">2</h2>"##);
        assert!(c.body.contains("공식 문서 (docs.example.com)"), "{}", c.body);
        assert!(c.body.contains(r##"<a href="#s2">아래</a>"##), "{}", c.body);
        for bad in ["https://", "evil.example", "javascript", "mailto", "fake.example", "dduck"] {
            assert!(!c.body.contains(bad), "{bad} in {}", c.body);
        }
        assert!(c.body.contains("상대") && c.body.contains("js") && c.body.contains("메일") && c.body.contains("가짜"));
    }

    #[test]
    fn css_loses_every_remote_or_font_rule() {
        let css = r#"@import url(https://evil.example/a.css); @import "x.css";
            @font-face { font-family: x; src: url(https://evil.example/f.woff) }
            @namespace svg url(http://www.w3.org/2000/svg);
            .a { color: red; background: url(https://evil.example/b.png); }
            .b { background: u\72 l(https://evil.example/c.png); font-weight: bold }
            .c { background-image: image-set("x.png" 1x) } .d { behavior: url(x.htc); width: expression(alert(1)) }
            .e { background: ur/**/l(x) } @media (max-width: 600px) { .f { color: blue; background: url(x) } }
            .g:after { content: "a;b" } td < x { color: red }"#;
        let out = filter_css(css);
        let lower = out.to_lowercase();
        for bad in ["url(", "@import", "@font-face", "@namespace", "image-set", "behavior", "expression", "evil", "\\", "<"] {
            assert!(!lower.contains(bad), "{bad} in {out}");
        }
        assert!(out.contains(".a{color:red}") && out.contains(".b{font-weight:bold}") && out.contains("@media (max-width: 600px){.f{color:blue}}"), "{out}");
        assert!(out.contains(r#".g:after{content:"a;b"}"#), "{out}");
        assert_eq!(filter_css(&out), out, "idempotent");
        let c = clean(r#"<style>@import url(https://evil.example/x.css); .t { color: green }</style><p class="t" style="background: url(https://evil.example/y); color: blue">x</p>"#);
        assert!(c.css.contains(".t{color:green}") && !c.css.contains("evil"), "{}", c.css);
        assert!(!c.body.contains("evil") && c.body.contains("color:blue") && c.body.contains("class=\"t\""), "{}", c.body);
    }

    #[test]
    fn images_are_checked_and_budgeted() {
        use base64::Engine;
        let t = tempfile::tempdir().unwrap();
        let dir = crate::platform::canon(t.path()).unwrap();
        std::fs::write(dir.join("a.png"), png_bytes(100, 50)).unwrap();
        std::fs::write(dir.join("fake.png"), "<svg/>").unwrap();
        let ok = base64::engine::general_purpose::STANDARD.encode(jpeg_bytes(20, 10));
        let bad = base64::engine::general_purpose::STANDARD.encode(b"<script>");
        let c = clean_html(
            &format!(r##"<p>x</p><img src="a.png" alt="a"><img src="fake.png"><img src="data:image/jpeg;base64,{ok}"><img src="data:image/png;base64,{bad}"><img src="https://evil.example/r.png"><img src="../a.png"><img src="a%2epng"><img src="data:image/svg+xml;base64,PHN2Zz4="><img src="#dduckimg-0">"##),
            Some(&dir),
        );
        assert_eq!(c.body.matches("<img").count(), 2, "{}", c.body);
        assert!(c.body.contains("src=\"data:image/png;base64,") && c.body.contains("src=\"data:image/jpeg;base64,"));
        assert!(!c.body.contains("evil") && !c.body.contains("a.png\""));
        // Per reference: a 3 MB image used twice keeps only the first.
        let mut big = png_bytes(1000, 1000);
        let idat = vec![0u8; 3 * 1024 * 1024];
        let iend = big.split_off(big.len() - 12);
        big.extend_from_slice(&(idat.len() as u32).to_be_bytes());
        big.extend_from_slice(b"IDAT");
        big.extend_from_slice(&idat);
        big.extend_from_slice(&[0; 4]);
        big.extend_from_slice(&iend);
        std::fs::write(dir.join("big.png"), &big).unwrap();
        let c = clean_html(r#"<p>a</p><img src="big.png" alt="1"><img src="big.png" alt="2">"#, Some(&dir));
        assert_eq!(c.body.matches("<img").count(), 1);
        assert!(c.body.contains("alt=\"1\""));
        // Cleaning again gives the same document (images included).
        let once = clean_html(r##"<style>.t{color:red}</style><p class="t"><a href="https://x.example/a">x</a> <a href="#b">b</a></p><img src="a.png">"##, Some(&dir));
        assert_eq!(clean_html(&once.as_html(), Some(&dir)), once);
    }

    #[test]
    fn script_only_documents_are_empty() {
        assert!(clean("<script>document.write('hi')</script>").is_empty());
        assert!(clean("<style>p{color:red}</style>  ").is_empty());
        assert!(!clean("<p>글</p>").is_empty());
        let doc = srcdoc(&clean("<p>x</p>"));
        assert!(doc.starts_with(&format!("<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"{PREVIEW_CSP}\">")));
        assert!(!PREVIEW_CSP.contains("font-src") && !PREVIEW_CSP.contains("script-src"));
    }

    fn report_dir() -> (tempfile::TempDir, std::path::PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let d = crate::platform::canon(t.path()).unwrap().join("r");
        std::fs::create_dir_all(d.join(DIR)).unwrap();
        (t, d)
    }

    fn att(file: &str, kind: AttachmentKind) -> Attachment {
        Attachment { file: file.into(), title: "DB 후보 비교".into(), kind }
    }

    #[test]
    fn attachments_are_read_safely() {
        let (_t, d) = report_dir();
        std::fs::write(d.join("attachments/a.html"), "<h1>DB 후보 비교</h1><p>세 후보를 비용·운영으로 비교했어요</p><script>x</script>").unwrap();
        std::fs::write(d.join("attachments/bad.md"), b"ok \xff").unwrap();
        std::fs::write(d.join("attachments/big.html"), vec![b'a'; MAX_HTML as usize + 1]).unwrap();
        std::fs::write(d.join("attachments/s.html"), "<script>alert(1)</script>").unwrap();
        let big_md = format!("# 표\n\n{}", "가나다 라마바\n".repeat(50 * 1024 / 20));
        std::fs::write(d.join("attachments/long.md"), &big_md).unwrap();
        let Opened::Html { srcdoc } = open(&d, &att("a.html", AttachmentKind::Html)) else { panic!() };
        assert!(srcdoc.contains("세 후보") && !srcdoc.contains("<script"));
        assert_eq!(open(&d, &att("bad.md", AttachmentKind::Markdown)), Opened::Invalid { code: Status::BadType });
        assert_eq!(open(&d, &att("big.html", AttachmentKind::Html)), Opened::Invalid { code: Status::TooLarge });
        assert_eq!(open(&d, &att("s.html", AttachmentKind::Html)), Opened::Empty);
        assert_eq!(open(&d, &att("none.html", AttachmentKind::Html)), Opened::Invalid { code: Status::Missing });
        let Opened::Markdown { text, .. } = open(&d, &att("long.md", AttachmentKind::Markdown)) else { panic!() };
        assert_eq!(text, big_md, "50 KB Markdown is not cut");
        let size = evidence::measure(&d);
        assert_eq!(first_line("id1", &d, &att("a.html", AttachmentKind::Html), size).as_deref(), Some("세 후보를 비용·운영으로 비교했어요"));
        // The title is part of the cache key: a new title gives the paragraph that differs from it.
        let renamed = Attachment { title: "세 후보를 비용·운영으로 비교했어요".into(), ..att("a.html", AttachmentKind::Html) };
        assert_eq!(first_line("id1", &d, &renamed, size).as_deref(), Some("DB 후보 비교"));
        let v = views("id1", &d, &[att("a.html", AttachmentKind::Html), att("bad.md", AttachmentKind::Markdown), att("s.html", AttachmentKind::Html)], size);
        assert_eq!((v[0].status, v[1].status, v[2].status, v[2].empty, v[0].empty), (Status::Valid, Status::BadType, Status::Valid, true, false));
        // A second check is answered from the signature cache: replace the bytes in place, same size and time.
        assert_eq!(views("id1", &d, &[att("a.html", AttachmentKind::Html)], size)[0].status, Status::Valid);
        assert!(!serde_json::to_string(&v).unwrap().contains("a.html"));
        // Shape: kind must match the extension, names follow the evidence rule.
        assert!(!att("a.md", AttachmentKind::Html).valid() && !att("../a.html", AttachmentKind::Html).valid() && att("a.html", AttachmentKind::Html).valid());
        // 51 entries in the folder refuse everything.
        for i in 0..MAX_ENTRIES {
            std::fs::write(d.join(format!("attachments/x{i}.md")), "x").unwrap();
        }
        assert_eq!(open(&d, &att("a.html", AttachmentKind::Html)), Opened::Invalid { code: Status::TooMany });
    }

    #[test]
    fn first_line_reads_16_kb_once_per_signature() {
        let (_t, d) = report_dir();
        let size = evidence::measure(&d);
        // The first paragraph after the title lies past 16 KB: only the cut text is seen (no line).
        let pad = format!("<h1>DB 후보 비교</h1><!--{}-->", "x".repeat(FIRST_LINE_BYTES));
        std::fs::write(d.join("attachments/far.html"), format!("{pad}<p>너무 뒤에 있는 문단</p>")).unwrap();
        assert_eq!(first_line("c1", &d, &att("far.html", AttachmentKind::Html), size), None);
        // A multi-byte character cut at 16 KB keeps the valid part.
        let cut = format!("<h1>DB 후보 비교</h1><p>a{}</p>", "가".repeat(FIRST_LINE_BYTES / 3 + 10));
        std::fs::write(d.join("attachments/cut.html"), &cut).unwrap();
        assert!(first_line("c1", &d, &att("cut.html", AttachmentKind::Html), size).is_some_and(|l| l.chars().count() == FIRST_LINE_CHARS));
        // Same signature (size, time, inode): the cached line comes back without reading again.
        let f = d.join("attachments/a.md");
        std::fs::write(&f, "# DB 후보 비교\n\n첫 문단이에요\n").unwrap();
        let when = std::fs::metadata(&f).unwrap().modified().unwrap();
        assert_eq!(first_line("c1", &d, &att("a.md", AttachmentKind::Markdown), size).as_deref(), Some("첫 문단이에요"));
        {
            use std::io::{Seek, Write};
            let mut w = std::fs::OpenOptions::new().write(true).open(&f).unwrap();
            w.seek(std::io::SeekFrom::End(-("문단이에요\n".len() as i64))).unwrap();
            w.write_all("단락이에요\n".as_bytes()).unwrap();
            w.set_modified(when).unwrap();
        }
        assert_eq!(first_line("c1", &d, &att("a.md", AttachmentKind::Markdown), size).as_deref(), Some("첫 문단이에요"), "served from the cache");
        // A different size is a different signature: read again.
        std::fs::write(&f, "# DB 후보 비교\n\n새 문단\n").unwrap();
        assert_eq!(first_line("c1", &d, &att("a.md", AttachmentKind::Markdown), size).as_deref(), Some("새 문단"));
    }

    #[test]
    fn originals_are_read_with_the_open_rules() {
        let (_t, d) = report_dir();
        let raw = "<h1>비교</h1><a href=\"https://x.test/\">링크</a><script>x</script>";
        std::fs::write(d.join("attachments/a.html"), raw).unwrap();
        std::fs::write(d.join("attachments/a.md"), "# 제목\n\n- 항목\n").unwrap();
        std::fs::write(d.join("attachments/bad.md"), b"ok \xff").unwrap();
        std::fs::write(d.join("attachments/zero.md"), "a\0b").unwrap();
        std::fs::write(d.join("attachments/big.md"), vec![b'a'; MAX_MARKDOWN as usize + 1]).unwrap();
        // The original, not the cleaned srcdoc: the link and script are still there.
        assert_eq!(read_original(&d, &att("a.html", AttachmentKind::Html)).unwrap(), raw.as_bytes());
        assert_eq!(read_original(&d, &att("a.md", AttachmentKind::Markdown)).unwrap(), "# 제목\n\n- 항목\n".as_bytes());
        assert_eq!(read_original(&d, &att("bad.md", AttachmentKind::Markdown)), Err(Status::BadType));
        assert_eq!(read_original(&d, &att("zero.md", AttachmentKind::Markdown)), Err(Status::BadType));
        assert_eq!(read_original(&d, &att("big.md", AttachmentKind::Markdown)), Err(Status::TooLarge));
        assert_eq!(read_original(&d, &att("none.md", AttachmentKind::Markdown)), Err(Status::Missing));
    }

    #[cfg(unix)]
    #[test]
    fn originals_behind_links_are_refused() {
        let (_t, d) = report_dir();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("s.md"), "secret").unwrap();
        std::os::unix::fs::symlink(outside.path().join("s.md"), d.join("attachments/l.md")).unwrap();
        std::fs::hard_link(outside.path().join("s.md"), d.join("attachments/h.md")).unwrap();
        assert_eq!(read_original(&d, &att("l.md", AttachmentKind::Markdown)), Err(Status::Refused));
        assert_eq!(read_original(&d, &att("h.md", AttachmentKind::Markdown)), Err(Status::Refused));
    }

    #[test]
    fn saved_copies_are_whole_and_leave_no_temporary_file() {
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("비교.md");
        save_copy(&dest, "# 제목\n".as_bytes(), false).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), "# 제목\n".as_bytes());
        // Saving over an existing file (the panel already asked) replaces it.
        save_copy(&dest, b"new", false).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"new");
        assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 1, "no temporary file left");
        // A folder, a missing folder or a bare name is not a place to save.
        assert_eq!(save_copy(out.path(), b"x", false), Err(SaveError::BadPlace));
        assert_eq!(save_copy(&out.path().join("no/such/a.md"), b"x", false), Err(SaveError::BadPlace));
        assert_eq!(save_copy(Path::new("a.md"), b"x", false), Err(SaveError::BadPlace));
        assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_save_leaves_nothing() {
        use std::os::unix::fs::PermissionsExt;
        let out = tempfile::tempdir().unwrap();
        let ro = out.path().join("ro");
        std::fs::create_dir(&ro).unwrap();
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o500)).unwrap();
        assert_eq!(save_copy(&ro.join("a.md"), b"x", false), Err(SaveError::Write));
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(std::fs::read_dir(&ro).unwrap().count(), 0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn saved_html_is_quarantined() {
        let out = tempfile::tempdir().unwrap();
        let dest = out.path().join("a.html");
        save_copy(&dest, b"<p>x</p>", true).unwrap();
        let o = std::process::Command::new("xattr").arg("-p").arg("com.apple.quarantine").arg(&dest).output().unwrap();
        assert!(o.status.success() && String::from_utf8_lossy(&o.stdout).contains(";D.Duck;"));
        let md = out.path().join("a.md");
        save_copy(&md, b"x", false).unwrap();
        assert!(!std::process::Command::new("xattr").arg("-p").arg("com.apple.quarantine").arg(&md).output().unwrap().status.success());
    }

    #[cfg(unix)]
    #[test]
    fn linked_folders_and_files_are_refused() {
        let (_t, d) = report_dir();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("s.html"), "<p>secret</p>").unwrap();
        std::os::unix::fs::symlink(outside.path().join("s.html"), d.join("attachments/l.html")).unwrap();
        std::fs::hard_link(outside.path().join("s.html"), d.join("attachments/h.html")).unwrap();
        assert_eq!(open(&d, &att("l.html", AttachmentKind::Html)), Opened::Invalid { code: Status::Refused });
        assert_eq!(open(&d, &att("h.html", AttachmentKind::Html)), Opened::Invalid { code: Status::Refused });
        std::fs::remove_dir_all(d.join(DIR)).unwrap();
        std::os::unix::fs::symlink(outside.path(), d.join(DIR)).unwrap();
        assert_eq!(open(&d, &att("s.html", AttachmentKind::Html)), Opened::Invalid { code: Status::Refused });
    }

    #[test]
    fn markdown_images_follow_lines_outside_fences() {
        let (_t, d) = report_dir();
        std::fs::write(d.join("attachments/a.png"), png_bytes(10, 10)).unwrap();
        // A lone \r and a zero-width character are dropped first, as in safeMarkdown.ts.
        std::fs::write(d.join("attachments/m.md"), "# t\n\n![화면](a.png)\n\n```\n![x](a.png)\n```\n![없음](none.png)\n![원격](https://x.example/a.png)\n텍스트\r![숨김](a.png)\n![z\u{200b}w](a.png)\n").unwrap();
        let Opened::Markdown { images, .. } = open(&d, &att("m.md", AttachmentKind::Markdown)) else { panic!() };
        let names: Vec<_> = images.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["a.png", "none.png", "a.png"]);
        assert!(images[0].data.as_deref().is_some_and(|s| s.starts_with("data:image/png;base64,")) && images[1].data.is_none());
    }
}
