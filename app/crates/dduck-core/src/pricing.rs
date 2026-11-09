//! Cost estimation from a bundled price table that the user can override (D5).
//!
//! Costs are never stored; they are computed at query time so editing the table re-prices history.

use std::path::Path;

use serde::Deserialize;

const BUNDLED: &str = include_str!("../data/pricing.json");

/// USD per million tokens.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ModelPrice {
    pub prefix: String,
    pub input: f64,
    pub output: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
    pub cache_read: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PriceTable {
    pub models: Vec<ModelPrice>,
    /// Date the table was last checked against the official price page (`YYYY-MM-DD`); `None` for user files.
    #[serde(rename = "_checked", default)]
    pub checked: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Usage {
    pub input: i64,
    /// Includes thinking tokens, as billed.
    pub output: i64,
    pub cache_write_5m: i64,
    pub cache_write_1h: i64,
    pub cache_read: i64,
}

impl PriceTable {
    pub fn bundled() -> Self {
        serde_json::from_str(BUNDLED).unwrap_or(PriceTable { models: Vec::new(), checked: None })
    }

    /// User override file if present and valid, otherwise the bundled table.
    pub fn load(override_path: Option<&Path>) -> Self {
        override_path
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<PriceTable>(&s).ok())
            .filter(|t| !t.models.is_empty())
            .unwrap_or_else(Self::bundled)
    }

    /// Longest matching prefix wins, so `claude-opus-5-5` is not priced as `claude-opus-5`.
    pub fn find(&self, model: &str) -> Option<&ModelPrice> {
        self.models.iter().filter(|m| model.starts_with(&m.prefix)).max_by_key(|m| m.prefix.len())
    }

    /// Output price for a model id as written anywhere (provider prefix, `@date`, `-v1:0`, `[1m]` removed first).
    pub fn output_of(&self, model: &str) -> Option<f64> {
        self.find(&price_key(model)).map(|p| p.output)
    }

    /// `None` when the model has no price ("단가 미등록").
    pub fn cost(&self, model: &str, u: Usage) -> Option<f64> {
        let p = self.find(model)?;
        let per = |tokens: i64, price: f64| tokens.max(0) as f64 * price / 1_000_000.0;
        Some(
            per(u.input, p.input)
                + per(u.output, p.output)
                + per(u.cache_write_5m, p.cache_write_5m)
                + per(u.cache_write_1h, p.cache_write_1h)
                + per(u.cache_read, p.cache_read),
        )
    }
}

/// The id the price table is keyed by: `us.anthropic.claude-opus-5-5@20250929[1m]` → `claude-opus-5-5`
/// (model-select, design 6).
pub fn price_key(model: &str) -> String {
    let mut s = model.trim().strip_suffix("[1m]").unwrap_or(model.trim());
    for p in ["us.", "eu.", "apac.", "global."] {
        if let Some(rest) = s.strip_prefix(p) {
            s = rest;
            break;
        }
    }
    s = s.strip_prefix("anthropic.").or_else(|| s.strip_prefix("anthropic/")).unwrap_or(s);
    if let Some(i) = s.find('@') {
        s = &s[..i];
    }
    // Bedrock version tail: `-v1:0`.
    if let Some(i) = s.rfind("-v") {
        let tail = &s[i + 2..];
        if tail.split_once(':').is_some_and(|(a, b)| !a.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit())) {
            s = &s[..i];
        }
    }
    s.to_string()
}

/// Output-price ratio `chosen / default` from the app's table. `None` when either id has no price.
pub fn compare(table: &PriceTable, chosen_id: &str, default_id: &str) -> Option<f64> {
    let (c, d) = (table.output_of(chosen_id)?, table.output_of(default_id)?);
    (d > 0.0).then(|| c / d)
}

/// "기본값보다 토큰당 약 N배 비싸요 · 앱 단가표 기준" for ratios of 1.2 and more (N with one decimal).
pub fn cost_line(ratio: Option<f64>) -> Option<String> {
    let r = ratio?;
    (r >= 1.2).then(|| crate::tr_fmt!(crate::i18n::Fail::PriceMultiple, a0 = format!("{:.1}", (r * 10.0).round() / 10.0)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn compare_strips_provider_parts_and_needs_both_prices() {
        let t = PriceTable::bundled();
        for (raw, key) in [
            ("us.anthropic.claude-opus-5-5", "claude-opus-5-5"),
            ("claude-sonnet-4-5@20250929", "claude-sonnet-4-5"),
            ("anthropic.claude-haiku-4-5-v1:0", "claude-haiku-4-5"),
            ("claude-opus-4-6[1m]", "claude-opus-4-6"),
            ("claude-opus-5-5", "claude-opus-5-5"),
        ] {
            assert_eq!(price_key(raw), key, "{raw}");
        }
        let r = compare(&t, "claude-opus-5-5", "claude-sonnet-5-5").unwrap();
        assert!(r > 1.0);
        assert_eq!(compare(&t, "us.anthropic.claude-opus-5-5", "claude-sonnet-5-5"), Some(r));
        assert_eq!(compare(&t, "claude-unknown-9", "claude-sonnet-5-5"), None, "no price → no line");
        assert_eq!(cost_line(compare(&t, "claude-opus-5-5", "claude-opus-5-5")), None, "same model");
        assert_eq!(cost_line(Some(1.66)).as_deref(), Some("기본값보다 토큰당 약 1.7배 비싸요 · 앱 단가표 기준"));
        assert_eq!(cost_line(Some(1.19)), None);
        assert_eq!(cost_line(None), None);
    }

    #[test]
    fn bundled_table_loads_and_matches_longest_prefix() {
        let t = PriceTable::bundled();
        assert!(t.models.len() >= 10);
        assert!(t.checked.as_deref().is_some_and(|d| d.len() == 10 && d.starts_with("20")));
        assert_eq!(t.find("claude-opus-5-5").unwrap().input, 4.0);
        assert_eq!(t.find("claude-opus-5").unwrap().input, 5.0);
        assert_eq!(t.find("claude-fable-5-1").unwrap().cache_read, 0.25);
        assert_eq!(t.find("claude-fable-5").unwrap().cache_read, 1.0);
    }

    #[test]
    fn cost_math() {
        let t = PriceTable::bundled();
        let u = Usage { input: 1_000_000, output: 1_000_000, cache_write_5m: 1_000_000, cache_write_1h: 1_000_000, cache_read: 1_000_000 };
        let c = t.cost("claude-opus-5-5", u).unwrap();
        assert!((c - (4.0 + 20.0 + 5.0 + 8.0 + 0.2)).abs() < 1e-9, "{c}");
    }

    #[test]
    fn unknown_model_has_no_price() {
        assert_eq!(PriceTable::bundled().cost("gpt-x", Usage::default()), None);
    }

    #[test]
    fn override_file_wins_and_bad_file_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("p.json");
        std::fs::write(&good, r#"{"models":[{"prefix":"claude-","input":1,"output":1,"cache_write_5m":1,"cache_write_1h":1,"cache_read":1}]}"#).unwrap();
        assert_eq!(PriceTable::load(Some(&good)).models.len(), 1);
        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "{nope").unwrap();
        assert_eq!(PriceTable::load(Some(&bad)), PriceTable::bundled());
    }
}
