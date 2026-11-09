//! Step durations for slow-feeling commands (D-159), e.g. `start_task`. Only fixed step names and milliseconds are
//! recorded — never a prompt, path, name or CLI output (D-021). This is the one module that emits `tracing` events;
//! `tests/no_content_logging.rs` exempts it and checks that it records nothing but `&'static str` names and numbers.
//!
//! Events go nowhere unless the app was launched with `DDUCK_TIMINGS=1`; then one line per command is written to
//! stderr: `D.Duck timing flow=start_task gate_ms=3 prepare_ms=40 … total_ms=410`.

use std::fmt::Write as _;
use std::time::Instant;

const TARGET: &str = "dduck::timing";

/// Laps of one command. Dropping it emits the event, so early returns are timed too.
pub struct Steps {
    flow: &'static str,
    start: Instant,
    last: Instant,
    laps: Vec<(&'static str, u64)>,
    /// Fixed-name numeric codes (e.g. an SQLite result code), never text.
    codes: Vec<(&'static str, i64)>,
}

impl Steps {
    pub fn new(flow: &'static str) -> Self {
        let now = Instant::now();
        Self { flow, start: now, last: now, laps: Vec::new(), codes: Vec::new() }
    }

    /// Ends the step called `name` (time since the previous lap).
    pub fn lap(&mut self, name: &'static str) {
        let now = Instant::now();
        self.laps.push((name, ms(now - self.last)));
        self.last = now;
    }

    /// Records a numeric code under a fixed name (D-164: which SQLite error a record hit).
    pub fn code(&mut self, name: &'static str, value: i64) {
        self.codes.push((name, value));
    }

    fn line(&self) -> String {
        let mut s = String::new();
        for (name, ms) in &self.laps {
            let _ = write!(s, "{name}_ms={ms} ");
        }
        for (name, v) in &self.codes {
            let _ = write!(s, "{name}={v} ");
        }
        s.trim_end().to_string()
    }
}

impl Drop for Steps {
    fn drop(&mut self) {
        let steps = self.line();
        tracing::info!(target: TARGET, flow = self.flow, steps = steps.as_str(), total_ms = ms(self.start.elapsed()));
    }
}

fn ms(d: std::time::Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Installs the stderr writer when `DDUCK_TIMINGS=1`. Without it the events are dropped.
pub fn init() {
    if std::env::var_os("DDUCK_TIMINGS").is_some_and(|v| v == "1") {
        let _ = tracing::subscriber::set_global_default(Stderr);
    }
}

/// Accepts only this module's events and only string and number fields.
struct Stderr;

impl tracing::Subscriber for Stderr {
    fn enabled(&self, m: &tracing::Metadata<'_>) -> bool {
        m.target() == TARGET && m.is_event()
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, e: &tracing::Event<'_>) {
        use std::io::Write;
        let mut line = Line(String::new());
        e.record(&mut line);
        let _ = writeln!(std::io::stderr(), "D.Duck timing{}", line.0);
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

struct Line(String);

impl tracing::field::Visit for Line {
    fn record_u64(&mut self, f: &tracing::field::Field, v: u64) {
        let _ = write!(self.0, " {}={v}", f.name());
    }
    fn record_str(&mut self, f: &tracing::field::Field, v: &str) {
        let _ = write!(self.0, " {}={v}", f.name());
    }
    // Anything else (debug-formatted values) is never written.
    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn laps_are_names_and_milliseconds_only() {
        let mut s = Steps::new("start_task");
        s.lap("gate");
        std::thread::sleep(std::time::Duration::from_millis(15));
        s.lap("prepare");
        let line = s.line();
        let re = regex_lite(&line);
        assert!(re, "{line}");
        let prepare: u64 = line.split("prepare_ms=").nth(1).unwrap().parse().unwrap();
        assert!(prepare >= 15, "{line}");
    }

    fn regex_lite(line: &str) -> bool {
        line.split(' ').all(|part| {
            let (k, v) = part.split_once('=').unwrap_or(("", "x"));
            k.ends_with("_ms") && k.chars().all(|c| c.is_ascii_lowercase() || c == '_') && v.chars().all(|c| c.is_ascii_digit())
        })
    }
}
