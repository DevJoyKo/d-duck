//! Scans `claude auth login` output for exactly two things (claude-login, design 5): one allowed sign-in URL and a
//! "paste code" prompt. Every other line is dropped as soon as it is looked at; nothing is stored or logged.
//! At most [`MAX_LINE`] bytes per line and [`MAX_SCANNED`] in total are inspected; the rest is read to EOF and
//! thrown away so the child never blocks on a full pipe.

use std::io::Read;

use url::Url;

pub const MAX_LINE: usize = 4 * 1024;
pub const MAX_SCANNED: usize = 64 * 1024;
pub const MAX_URL: usize = 2048;
const HOSTS: [&str; 3] = ["claude.ai", "platform.claude.com", "console.anthropic.com"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    Link(Url),
    NeedsTerminal,
}

/// `https`, host exactly one of [`HOSTS`], no userinfo, no explicit non-default port, at most [`MAX_URL`] bytes.
pub fn allowed_url(candidate: &str) -> Option<Url> {
    if candidate.len() > MAX_URL || !candidate.starts_with("https://") {
        return None;
    }
    let url = Url::parse(candidate).ok()?;
    let ok = url.scheme() == "https"
        && url.host_str().is_some_and(|h| HOSTS.contains(&h))
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        // `https://claude.ai:443` parses with port() == None; refuse any written port too.
        && !candidate["https://".len()..].split(['/', '?', '#']).next().is_some_and(|auth| auth.contains(':'));
    ok.then_some(url)
}

/// Drops ANSI escape sequences (CSI `ESC [ … letter` and OSC `ESC ] … BEL|ESC \`) and other control bytes.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            match it.next() {
                Some('[') => {
                    for d in it.by_ref() {
                        if ('@'..='~').contains(&d) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(d) = it.next() {
                        if d == '\u{7}' || (d == '\u{1b}' && it.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else if !c.is_control() || c == '\t' {
            out.push(c);
        }
    }
    out
}

const URL_END: [char; 10] = ['"', '\'', '<', '>', '(', ')', '[', ']', '`', ' '];

/// Looks at one line: the first allowed URL and the paste-code prompt.
pub fn scan_line(line: &str) -> Vec<Finding> {
    let clean = strip_ansi(line);
    let mut found = Vec::new();
    let lower = clean.to_ascii_lowercase();
    if lower.contains("paste code") || lower.contains("paste the code") {
        found.push(Finding::NeedsTerminal);
    }
    for (i, _) in clean.match_indices("https://") {
        let rest = &clean[i..];
        let end = rest.find(|c: char| c.is_whitespace() || URL_END.contains(&c)).unwrap_or(rest.len());
        let cand = rest[..end].trim_end_matches(['.', ',', ';']);
        if let Some(u) = allowed_url(cand) {
            found.push(Finding::Link(u));
            break;
        }
    }
    found
}

/// Reads `r` to EOF, reporting findings from the first [`MAX_SCANNED`] bytes. Lines end at `\n` or `\r`. A line
/// longer than [`MAX_LINE`], or one cut by the [`MAX_SCANNED`] cap, is dropped whole (never scanned as a fragment).
/// Returns the total bytes read.
pub fn scan<R: Read>(mut r: R, mut report: impl FnMut(Finding)) -> u64 {
    let mut chunk = [0u8; 8 * 1024];
    let mut line: Vec<u8> = Vec::new();
    let mut too_long = false;
    let mut scanned = 0usize;
    let mut total = 0u64;
    let flush = |line: &mut Vec<u8>, report: &mut dyn FnMut(Finding)| {
        if !line.is_empty() {
            for f in scan_line(&String::from_utf8_lossy(line)) {
                report(f);
            }
            line.clear();
        }
    };
    loop {
        let n = match r.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        total += n as u64;
        for &b in &chunk[..n] {
            if scanned >= MAX_SCANNED {
                break;
            }
            scanned += 1;
            if b == b'\n' || b == b'\r' {
                if too_long {
                    line.clear();
                    too_long = false;
                } else {
                    flush(&mut line, &mut report);
                }
            } else if line.len() < MAX_LINE {
                line.push(b);
            } else {
                too_long = true;
            }
        }
    }
    if !too_long && scanned < MAX_SCANNED {
        flush(&mut line, &mut report);
    }
    total
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn links(s: &str) -> Vec<String> {
        scan_line(s).into_iter().filter_map(|f| if let Finding::Link(u) = f { Some(u.as_str().to_string()) } else { None }).collect()
    }

    #[test]
    fn url_allowlist() {
        assert!(allowed_url("https://claude.ai/oauth/authorize?code=true&state=x").is_some());
        assert!(allowed_url("https://platform.claude.com/oauth/authorize").is_some());
        assert!(allowed_url("https://console.anthropic.com/x").is_some());
        for bad in [
            "https://claude.ai.example.com/oauth",
            "http://claude.ai/oauth",
            "https://user@claude.ai/oauth",
            "https://user:pw@claude.ai/oauth",
            "https://claude.ai:8443/oauth",
            "https://claude.ai:443/oauth",
            "https://evil.com/?https://claude.ai",
            "https://xclaude.ai/",
        ] {
            assert!(allowed_url(bad).is_none(), "{bad}");
        }
        let long = format!("https://claude.ai/{}", "a".repeat(MAX_URL));
        assert!(allowed_url(&long[..MAX_URL]).is_some(), "2048 ok");
        assert!(allowed_url(&long[..MAX_URL + 1]).is_none(), "2049 refused");
    }

    #[test]
    fn urls_inside_text() {
        assert_eq!(links("If the browser didn't open, visit: https://claude.ai/oauth/authorize?a=1"), ["https://claude.ai/oauth/authorize?a=1"]);
        assert_eq!(links("open \"https://claude.ai/o?a=1\" now"), ["https://claude.ai/o?a=1"]);
        assert_eq!(links("(https://claude.ai/o?a=1)."), ["https://claude.ai/o?a=1"]);
        assert_eq!(links("\u{1b}[1;34mhttps://claude.ai/o?a=1\u{1b}[0m"), ["https://claude.ai/o?a=1"]);
        assert_eq!(links("\u{1b}]8;;https://claude.ai/o\u{7}click\u{1b}]8;;\u{7}"), Vec::<String>::new(), "OSC link payloads are dropped");
        assert!(links("go to https://claude.ai.example.com/o").is_empty());
        assert!(links("http://claude.ai/o").is_empty());
        assert_eq!(links("https://evil.com/x https://claude.ai/ok"), ["https://claude.ai/ok"], "first allowed one");
    }

    #[test]
    fn paste_prompt() {
        assert_eq!(scan_line("Paste code here if prompted > "), [Finding::NeedsTerminal]);
        assert!(scan_line("Opening browser to sign in…").is_empty());
    }

    #[test]
    fn reader_consumes_everything_but_scans_64kb() {
        let mut data = b"line one\r\nhttps://claude.ai/first\n".to_vec();
        data.extend(std::iter::repeat_n(b'x', 1024 * 1024));
        data.extend(b"\nhttps://claude.ai/late\nPaste code here\n");
        let mut found = Vec::new();
        let total = scan(std::io::Cursor::new(&data), |f| found.push(f));
        assert_eq!(total, data.len() as u64, "read to EOF");
        assert_eq!(found, [Finding::Link(Url::parse("https://claude.ai/first").unwrap())], "nothing after 64 KB is looked at");
    }

    #[test]
    fn long_lines_are_dropped_whole() {
        // A URL at the start of an over-long line is not taken from a truncated fragment.
        let mut data = b"https://claude.ai/start ".to_vec();
        data.extend(vec![b'x'; MAX_LINE]);
        data.extend(b"\nPaste code here\n");
        let mut found = Vec::new();
        scan(std::io::Cursor::new(&data), |f| found.push(f));
        assert_eq!(found, [Finding::NeedsTerminal], "the long line is skipped, the next one is read");
        let mut unterminated = b"https://claude.ai/tail ".to_vec();
        unterminated.extend(vec![b'x'; MAX_LINE]);
        let mut found = Vec::new();
        scan(std::io::Cursor::new(&unterminated), |f| found.push(f));
        assert!(found.is_empty());
        let exact = format!("https://claude.ai/{}", "a".repeat(MAX_LINE - 18));
        let mut found = Vec::new();
        scan(std::io::Cursor::new(exact.as_bytes()), |f| found.push(f));
        assert!(found.is_empty(), "a URL that long is over 2048 anyway");
    }

    #[test]
    fn long_lines_are_cut() {
        let mut data = vec![b'y'; MAX_LINE];
        data.extend(b" https://claude.ai/hidden\nhttps://claude.ai/next\n");
        let mut found = Vec::new();
        scan(std::io::Cursor::new(&data), |f| found.push(f));
        assert_eq!(found, [Finding::Link(Url::parse("https://claude.ai/next").unwrap())]);
    }
}
