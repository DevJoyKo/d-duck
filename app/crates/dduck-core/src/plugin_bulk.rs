//! Update several plugins in one go (plugin-bulk-update, D-203). The shell owns the whole batch: it plans from the
//! re-read CLI list (never the webview's copy), shows one native confirmation that names every target, re-checks each
//! target right before it runs, and runs them one at a time through the same `update` path as the single update.
//! Statuses are closed enums only: no CLI text, path or MCP setting can reach the webview through them.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::i18n::{Code, Params, tr, trp};
use crate::plugins::{Installed, Outcome, Scope, UpdateInfo, UpdateState, valid_id};

/// Ids the command accepts at most (the rest are dropped unread).
pub const MAX_INPUT: usize = 50;
/// Targets one batch runs at most: every one of them is named in the confirmation.
pub const MAX_RUN: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Skip {
    BadId,
    NotInstalled,
    /// Not a single global (user-scope) entry: project, local, managed, unknown scope, or ambiguous (D-083).
    Locked,
    Latest,
    /// Beyond [`MAX_RUN`]: not in the confirmation, so not run.
    TooMany,
    /// Differs from what the confirmation showed (scope, version, lastUpdated or the marketplace's new version).
    Changed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Status {
    Running,
    Finished { outcome: Outcome },
    Skipped { reason: Skip },
    /// Not run because the user pressed stop.
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub status: Status,
}

/// `confirmed: false` = nothing ran (the dialog was declined, or there was nothing to run).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkResult {
    pub confirmed: bool,
    pub items: Vec<Item>,
}

/// A plugin the confirmation names: the installed entry and the update state shown for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub plugin: Installed,
    pub update: UpdateState,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    pub run: Vec<Target>,
    pub skipped: Vec<Item>,
}

fn skipped(id: &str, reason: Skip) -> Item {
    Item { id: id.to_string(), status: Status::Skipped { reason } }
}

/// The one global entry for `id`; `None` when there is none or more than one.
fn global<'a>(installed: &'a [Installed], id: &str) -> Option<&'a Installed> {
    let mut it = installed.iter().filter(|p| p.id == id && p.scope == Scope::User);
    let first = it.next()?;
    it.next().is_none().then_some(first)
}

/// What runs and what is skipped, in the order asked (duplicates dropped, at most [`MAX_INPUT`] ids read).
pub fn plan(ids: &[String], installed: &[Installed], updates: &HashMap<String, UpdateInfo>) -> Plan {
    let mut seen = HashSet::new();
    let mut out = Plan::default();
    for id in ids.iter().take(MAX_INPUT) {
        if !seen.insert(id.as_str()) {
            continue;
        }
        if !valid_id(id) {
            // Never echo a malformed id back (it could be anything the webview sent).
            out.skipped.push(skipped("", Skip::BadId));
            continue;
        }
        if !installed.iter().any(|p| &p.id == id) {
            out.skipped.push(skipped(id, Skip::NotInstalled));
            continue;
        }
        let Some(p) = global(installed, id) else {
            out.skipped.push(skipped(id, Skip::Locked));
            continue;
        };
        let update = updates.get(id).map(|u| u.state.clone()).unwrap_or(UpdateState::Unknown);
        if update == UpdateState::Latest {
            out.skipped.push(skipped(id, Skip::Latest));
        } else if out.run.len() >= MAX_RUN {
            out.skipped.push(skipped(id, Skip::TooMany));
        } else {
            out.run.push(Target { plugin: p.clone(), update });
        }
    }
    out
}

/// Right before a target runs: `Some(Changed)` unless it is still the one global entry with the version,
/// lastUpdated and marketplace state the confirmation showed.
pub fn recheck(t: &Target, installed: &[Installed], updates: &HashMap<String, UpdateInfo>) -> Option<Skip> {
    let Some(now) = global(installed, &t.plugin.id) else { return Some(Skip::Changed) };
    let same = now.version == t.plugin.version
        && now.last_updated == t.plugin.last_updated
        && updates.get(&t.plugin.id).map(|u| &u.state).unwrap_or(&UpdateState::Unknown) == &t.update;
    (!same).then_some(Skip::Changed)
}

/// Runs targets one at a time. Before each: `stop()` ends the batch (the rest become `Stopped`), then `check` may
/// settle it without running (skipped as changed, or failed when the list can't be read). `update` runs one target; a failure there is just that row's outcome and the next one still runs.
/// `report` sees every status change in order.
pub fn run(
    targets: &[Target],
    stop: &dyn Fn() -> bool,
    check: &mut dyn FnMut(&Target) -> Option<Status>,
    update: &mut dyn FnMut(&Target) -> Outcome,
    report: &mut dyn FnMut(&Item),
) -> Vec<Item> {
    let mut out = Vec::with_capacity(targets.len());
    let push = |item: Item, out: &mut Vec<Item>, report: &mut dyn FnMut(&Item)| {
        report(&item);
        out.push(item);
    };
    for t in targets {
        let id = t.plugin.id.clone();
        if stop() {
            push(Item { id, status: Status::Stopped }, &mut out, report);
            continue;
        }
        if let Some(status) = check(t) {
            push(Item { id, status }, &mut out, report);
            continue;
        }
        report(&Item { id: id.clone(), status: Status::Running });
        let outcome = update(t);
        push(Item { id, status: Status::Finished { outcome } }, &mut out, report);
    }
    out
}

/// The "new version" text of the confirmation, shared with the single update.
pub fn target_text(state: Option<&UpdateState>, bulk: bool) -> String {
    match state {
        Some(UpdateState::Available { latest }) => latest.clone(),
        Some(UpdateState::Latest) => tr(Code::PluginLatestSame),
        _ if bulk => tr(Code::PluginBulkUnknown),
        _ => tr(Code::PluginLatestUnknown),
    }
}

/// Title and body of the one native confirmation. Every target gets its own line (there are at most [`MAX_RUN`]).
pub fn confirm_text(targets: &[Target], skipped: usize) -> (String, String) {
    let title = trp(Code::DlgPluginBulkTitle, &Params::new().num("count", targets.len() as i64));
    let mut lines = vec![tr(Code::DlgPluginBulkIntro), String::new()];
    let mut markets: Vec<&str> = Vec::new();
    for t in targets {
        let market = t.plugin.id.split_once('@').map(|(_, m)| m).unwrap_or("");
        if !markets.contains(&market) {
            markets.push(market);
        }
        let current = t.plugin.version.clone().unwrap_or_else(|| tr(Code::PluginNoVersion));
        lines.push(trp(
            Code::DlgPluginBulkItem,
            &Params::new()
                .text("id", &t.plugin.id, 201)
                .text("scope", &tr(Code::PlaceGlobalShort), 20)
                .text("current", &current, 40)
                .text("target", &target_text(Some(&t.update), true), 60),
        ));
    }
    if skipped > 0 {
        lines.push(String::new());
        lines.push(trp(Code::DlgPluginBulkSkipped, &Params::new().num("count", skipped as i64)));
    }
    lines.push(String::new());
    lines.push(trp(Code::DlgPluginBulkRisk, &Params::new().text("markets", &markets.join(", "), 1600)));
    (title, lines.join("\n"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    fn p(id: &str, version: Option<&str>, scope: Scope) -> Installed {
        Installed {
            id: id.into(),
            version: version.map(Into::into),
            scope,
            enabled: true,
            project_enabled: false,
            install_path: None,
            installed_at: None,
            last_updated: Some("t1".into()),
            mcp: vec![],
        }
    }

    fn info(state: UpdateState) -> UpdateInfo {
        UpdateInfo { state, marketplace_updated: None }
    }

    fn avail(v: &str) -> UpdateState {
        UpdateState::Available { latest: v.into() }
    }

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn fixture() -> (Vec<Installed>, HashMap<String, UpdateInfo>) {
        let installed = vec![
            p("a@m", Some("1.0.0"), Scope::User),
            p("b@m", Some("2.0.0"), Scope::User),
            p("unk@m", None, Scope::User),
            p("same@m", Some("1.0.0"), Scope::User),
            p("corp@internal", Some("2.0.0"), Scope::Managed),
            p("team@acme", Some("0.3.1"), Scope::Project),
            p("mine@acme", Some("0.1.0"), Scope::Local),
        ];
        let updates = HashMap::from([
            ("a@m".to_string(), info(avail("1.1.0"))),
            ("b@m".to_string(), info(avail("2.1.0"))),
            ("unk@m".to_string(), info(UpdateState::Unknown)),
            ("same@m".to_string(), info(UpdateState::Latest)),
            ("corp@internal".to_string(), info(avail("3.0.0"))),
            ("team@acme".to_string(), info(avail("0.4.0"))),
            ("mine@acme".to_string(), info(avail("0.2.0"))),
        ]);
        (installed, updates)
    }

    fn run_ids(plan: &Plan) -> Vec<&str> {
        plan.run.iter().map(|t| t.plugin.id.as_str()).collect()
    }

    #[test]
    fn plan_runs_available_and_unknown_global_plugins_in_the_order_asked() {
        let (installed, updates) = fixture();
        let plan = plan(&ids(&["unk@m", "b@m", "a@m", "b@m"]), &installed, &updates);
        assert_eq!(run_ids(&plan), ["unk@m", "b@m", "a@m"], "order kept, duplicate dropped");
        assert_eq!(plan.run[0].update, UpdateState::Unknown);
        assert_eq!(plan.run[1].update, avail("2.1.0"));
        assert!(plan.skipped.is_empty());
    }

    #[test]
    fn plan_skips_latest_locked_missing_and_bad_ids() {
        let (installed, updates) = fixture();
        let plan = plan(&ids(&["same@m", "corp@internal", "team@acme", "mine@acme", "gone@m", "--yes@m", "a@m"]), &installed, &updates);
        assert_eq!(run_ids(&plan), ["a@m"]);
        let reasons: Vec<(&str, Status)> = plan.skipped.iter().map(|i| (i.id.as_str(), i.status.clone())).collect();
        assert_eq!(
            reasons,
            [
                ("same@m", Status::Skipped { reason: Skip::Latest }),
                ("corp@internal", Status::Skipped { reason: Skip::Locked }),
                ("team@acme", Status::Skipped { reason: Skip::Locked }),
                ("mine@acme", Status::Skipped { reason: Skip::Locked }),
                ("gone@m", Status::Skipped { reason: Skip::NotInstalled }),
                ("", Status::Skipped { reason: Skip::BadId }),
            ]
        );
    }

    #[test]
    fn plan_treats_two_global_entries_of_one_id_as_locked() {
        let (mut installed, updates) = fixture();
        installed.push(p("a@m", Some("0.9.0"), Scope::User));
        let plan = plan(&ids(&["a@m"]), &installed, &updates);
        assert!(plan.run.is_empty());
        assert_eq!(plan.skipped[0].status, Status::Skipped { reason: Skip::Locked });
        // A project copy next to the one global entry is fine: the global one is picked.
        let (mut installed, updates) = fixture();
        installed.push(p("a@m", Some("0.9.0"), Scope::Project));
        let plan = super::plan(&ids(&["a@m"]), &installed, &updates);
        assert_eq!(plan.run[0].plugin.scope, Scope::User);
        assert_eq!(plan.run[0].plugin.version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn plan_runs_at_most_fifteen_and_reads_at_most_fifty_ids() {
        let installed: Vec<Installed> = (0..60).map(|i| p(&format!("p{i}@m"), Some("1.0.0"), Scope::User)).collect();
        let updates: HashMap<String, UpdateInfo> = installed.iter().map(|x| (x.id.clone(), info(avail("1.0.1")))).collect();
        let asked: Vec<String> = installed.iter().map(|x| x.id.clone()).collect();
        let plan = plan(&asked, &installed, &updates);
        assert_eq!(plan.run.len(), MAX_RUN);
        assert_eq!(plan.skipped.len(), MAX_INPUT - MAX_RUN, "ids past fifty are not read");
        assert!(plan.skipped.iter().all(|i| i.status == Status::Skipped { reason: Skip::TooMany }));
    }

    #[test]
    fn recheck_flags_anything_the_confirmation_did_not_show() {
        let (installed, updates) = fixture();
        let plan = plan(&ids(&["a@m"]), &installed, &updates);
        let t = &plan.run[0];
        assert_eq!(recheck(t, &installed, &updates), None);
        let mut moved = installed.clone();
        moved[0].version = Some("1.1.0".into());
        assert_eq!(recheck(t, &moved, &updates), Some(Skip::Changed), "updated elsewhere meanwhile");
        let mut touched = installed.clone();
        touched[0].last_updated = Some("t2".into());
        assert_eq!(recheck(t, &touched, &updates), Some(Skip::Changed));
        let mut newer = updates.clone();
        newer.insert("a@m".into(), info(avail("1.2.0")));
        assert_eq!(recheck(t, &installed, &newer), Some(Skip::Changed), "marketplace moved past what was shown");
        let mut scoped = installed.clone();
        scoped[0].scope = Scope::Project;
        assert_eq!(recheck(t, &scoped, &updates), Some(Skip::Changed), "no longer global");
        assert_eq!(recheck(t, &[], &updates), Some(Skip::Changed), "uninstalled");
    }

    fn targets(n: usize) -> Vec<Target> {
        (0..n).map(|i| Target { plugin: p(&format!("p{i}@m"), Some("1.0.0"), Scope::User), update: avail("1.0.1") }).collect()
    }

    #[test]
    fn runs_one_at_a_time_and_keeps_going_after_a_failure() {
        let ts = targets(3);
        let order = RefCell::new(Vec::new());
        let running = Cell::new(false);
        let mut update = |t: &Target| {
            assert!(!running.replace(true), "never two at once");
            order.borrow_mut().push(format!("run {}", t.plugin.id));
            running.set(false);
            match t.plugin.id.as_str() {
                "p1@m" => Outcome::Failed,
                _ => Outcome::Done,
            }
        };
        let mut report = |i: &Item| order.borrow_mut().push(format!("{} {:?}", i.id, i.status));
        let out = run(&ts, &|| false, &mut |_| None, &mut update, &mut report);
        assert_eq!(
            out.iter().map(|i| i.status.clone()).collect::<Vec<_>>(),
            [Status::Finished { outcome: Outcome::Done }, Status::Finished { outcome: Outcome::Failed }, Status::Finished { outcome: Outcome::Done }]
        );
        assert_eq!(
            order.into_inner(),
            [
                "p0@m Running", "run p0@m", "p0@m Finished { outcome: Done }",
                "p1@m Running", "run p1@m", "p1@m Finished { outcome: Failed }",
                "p2@m Running", "run p2@m", "p2@m Finished { outcome: Done }",
            ]
        );
    }

    #[test]
    fn needs_approval_and_timeout_do_not_stop_the_rest() {
        let ts = targets(3);
        let mut update = |t: &Target| match t.plugin.id.as_str() {
            "p0@m" => Outcome::NeedsApproval,
            "p1@m" => Outcome::Timeout,
            _ => Outcome::Done,
        };
        let out = run(&ts, &|| false, &mut |_| None, &mut update, &mut |_| {});
        assert_eq!(out[2].status, Status::Finished { outcome: Outcome::Done });
    }

    #[test]
    fn stop_finishes_the_current_one_and_runs_nothing_after() {
        let ts = targets(3);
        let stop = Cell::new(false);
        let ran = RefCell::new(Vec::new());
        let mut update = |t: &Target| {
            ran.borrow_mut().push(t.plugin.id.clone());
            stop.set(true); // the user presses stop while the first one is running
            Outcome::Done
        };
        let out = run(&ts, &|| stop.get(), &mut |_| None, &mut update, &mut |_| {});
        assert_eq!(ran.into_inner(), ["p0@m"]);
        assert_eq!(out.iter().map(|i| i.status.clone()).collect::<Vec<_>>(), [Status::Finished { outcome: Outcome::Done }, Status::Stopped, Status::Stopped]);
    }

    #[test]
    fn a_changed_target_is_skipped_without_running() {
        let ts = targets(2);
        let ran = RefCell::new(Vec::new());
        let mut check = |t: &Target| (t.plugin.id == "p0@m").then_some(Status::Skipped { reason: Skip::Changed });
        let mut update = |t: &Target| {
            ran.borrow_mut().push(t.plugin.id.clone());
            Outcome::Done
        };
        let out = run(&ts, &|| false, &mut check, &mut update, &mut |_| {});
        assert_eq!(ran.into_inner(), ["p1@m"]);
        assert_eq!(out[0].status, Status::Skipped { reason: Skip::Changed });
    }

    #[test]
    fn statuses_serialize_as_closed_codes_only() {
        let r = BulkResult {
            confirmed: true,
            items: vec![
                Item { id: "a@m".into(), status: Status::Finished { outcome: Outcome::Failed } },
                Item { id: "b@m".into(), status: Status::Skipped { reason: Skip::TooMany } },
                Item { id: "c@m".into(), status: Status::Stopped },
            ],
        };
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(
            json,
            r#"{"confirmed":true,"items":[{"id":"a@m","status":{"kind":"finished","outcome":{"kind":"failed"}}},{"id":"b@m","status":{"kind":"skipped","reason":"tooMany"}},{"id":"c@m","status":{"kind":"stopped"}}]}"#
        );
        assert_eq!(serde_json::to_string(&Item { id: "a@m".into(), status: Status::Running }).unwrap(), r#"{"id":"a@m","status":{"kind":"running"}}"#);
    }

    #[test]
    fn confirmation_names_every_target_with_scope_and_versions() {
        let (installed, updates) = fixture();
        let plan = plan(&ids(&["a@m", "unk@m", "same@m"]), &installed, &updates);
        let (title, body) = confirm_text(&plan.run, plan.skipped.len());
        assert_eq!(title, "플러그인 2개 업데이트");
        assert!(body.contains("· a@m (전역) 1.0.0 → 1.1.0"), "{body}");
        assert!(body.contains("· unk@m (전역) 버전 없음 → 새 버전 확인 불가(최신일 수 있음)"), "{body}");
        assert!(body.contains("1개는 건너뛰어요"), "{body}");
        assert!(body.contains("(m)") && body.contains("내 권한으로 실행돼요") && body.contains("실행하지 않고 터미널 안내만"), "{body}");
        assert!(!body.contains("same@m"));
    }

    /// Fake CLI: every target goes through `plugin update --json -s user <id>`, one process at a time, each with its
    /// own time limit, never with `-y` or `--accept-command`.
    #[cfg(unix)]
    #[test]
    fn each_target_runs_the_single_update_command() {
        use crate::claude_cli::ClaudeCli;
        use crate::plugins::Action;
        use std::os::unix::fs::PermissionsExt;
        use std::time::{Duration, Instant};
        let tmp = tempfile::tempdir().unwrap();
        let log = tmp.path().join("argv");
        let bin = tmp.path().join("claude");
        let script = format!(
            "#!/bin/sh\necho \"start $6\" >> '{log}'\nprintf '%s\\n' \"$@\" >> '{log}'\ncat >/dev/null\n\
             case \"$6\" in\n\
               slow@m) sleep 20;;\n\
               bad@m) echo boom >&2; exit 1;;\n\
               needs@m) echo '{{\"ok\":false,\"shownCommand\":{{\"command\":\"curl x\"}}}}'; exit 1;;\n\
               *) echo '{{\"ok\":true}}';;\n\
             esac\necho \"end $6\" >> '{log}'\n",
            log = log.display()
        );
        std::fs::write(&bin, script).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let cli = ClaudeCli::new(bin, None);
        let ts: Vec<Target> = ["a@m", "bad@m", "slow@m", "needs@m", "b@m"]
            .iter()
            .map(|id| Target { plugin: p(id, Some("1.0.0"), Scope::User), update: avail("1.0.1") })
            .collect();
        let t0 = Instant::now();
        let mut update = |t: &Target| {
            cli.plugin_action_within(tmp.path(), Action::Update, &t.plugin.id, t.plugin.scope, Duration::from_secs(3)).unwrap_or(Outcome::Failed)
        };
        let out = run(&ts, &|| false, &mut |_| None, &mut update, &mut |_| {});
        assert!(t0.elapsed() < Duration::from_secs(15), "a slow one is cut at its own limit");
        let got: Vec<Status> = out.into_iter().map(|i| i.status).collect();
        assert_eq!(
            got,
            [Outcome::Done, Outcome::Failed, Outcome::Timeout, Outcome::NeedsApproval, Outcome::Done].map(|outcome| Status::Finished { outcome })
        );
        let argv = std::fs::read_to_string(&log).unwrap();
        for id in ["a@m", "bad@m", "slow@m", "needs@m", "b@m"] {
            assert!(argv.contains(&format!("plugin\nupdate\n--json\n-s\nuser\n{id}\n")), "{argv}");
        }
        assert!(!argv.lines().any(|l| l == "-y" || l == "--yes" || l.starts_with("--accept-command")), "{argv}");
        // One at a time: every start is followed by its own end before the next start (slow@m was killed, no end).
        let marks: Vec<&str> = argv.lines().filter(|l| l.starts_with("start ") || l.starts_with("end ")).collect();
        assert_eq!(marks, ["start a@m", "end a@m", "start bad@m", "start slow@m", "start needs@m", "start b@m", "end b@m"], "{argv}");
    }
}
