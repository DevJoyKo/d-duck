//! The app's only outbound HTTP (install-from-link, D-087): GET over https to two GitHub hosts, nothing else.
//! rustls with bundled roots, 10 s connect / 30 s total, no cookies, no proxy from the environment, fixed
//! User-Agent, no credentials of any kind. Redirects are followed by hand and only within the same host.
//! Bodies are capped on the bytes actually read, not on any size a server or API reports.

use std::io::Read;
use std::time::Duration;

use url::Url;

pub const ALLOWED_HOSTS: [&str; 2] = ["api.github.com", "raw.githubusercontent.com"];
const MAX_REDIRECTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HttpError {
    #[error("{}", crate::i18n::Fail::AddrNotAllowed.str())]
    NotAllowed,
    #[error("{}", crate::i18n::Fail::GithubConnect.str())]
    Network,
    #[error("{}", crate::i18n::Fail::ReplyTooBig.str())]
    TooLarge,
    #[error("{}", crate::i18n::Fail::RedirectOut.str())]
    BadRedirect,
}

pub struct Response {
    pub status: u16,
    pub location: Option<String>,
    pub body: Vec<u8>,
}

/// One GET, no redirect handling. `max` caps the body bytes read; reading more than `max` is `TooLarge`.
pub trait Transport: Send + Sync {
    fn get(&self, url: &Url, accept: Option<&str>, max: u64) -> Result<Response, HttpError>;
}

/// `https`, an allow-listed host, default port, no user info.
pub fn allowed(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.host_str().is_some_and(|h| ALLOWED_HOSTS.contains(&h))
}

/// GET with the allow-list and same-host redirects (at most three). Returns the final status and body.
pub fn get(t: &dyn Transport, url: &Url, accept: Option<&str>, max: u64) -> Result<Response, HttpError> {
    if !allowed(url) {
        return Err(HttpError::NotAllowed);
    }
    let mut current = url.clone();
    for _ in 0..=MAX_REDIRECTS {
        let r = t.get(&current, accept, max)?;
        if !(300..400).contains(&r.status) {
            return Ok(r);
        }
        let next = r.location.as_deref().and_then(|l| current.join(l).ok()).ok_or(HttpError::BadRedirect)?;
        if !allowed(&next) || next.host_str() != current.host_str() {
            return Err(HttpError::BadRedirect);
        }
        current = next;
    }
    Err(HttpError::BadRedirect)
}

/// The real transport (ureq + rustls).
pub struct Ureq(ureq::Agent);

impl Default for Ureq {
    fn default() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_global(Some(Duration::from_secs(30)))
            .max_redirects(0)
            .http_status_as_error(false)
            .https_only(true)
            .proxy(None)
            .user_agent("D.Duck")
            .build();
        Ureq(config.into())
    }
}

impl Transport for Ureq {
    fn get(&self, url: &Url, accept: Option<&str>, max: u64) -> Result<Response, HttpError> {
        if !allowed(url) {
            return Err(HttpError::NotAllowed);
        }
        let mut req = self.0.get(url.as_str());
        if let Some(a) = accept {
            req = req.header("Accept", a);
        }
        let mut resp = req.call().map_err(|_| HttpError::Network)?;
        let status = resp.status().as_u16();
        let location = resp.headers().get("location").and_then(|v| v.to_str().ok()).map(str::to_string);
        let mut body = Vec::new();
        resp.body_mut()
            .with_config()
            .limit(max.saturating_add(1))
            .reader()
            .take(max.saturating_add(1))
            .read_to_end(&mut body)
            // The limit is one byte above the cap and checked on the length below, so a read error is the connection.
            .map_err(|_| HttpError::Network)?;
        if body.len() as u64 > max {
            return Err(HttpError::TooLarge);
        }
        Ok(Response { status, location, body })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod fake {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Answers from a table (URL → status, body, location) and records every request.
    #[derive(Default)]
    pub struct Fake {
        pub routes: HashMap<String, (u16, Vec<u8>, Option<String>)>,
        pub seen: Mutex<Vec<String>>,
    }

    impl Fake {
        pub fn on(&mut self, url: &str, status: u16, body: impl Into<Vec<u8>>) -> &mut Self {
            self.routes.insert(url.to_string(), (status, body.into(), None));
            self
        }
        pub fn redirect(&mut self, url: &str, to: &str) -> &mut Self {
            self.routes.insert(url.to_string(), (302, Vec::new(), Some(to.to_string())));
            self
        }
        pub fn count(&self, prefix: &str) -> usize {
            self.seen.lock().unwrap().iter().filter(|u| u.starts_with(prefix)).count()
        }
    }

    impl Transport for Fake {
        fn get(&self, url: &Url, _accept: Option<&str>, max: u64) -> Result<Response, HttpError> {
            self.seen.lock().unwrap().push(url.to_string());
            let (status, body, location) = self.routes.get(url.as_str()).cloned().unwrap_or((404, b"{}".to_vec(), None));
            if body.len() as u64 > max {
                return Err(HttpError::TooLarge);
            }
            Ok(Response { status, location, body })
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::fake::Fake;
    use super::*;

    #[test]
    fn allow_list_and_redirects() {
        for bad in ["http://api.github.com/x", "https://evil.com/x", "https://u:p@api.github.com/x", "https://api.github.com:8443/x", "https://api.github.com.evil.com/"] {
            assert!(!allowed(&Url::parse(bad).unwrap()), "{bad}");
        }
        let mut f = Fake::default();
        f.redirect("https://raw.githubusercontent.com/a", "/b").on("https://raw.githubusercontent.com/b", 200, "ok");
        f.redirect("https://raw.githubusercontent.com/c", "https://api.github.com/x");
        f.redirect("https://raw.githubusercontent.com/loop", "/loop");
        let u = |s: &str| Url::parse(s).unwrap();
        assert_eq!(get(&f, &u("https://raw.githubusercontent.com/a"), None, 10).unwrap().body, b"ok");
        assert_eq!(get(&f, &u("https://raw.githubusercontent.com/c"), None, 10).err(), Some(HttpError::BadRedirect), "other host");
        assert_eq!(get(&f, &u("https://raw.githubusercontent.com/loop"), None, 10).err(), Some(HttpError::BadRedirect));
        assert_eq!(get(&f, &u("https://example.com/"), None, 10).err(), Some(HttpError::NotAllowed));
        assert_eq!(f.count("https://example.com"), 0, "never sent");
        assert_eq!(get(&f, &u("https://raw.githubusercontent.com/b"), None, 1).err(), Some(HttpError::TooLarge));
    }
}
