//! Aggregations behind the usage tab (usage-analytics spec). Read-only; every value is bound as a parameter.

use std::collections::BTreeMap;

use rusqlite::{Connection, ToSql, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::pricing::{PriceTable, Usage};

/// Period and filters. `from_ms`/`to_ms` are epoch millis, `[from, to)`, already aligned to local midnight by the
/// caller. `tz_offset_minutes` is the viewer's UTC offset, used for day/hour buckets.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub from_ms: i64,
    pub to_ms: i64,
    pub tz_offset_minutes: i32,
    #[serde(default)]
    pub projects: Vec<String>,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub input: i64,
    pub output: i64,
    pub thinking: i64,
    pub cache_write: i64,
    pub cache_read: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub tokens: Tokens,
    pub est_cost_usd: f64,
    pub unpriced_models: Vec<String>,
    pub sessions: i64,
    pub responses: i64,
    pub tool_calls: i64,
    pub skill_calls: i64,
    pub command_calls: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Bucket {
    Day,
    Hour,
}

impl Bucket {
    /// Hourly buckets for periods of two days or less (usage-analytics: 시계열 추이).
    pub fn for_range(from_ms: i64, to_ms: i64) -> Self {
        if to_ms - from_ms <= 2 * 24 * 3600 * 1000 { Bucket::Hour } else { Bucket::Day }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    /// Local time label: `YYYY-MM-DD` or `YYYY-MM-DD HH:00`.
    pub bucket: String,
    pub tokens: Tokens,
    pub est_cost_usd: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Dimension {
    Model,
    Project,
    Tool,
    Skill,
    Command,
    McpServer,
    Subagent,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub key: String,
    pub count: i64,
    /// Percentage of `count` over the dimension total, 0–100.
    pub share: f64,
    /// Skill calls via the Skill tool (skills only).
    pub via_tool: i64,
    /// Skill calls via typed slash command (skills only).
    pub via_slash: i64,
    pub tokens: Option<Tokens>,
    pub est_cost_usd: Option<f64>,
    pub priced: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub session_id: String,
    /// Stored AI title, or `None` (UI shows the id prefix).
    pub title: Option<String>,
    /// Title the user gave the session in D.Duck (wins over `title`).
    pub label: Option<String>,
    pub project_name: Option<String>,
    pub git_branch: Option<String>,
    pub first_ts: i64,
    pub last_ts: i64,
    pub responses: i64,
    pub tokens: Tokens,
    pub est_cost_usd: f64,
    pub reported_cost_usd: Option<f64>,
    /// Most-used tools (up to 3), skills and subagent types of the session, names and counts only (D-074).
    /// Filled by [`sessions_page`]; empty elsewhere.
    pub top_tools: Vec<NameCount>,
    pub skills: Vec<NameCount>,
    pub subagents: Vec<NameCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameCount {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session: SessionRow,
    pub models: Vec<Item>,
    pub tools: Vec<Item>,
    pub skills: Vec<Item>,
}

/// The catch-all row key: a fixed key the webview matches and words (DistributionList `OTHER_BUCKET`), not text.
const OTHER_KEY: &str = "기타";

/// Builds `WHERE` fragments shared by all queries. `alias` is the table holding `ts`/`session_id`.
pub(crate) struct Filter {
    pub(crate) sql: String,
    pub(crate) args: Vec<Box<dyn ToSql>>,
}

impl Filter {
    pub(crate) fn new(q: &Query, alias: &str, model_col: Option<&str>) -> Self {
        let mut sql = format!("{alias}.ts >= ? AND {alias}.ts < ?");
        let mut args: Vec<Box<dyn ToSql>> = vec![Box::new(q.from_ms), Box::new(q.to_ms)];
        if !q.projects.is_empty() {
            sql.push_str(&format!(
                " AND {alias}.session_id IN (SELECT session_id FROM sessions WHERE project_name IN ({}))",
                placeholders(q.projects.len())
            ));
            args.extend(q.projects.iter().map(|p| Box::new(p.clone()) as Box<dyn ToSql>));
        }
        if !q.models.is_empty() {
            match model_col {
                Some(col) => sql.push_str(&format!(" AND {col} IN ({})", placeholders(q.models.len()))),
                // Rows without a model (slash commands) count when their session used a selected model.
                None => sql.push_str(&format!(
                    " AND {alias}.session_id IN (SELECT DISTINCT session_id FROM api_calls WHERE model IN ({}))",
                    placeholders(q.models.len())
                )),
            }
            args.extend(q.models.iter().map(|m| Box::new(m.clone()) as Box<dyn ToSql>));
        }
        Filter { sql, args }
    }
}

fn placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

fn bucket_expr(bucket: Bucket) -> &'static str {
    match bucket {
        Bucket::Day => "strftime('%Y-%m-%d', a.ts / 1000, 'unixepoch', ? || ' minutes')",
        Bucket::Hour => "strftime('%Y-%m-%d %H:00', a.ts / 1000, 'unixepoch', ? || ' minutes')",
    }
}

/// Per-model usage rows, the basis for every cost figure.
struct ModelUsage {
    model: String,
    tokens: Tokens,
    usage: Usage,
}

fn model_usage(conn: &Connection, extra_where: &str, f: &Filter) -> Result<Vec<ModelUsage>, Error> {
    let sql = format!(
        "SELECT a.model, SUM(a.input), SUM(a.output), SUM(a.thinking), SUM(a.cache_write_5m), SUM(a.cache_write_1h),
                SUM(a.cache_read)
         FROM api_calls a WHERE {} {extra_where} GROUP BY a.model",
        f.sql
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(f.args.iter()), |r| {
        let (i, o, t, w5, w1, cr): (i64, i64, i64, i64, i64, i64) =
            (r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?);
        Ok(ModelUsage {
            model: r.get(0)?,
            tokens: Tokens { input: i, output: o, thinking: t, cache_write: w5 + w1, cache_read: cr },
            usage: Usage { input: i, output: o, cache_write_5m: w5, cache_write_1h: w1, cache_read: cr },
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn add(a: &mut Tokens, b: Tokens) {
    a.input += b.input;
    a.output += b.output;
    a.thinking += b.thinking;
    a.cache_write += b.cache_write;
    a.cache_read += b.cache_read;
}

fn count(conn: &Connection, sql: &str, f: &Filter) -> Result<i64, Error> {
    Ok(conn.query_row(sql, params_from_iter(f.args.iter()), |r| r.get(0))?)
}

pub fn summary(conn: &Connection, q: &Query, prices: &PriceTable) -> Result<Summary, Error> {
    let fa = Filter::new(q, "a", Some("a.model"));
    let mut s = Summary::default();
    let mut unpriced = Vec::new();
    for m in model_usage(conn, "", &fa)? {
        add(&mut s.tokens, m.tokens);
        match prices.cost(&m.model, m.usage) {
            Some(c) => s.est_cost_usd += c,
            None => unpriced.push(m.model),
        }
    }
    s.unpriced_models = unpriced;
    s.responses = count(conn, &format!("SELECT COUNT(*) FROM api_calls a WHERE {}", fa.sql), &fa)?;
    s.sessions = count(conn, &format!("SELECT COUNT(DISTINCT a.session_id) FROM api_calls a WHERE {}", fa.sql), &fa)?;
    let ft = Filter::new(q, "t", Some("(SELECT model FROM api_calls WHERE message_id = t.message_id LIMIT 1)"));
    s.tool_calls = count(conn, &format!("SELECT COUNT(*) FROM tool_calls t WHERE {}", ft.sql), &ft)?;
    let fc = Filter::new(q, "c", None);
    s.skill_calls = count(conn, &format!("SELECT COUNT(*) FROM command_invocations c WHERE {} AND c.kind = 'skill'", fc.sql), &fc)?;
    s.command_calls =
        count(conn, &format!("SELECT COUNT(*) FROM command_invocations c WHERE {} AND c.kind = 'builtin'", fc.sql), &fc)?;
    Ok(s)
}

pub fn timeseries(conn: &Connection, q: &Query, prices: &PriceTable, bucket: Bucket) -> Result<Vec<Point>, Error> {
    let f = Filter::new(q, "a", Some("a.model"));
    let sql = format!(
        "SELECT {b} AS bucket, a.model, SUM(a.input), SUM(a.output), SUM(a.thinking), SUM(a.cache_write_5m),
                SUM(a.cache_write_1h), SUM(a.cache_read)
         FROM api_calls a WHERE {w} GROUP BY bucket, a.model ORDER BY bucket",
        b = bucket_expr(bucket),
        w = f.sql
    );
    let mut args: Vec<&dyn ToSql> = Vec::with_capacity(f.args.len() + 1);
    let offset = format!("{:+}", q.tz_offset_minutes);
    args.push(&offset);
    args.extend(f.args.iter().map(|a| a.as_ref() as &dyn ToSql));
    let mut stmt = conn.prepare(&sql)?;
    let mut out: BTreeMap<String, Point> = BTreeMap::new();
    let rows = stmt.query_map(params_from_iter(args), |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            [r.get::<_, i64>(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?],
        ))
    })?;
    for row in rows {
        let (label, model, [i, o, t, w5, w1, cr]) = row?;
        let p = out.entry(label.clone()).or_insert(Point { bucket: label, tokens: Tokens::default(), est_cost_usd: 0.0 });
        add(&mut p.tokens, Tokens { input: i, output: o, thinking: t, cache_write: w5 + w1, cache_read: cr });
        let usage = Usage { input: i, output: o, cache_write_5m: w5, cache_write_1h: w1, cache_read: cr };
        p.est_cost_usd += prices.cost(&model, usage).unwrap_or(0.0);
    }
    Ok(out.into_values().collect())
}

/// Sorted by count (descending). `top` > 0 keeps that many and folds the rest into "기타".
pub fn distribution(conn: &Connection, q: &Query, prices: &PriceTable, dim: Dimension, top: usize) -> Result<Vec<Item>, Error> {
    let mut items: Vec<Item> = match dim {
        Dimension::Model => {
            let f = Filter::new(q, "a", Some("a.model"));
            let counts = keyed_counts(conn, "SELECT a.model, COUNT(*) FROM api_calls a WHERE {w} GROUP BY a.model", &f)?;
            model_usage(conn, "", &f)?
                .into_iter()
                .map(|m| {
                    let cost = prices.cost(&m.model, m.usage);
                    Item {
                        count: counts.get(&m.model).copied().unwrap_or(0),
                        key: m.model,
                        share: 0.0,
                        via_tool: 0,
                        via_slash: 0,
                        tokens: Some(m.tokens),
                        est_cost_usd: cost,
                        priced: cost.is_some(),
                    }
                })
                .collect()
        }
        Dimension::Project => {
            let f = Filter::new(q, "a", Some("a.model"));
            let sql = format!(
                "SELECT IFNULL(s.project_name, '?'), a.model, COUNT(*), SUM(a.input), SUM(a.output), SUM(a.thinking),
                        SUM(a.cache_write_5m), SUM(a.cache_write_1h), SUM(a.cache_read)
                 FROM api_calls a LEFT JOIN sessions s ON s.session_id = a.session_id WHERE {} GROUP BY 1, 2",
                f.sql
            );
            let mut stmt = conn.prepare(&sql)?;
            let mut by: BTreeMap<String, Item> = BTreeMap::new();
            let rows = stmt.query_map(params_from_iter(f.args.iter()), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    [r.get::<_, i64>(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?],
                ))
            })?;
            for row in rows {
                let (project, model, [n, i, o, t, w5, w1, cr]) = row?;
                let it = by.entry(project.clone()).or_insert(Item {
                    key: project,
                    count: 0,
                    share: 0.0,
                    via_tool: 0,
                    via_slash: 0,
                    tokens: Some(Tokens::default()),
                    est_cost_usd: Some(0.0),
                    priced: true,
                });
                it.count += n;
                if let Some(tok) = it.tokens.as_mut() {
                    add(tok, Tokens { input: i, output: o, thinking: t, cache_write: w5 + w1, cache_read: cr });
                }
                let usage = Usage { input: i, output: o, cache_write_5m: w5, cache_write_1h: w1, cache_read: cr };
                match prices.cost(&model, usage) {
                    Some(c) => it.est_cost_usd = it.est_cost_usd.map(|x| x + c),
                    None => it.priced = false,
                }
            }
            by.into_values().collect()
        }
        Dimension::Tool | Dimension::McpServer | Dimension::Subagent => {
            let col = match dim {
                Dimension::Tool => "t.tool_name",
                Dimension::McpServer => "t.mcp_server",
                _ => "t.subagent_type",
            };
            let f = Filter::new(q, "t", Some("(SELECT model FROM api_calls WHERE message_id = t.message_id LIMIT 1)"));
            let sql = format!("SELECT {col}, COUNT(*) FROM tool_calls t WHERE {{w}} AND {col} IS NOT NULL GROUP BY {col}");
            keyed_counts(conn, &sql, &f)?.into_iter().map(|(k, n)| plain_item(k, n)).collect()
        }
        Dimension::Skill | Dimension::Command => {
            let kind = if dim == Dimension::Skill { "skill" } else { "builtin" };
            let f = Filter::new(q, "c", None);
            let sql = format!(
                "SELECT c.name, SUM(c.via = 'tool'), SUM(c.via = 'slash'), COUNT(*) FROM command_invocations c
                 WHERE {} AND c.kind = '{kind}' GROUP BY c.name",
                f.sql
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(f.args.iter()), |r| {
                Ok(Item {
                    key: r.get(0)?,
                    via_tool: r.get(1)?,
                    via_slash: r.get(2)?,
                    count: r.get(3)?,
                    share: 0.0,
                    tokens: None,
                    est_cost_usd: None,
                    priced: true,
                })
            })?;
            rows.collect::<Result<_, _>>()?
        }
    };
    items.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
    if top > 0 && items.len() > top {
        let rest = items.split_off(top);
        let mut other = plain_item(OTHER_KEY.to_string(), rest.iter().map(|i| i.count).sum());
        other.via_tool = rest.iter().map(|i| i.via_tool).sum();
        other.via_slash = rest.iter().map(|i| i.via_slash).sum();
        items.push(other);
    }
    let total: i64 = items.iter().map(|i| i.count).sum();
    for it in &mut items {
        it.share = if total > 0 { it.count as f64 * 100.0 / total as f64 } else { 0.0 };
    }
    Ok(items)
}

fn plain_item(key: String, count: i64) -> Item {
    Item { key, count, share: 0.0, via_tool: 0, via_slash: 0, tokens: None, est_cost_usd: None, priced: true }
}

fn keyed_counts(conn: &Connection, sql_tpl: &str, f: &Filter) -> Result<BTreeMap<String, i64>, Error> {
    let sql = sql_tpl.replace("{w}", &f.sql);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(f.args.iter()), |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Sessions with activity in the period, most recent first.
pub fn sessions(conn: &Connection, q: &Query, prices: &PriceTable, limit: usize) -> Result<Vec<SessionRow>, Error> {
    let f = Filter::new(q, "a", Some("a.model"));
    let sql = format!(
        "SELECT a.session_id, MAX(a.ts) AS last FROM api_calls a WHERE {} GROUP BY a.session_id ORDER BY last DESC LIMIT {}",
        f.sql,
        limit.min(10_000)
    );
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(f.args.iter()), |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    ids.iter().map(|id| session_row(conn, q, prices, id)).collect()
}

fn session_row(conn: &Connection, q: &Query, prices: &PriceTable, id: &str) -> Result<SessionRow, Error> {
    let session_q = Query { projects: Vec::new(), ..q.clone() };
    let mut f = Filter::new(&session_q, "a", Some("a.model"));
    f.sql.push_str(" AND a.session_id = ?");
    f.args.push(Box::new(id.to_string()));
    let mut tokens = Tokens::default();
    let mut cost = 0.0;
    for m in model_usage(conn, "", &f)? {
        add(&mut tokens, m.tokens);
        cost += prices.cost(&m.model, m.usage).unwrap_or(0.0);
    }
    let responses = count(conn, &format!("SELECT COUNT(*) FROM api_calls a WHERE {}", f.sql), &f)?;
    let meta = conn.query_row(
        "SELECT title, project_name, git_branch, first_ts, last_ts,
                (SELECT SUM(total_cost_usd) FROM cost_segments WHERE session_id = ?1)
         FROM sessions WHERE session_id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
    );
    let (title, project_name, git_branch, first_ts, last_ts, reported) = match meta {
        Ok(m) => m,
        Err(rusqlite::Error::QueryReturnedNoRows) => (None, None, None, 0, 0, None),
        Err(e) => return Err(e.into()),
    };
    let label = crate::labels::get(conn, id)?;
    Ok(SessionRow {
        session_id: id.to_string(),
        title,
        label,
        project_name,
        git_branch,
        first_ts,
        last_ts,
        responses,
        tokens,
        est_cost_usd: cost,
        reported_cost_usd: reported,
        top_tools: Vec::new(),
        skills: Vec::new(),
        subagents: Vec::new(),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPage {
    pub rows: Vec<SessionRow>,
    pub total: i64,
}

/// One page of sessions, most recent activity first, with the total for the same filter.
pub fn sessions_page(conn: &Connection, q: &Query, prices: &PriceTable, offset: usize, limit: usize) -> Result<SessionPage, Error> {
    let f = Filter::new(q, "a", Some("a.model"));
    let total = count(conn, &format!("SELECT COUNT(DISTINCT a.session_id) FROM api_calls a WHERE {}", f.sql), &f)?;
    let sql = format!(
        "SELECT a.session_id, MAX(a.ts) AS last FROM api_calls a WHERE {} GROUP BY a.session_id ORDER BY last DESC, a.session_id LIMIT {} OFFSET {}",
        f.sql,
        limit.clamp(1, 50),
        offset.min(100_000)
    );
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(f.args.iter()), |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut rows: Vec<SessionRow> = ids.iter().map(|id| session_row(conn, q, prices, id)).collect::<Result<_, _>>()?;
    fill_usage_chips(conn, &mut rows)?;
    Ok(SessionPage { rows, total })
}

/// Tool, skill and subagent counts for one page of sessions, three grouped queries over the page's ids.
fn fill_usage_chips(conn: &Connection, rows: &mut [SessionRow]) -> Result<(), Error> {
    if rows.is_empty() {
        return Ok(());
    }
    let ids: Vec<&str> = rows.iter().map(|r| r.session_id.as_str()).collect();
    let ph = placeholders(ids.len());
    let grouped = |sql: String| -> Result<BTreeMap<String, Vec<NameCount>>, Error> {
        let mut stmt = conn.prepare(&sql)?;
        let found = stmt.query_map(params_from_iter(ids.iter()), |r| {
            Ok((r.get::<_, String>(0)?, NameCount { name: r.get(1)?, count: r.get(2)? }))
        })?;
        let mut by: BTreeMap<String, Vec<NameCount>> = BTreeMap::new();
        for row in found {
            let (sid, nc) = row?;
            by.entry(sid).or_default().push(nc);
        }
        for list in by.values_mut() {
            list.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
        }
        Ok(by)
    };
    let tools = grouped(format!(
        "SELECT session_id, tool_name, COUNT(*) FROM tool_calls WHERE session_id IN ({ph}) GROUP BY session_id, tool_name"
    ))?;
    // command_invocations already merges Skill-tool and slash use without double counting (D-071).
    let skills = grouped(format!(
        "SELECT session_id, name, COUNT(*) FROM command_invocations WHERE session_id IN ({ph}) AND kind = 'skill' GROUP BY session_id, name"
    ))?;
    let subagents = grouped(format!(
        "SELECT session_id, subagent_type, COUNT(*) FROM tool_calls
         WHERE session_id IN ({ph}) AND subagent_type IS NOT NULL GROUP BY session_id, subagent_type"
    ))?;
    for r in rows.iter_mut() {
        r.top_tools = tools.get(&r.session_id).map(|l| l.iter().take(3).cloned().collect()).unwrap_or_default();
        r.skills = skills.get(&r.session_id).map(|l| l.iter().take(8).cloned().collect()).unwrap_or_default();
        r.subagents = subagents.get(&r.session_id).map(|l| l.iter().take(8).cloned().collect()).unwrap_or_default();
    }
    Ok(())
}

/// Detail of one session over all time.
pub fn session_detail(conn: &Connection, prices: &PriceTable, session_id: &str) -> Result<SessionDetail, Error> {
    let all = Query { from_ms: i64::MIN / 2, to_ms: i64::MAX / 2, ..Query::default() };
    let session = session_row(conn, &all, prices, session_id)?;
    Ok(SessionDetail {
        session,
        models: distribution_for_session(conn, prices, Dimension::Model, session_id)?,
        tools: distribution_for_session(conn, prices, Dimension::Tool, session_id)?,
        skills: distribution_for_session(conn, prices, Dimension::Skill, session_id)?,
    })
}

fn distribution_for_session(conn: &Connection, prices: &PriceTable, dim: Dimension, session_id: &str) -> Result<Vec<Item>, Error> {
    let sql = match dim {
        Dimension::Model => "SELECT a.model, COUNT(*) FROM api_calls a WHERE a.session_id = ?1 GROUP BY a.model",
        Dimension::Tool => "SELECT t.tool_name, COUNT(*) FROM tool_calls t WHERE t.session_id = ?1 GROUP BY t.tool_name",
        _ => "SELECT c.name, COUNT(*) FROM command_invocations c WHERE c.session_id = ?1 AND c.kind = 'skill' GROUP BY c.name",
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([session_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    let mut items: Vec<Item> = rows.collect::<Result<Vec<_>, _>>()?.into_iter().map(|(k, n)| plain_item(k, n)).collect();
    if dim == Dimension::Model {
        for it in &mut items {
            let priced = prices.find(&it.key).is_some();
            it.priced = priced;
        }
    }
    items.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
    let total: i64 = items.iter().map(|i| i.count).sum();
    for it in &mut items {
        it.share = if total > 0 { it.count as f64 * 100.0 / total as f64 } else { 0.0 };
    }
    Ok(items)
}

/// Distinct project names and models for the filter pickers.
pub fn filter_options(conn: &Connection) -> Result<(Vec<String>, Vec<String>), Error> {
    let list = |sql: &str| -> Result<Vec<String>, Error> {
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    };
    Ok((
        list("SELECT DISTINCT project_name FROM sessions WHERE project_name IS NOT NULL ORDER BY 1")?,
        list("SELECT DISTINCT model FROM api_calls ORDER BY 1")?,
    ))
}
