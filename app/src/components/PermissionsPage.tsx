import { keepPreviousData, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Plus, ShieldCheck, X } from "lucide-react";
import { useDeferredValue, useState } from "react";
import { api, reasonIs, failure, later, messageOf, shown as shownLine, type Shown } from "../lib/api";
import {
  applyChanges,
  changeCount,
  checkRule,
  customRules,
  editOf,
  hasHidden,
  LIST_LABEL,
  LISTS,
  MODE_HINT,
  MODE_LABEL,
  parseJson,
  permApi,
  recommendedDiff,
  recommendedFor,
  RISK_LABEL,
  ruleWarnings,
  sameEdit,
  setToggle,
  shown,
  stateOf,
  toJson,
  type Catalog,
  type Explanation,
  type ListName,
  type PermAgent,
  type PermEdit,
  type PermissionScopes,
  type RecChange,
  type Risk,
  type RowEffect,
  type ScopeView,
  type SettingsScope,
  type Toggle,
  scopeLabel,
} from "../lib/permissions";
import { Button, Empty, PageBody, PageHeader, Segmented, Select, Sheet, fieldClass } from "./ui";
import { useDirty, useScope } from "./ProjectContext";
import { ScopeBar } from "./ScopeBar";
import { t } from "../i18n";
import { useT } from "../i18n/react";

type Tab = "rules" | "agent";
const tabs = (): { id: Tab; label: string }[] => [
  { id: "rules", label: t("agents.perm.tabRules") },
  { id: "agent", label: t("agents.perm.tabAgent") },
];
type ProjectScope = "shared" | "local";
const projectScopes = (): { id: ProjectScope; label: string }[] => [
  { id: "shared", label: t("agents.perm.shared") },
  { id: "local", label: t("agents.perm.local") },
];
const scopeHint = (s: SettingsScope) => t(s === "user" ? "agents.perm.hintUser" : s === "shared" ? "agents.perm.hintShared" : "agents.perm.hintLocal");
const RISK_CLS: Record<Risk, string> = { high: "bg-danger/12 text-danger", medium: "bg-warn-soft text-ink", low: "bg-ok/10 text-ok" };
const TOGGLE_CLS: Record<Toggle, string> = { allow: "bg-ok text-paper", ask: "bg-warn text-paper", deny: "bg-danger text-paper", none: "bg-ink-3/25 text-ink" };
const DECISION_CLS: Record<Explanation["decision"], string> = {
  allow: "bg-ok text-paper",
  ask: "bg-warn text-paper",
  deny: "bg-danger text-paper",
  none: "bg-ink-3/25 text-ink",
  unavailable: "bg-ink-3/25 text-ink",
};
const decisionLabel = (d: Explanation["decision"]) =>
  d === "none" ? t("agents.perm.byMode") : d === "unavailable" ? t("agents.perm.noTool") : t(`agents.perm.list.${d}`);
const baseName = (p: string) => p.split("/").filter(Boolean).pop() ?? p;
const asToggle = (d: Explanation["decision"]): Toggle => (d === "allow" || d === "ask" || d === "deny" ? d : "none");

function RiskBadge({ risk }: { risk: Risk }) {
  const tr = useT();
  return <span className={`inline-flex h-5 shrink-0 items-center rounded px-1.5 text-[11px] font-semibold ${RISK_CLS[risk]}`}>{tr("agents.perm.riskBadge", { level: RISK_LABEL[risk] })}</span>;
}

/** Four-way radio group: roving tabindex and arrow keys, like `Segmented`. */
function FourWay({ value, onChange, disabled, label, recommended, blocked }: { value: Toggle; onChange: (t: Toggle) => void; disabled: boolean; label: string; recommended: Toggle; blocked: Toggle[] }) {
  const tr = useT();
  const opts: Toggle[] = ["allow", "ask", "deny", "none"];
  const usable = opts.filter((o) => !blocked.includes(o));
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className="inline-flex shrink-0 rounded-lg border border-line bg-sheet p-0.5"
      onKeyDown={(e) => {
        if (disabled || (e.key !== "ArrowRight" && e.key !== "ArrowLeft")) return;
        e.preventDefault();
        const i = Math.max(0, usable.indexOf(value));
        const next = usable[(i + (e.key === "ArrowRight" ? 1 : usable.length - 1)) % usable.length];
        onChange(next);
        (e.currentTarget.querySelector(`[data-id="${next}"]`) as HTMLElement | null)?.focus();
      }}
    >
      {opts.map((o) => (
        <button
          key={o}
          data-id={o}
          role="radio"
          aria-checked={value === o}
          tabIndex={value === o ? 0 : -1}
          disabled={disabled || blocked.includes(o)}
          title={blocked.includes(o) ? tr("agents.perm.ignoredHere") : o === recommended ? tr("agents.perm.recommended") : undefined}
          onClick={() => onChange(o)}
          className={`relative h-6 min-w-10 rounded-md px-2 text-xs transition-colors disabled:cursor-not-allowed ${value === o ? `${TOGGLE_CLS[o]} font-semibold` : "text-ink-2 hover:text-ink disabled:opacity-40"}`}
        >
          {LIST_LABEL[o]}
          {o === recommended && <span aria-label={tr("agents.perm.recommendedShort")} className="absolute top-0.5 right-0.5 h-1.5 w-1.5 rounded-full bg-ink ring-1 ring-sheet" />}
        </button>
      ))}
    </div>
  );
}

function effectHint(e: RowEffect | undefined, state: Toggle, rule: string): string | null {
  if (!e) return null;
  const actual = asToggle(e.decision);
  const own = e.byRule === rule;
  if (actual === state && (state === "none" || own)) return null;
  if (!e.byRule) return e.decision === "allow" ? t("agents.perm.actualReadOnly") : t("agents.perm.actualNoRule");
  return t("agents.perm.actual", { decision: decisionLabel(e.decision), rule: shown(e.byRule, 80), scope: scopeLabel(e.byScopeCode, e.byScope) });
}

function CatalogList({ catalog, edit, scope, effects, readOnly, onChange }: { catalog: Catalog; edit: PermEdit; scope: SettingsScope; effects: Map<string, RowEffect>; readOnly: boolean; onChange: (e: PermEdit) => void }) {
  const tr = useT();
  return (
    <Sheet className="divide-y divide-line">
      <p className="px-4 py-2 text-[11px] text-ink-3">
        <span className="mr-1 inline-block h-1.5 w-1.5 rounded-full bg-ink align-middle" /> {tr("agents.perm.legend")}
      </p>
      {catalog.groups.map((g) => {
        const items = catalog.items.filter((i) => i.group === g.id);
        if (!items.length) return null;
        return (
          <section key={g.id} className="px-4 py-3">
            <h3 className="mb-1.5 text-xs font-semibold text-ink-3">{g.label}</h3>
            <ul className="space-y-1">
              {items.map((i) => {
                const state = stateOf(edit, i.rule);
                const hint = effectHint(effects.get(i.rule), state, i.rule);
                const blocked: Toggle[] = ruleWarnings(i.rule, "allow").length ? ["allow"] : [];
                return (
                  <li key={i.rule} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-lg py-1.5">
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <code className="truncate font-mono text-[12px] font-medium">{i.rule}</code>
                        <RiskBadge risk={i.risk} />
                      </div>
                      <p className="mt-0.5 truncate text-xs text-ink-3" title={i.reason}>
                        {i.description}
                        {i.reason && ` · ${i.reason}`}
                      </p>
                      {hint && <p className={`mt-0.5 text-[11px] font-medium ${effects.get(i.rule)?.decision === "allow" && i.risk === "high" ? "text-danger" : "text-ink-2"}`}>{hint}</p>}
                      {state === "allow" && i.risk === "high" && <p className="mt-0.5 text-[11px] text-danger">{tr("agents.perm.highAllowed")}</p>}
                    </div>
                    <FourWay value={state} recommended={recommendedFor(i, scope)} blocked={blocked} disabled={readOnly} label={tr("agents.perm.ruleToggle", { rule: i.rule })} onChange={(t) => onChange(setToggle(edit, i.rule, t))} />
                  </li>
                );
              })}
            </ul>
          </section>
        );
      })}
    </Sheet>
  );
}

function CustomRules({ catalog, edit, base, readOnly, onChange }: { catalog: Catalog; edit: PermEdit; base: ScopeView; readOnly: boolean; onChange: (e: PermEdit) => void }) {
  const tr = useT();
  const [tab, setTab] = useState<ListName>("allow");
  const [draft, setDraft] = useState("");
  const [target, setTarget] = useState<ListName>("deny");
  const rules = customRules(edit, catalog);
  const rule = draft.trim();
  const bad = rule ? checkRule(rule) : null;
  const warns = rule && !bad ? ruleWarnings(rule, target) : [];
  const dup = rule !== "" && stateOf(edit, rule) !== "none";
  const deferred = useDeferredValue(rule);
  const covers = useQuery({ queryKey: ["permCovers", deferred], queryFn: () => permApi.ruleCovers(deferred), enabled: target === "allow" && !!deferred && !checkRule(deferred) });
  const covered = target === "allow" && deferred === rule ? (covers.data ?? []) : [];
  const add = () => {
    if (!rule || bad || dup) return;
    onChange(setToggle(edit, rule, target));
    setTab(target);
    setDraft("");
  };
  const mine = rules.filter((r) => r.list === tab);
  return (
    <Sheet className="p-4">
      <div className="mb-3 flex flex-wrap items-baseline gap-3">
        <h2 className="text-[13px] font-semibold">{tr("agents.perm.custom")}</h2>
        <span className="text-xs text-ink-3">{tr("agents.perm.customNote")}</span>
      </div>
      <div className="mb-3">
        <Segmented label={tr("agents.perm.ruleKind")} value={tab} onChange={setTab} options={LISTS.map((l) => ({ id: l, label: tr("agents.perm.listCount", { list: LIST_LABEL[l], count: rules.filter((r) => r.list === l).length }) }))} />
      </div>
      {mine.length === 0 ? (
        <p className="rounded-lg border border-dashed border-line px-3 py-3 text-xs text-ink-3">{tr("agents.perm.none")}</p>
      ) : (
        <ul className="grid gap-1 lg:grid-cols-2">
          {mine.map(({ rule: r }) => {
            const unknown = checkRule(r) !== null;
            const w = ruleWarnings(r, tab);
            const fresh = !base[tab].includes(r);
            return (
              <li key={r} className="rounded-lg border border-line px-2.5 py-1.5">
                <div className="flex items-center gap-1.5">
                  <code className="min-w-0 flex-1 truncate font-mono text-[12px]" title={shown(r, 1000)}>
                    {shown(r, 160)}
                  </code>
                  <select aria-label={tr("agents.perm.moveRule", { rule: shown(r, 60) })} disabled={readOnly} value={tab} onChange={(e) => onChange(setToggle(edit, r, e.target.value as ListName))} className="h-6 rounded border border-line bg-sheet px-1 text-[11px]">
                    {LISTS.map((x) => (
                      <option key={x} value={x}>
                        {LIST_LABEL[x]}
                      </option>
                    ))}
                  </select>
                  <button aria-label={tr("agents.perm.deleteRule", { rule: shown(r, 60) })} disabled={readOnly} onClick={() => onChange(setToggle(edit, r, "none"))} className="grid h-6 w-6 place-items-center rounded text-ink-3 hover:text-danger disabled:opacity-40">
                    <X size={13} />
                  </button>
                </div>
                {(unknown || w.length > 0 || fresh || hasHidden(r)) && (
                  <div className="mt-1 flex flex-wrap gap-1.5 text-[11px]">
                    {unknown && <span className="rounded bg-ink-3/15 px-1.5 text-ink-2">{tr("agents.perm.unknown")}</span>}
                    {hasHidden(r) && <span className="rounded bg-danger/12 px-1.5 text-danger">{tr("agents.perm.hiddenChars")}</span>}
                    {fresh && <span className="rounded bg-warn-soft px-1.5 text-ink">{tr("agents.perm.unsaved")}</span>}
                    {w.map((x) => (
                      <span key={x} className="text-danger">
                        {x}
                      </span>
                    ))}
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {!readOnly && (
        <div className="mt-4 border-t border-line pt-3">
          <div className="flex flex-wrap items-center gap-2">
            <input
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && add()}
              placeholder="Bash(npm run lint:*) · Read(./secrets/**) · WebFetch(domain:docs.rs) · mcp__notion"
              aria-label={tr("agents.perm.newRule")}
              spellCheck={false}
              className={`${fieldClass} h-8 min-w-0 flex-1 py-0 font-mono text-[12px]`}
            />
            <Select label="" aria-label={tr("agents.perm.target")} value={target} onChange={(e) => setTarget(e.target.value as ListName)}>
              {LISTS.map((x) => (
                <option key={x} value={x}>
                  {LIST_LABEL[x]}
                </option>
              ))}
            </Select>
            <Button onClick={add} disabled={!rule || !!bad || dup}>
              <Plus size={14} /> {tr("agents.perm.add")}
            </Button>
          </div>
          {(bad || dup || warns.length > 0) && <p className={`mt-1.5 text-xs ${bad || dup ? "text-danger" : "text-ink-2"}`}>{bad ?? (dup ? tr("agents.perm.duplicate") : warns.join(" · "))}</p>}
          {covered.length > 0 && (
            <p className="mt-1.5 flex items-start gap-1.5 text-xs text-danger">
              <AlertTriangle size={13} className="mt-0.5 shrink-0" /> {tr("agents.perm.covers", { rules: covered.join(", ") })}
            </p>
          )}
          <details className="mt-1.5 text-[11px] text-ink-3">
            <summary className="cursor-pointer select-none">{tr("agents.perm.pathSyntax")}</summary>
            <p className="mt-1">
              {tr("agents.perm.pathSyntaxMore")}
            </p>
          </details>
        </div>
      )}
    </Sheet>
  );
}

function JsonPanel({ edit, base, readOnly, onChange }: { edit: PermEdit; base: ScopeView; readOnly: boolean; onChange: (e: PermEdit) => void }) {
  const tr = useT();
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const preview = toJson(edit, base);
  // Two-way: while the text is valid, a toggle elsewhere replaces it; invalid text is kept until fixed.
  const parsed = editing && !error ? parseJson(text, base) : null;
  const shownText = !editing ? preview : parsed && "edit" in parsed && !sameEdit(parsed.edit, edit) ? preview : text;
  return (
    <Sheet className="flex flex-col overflow-hidden">
      <div className="flex items-center gap-2 border-b border-line px-4 py-2.5">
        <h2 className="flex-1 text-[13px] font-semibold">permissions JSON</h2>
        {!readOnly && (
          <Button
            onClick={() => {
              setEditing(!editing);
              setText(preview);
              setError(null);
            }}
          >
            {editing ? tr("agents.perm.toPreview") : tr("agents.perm.editJson")}
          </Button>
        )}
      </div>
      {editing ? (
        <textarea
          aria-label={tr("agents.perm.jsonEditor")}
          value={shownText}
          spellCheck={false}
          onChange={(e) => {
            setText(e.target.value);
            const r = parseJson(e.target.value, base);
            if ("error" in r) setError(r.error);
            else {
              setError(null);
              onChange(r.edit);
            }
          }}
          className="h-[420px] w-full resize-y bg-sheet px-4 py-3 font-mono text-[12px] leading-5 text-ink outline-none"
        />
      ) : (
        <pre className="max-h-[520px] overflow-auto px-4 py-3 font-mono text-[12px] leading-5 text-ink-2">{shownText}</pre>
      )}
      <div className="border-t border-line px-4 py-2 text-[11px]">
        {error ? (
          <p role="alert" className="text-danger">
            {tr("agents.perm.keepLast", { error })}
          </p>
        ) : (
          <p className="text-ink-3">
            {tr("agents.perm.onlyPermissions")}
            {base.other.length ? tr("agents.perm.otherReadOnly", { keys: base.other.map(([k]) => shown(k, 40)).join(", ") }) : ""}
          </p>
        )}
      </div>
    </Sheet>
  );
}

function RecommendPreview({ changes, onApply, onCancel }: { changes: RecChange[]; onApply: () => void; onCancel: () => void }) {
  const tr = useT();
  return (
    <Sheet className="bg-warn-soft p-4">
      <h2 className="text-[13px] font-semibold">{tr("agents.perm.applyTitle", { count: changes.length })}</h2>
      {changes.length === 0 ? (
        <p className="mt-2 text-xs text-ink-3">{tr("agents.perm.allRecommended")}</p>
      ) : (
        <ul className="mt-2 max-h-56 space-y-1 overflow-auto text-xs">
          {changes.map((c) => (
            <li key={c.rule} className="flex items-center gap-2">
              <code className="min-w-0 flex-1 truncate font-mono text-[12px]">{c.rule}</code>
              <RiskBadge risk={c.risk} />
              <span className="text-ink-3">{LIST_LABEL[c.from]}</span>→<span className="font-semibold">{LIST_LABEL[c.to]}</span>
            </li>
          ))}
        </ul>
      )}
      <p className="mt-2 text-[11px] text-ink-3">{tr("agents.perm.applyNote")}</p>
      <div className="mt-3 flex gap-2">
        <Button variant="primary" onClick={onApply} disabled={changes.length === 0}>
          {tr("agents.perm.apply")}
        </Button>
        <Button onClick={onCancel}>{tr("agents.perm.cancel")}</Button>
      </div>
    </Sheet>
  );
}

function ScopeEditor({ data, view, scope, onSaved }: { data: PermissionScopes; view: ScopeView; scope: SettingsScope; onSaved: () => void }) {
  const tr = useT();
  const base = editOf(view);
  const [edit, setEdit] = useState<PermEdit>(base);
  const [rec, setRec] = useState<RecChange[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: Shown } | null>(null);
  const readOnly = view.readonlyReason !== null;
  const changes = changeCount(base, edit);
  useDirty("permissions", changes > 0);
  const deferredEdit = useDeferredValue(edit);
  const eff = useQuery({
    queryKey: ["permEffective", data.project, scope, JSON.stringify(deferredEdit)],
    queryFn: () => permApi.effective(data.project, scope, deferredEdit),
    placeholderData: keepPreviousData,
  });
  const effects = new Map((eff.data?.rows ?? []).map((r) => [r.rule, r]));
  const change = (e: PermEdit) => {
    setEdit(e);
    setMsg(null);
  };
  const save = async () => {
    setBusy(true);
    setMsg(null);
    try {
      await permApi.save(scope, data.project, edit, view.token);
      setMsg({ ok: true, text: later(() => tr("agents.perm.saved")) });
      onSaved();
    } catch (e) {
      setMsg({ ok: false, text: failure(e) });
    } finally {
      setBusy(false);
    }
  };
  const current = edit.defaultMode;
  const modes = !current || data.modes.includes(current) ? data.modes : [...data.modes, current];
  return (
    <div className="space-y-4">
      <Sheet className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
        <div className="min-w-0 flex-1">
          <p className="truncate font-mono text-[12px]" title={view.path}>
            {view.path}
          </p>
          <p className="text-xs text-ink-3">{scopeHint(scope)}</p>
          <p className="text-xs text-ink-3">
            {!view.exists ? tr("agents.perm.noFile") : view.readonlyReason ? <span className="text-danger">{view.readonlyReason}</span> : tr("agents.perm.counts", { allow: view.allow.length, ask: view.ask.length, deny: view.deny.length })}
            {view.error && <span className="text-danger"> · {view.error}</span>}
          </p>
        </div>
        <Select label={tr("agents.perm.defaultMode")} className="max-w-56" value={current ?? ""} disabled={readOnly} onChange={(e) => change({ ...edit, defaultMode: e.target.value || null })}>
          <option value="">{tr("agents.perm.unset")}</option>
          {modes.map((m) => (
            <option key={m} value={m} disabled={!data.modes.includes(m)}>
              {MODE_LABEL[m] ?? m}
              {!data.modes.includes(m) ? tr("agents.perm.notHere") : ""}
            </option>
          ))}
        </Select>
        <Button disabled={readOnly} onClick={() => setRec(recommendedDiff(edit, data.catalog, scope))}>
          {tr("agents.perm.applyRecommended")}
        </Button>
        {current && MODE_HINT[current] && (
          <p className="flex w-full items-center gap-1.5 text-xs text-danger">
            <AlertTriangle size={13} /> {MODE_LABEL[current]}: {MODE_HINT[current]}
          </p>
        )}
      </Sheet>
      {rec && (
        <RecommendPreview
          changes={rec}
          onCancel={() => setRec(null)}
          onApply={() => {
            change(applyChanges(edit, rec));
            setRec(null);
          }}
        />
      )}
      <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_340px]">
        <div className="min-w-0 space-y-4">
          <CatalogList catalog={data.catalog} edit={edit} scope={scope} effects={effects} readOnly={readOnly} onChange={change} />
          <CustomRules catalog={data.catalog} edit={edit} base={view} readOnly={readOnly} onChange={change} />
        </div>
        <div className="xl:sticky xl:top-20">
          <JsonPanel edit={edit} base={view} readOnly={readOnly} onChange={change} />
        </div>
      </div>
      {(changes > 0 || msg) && (
        <div className="sticky bottom-3 z-10">
          <Sheet className="flex flex-wrap items-center gap-3 px-4 py-2.5 shadow-e3">
            <span className="flex-1 text-[13px]">
              {msg ? (
                <span role={msg.ok ? "status" : "alert"} className={msg.ok ? "text-ok" : "text-danger"}>
                  {shownLine(msg.text)}
                </span>
              ) : (
                <>
                  <span className="font-semibold">{tr("agents.perm.changes", { count: changes })}</span> <span className="text-xs text-ink-3">{tr("agents.perm.untilSaved")}</span>
                </>
              )}
            </span>
            {msg && !msg.ok && ["fileChanged", "changedOutside", "listChanged"].some((c) => reasonIs(shownLine(msg.text), c)) && <Button onClick={onSaved}>{tr("agents.perm.reload")}</Button>}
            <Button disabled={changes === 0 || busy} onClick={() => change(base)}>
              {tr("agents.perm.revert")}
            </Button>
            <Button variant="primary" disabled={changes === 0 || busy || readOnly} onClick={save}>
              {busy ? tr("agents.perm.confirming") : tr("agents.perm.save")}
            </Button>
          </Sheet>
        </div>
      )}
    </div>
  );
}

const QUICK_TOOLS: [string, Risk][] = [
  ["Read", "low"],
  ["Grep", "low"],
  ["Glob", "low"],
  ["WebSearch", "low"],
  ["Agent", "low"],
  ["Edit", "medium"],
  ["Write", "medium"],
  ["WebFetch", "medium"],
  ["Bash", "high"],
];
const CHIP_CLS: Record<Risk, string> = { low: "border-ok/40 text-ok", medium: "border-warn/50 text-warn", high: "border-danger/50 text-danger" };

function ToolList({ title, hint, items, readOnly, onChange }: { title: string; hint: string; items: string[]; readOnly: boolean; onChange: (v: string[]) => void }) {
  const tr = useT();
  const [draft, setDraft] = useState("");
  const forbidden = /[,#"'[\]]/;
  const bad = draft.trim() ? (forbidden.test(draft) ? tr("agents.perm.forbidden") : checkRule(draft.trim())) : null;
  const add = (t: string) => {
    if (!t || items.includes(t) || checkRule(t) || forbidden.test(t)) return;
    onChange([...items, t]);
    setDraft("");
  };
  return (
    <div>
      <h3 className="text-[13px] font-semibold">{title}</h3>
      <p className="mb-2 text-xs text-ink-3">{hint}</p>
      <div className="flex min-h-9 flex-wrap gap-1.5 rounded-lg border border-line p-1.5">
        {items.length === 0 && <span className="px-1.5 py-0.5 text-xs text-ink-3">{tr("agents.perm.empty")}</span>}
        {items.map((t) => (
          <span key={t} title={shown(t, 1000)} className="inline-flex max-w-full items-center gap-1 rounded-md bg-ink-3/15 py-0.5 pr-1 pl-2 font-mono text-[12px]">
            <span className="truncate">{shown(t, 80)}</span>
            {!readOnly && (
              <button aria-label={tr("agents.perm.remove", { tool: shown(t, 60) })} onClick={() => onChange(items.filter((x) => x !== t))} className="text-ink-3 hover:text-danger">
                <X size={12} />
              </button>
            )}
          </span>
        ))}
      </div>
      {!readOnly && (
        <>
          <div className="mt-2 flex flex-wrap gap-1">
            {QUICK_TOOLS.filter(([t]) => !items.includes(t)).map(([t, risk]) => (
              <button key={t} onClick={() => add(t)} title={tr("agents.perm.riskBadge", { level: RISK_LABEL[risk] })} className={`h-6 rounded-md border px-2 font-mono text-[11px] hover:bg-ink/5 ${CHIP_CLS[risk]}`}>
                + {t}
              </button>
            ))}
          </div>
          <div className="mt-2 flex gap-2">
            <input value={draft} onChange={(e) => setDraft(e.target.value)} onKeyDown={(e) => e.key === "Enter" && add(draft.trim())} placeholder="mcp__notion · Bash(git push:*)" spellCheck={false} aria-label={tr("agents.perm.addTo", { title })} className={`${fieldClass} h-8 py-0 font-mono text-[12px]`} />
            <Button disabled={!draft.trim() || !!bad} onClick={() => add(draft.trim())}>
              {tr("agents.perm.add")}
            </Button>
          </div>
          {bad && <p className="mt-1 text-xs text-danger">{bad}</p>}
        </>
      )}
    </div>
  );
}

function AgentEditor({ agent, project, onSaved }: { agent: PermAgent; project: string | null; onSaved: () => void }) {
  const tr = useT();
  const [tools, setTools] = useState(agent.toolsRaw);
  const [disallowed, setDisallowed] = useState(agent.disallowedRaw);
  const [msg, setMsg] = useState<{ ok: boolean; text: Shown } | null>(null);
  const [busy, setBusy] = useState(false);
  const def = useQuery({ queryKey: ["agentDefinition", agent.id], queryFn: () => api.agentDefinition(agent.id), enabled: agent.editable });
  const readOnly = !agent.editable;
  const dirty = JSON.stringify([tools, disallowed]) !== JSON.stringify([agent.toolsRaw, agent.disallowedRaw]);
  useDirty("agent-tools", dirty);
  const save = async () => {
    if (!def.data) return;
    setBusy(true);
    setMsg(null);
    try {
      await permApi.saveAgentTools(agent.id, project, tools, disallowed, def.data.token);
      setMsg({ ok: true, text: later(() => tr("agents.perm.saved")) });
      onSaved();
    } catch (e) {
      setMsg({ ok: false, text: failure(e) });
    } finally {
      setBusy(false);
    }
  };
  return (
    <Sheet className="space-y-5 p-4">
      <div className="flex flex-wrap items-center gap-3">
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-semibold">{agent.name}</p>
          <p className="truncate font-mono text-[11px] text-ink-3" title={agent.path ?? ""}>
            {agent.path}
          </p>
          {agent.readonlyReason && <p className="text-xs text-danger">{agent.readonlyReason}</p>}
        </div>
        <Button
          disabled={!dirty || busy}
          onClick={() => {
            setTools(agent.toolsRaw);
            setDisallowed(agent.disallowedRaw);
          }}
        >
          {tr("agents.perm.revert")}
        </Button>
        <Button variant="primary" disabled={readOnly || !dirty || busy || !def.data} onClick={save}>
          {busy ? tr("agents.perm.confirming") : tr("agents.perm.save")}
        </Button>
      </div>
      {msg && <p className={`text-xs ${msg.ok ? "text-ok" : "text-danger"}`}>{shownLine(msg.text)}</p>}
      <div className="grid gap-5 lg:grid-cols-2">
        <ToolList title={tr("agents.perm.toolsTitle")} hint={tr("agents.perm.toolsHint")} items={tools} readOnly={readOnly} onChange={setTools} />
        <ToolList title={tr("agents.perm.disallowedTitle")} hint={tr("agents.perm.disallowedHint")} items={disallowed} readOnly={readOnly} onChange={setDisallowed} />
      </div>
      <p className="text-[11px] text-ink-3">{tr("agents.perm.agentNote")}</p>
    </Sheet>
  );
}

function AgentScope({ data, onSaved }: { data: PermissionScopes; onSaved: () => void }) {
  const tr = useT();
  const [id, setId] = useState<string | null>(data.agents[0]?.id ?? null);
  const agent = data.agents.find((a) => a.id === id) ?? data.agents[0];
  if (!agent) return <Empty title={tr("agents.perm.noAgents")}>{tr("agents.perm.noAgentsMore")}</Empty>;
  const where = (a: PermAgent) => (a.source === "project" ? tr("agents.perm.whereProject", { name: baseName(a.scope ?? "") }) : a.source === "user" ? tr("agents.perm.whereUser") : tr("agents.perm.wherePlugin"));
  return (
    <div className="grid items-start gap-4 lg:grid-cols-[240px_minmax(0,1fr)]">
      <Sheet className="p-1.5">
        <ul aria-label={tr("agents.perm.agents")}>
          {data.agents.map((a) => (
            <li key={a.id}>
              <button onClick={() => setId(a.id)} aria-current={a.id === agent.id ? "true" : undefined} className={`w-full rounded-lg px-3 py-2 text-left ${a.id === agent.id ? "bg-ink/8 font-medium" : "hover:bg-ink/5"}`}>
                <span className="block truncate text-[13px]">{a.name}</span>
                <span className="block text-[11px] text-ink-3">
                  {where(a)}
                  {a.toolsRaw.length ? tr("agents.perm.toolsOnly", { count: a.toolsRaw.length }) : tr("agents.perm.inheritAll")}
                  {a.disallowedRaw.length ? tr("agents.perm.disallowedCount", { count: a.disallowedRaw.length }) : ""}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </Sheet>
      <AgentEditor key={`${agent.id}|${agent.toolsRaw.join()}|${agent.disallowedRaw.join()}`} agent={agent} project={data.project} onSaved={onSaved} />
    </div>
  );
}

function Explainer({ data }: { data: PermissionScopes }) {
  const tr = useT();
  const [call, setCall] = useState("Bash(git push origin main)");
  const [agentId, setAgentId] = useState("");
  const [result, setResult] = useState<Explanation | null>(null);
  const [error, setError] = useState<Shown | null>(null);
  const run = async () => {
    setError(null);
    try {
      setResult(await permApi.explain(data.project, agentId || null, call));
    } catch (e) {
      setError(failure(e));
      setResult(null);
    }
  };
  const reset = () => {
    setResult(null);
    setError(null);
  };
  return (
    <Sheet className="p-4">
      <div className="mb-3 flex flex-wrap items-baseline gap-3">
        <h2 className="text-[13px] font-semibold">{tr("agents.perm.effective")}</h2>
        <span className="text-xs text-ink-3">
          {tr("agents.perm.effectiveNote", { project: data.projectName ? tr("agents.perm.withProject", { name: data.projectName }) : tr("agents.perm.pickProjectNote") })}
        </span>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <input
          value={call}
          onChange={(e) => {
            setCall(e.target.value);
            reset();
          }}
          onKeyDown={(e) => e.key === "Enter" && run()}
          aria-label={tr("agents.perm.toolCall")}
          spellCheck={false}
          placeholder="Bash(git push origin main) · Read(.env) · mcp__notion__search"
          className={`${fieldClass} h-8 min-w-0 flex-1 py-0 font-mono text-[12px]`}
        />
        <Select
          label={tr("agents.perm.agent")}
          value={agentId}
          onChange={(e) => {
            setAgentId(e.target.value);
            reset();
          }}
        >
          <option value="">{tr("agents.perm.none")}</option>
          {data.agents.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
            </option>
          ))}
        </Select>
        <Button variant="primary" onClick={run} disabled={!call.trim()}>
          {tr("agents.perm.check")}
        </Button>
      </div>
      {error && <p className="mt-2 text-xs text-danger">{shownLine(error)}</p>}
      {result && (
        <div className="mt-3 space-y-1.5" role="status">
          <div className="flex flex-wrap items-center gap-2 text-[13px]">
            <span className={`rounded-md px-2 py-0.5 text-xs font-semibold ${DECISION_CLS[result.decision]}`}>{decisionLabel(result.decision)}</span>
            {result.rule && (
              <>
                <span className="text-ink-2">{scopeLabel(result.scopeCode, result.scope)}</span>
                <span className="text-ink-3">{result.list}</span>
                <code className="font-mono text-[12px]">{shown(result.rule, 200)}</code>
              </>
            )}
            {!result.rule && result.scope && <span className="text-ink-2">{result.scope}</span>}
          </div>
          <ul className="space-y-0.5 text-xs text-ink-3">
            {result.notes.map((n) => (
              <li key={n}>· {n}</li>
            ))}
          </ul>
        </div>
      )}
    </Sheet>
  );
}

function ManagedBanner({ data }: { data: PermissionScopes }) {
  const tr = useT();
  const m = data.managed;
  if (!m.sources.length && !m.mdm) return null;
  return (
    <Sheet className="flex gap-3 bg-warn-soft p-4 text-xs">
      <ShieldCheck size={16} className="mt-0.5 shrink-0" />
      <div className="min-w-0 space-y-1">
        <p className="text-[13px] font-semibold">{tr("agents.perm.managed")}</p>
        {m.sources.map((s) => (
          <p key={s.path} className="truncate text-ink-2">
            <span className="font-mono">{s.path}</span> {tr("agents.perm.managedCounts", { allow: s.allow.length, ask: s.ask.length, deny: s.deny.length })}
          </p>
        ))}
        {m.mdm && <p className="text-ink-2">{tr("agents.perm.mdm", { name: m.mdm })}</p>}
        {m.managedOnly && <p className="font-medium text-danger">{tr("agents.perm.managedOnly")}</p>}
        <p className="text-ink-3">{tr("agents.perm.managedNote")}</p>
      </div>
    </Sheet>
  );
}

export function PermissionsPage() {
  const tr = useT();
  const [tab, setTab] = useState<Tab>("rules");
  const [sub, setSub] = useState<ProjectScope>("shared");
  const { scope: side, projectId } = useScope("folder");
  // The current project is always included: the effective result on the global side is judged for it too.
  const project = projectId;
  const qc = useQueryClient();
  const q = useQuery({ queryKey: ["permissions", project], queryFn: () => permApi.scopes(project) });
  const data = q.data;
  const reload = () => qc.invalidateQueries({ queryKey: ["permissions"] });
  const scope: SettingsScope = side === "project" ? sub : "user";
  const view = data && tab === "rules" ? data[scope] : null;
  return (
    <div>
      <PageHeader title={tr("agents.perm.title")}>
        <Segmented label={tr("agents.perm.view")} value={tab} options={tabs()} onChange={setTab} />
      </PageHeader>
      <PageBody>
        <ScopeBar need="folder" note={side === "global" && data?.projectName ? tr("agents.perm.globalNote") : undefined}>
          {side === "project" && tab === "rules" && <Segmented label={tr("agents.perm.projectFile")} value={sub} options={projectScopes()} onChange={setSub} />}
        </ScopeBar>
        {q.isError ? (
          <Empty title={tr("agents.perm.loadFailed")}>{messageOf(q.error)}</Empty>
        ) : !data ? (
          <Empty title={tr("agents.perm.loading")} />
        ) : (
          <>
            <ManagedBanner data={data} />
            {tab === "agent" ? (
              <AgentScope key={data.project ?? ""} data={data} onSaved={reload} />
            ) : view ? (
              <ScopeEditor key={`${scope}|${data.project}|${view.token}|${view.exists}`} data={data} view={view} scope={scope} onSaved={reload} />
            ) : (
              <Sheet>
                <Empty title={tr("agents.perm.pickProject")}>{scopeHint(scope)}</Empty>
              </Sheet>
            )}
            <Explainer key={`${scope}|${data.project}`} data={data} />
          </>
        )}
      </PageBody>
    </div>
  );
}
