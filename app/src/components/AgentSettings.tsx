// "에이전트 설정" sheet behind the agents page gear (team-presets-editable, D-081):
// agent on/off · hide · delete, team rules, task types. The backend validates everything again.
import { useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, ArrowDown, ArrowUp, Backpack, EyeOff, Lock, Plus, RotateCcw, Trash2, X } from "lucide-react";
import { Fragment, useEffect, useRef, useState, type ReactNode } from "react";
import { api, reasonIs, type AgentDef, failure, shown, type Shown } from "../lib/api";
import { addedTools, agentListsFor, wouldEmptyTools, agentLoadoutLock, chosenFromAgent, frontmatterList, loadoutApi, useLoadoutOptions } from "../lib/loadout";
import { LoadoutSection, namesToPins, type LoadoutValue } from "./LoadoutSection";
import { sourceText } from "../lib/agents";
import { EFFORTS } from "../lib/brief";
import { usd } from "../lib/format";
import { refineBudget } from "../lib/refine";
import { roleOf } from "../lib/roles";
import { tx, useT } from "../i18n/react";
import {
  AUTO_ALLOWED_LABEL,
  EDITABLE_MODES,
  isOffAnywhere,
  MODE_LABEL,
  offState,
  STRICT_MODES,
  teamApi,
  typeProblems,
  useAgentControls,
  useTeamConfig,
  type TeamConfig,
  type TeamType,
} from "../lib/team";
import { Button, fieldClass, MoreInfo, Segmented } from "./ui";

type Tab = "agents" | "rules" | "types";

function Switch({ checked, onChange, label, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-5 w-9 shrink-0 rounded-full transition-colors disabled:opacity-40 ${checked ? "bg-ink" : "bg-ink-3/40"}`}
    >
      <span className={`absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition-all ${checked ? "left-[18px]" : "left-0.5"}`} />
    </button>
  );
}

const Dot = ({ title, tone = "duck" }: { title: string; tone?: "duck" | "danger" }) => (
  <span title={title} aria-label={title} className={`inline-block h-1.5 w-1.5 shrink-0 rounded-full ${tone === "duck" ? "bg-warn" : "bg-danger"}`} />
);

function Label({ text, children, hint }: { text: string; children: ReactNode; hint?: ReactNode }) {
  return (
    <label className="block space-y-1">
      <span className="text-xs font-medium text-ink-2">{text}</span>
      {children}
      {hint && <span className="block text-[11px] text-ink-3">{hint}</span>}
    </label>
  );
}

// ---------- 에이전트 관리 ----------

/** "기본 장비" of one agent definition: frontmatter `skills`, and MCP through `tools` (when it has one) or
 * `disallowedTools`. The save builds on the file's raw lists; Rust refuses any change outside MCP entries. */
function AgentLoadout({ d, onSaved }: { d: AgentDef; onSaved: () => void }) {
  const tr = useT();
  const opts = useLoadoutOptions();
  const lock = agentLoadoutLock(d);
  const servers = (opts.data?.mcp ?? []).map((s) => s.name);
  const initial: LoadoutValue = { skills: namesToPins(d.skills ?? [], opts.data?.skills ?? []), mcp: chosenFromAgent(d.tools, d.disallowedTools, servers) };
  const [value, setValue] = useState<LoadoutValue | null>(null);
  const [msg, setMsg] = useState<{ ok: boolean; text: Shown } | null>(null);
  const [busy, setBusy] = useState(false);
  const cur = value ?? initial;
  const dirty = !!value && JSON.stringify([value.skills.map((p) => p.name), value.mcp]) !== JSON.stringify([initial.skills.map((p) => p.name), initial.mcp]);
  const emptiesTools = wouldEmptyTools(d.tools, cur.mcp, servers);
  const toolsNote = d.tools.length > 0 ? addedTools(d.tools, agentListsFor(d.tools, d.disallowedTools, cur.mcp, servers).tools) : [];
  const save = async () => {
    if (!value) return;
    setBusy(true);
    setMsg(null);
    try {
      const def = await api.agentDefinition(d.id);
      const lists = agentListsFor(frontmatterList(def.content, "tools"), frontmatterList(def.content, "disallowedTools"), value.mcp, servers);
      await loadoutApi.saveAgent(d.id, null, value.skills.map((p) => p.name), lists.tools, lists.disallowed, def.token);
      setValue(null);
      setMsg({ ok: true, text: tr("agents.manage.loadoutSaved") });
      onSaved();
    } catch (e) {
      const m = e;
      setMsg(reasonIs(m, "cancelled") ? { ok: true, text: tr("agents.manage.unchanged") } : { ok: false, text: failure(m) });
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="space-y-2">
      <LoadoutSection value={cur} onChange={setValue} byName toolList={d.tools.length > 0} lock={lock} toolsNote={toolsNote} />
      {!lock && (
        <div className="flex flex-wrap items-center gap-2">
          <p className={`min-w-0 flex-1 text-[11px] ${emptiesTools || (msg && !msg.ok) ? "text-danger" : "text-ink-3"}`}>{emptiesTools ? tr("agents.manage.emptiesTools") : (msg ? shown(msg.text) : tr("agents.manage.loadoutNote"))}</p>
          <Button disabled={!dirty || busy} onClick={() => setValue(null)}>
            {tr("agents.manage.revert")}
          </Button>
          <Button variant="primary" disabled={!dirty || busy || emptiesTools} onClick={save}>
            {busy ? tr("agents.manage.confirming") : tr("agents.manage.saveLoadout")}
          </Button>
        </div>
      )}
    </div>
  );
}

function AgentsTab({ catalog, types }: { catalog: AgentDef[]; types: TeamType[] }) {
  const tr = useT();
  const qc = useQueryClient();
  const controls = useAgentControls();
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<{ tone: "ok" | "error"; text: Shown } | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const loadout = useLoadoutOptions();
  const mcpNames = (loadout.data?.mcp ?? []).map((s) => s.name);
  const c = controls.data;
  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["agent-controls"] });
    qc.invalidateQueries({ queryKey: ["agent-catalog"] });
  };
  const run = async (key: string, f: () => Promise<string | void | null>) => {
    setBusy(key);
    setMsg(null);
    try {
      const warn = await f();
      if (warn) setMsg({ tone: "error", text: warn });
    } catch (e) {
      const m = e;
      setMsg(reasonIs(m, "cancelled") ? { tone: "ok", text: tr("agents.manage.unchanged") } : { tone: "error", text: failure(m) });
    } finally {
      setBusy(null);
      refresh();
    }
  };
  const toggleOff = (d: AgentDef, on: boolean) =>
    run(`off-${d.id}`, async () => {
      if (!on) {
        const scope = d.source === "project" ? tr("agents.manage.scopeProject") : tr("agents.manage.scopeUser");
        const extra =
          d.source === "builtin"
            ? `\n\n${tr("agents.manage.offBuiltin")}`
            : d.source !== "project"
              ? `\n\n${tr("agents.manage.offGlobal")}`
              : "";
        if (!window.confirm(tr("agents.manage.offConfirm", { name: d.name, scope }) + extra)) return null;
      }
      return teamApi.setEnabled(d.id, on);
    });
  const trash = (d: AgentDef) =>
    run(`trash-${d.id}`, async () => {
      const r = await teamApi.trash(d.id);
      if ((r.off || r.hidden) && window.confirm(tr("agents.manage.trashed", { name: r.name, what: [r.off ? tr("agents.manage.offRule") : "", r.hidden ? tr("agents.manage.hiddenMark") : ""].filter(Boolean).join("·") }))) {
        await teamApi.forget(r.name, r.project);
      }
      return null;
    });

  const defs = [...catalog].sort((a, b) => a.name.localeCompare(b.name) || a.source.localeCompare(b.source));
  const roleUsers = (name: string) => types.filter((t) => t.agent === name && !t.hidden).map((t) => t.label);
  return (
    <div className="space-y-3">
      {/* One line, the three columns explained under 자세히 (one-line rule, tour-v2 test review). */}
      <div>
        <p className="text-xs text-ink-3">{tr("agents.manage.summary")}</p>
        <MoreInfo className="mt-1">
          <span className="block">
            {tx("agents.manage.onMore", { name: <b className="font-medium text-ink-2">{tr("agents.manage.on")}</b>, rule: <code className="font-mono">{tr("agents.manage.ruleName")}</code> })}
          </span>
          <span className="block">
            {tx("agents.manage.shownMore", { name: <b className="font-medium text-ink-2">{tr("agents.manage.shown")}</b> })}
          </span>
          <span className="block">
            {tx("agents.manage.deleteMore", { name: <b className="font-medium text-ink-2">{tr("agents.manage.delete")}</b> })}
          </span>
        </MoreInfo>
      </div>
      {controls.isError && <p className="text-xs text-danger">{tr("agents.manage.controlsFailed", { error: String(controls.error) })}</p>}
      {msg && (
        <p role="status" className={`text-xs ${msg.tone === "error" ? "text-danger" : "text-ink-3"}`}>
          {shown(msg.text)}
        </p>
      )}
      <div className="overflow-hidden rounded-lg border border-line">
        <table className="w-full text-[13px]">
          <thead className="bg-paper text-left text-[11px] text-ink-3">
            <tr>
              <th className="px-3 py-2 font-medium">{tr("agents.manage.agent")}</th>
              <th className="w-20 px-2 py-2 font-medium" title={tr("agents.manage.onTitle")}>
                {tr("agents.manage.on")}
              </th>
              <th className="w-28 px-2 py-2 font-medium" title={tr("agents.manage.shownTitle")}>
                {tr("agents.manage.shown")}
              </th>
              <th className="w-14 px-2 py-2 font-medium">
                <span className="sr-only">{tr("agents.manage.delete")}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {defs.map((d) => {
              const state = offState(d, c);
              const hidden = c?.hidden.includes(d.name) ?? false;
              const role = roleOf(d.name, d.description);
              const users = roleUsers(d.name);
              const nMcp = chosenFromAgent(d.tools, d.disallowedTools, mcpNames).length;
              const gear = [(d.skills?.length ?? 0) > 0 ? tr("agents.manage.skillCount", { count: d.skills?.length ?? 0 }) : "", nMcp > 0 ? tr("agents.manage.mcpCount", { count: nMcp }) : ""].filter(Boolean).join(" · ");
              return (
                <Fragment key={d.id}>
                <tr className={`border-t border-line ${state ? "bg-paper/60" : ""}`}>
                  <td className="px-3 py-2">
                    <div className="flex min-w-0 items-center gap-2">
                      <span aria-hidden className="h-2.5 w-2.5 shrink-0 rounded-full" style={{ background: role.color.scarf || role.color.body }} />
                      <span className={`truncate font-medium ${state ? "text-ink-3" : ""}`}>{d.name}</span>
                      {state === "off" && <span className="shrink-0 rounded-full bg-line px-1.5 py-0.5 text-[10px] font-semibold text-ink-2">{tr("agents.manage.off")}</span>}
                      {state === "elsewhere" && <span className="shrink-0 rounded-full bg-line px-1.5 py-0.5 text-[10px] text-ink-2">{tr("agents.manage.offElsewhere")}</span>}
                    </div>
                    <p className="truncate text-[11px] text-ink-3">
                      {sourceText(d)}
                      {users.length > 0 && tr("agents.manage.roleUsers", { types: users.join(", ") })}
                    </p>
                    <button
                      type="button"
                      aria-expanded={open === d.id}
                      onClick={() => setOpen(open === d.id ? null : d.id)}
                      className="mt-0.5 inline-flex items-center gap-1 text-[11px] text-ink-2 underline-offset-2 hover:text-ink hover:underline"
                    >
                      <Backpack size={11} /> {tr("agents.loadout.title")}{gear ? ` · ${gear}` : ""}
                    </button>
                    {state && users.length > 0 && <p className="text-[11px] text-danger">{tr("agents.manage.roleBlocked")}</p>}
                  </td>
                  <td className="px-2 py-2">
                    <Switch label={tr("agents.manage.turnOn", { name: d.name })} checked={!state} disabled={state === "elsewhere" || busy !== null || !c} onChange={(on) => toggleOff(d, on)} />
                  </td>
                  <td className="px-2 py-2">
                    <Switch
                      label={tr("agents.manage.showIn", { name: d.name })}
                      checked={!hidden}
                      disabled={busy !== null || !c}
                      onChange={(show) => run(`hide-${d.id}`, () => teamApi.setHidden(d.name, !show))}
                    />
                  </td>
                  <td className="px-2 py-2 text-right">
                    {(() => {
                      const deletable = (d.source === "user" || d.source === "project") && d.editable;
                      const why = deletable ? tr("agents.manage.toTrash") : d.source === "builtin" || d.source === "plugin" ? tr("agents.manage.cantDeleteBundled") : (d.readonlyReason ?? tr("agents.manage.cantDelete"));
                      return (
                        // The wrapper carries the tooltip: disabled buttons do not show one.
                        <span title={why} className="inline-block">
                          <button
                            type="button"
                            aria-label={deletable ? tr("agents.manage.deleteName", { name: d.name }) : tr("agents.manage.deleteBlocked", { name: d.name, why })}
                            disabled={!deletable || busy !== null}
                            onClick={() => trash(d)}
                            className="rounded-md p-1 text-ink-3 hover:bg-danger/10 hover:text-danger disabled:pointer-events-none disabled:opacity-30"
                          >
                            <Trash2 size={14} />
                          </button>
                        </span>
                      );
                    })()}
                  </td>
                </tr>
                {open === d.id && (
                  <tr className="border-t border-line bg-paper/40">
                    <td colSpan={4} className="px-3 py-3">
                      <AgentLoadout d={d} onSaved={refresh} />
                    </td>
                  </tr>
                )}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}

// ---------- 팀 규칙 ----------

/** Examples that fit each safety rule's appended note. */
const EXTRA_HINT = {
  credentials: "agents.rules.hintCredentials",
  branch: "agents.rules.hintBranch",
  "stop-risky": "agents.rules.hintStopRisky",
} as const;
const extraHint = (id: string) => (id in EXTRA_HINT ? EXTRA_HINT[id as keyof typeof EXTRA_HINT] : null);

function RulesTab({ draft, defaults, onChange }: { draft: TeamConfig; defaults: TeamConfig; onChange: (c: TeamConfig) => void }) {
  const tr = useT();
  const [newText, setNewText] = useState("");
  const [openExtra, setOpenExtra] = useState<string[]>([]);
  const rules = draft.rules;
  const set = (i: number, patch: Partial<TeamConfig["rules"][number]>) => onChange({ ...draft, rules: rules.map((r, j) => (j === i ? { ...r, ...patch } : r)) });
  const move = (i: number, d: -1 | 1) => {
    const list = [...rules];
    [list[i], list[i + d]] = [list[i + d], list[i]];
    onChange({ ...draft, rules: list });
  };
  const deleted = defaults.rules.filter((b) => !rules.some((r) => r.id === b.id));
  return (
    <div className="space-y-3">
      <p className="text-xs text-ink-3">
        {tx("agents.rules.intro", { lock: <Lock size={11} className="inline" /> })}
      </p>
      <ol className="space-y-2">
        {rules.map((r, i) => {
          const def = defaults.rules.find((b) => b.id === r.id);
          const changed = !!def && (r.text !== def.text || r.extra !== "");
          return (
            <li key={r.id} className={`rounded-lg border p-2.5 ${r.locked ? "border-line bg-paper/60" : "border-line bg-sheet"}`}>
              <div className="flex items-start gap-2">
                <span className="mt-2 w-5 shrink-0 text-right text-xs text-ink-3 tabular-nums">{i + 1}.</span>
                <div className="min-w-0 flex-1 space-y-1.5">
                  {r.locked ? (
                    <p className="flex items-start gap-1.5 py-1.5 text-[13px]">
                      <Lock size={13} className="mt-0.5 shrink-0 text-ink-3" aria-label={tr("agents.rules.safety")} />
                      <span>{r.text}</span>
                    </p>
                  ) : (
                    <input aria-label={tr("agents.rules.rule", { n: i + 1 })} value={r.text} maxLength={160} onChange={(e) => set(i, { text: e.target.value })} className={fieldClass} />
                  )}
                  {r.locked &&
                    (r.extra || openExtra.includes(r.id) ? (
                      <input
                        aria-label={tr("agents.rules.ruleExtra", { n: i + 1 })}
                        value={r.extra}
                        maxLength={160}
                        autoFocus={!r.extra}
                        placeholder={tr(extraHint(r.id) ?? "agents.rules.extraHint")}
                        onChange={(e) => set(i, { extra: e.target.value })}
                        className={`${fieldClass} py-1.5 text-[12px]`}
                      />
                    ) : (
                      <button type="button" onClick={() => setOpenExtra([...openExtra, r.id])} className="text-[12px] text-ink-2 underline underline-offset-2 hover:text-ink">
                        {tr("agents.rules.addExtra")}
                      </button>
                    ))}
                  {r.defaultChanged && <p className="text-[11px] text-ink-2">{tr("agents.rules.defaultChanged")}</p>}
                </div>
                <div className="flex shrink-0 items-center gap-0.5 pt-1">
                  {changed && <Dot title={tr("agents.rules.differs")} />}
                  {changed && (
                    <button type="button" title={tr("agents.rules.toDefault")} aria-label={tr("agents.rules.ruleToDefault", { n: i + 1 })} onClick={() => set(i, { text: def!.text, extra: "" })} className="rounded p-1 text-ink-3 hover:bg-paper hover:text-ink">
                      <RotateCcw size={13} />
                    </button>
                  )}
                  <button type="button" aria-label={tr("agents.rules.ruleUp", { n: i + 1 })} disabled={i === 0} onClick={() => move(i, -1)} className="rounded p-1 text-ink-3 hover:bg-paper hover:text-ink disabled:opacity-30">
                    <ArrowUp size={13} />
                  </button>
                  <button type="button" aria-label={tr("agents.rules.ruleDown", { n: i + 1 })} disabled={i === rules.length - 1} onClick={() => move(i, 1)} className="rounded p-1 text-ink-3 hover:bg-paper hover:text-ink disabled:opacity-30">
                    <ArrowDown size={13} />
                  </button>
                  <button
                    type="button"
                    aria-label={tr("agents.rules.ruleDelete", { n: i + 1 })}
                    title={r.locked ? tr("agents.rules.lockedDelete") : tr("agents.rules.delete")}
                    disabled={r.locked}
                    onClick={() => onChange({ ...draft, rules: rules.filter((_, j) => j !== i) })}
                    className="rounded p-1 text-ink-3 hover:bg-danger/10 hover:text-danger disabled:opacity-30"
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              </div>
            </li>
          );
        })}
      </ol>
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (!newText.trim()) return;
          onChange({ ...draft, rules: [...rules, { id: `u-${Date.now().toString(36)}`, text: newText.trim(), extra: "", locked: false, bundled: false, defaultText: null, changed: true, defaultChanged: false }] });
          setNewText("");
        }}
      >
        <input aria-label={tr("agents.rules.newRule")} value={newText} maxLength={160} onChange={(e) => setNewText(e.target.value)} placeholder={tr("agents.rules.newRuleHint")} className={fieldClass} />
        <Button type="submit" disabled={!newText.trim() || rules.length >= 30}>
          <Plus size={14} /> {tr("agents.rules.add")}
        </Button>
      </form>
      {deleted.length > 0 && (
        <p className="text-[11px] text-ink-3">
          {tr("agents.rules.deleted")}{" "}
          {deleted.map((b) => (
            <button key={b.id} type="button" className="mr-2 underline underline-offset-2" onClick={() => onChange({ ...draft, rules: [...rules, { ...b }] })}>
              {tr("agents.rules.restore", { text: b.text.slice(0, 18) })}
            </button>
          ))}
        </p>
      )}
      <RefineSection draft={draft} defaults={defaults} onChange={onChange} />
    </div>
  );
}

/** "요구사항 다듬기" guidance (brief-grilling): sent with every refine run; the backend caps and re-checks it. */
function RefineSection({ draft, defaults, onChange }: { draft: TeamConfig; defaults: TeamConfig; onChange: (c: TeamConfig) => void }) {
  const tr = useT();
  const r = draft.refine ?? defaults.refine;
  const set = (patch: Partial<TeamConfig["refine"]>) => onChange({ ...draft, refine: { ...r, ...patch } });
  const changed = JSON.stringify(r) !== JSON.stringify(defaults.refine);
  return (
    <fieldset className="space-y-2 rounded-lg border border-line p-3">
      <legend className="flex items-center gap-1.5 px-1 text-xs font-medium text-ink-2">
        {tr("agents.refine.title")} {changed && <Dot title={tr("agents.rules.differs")} />}
      </legend>
      <p className="text-[11px] text-ink-3">{tr("agents.refine.intro")}</p>
      <Label text={tr("agents.refine.guidance")} hint={tr("agents.refine.chars", { n: [...r.guidance].length, max: 1000 })}>
        <textarea value={r.guidance} rows={3} maxLength={1000} onChange={(e) => set({ guidance: e.target.value })} className={`${fieldClass} text-[12px]`} />
      </Label>
      <div className="grid gap-3 sm:grid-cols-2">
        <Label text={tr("agents.refine.questions")}>
          <select value={r.maxQuestions} onChange={(e) => set({ maxQuestions: Number(e.target.value) })} className={fieldClass}>
            {[3, 4, 5, 6, 7, 8].map((n) => (
              <option key={n} value={n}>
                {tr("agents.refine.maxQuestions", { n })}
              </option>
            ))}
          </select>
        </Label>
        <Label text={tr("agents.refine.rounds")} hint={tr("agents.refine.roundsHint", { cost: usd(refineBudget(r.rounds)) })}>
          <select value={r.rounds} onChange={(e) => set({ rounds: Number(e.target.value) })} className={fieldClass}>
            {[1, 2, 3, 4, 5, 6].map((n) => (
              <option key={n} value={n}>
                {tr("agents.refine.maxRounds", { n })}
              </option>
            ))}
          </select>
        </Label>
      </div>
      {changed && (
        <Button onClick={() => onChange({ ...draft, refine: { ...defaults.refine } })}>
          <RotateCcw size={13} /> {tr("agents.refine.toDefault")}
        </Button>
      )}
    </fieldset>
  );
}

// ---------- 작업 유형 ----------

/** Comma-separated tool rules. Keeps its own text so a trailing comma survives while typing. */
function ToolsField({ value, placeholder, onChange }: { value: string[] | null; placeholder: string; onChange: (v: string[] | null) => void }) {
  const [text, setText] = useState(value?.join(", ") ?? "");
  return (
    <input
      value={text}
      placeholder={placeholder}
      onChange={(e) => {
        setText(e.target.value);
        const list = e.target.value
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean);
        onChange(list.length ? list : null);
      }}
      className={`${fieldClass} font-mono text-[12px]`}
    />
  );
}

const newType = (n: number, label: string): TeamType => ({
  key: `custom-${n}`,
  label,
  role: "",
  roleLabel: "",
  agent: null,
  rolePrompt: "",
  roleTools: ["Read", "Glob", "Grep"],
  mode: "manual",
  modes: ["plan", "manual"],
  effort: "high",
  review: false,
  strict: true,
  hidden: false,
  autoAllowed: [],
  note: "",
  refineFocus: "",
  skills: [],
  mcpAllow: [],
  bundled: false,
  changed: true,
  defaultChanged: false,
  fixedStrict: false,
});

function TypesTab({
  draft,
  defaults,
  saved,
  usedTypes,
  autoAllowed,
  catalog,
  onChange,
}: {
  draft: TeamConfig;
  defaults: TeamConfig;
  saved: TeamConfig;
  usedTypes: string[];
  autoAllowed: string[];
  catalog: AgentDef[];
  onChange: (c: TeamConfig) => void;
}) {
  const tr = useT();
  const controls = useAgentControls();
  const [sel, setSel] = useState(draft.types[0]?.key ?? "");
  const [resetN, setResetN] = useState(0);
  const idx = Math.max(0, draft.types.findIndex((t) => t.key === sel));
  const t = draft.types[idx];
  const bundled = defaults.types.find((b) => b.key === t?.key);
  const set = (patch: Partial<TeamType>) => onChange({ ...draft, types: draft.types.map((x, j) => (j === idx ? { ...x, ...patch } : x)) });
  const agentNames = [...new Set(catalog.map((d) => d.name))].sort();
  const isNew = !!t && !saved.types.some((x) => x.key === t.key);
  const problems = t ? typeProblems(t, bundled, catalog) : [];
  const promptBytes = t ? new TextEncoder().encode(t.rolePrompt).length : 0;

  const list = (
    <ul className="space-y-0.5" aria-label={tr("agents.types.list")}>
      {draft.types.map((x) => {
        const d = defaults.types.find((b) => b.key === x.key);
        const changed = !d || JSON.stringify({ ...x, changed: 0, defaultChanged: 0 }) !== JSON.stringify({ ...d, changed: 0, defaultChanged: 0 });
        return (
          <li key={x.key}>
            <button
              type="button"
              onClick={() => setSel(x.key)}
              aria-current={x.key === t?.key}
              className={`flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-[13px] ${x.key === t?.key ? "bg-ink text-paper" : "text-ink-2 hover:bg-paper"}`}
            >
              <span className={`flex-1 truncate ${x.hidden ? "line-through opacity-60" : ""}`}>{x.label}</span>
              {x.strict && <Lock size={11} aria-label={tr("agents.types.strict")} className="shrink-0 opacity-60" />}
              {x.hidden && <EyeOff size={11} aria-label={tr("agents.types.hidden")} className="shrink-0 opacity-60" />}
              {changed && <Dot title={d ? tr("agents.types.differs") : tr("agents.types.added")} />}
              {x.defaultChanged && <Dot title={tr("agents.types.defaultChanged")} tone="danger" />}
            </button>
          </li>
        );
      })}
    </ul>
  );

  return (
    <div className="grid gap-4 lg:grid-cols-[180px_1fr]">
      <div className="space-y-2">
        <div className="hidden lg:block">{list}</div>
        <select aria-label={tr("agents.types.pick")} value={t?.key ?? ""} onChange={(e) => setSel(e.target.value)} className={`${fieldClass} lg:hidden`}>
          {draft.types.map((x) => (
            <option key={x.key} value={x.key}>
              {x.label}
              {x.hidden ? tr("agents.types.hiddenSuffix") : ""}
            </option>
          ))}
        </select>
        <Button
          className="w-full justify-center"
          disabled={draft.types.length >= 38}
          onClick={() => {
            let n = 1;
            while (draft.types.some((x) => x.key === `custom-${n}`)) n++;
            const nt = newType(n, tr("agents.types.newLabel"));
            onChange({ ...draft, types: [...draft.types, nt] });
            setSel(nt.key);
          }}
        >
          <Plus size={14} /> {tr("agents.types.add")}
        </Button>
      </div>
      {t && (
        <div className="min-w-0 space-y-3">
          <div className={`grid gap-3 ${isNew ? "sm:grid-cols-[1fr_150px_170px]" : "sm:grid-cols-[1fr_170px]"}`}>
            <Label text={tr("agents.types.name")}>
              <input value={t.label} maxLength={20} onChange={(e) => set({ label: e.target.value })} className={fieldClass} />
            </Label>
            {isNew && (
              <Label text={tr("agents.types.key")} hint={tr("agents.types.keyHint")}>
                <input
                  value={t.key}
                  maxLength={24}
                  onChange={(e) => {
                    set({ key: e.target.value });
                    setSel(e.target.value);
                  }}
                  className={`${fieldClass} font-mono`}
                />
              </Label>
            )}
            <Label text={tr("agents.types.effort")}>
              <select value={t.effort} onChange={(e) => set({ effort: e.target.value })} className={fieldClass}>
                {EFFORTS.map((x) => (
                  <option key={x.id} value={x.id}>
                    {x.label}
                  </option>
                ))}
              </select>
            </Label>
          </div>
          {t.defaultChanged && <p className="text-[11px] text-ink-2">{tr("agents.types.updated")}</p>}

          <fieldset className="space-y-2 rounded-lg border border-line p-3">
            <legend className="px-1 text-xs font-medium text-ink-2">{tr("agents.types.role")}</legend>
            <div className="flex flex-wrap gap-4 text-[13px]">
              <label className="flex items-center gap-1.5">
                <input type="radio" checked={!t.agent} onChange={() => set({ agent: null })} />
                {t.bundled ? tr("agents.types.bundledRole", { role: t.roleLabel }) : tr("agents.types.ownRole")}
              </label>
              <label className={`flex items-center gap-1.5 ${t.fixedStrict ? "text-ink-3" : ""}`} title={t.fixedStrict ? tr("agents.types.fixedStrictRole") : undefined}>
                <input type="radio" checked={!!t.agent} onChange={() => set({ agent: agentNames[0] ?? null })} disabled={agentNames.length === 0 || t.fixedStrict} />
                {tr("agents.types.myAgent")}
                {t.fixedStrict && <span className="text-[11px]">{tr("agents.types.fixedStrictNote")}</span>}
              </label>
            </div>
            {t.agent ? (
              <>
                <select aria-label={tr("agents.types.roleAgent")} value={t.agent} onChange={(e) => set({ agent: e.target.value })} className={fieldClass}>
                  {agentNames.map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </select>
                <p className="text-[11px] text-ink-3">
                  {tr("agents.types.agentRole")}
                  {t.strict ? tr("agents.types.agentRoleStrict") : tr("agents.types.agentRoleRisk")}
                </p>
                {isOffAnywhere(t.agent, controls.data) && <p className="text-[11px] text-danger">{tr("agents.types.agentOff")}</p>}
              </>
            ) : (
              <>
                <Label text={tr("agents.types.prompt")} hint={promptBytes > 8192 * 0.8 ? tr("agents.types.bytes", { n: promptBytes, max: 8192 }) : undefined}>
                  <textarea value={t.rolePrompt} rows={5} onChange={(e) => set({ rolePrompt: e.target.value })} className={`${fieldClass} text-[12px] leading-relaxed`} />
                </Label>
                <details className="text-[13px]">
                  <summary className="cursor-pointer text-xs font-medium text-ink-2">{t.roleTools ? tr("agents.types.advancedCount", { count: t.roleTools.length }) : tr("agents.types.advancedAll")}</summary>
                  <div className="mt-2">
                    <Label
                      text={tr("agents.types.tools")}
                      hint={t.strict ? tr("agents.types.toolsStrict") : tr("agents.types.toolsHint")}
                    >
                      <ToolsField key={`${t.key}-${resetN}`} value={t.roleTools} placeholder={t.strict ? "Read, Glob, Grep" : tr("agents.types.allTools")} onChange={(roleTools) => set({ roleTools })} />
                    </Label>
                  </div>
                </details>
              </>
            )}
            {t.lockedRule && (
              <div className="rounded-md border border-line bg-paper px-3 py-2" data-locked-rule>
                <p className="flex items-center gap-1.5 text-xs font-semibold text-ink-2">
                  <Lock size={12} aria-hidden />
                  {tr("agents.types.evidence")}
                </p>
                <p className="mt-1 text-[12px] leading-relaxed text-ink select-text">{t.lockedRule}</p>
                <p className="mt-1 text-[11px] text-ink-3">{tr("agents.types.evidenceNote")}</p>
              </div>
            )}
          </fieldset>

          {!t.agent && <LoadoutSection key={`${t.key}-${resetN}`} value={{ skills: t.skills, mcp: t.mcpAllow }} onChange={(v) => set({ skills: v.skills, mcpAllow: v.mcp })} strict={t.strict} toolList={!!t.roleTools} />}

          <fieldset className="space-y-3 rounded-lg border border-line p-3">
            <legend className="px-1 text-xs font-medium text-ink-2">{tr("agents.types.permissions")}</legend>
            <Label text={tr("agents.types.mode")}>
              <select value={t.mode} onChange={(e) => set({ mode: e.target.value })} className={`${fieldClass} block sm:max-w-[280px]`}>
                {t.modes.map((m) => (
                  <option key={m} value={m}>
                    {MODE_LABEL[m] ?? m}
                  </option>
                ))}
              </select>
            </Label>
            <div className="space-y-1">
              <span className="text-xs font-medium text-ink-2">{tr("agents.types.modes")}</span>
              <div className="flex flex-wrap gap-x-4 gap-y-1 text-[13px]">
                {EDITABLE_MODES.map((m) => {
                  const blocked = t.strict && !STRICT_MODES.includes(m);
                  return (
                    <label key={m} className={`flex items-center gap-1.5 ${blocked ? "text-ink-3" : ""}`}>
                      <input
                        type="checkbox"
                        disabled={blocked}
                        checked={t.modes.includes(m)}
                        onChange={(e) => {
                          const modes = e.target.checked ? EDITABLE_MODES.filter((x) => x === m || t.modes.includes(x)) : t.modes.filter((x) => x !== m);
                          set({ modes, mode: modes.includes(t.mode) ? t.mode : (modes[0] ?? t.mode) });
                        }}
                      />
                      {MODE_LABEL[m]}
                    </label>
                  );
                })}
              </div>
              <p className="text-[11px] text-ink-3">{tr("agents.types.noDanger")}</p>
            </div>
            <div className="space-y-1">
              <span className="text-xs font-medium text-ink-2">{tr("agents.types.autoAllowed")}</span>
              <div className="flex flex-wrap gap-x-4 gap-y-1 text-[13px]">
                {autoAllowed.map((a) => (
                  <label key={a} className="flex items-center gap-1.5">
                    <input type="checkbox" checked={t.autoAllowed.includes(a)} onChange={(e) => set({ autoAllowed: e.target.checked ? [...t.autoAllowed, a] : t.autoAllowed.filter((x) => x !== a) })} />
                    {AUTO_ALLOWED_LABEL[a] ?? a}
                  </label>
                ))}
              </div>
            </div>
          </fieldset>

          <fieldset className="space-y-2 rounded-lg border border-line p-3">
            <legend className="px-1 text-xs font-medium text-ink-2">{tr("agents.types.misc")}</legend>
            <div className="flex flex-wrap gap-x-5 gap-y-2 text-[13px]">
              <label className="flex items-center gap-1.5">
                <input type="checkbox" checked={t.review} onChange={(e) => set({ review: e.target.checked })} />
                {tr("agents.types.review")}
              </label>
              <label className={`flex items-center gap-1.5 ${t.fixedStrict ? "text-ink-3" : ""}`} title={t.fixedStrict ? tr("agents.types.alwaysStrict") : undefined}>
                <input
                  type="checkbox"
                  checked={t.strict}
                  disabled={t.fixedStrict}
                  onChange={(e) => set(e.target.checked ? { strict: true, modes: t.modes.filter((m) => STRICT_MODES.includes(m)).length ? t.modes.filter((m) => STRICT_MODES.includes(m)) : ["manual"], mode: STRICT_MODES.includes(t.mode) ? t.mode : "manual" } : { strict: false })}
                />
                {tr("agents.types.strictType")}
                {t.fixedStrict ? tr("agents.types.fixed") : ""}
              </label>
              <label className="flex items-center gap-1.5">
                <input type="checkbox" checked={t.hidden} onChange={(e) => set({ hidden: e.target.checked })} />
                {tr("agents.types.hide")}
              </label>
            </div>
            <Label text={tr("agents.types.focus")} hint={tr("agents.types.focusHint", { n: [...t.refineFocus].length, max: 300 })}>
              <input value={t.refineFocus} maxLength={300} onChange={(e) => set({ refineFocus: e.target.value })} placeholder={tr("agents.types.focusPlaceholder")} className={fieldClass} />
            </Label>
            <p className="text-[11px] text-ink-3">{tr("agents.types.strictNote")}</p>
          </fieldset>
          {problems.length > 0 && (
            <ul className="space-y-0.5 text-xs text-danger" role="alert">
              {problems.map((p) => (
                <li key={p} className="flex items-center gap-1">
                  <AlertTriangle size={12} /> {p}
                </li>
              ))}
            </ul>
          )}
          <div className="flex gap-2 border-t border-line pt-3">
            {bundled ? (
              <Button
                onClick={() => {
                  set({ ...bundled, hidden: t.hidden });
                  setResetN((n) => n + 1);
                }}
              >
                <RotateCcw size={13} /> {tr("agents.types.toDefault")}
              </Button>
            ) : (
              <Button
                variant="danger"
                disabled={usedTypes.includes(t.key)}
                title={usedTypes.includes(t.key) ? tr("agents.types.usedHideOnly") : undefined}
                onClick={() => {
                  onChange({ ...draft, types: draft.types.filter((x) => x.key !== t.key) });
                  setSel(draft.types[0]?.key ?? "");
                }}
              >
                <Trash2 size={13} /> {tr("agents.types.delete")}
              </Button>
            )}
            {!bundled && usedTypes.includes(t.key) && <span className="self-center text-[11px] text-ink-3">{tr("agents.types.usedHideOnly")}</span>}
          </div>
        </div>
      )}
    </div>
  );
}

// ---------- sheet ----------

export function AgentSettings({ catalog, onClose }: { catalog: AgentDef[]; onClose: () => void }) {
  const tr = useT();
  const qc = useQueryClient();
  const team = useTeamConfig();
  const [tab, setTab] = useState<Tab>("agents");
  const [draft, setDraftRaw] = useState<TeamConfig | null>(null);
  // Bumped when the draft is dropped (save, discard, reset) so text fields re-read the saved values.
  const [version, setVersion] = useState(0);
  const setDraft = (d: TeamConfig | null) => {
    setDraftRaw(d);
    if (d === null) setVersion((v) => v + 1);
  };
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<{ tone: "ok" | "error"; text: Shown } | null>(null);
  const saved = team.data?.config;
  const current = draft ?? saved;
  const invalid = !!draft && draft.types.some((t) => typeProblems(t, team.data?.defaults.types.find((b) => b.key === t.key), catalog).length > 0);
  const dirty = !!draft && !!saved && JSON.stringify({ r: draft.rules, t: draft.types, f: draft.refine }) !== JSON.stringify({ r: saved.rules, t: saved.types, f: saved.refine });
  const close = () => {
    if (!dirty || window.confirm(tr("agents.sheet.unsavedClose"))) onClose();
  };
  const closeRef = useRef(close);
  useEffect(() => {
    closeRef.current = close;
  });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && closeRef.current();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const save = async () => {
    if (!draft) return;
    setSaving(true);
    setMsg(null);
    try {
      await teamApi.save({ rules: draft.rules.map((r) => ({ id: r.id, text: r.text, extra: r.extra })), types: draft.types, refine: draft.refine });
      await qc.invalidateQueries({ queryKey: ["team-config"] });
      setDraft(null);
      setMsg({ tone: "ok", text: tr("agents.sheet.saved") });
    } catch (e) {
      const m = e;
      setMsg(reasonIs(m, "cancelled") ? { tone: "ok", text: tr("agents.sheet.notSaved") } : { tone: "error", text: failure(m) });
    } finally {
      setSaving(false);
    }
  };
  const resetAll = async () => {
    if (!window.confirm(tr("agents.sheet.resetConfirm"))) return;
    try {
      await teamApi.reset(null);
      await qc.invalidateQueries({ queryKey: ["team-config"] });
      setDraft(null);
      setMsg({ tone: "ok", text: tr("agents.sheet.reset") });
    } catch (e) {
      setMsg({ tone: "error", text: failure(e) });
    }
  };

  return (
    <div className="fixed inset-0 z-30 flex justify-end bg-ink/20" onClick={close}>
      <div role="dialog" aria-modal="true" aria-label={tr("agents.sheet.title")} onClick={(e) => e.stopPropagation()} className="flex h-full w-[min(920px,96vw)] flex-col border-l border-line bg-sheet shadow-e3">
        <div className="flex flex-wrap items-center gap-3 border-b border-line px-6 py-4">
          <h2 className="mr-auto text-[15px] font-semibold">{tr("agents.sheet.title")}</h2>
          <Segmented
            label={tr("agents.sheet.area")}
            value={tab}
            options={[
              { id: "agents", label: tr("agents.manage.tab") },
              { id: "rules", label: tr("agents.rules.tab") },
              { id: "types", label: tr("agents.types.tab") },
            ]}
            onChange={setTab}
          />
          <button onClick={close} aria-label={tr("common.actions.close")} className="rounded-md p-1 text-ink-3 hover:text-ink">
            <X size={18} />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
          {tab === "agents" ? (
            <AgentsTab catalog={catalog} types={current?.types ?? []} />
          ) : team.isPending ? (
            <p className="text-xs text-ink-3">{tr("agents.sheet.loading")}</p>
          ) : team.isError || !current || !team.data ? (
            <div className="space-y-2 text-[13px]">
              <p>{tr("agents.sheet.readFailed")}</p>
              <Button onClick={() => team.refetch()}>{tr("agents.sheet.retry")}</Button>
            </div>
          ) : (
            <div className="space-y-3">
              {team.data.config.errors.length > 0 && (
                <div role="alert" className="rounded-lg border border-danger/40 bg-danger/5 px-3 py-2 text-xs text-danger">
                  <p className="font-medium">{tr("agents.sheet.partial")}</p>
                  <ul className="mt-1 list-disc pl-4">
                    {team.data.config.errors.map((e) => (
                      <li key={e}>{e}</li>
                    ))}
                  </ul>
                </div>
              )}
              {tab === "rules" ? (
                <RulesTab key={version} draft={current} defaults={team.data.defaults} onChange={setDraft} />
              ) : (
                <TypesTab key={version} draft={current} defaults={team.data.defaults} saved={team.data.config} usedTypes={team.data.usedTypes} autoAllowed={team.data.autoAllowed} catalog={catalog} onChange={setDraft} />
              )}
            </div>
          )}
        </div>
        {tab !== "agents" && team.data && (
          <div className="flex flex-wrap items-center gap-2 border-t border-line px-6 py-3">
            <p role="status" className={`min-w-0 flex-1 text-xs ${msg?.tone === "error" ? "whitespace-pre-wrap text-danger" : "text-ink-3"}`}>
              {(msg ? shown(msg.text) : null) ??
                (dirty
                  ? tr("agents.sheet.dirty")
                  : tab === "rules"
                    ? tr("agents.sheet.rulesNote")
                    : tr("agents.sheet.typesNote"))}
            </p>
            <Button onClick={resetAll}>{tr("agents.sheet.resetAll")}</Button>
            <Button onClick={() => setDraft(null)} disabled={!dirty || saving}>
              {tr("agents.sheet.revert")}
            </Button>
            <Button variant="primary" onClick={save} disabled={!dirty || saving || invalid} title={invalid ? tr("agents.sheet.invalid") : undefined}>
              {saving ? tr("agents.sheet.saving") : tr("common.actions.save")}
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
