// "기본 장비" (agent-loadout): skills preloaded at start and the MCP servers a role may use. Used by the task type
// detail (pins with source and hash) and by file agents (skill names, MCP through tools / disallowedTools).
import { AlertTriangle, Check, ChevronDown, Package, Search, X } from "lucide-react";
import { useId, useState, type KeyboardEvent } from "react";
import { addPins, heavyTokens, MAX_MCP, MAX_SKILLS, MCP_SCOPE_LABEL, pinnedBytes, pinOf, pinState, pluginBundles, separable, sourceLabel, tokenText, useLoadoutOptions, type SkillOption } from "../lib/loadout";
import type { SkillPin } from "../lib/team";
import { Badge, Button, fieldClass } from "./ui";
import { useT } from "../i18n/react";

export interface LoadoutValue {
  skills: SkillPin[];
  mcp: string[];
}

/** Square chip with a remove button; `state` shows "바뀜" (content changed) or "없음" (gone). */
function Chip({ label, badge, state, onRemove }: { label: string; badge?: string; state?: "changed" | "missing"; onRemove?: () => void }) {
  const t = useT();
  return (
    <span className={`inline-flex h-6 max-w-full items-center gap-1 rounded-md border pr-1 pl-2 text-[12px] ${state ? "border-danger/40 text-danger" : "border-line bg-paper text-ink"}`}>
      <span className="truncate font-mono">{label}</span>
      {badge && <Badge>{badge}</Badge>}
      {state && <span className="shrink-0 text-[10px] font-semibold">{state === "changed" ? t("agents.loadout.changed") : t("agents.loadout.missing")}</span>}
      {onRemove && (
        <button type="button" aria-label={t("agents.loadout.remove", { name: label })} onClick={onRemove} className="rounded text-ink-3 hover:text-danger">
          <X size={12} />
        </button>
      )}
    </span>
  );
}

function SkillPicker({ options, value, onChange, onClose }: { options: SkillOption[]; value: SkillPin[]; onChange: (v: SkillPin[]) => void; onClose: () => void }) {
  const t = useT();
  const [q, setQ] = useState("");
  const hit = (o: SkillOption) => !q.trim() || o.name.toLowerCase().includes(q.trim().toLowerCase()) || (o.description ?? "").toLowerCase().includes(q.trim().toLowerCase());
  const own = options.filter((o) => !o.plugin && hit(o));
  const bundles = pluginBundles(options).map((b) => ({ ...b, shown: b.skills.filter(hit) })).filter((b) => b.shown.length > 0);
  const full = value.length >= MAX_SKILLS;
  // Escape closes this panel only: stopped here so the settings sheet's window listener never sees it.
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "Escape" || e.nativeEvent.isComposing) return;
    e.preventDefault();
    e.stopPropagation();
    e.nativeEvent.stopImmediatePropagation();
    onClose();
  };
  const row = (o: SkillOption) => {
    const on = value.some((p) => p.name === o.name);
    return (
      <li key={`${o.source}:${o.name}`}>
        <label className={`flex items-center gap-2 rounded-md px-2 py-1 text-[13px] hover:bg-paper ${!on && full ? "opacity-50" : ""}`}>
          <input type="checkbox" checked={on} disabled={!on && full} onChange={(e) => onChange(e.target.checked ? addPins(value, [o]) : value.filter((p) => p.name !== o.name))} />
          <span className="min-w-0 flex-1 truncate font-mono text-[12px]" title={o.description ?? undefined}>
            {o.name}
          </span>
          {!o.plugin && <Badge>{sourceLabel(o.source)}</Badge>}
          <span className="w-24 shrink-0 text-right text-[11px] text-ink-3 tabular-nums">{tokenText(o.bytes)}</span>
        </label>
      </li>
    );
  };
  return (
    <div className="space-y-2 rounded-lg border border-line bg-sheet p-2" onKeyDown={onKeyDown}>
      <div className="flex items-center gap-2">
        <div className="relative min-w-0 flex-1">
          <Search size={13} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
          <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder={t("agents.loadout.findSkill")} aria-label={t("agents.loadout.findSkill")} className={`${fieldClass} h-8 py-0 pl-8`} />
        </div>
        <Button onClick={onClose}>{t("common.actions.close")}</Button>
      </div>
      {options.length === 0 && <p className="px-2 text-xs text-ink-3">{t("agents.loadout.noSkills")}</p>}
      <div className="max-h-72 space-y-2 overflow-y-auto">
        {own.length > 0 && <ul aria-label={t("agents.loadout.ownSkills")}>{own.map(row)}</ul>}
        {bundles.map((b) => {
          const missing = b.skills.filter((o) => !value.some((p) => p.name === o.name));
          return (
            <div key={b.plugin}>
              <div className="flex items-center gap-2 px-2 py-1">
                <Package size={12} className="shrink-0 text-ink-3" />
                <span className="min-w-0 truncate text-xs font-medium text-ink-2">{b.plugin}</span>
                <span className="min-w-0 flex-1 truncate text-[11px] text-ink-3">{t("agents.loadout.skillsOnly")}</span>
                <Button size="xs" disabled={missing.length === 0 || full} onClick={() => onChange(addPins(value, missing))}>
                  {missing.length > 0 ? t("agents.loadout.addBundleCount", { count: missing.length }) : t("agents.loadout.addBundle")}
                </Button>
              </div>
              <ul aria-label={t("agents.loadout.pluginSkills", { plugin: b.plugin })}>{b.shown.map(row)}</ul>
            </div>
          );
        })}
      </div>
      {full && <p className="px-2 text-[11px] text-ink-3">{t("agents.loadout.maxSkills", { max: MAX_SKILLS })}</p>}
    </div>
  );
}

/**
 * `byName`: file agents keep names only (no hash), so a name counts when any option has it.
 * `toolList`: the role has a tool list, so choosing nothing means no MCP at all.
 * `toolsNote`: file agents with a tool list, the `mcp__서버__*` entries the save adds to it.
 */
export function LoadoutSection({
  value,
  onChange,
  strict = false,
  toolList = false,
  byName = false,
  lock,
  toolsNote = [],
}: {
  value: LoadoutValue;
  onChange: (v: LoadoutValue) => void;
  strict?: boolean;
  toolList?: boolean;
  byName?: boolean;
  lock?: string | null;
  toolsNote?: string[];
}) {
  const t = useT();
  const opts = useLoadoutOptions();
  const [picking, setPicking] = useState(false);
  const panel = useId();
  const options = opts.data?.skills ?? [];
  const unreadableSkills = opts.data?.unreadableSkills ?? [];
  const servers = opts.data?.mcp ?? [];
  const mcpUnread = (opts.data?.unreadable.length ?? 0) > 0;
  const readOnly = !!lock;
  const state = (p: SkillPin) => (byName ? (unreadableSkills.includes(p.name) ? "changed" : options.some((o) => o.name === p.name) ? "ok" : "missing") : pinState(p, options, unreadableSkills));
  const bad = opts.data ? value.skills.filter((p) => state(p) !== "ok") : [];
  const goneServers = opts.data ? value.mcp.filter((m) => !servers.some((s) => s.name === m)) : [];
  const bytes = byName ? value.skills.reduce((n, p) => n + (options.find((o) => o.name === p.name)?.bytes ?? 0), 0) : pinnedBytes(value.skills, options);
  const size = tokenText(bytes);
  const heavy = heavyTokens(bytes);
  const setSkills = (skills: SkillPin[]) => onChange({ ...value, skills });
  const toggleServer = (name: string, on: boolean) => onChange({ ...value, mcp: on ? [...value.mcp, name] : value.mcp.filter((m) => m !== name) });
  const tidy = () => onChange({ skills: value.skills.filter((p) => !bad.includes(p)), mcp: value.mcp.filter((m) => !goneServers.includes(m)) });
  const closePicker = () => {
    setPicking(false);
    document.getElementById(`${panel}-pick`)?.focus();
  };
  const shared = servers.filter((s) => !separable(s.name));
  const now = value.mcp.length > 0 ? t("agents.loadout.nowOnly", { servers: value.mcp.join(", ") }) : toolList ? t("agents.loadout.nowNone") : t("agents.loadout.nowAll");
  // Inheriting roles (and strict ones) refuse to start with chosen servers when the list could not be read.
  const blindStop = mcpUnread && value.mcp.length > 0 && (strict || !toolList);

  return (
    <fieldset className="space-y-3 rounded-lg border border-line p-3" aria-describedby={lock ? `${panel}-lock` : undefined}>
      <legend className="px-1 text-xs font-medium text-ink-2">{t("agents.loadout.title")}</legend>
      {lock && (
        <p id={`${panel}-lock`} className="text-[11px] text-ink-2">
          {lock}
        </p>
      )}
      {opts.isError && <p className="text-[11px] text-danger">{t("agents.loadout.readFailed")}</p>}

      <div className="space-y-1.5">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs font-medium text-ink-2">{t("agents.loadout.skills")}</span>
          {value.skills.length > 0 && size && <span className={`text-[11px] tabular-nums ${heavy ? "font-medium text-warn" : "text-ink-3"}`}>{size}</span>}
          <span className="flex-1" />
          {bad.length + goneServers.length > 0 && !readOnly && (
            <Button size="sm" onClick={tidy}>
              {t("agents.loadout.prune")}
            </Button>
          )}
          {!readOnly && (
            <Button id={`${panel}-pick`} size="sm" aria-expanded={picking} aria-controls={panel} onClick={() => setPicking((v) => !v)}>
              {t("agents.loadout.pickSkills")} <ChevronDown size={12} className={picking ? "rotate-180" : ""} />
            </Button>
          )}
        </div>
        <p className="text-[11px] text-ink-3">{t("agents.loadout.preloadNote")}</p>
        {value.skills.length > 0 && heavy && <p className="text-[11px] text-warn">{t("agents.loadout.heavy")}</p>}
        <div className="flex flex-wrap gap-1">
          {value.skills.length === 0 && <span className="text-xs text-ink-3">{t("agents.loadout.noneChosen")}</span>}
          {value.skills.map((p) => {
            const st = state(p);
            return (
              <Chip
                key={p.name}
                label={p.name}
                badge={byName || p.source.startsWith("plugin:") ? undefined : sourceLabel(p.source)}
                state={st === "ok" ? undefined : st}
                onRemove={readOnly ? undefined : () => setSkills(value.skills.filter((x) => x.name !== p.name))}
              />
            );
          })}
        </div>
        {bad.some((p) => state(p) === "changed") && <p className="text-[11px] text-danger">{t("agents.loadout.repick")}</p>}
        {picking && !readOnly && (
          <div id={panel}>
            <SkillPicker options={options} value={value.skills} onChange={setSkills} onClose={closePicker} />
          </div>
        )}
      </div>

      <div className="space-y-1.5 border-t border-line pt-3">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs font-medium text-ink-2">{t("agents.loadout.mcpServers")}</span>
          <span className="text-[11px] text-ink-2">{now}</span>
          {mcpUnread && (
            <span className="inline-flex h-5 items-center gap-1 rounded-md border border-warn/40 px-1.5 text-[11px] font-medium text-warn">
              <AlertTriangle size={11} aria-hidden />
              {blindStop ? t("agents.loadout.mcpBlindStop") : t("agents.loadout.mcpReadFailed")}
            </span>
          )}
        </div>
        <p className="text-[11px] text-ink-3">{toolList ? t("agents.loadout.onlyChosenNone") : t("agents.loadout.onlyChosenAll")}</p>
        {opts.data && servers.length === 0 && <p className="text-xs text-ink-3">{t("agents.loadout.noServers")}</p>}
        <div className="flex flex-wrap gap-1.5">
          {servers.map((s) => {
            const on = value.mcp.includes(s.name);
            const blocked = !separable(s.name);
            const full = !on && value.mcp.length >= MAX_MCP;
            return (
              <label
                key={s.name}
                title={blocked ? t("agents.loadout.cantBlock") : undefined}
                className={`inline-flex h-7 items-center gap-1.5 rounded-md border px-2 text-[12px] has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ink has-[:focus-visible]:ring-offset-1 ${on ? "border-ink bg-ink/5 font-medium text-ink" : "border-line text-ink"} ${blocked || full || readOnly ? "opacity-50" : "cursor-pointer hover:border-ink-3"}`}
              >
                <input type="checkbox" className="sr-only" checked={on} disabled={blocked || full || readOnly} onChange={(e) => toggleServer(s.name, e.target.checked)} />
                {on ? <Check size={12} aria-hidden className="shrink-0" /> : <span aria-hidden className="h-3 w-3 shrink-0 rounded-sm border border-ink-3/60" />}
                <span className="font-mono">{s.name}</span>
                <Badge>{MCP_SCOPE_LABEL[s.scope] ?? s.scope}</Badge>
                {blocked && <AlertTriangle size={11} aria-label={t("agents.loadout.cantBlockShort")} />}
              </label>
            );
          })}
          {goneServers.map((m) => (
            <Chip key={m} label={m} state="missing" onRemove={readOnly ? undefined : () => toggleServer(m, false)} />
          ))}
        </div>
        {shared.length > 0 && <p className="text-[11px] text-ink-3">{t("agents.loadout.sharedNames", { names: shared.map((s) => s.name).join(", ") })}</p>}
        {toolsNote.length > 0 && <p className="text-[11px] text-ink-2">{t("agents.loadout.toolsNote", { tools: toolsNote.join(", ") })}</p>}
        {!toolList && <p className="text-[11px] text-ink-3">{t("agents.loadout.outsideList")}</p>}
      </div>
      {!byName && !readOnly && <p className="text-[11px] text-ink-2">{t("agents.loadout.confirmNote")}</p>}
    </fieldset>
  );
}

/** File agents store names only: shape them as pins the section can show. */
export function namesToPins(names: string[], options: SkillOption[]): SkillPin[] {
  return names.map((n) => {
    const o = options.find((x) => x.name === n);
    return o ? pinOf(o) : { name: n, source: "", hash: "" };
  });
}
