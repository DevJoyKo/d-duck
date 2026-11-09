import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ChevronRight, Lock } from "lucide-react";
import { useState, type ReactNode } from "react";
import { api, type Plugin, type PluginItem, type PluginOutcome, failure, shown, type Shown } from "../lib/api";
import { ConnectorIcon } from "../lib/connectorIcons";
import { dateTime } from "../lib/format";
import { t } from "../i18n";
import { currentLocale, subscribe } from "../i18n/language";
import { useT } from "../i18n/react";
import { Badge, Button } from "./ui";
import { transportLabel } from "../lib/mcpNames";

const SCOPE_KEY = { user: "user", project: "project", local: "local", managed: "managed", other: "unknown" } as const satisfies Record<Plugin["scope"], string>;
const scopeLabel = (s: Plugin["scope"]) => t(`extensions.plugins.scope.${SCOPE_KEY[s]}`);

type Note = { tone: "ok" | "warn" | "error"; text: Shown };

function noteFor(
  p: Plugin,
  o: PluginOutcome,
  verb: "toggle" | "update",
): Note | null {
  switch (o.kind) {
    case "done":
      return {
        tone: "ok",
        text: t(verb === "update" ? "extensions.plugins.note.updated" : p.enabled ? "extensions.plugins.note.turnedOff" : "extensions.plugins.note.turnedOn"),
      };
    case "needsApproval":
      return {
        tone: "warn",
        text: t("extensions.plugins.note.needsApproval", { id: p.id }),
      };
    case "locked":
      return {
        tone: "warn",
        text: t("extensions.plugins.note.locked"),
      };
    case "unchanged":
      return {
        tone: "warn",
        text:
          verb === "update"
            ? t("extensions.plugins.note.unchangedUpdate")
            : t("extensions.plugins.note.unchangedToggle"),
      };
    case "timeout":
      return {
        tone: "error",
        text: t("extensions.plugins.note.timeout"),
      };
    case "failed":
      return {
        tone: "error",
        text: t("extensions.plugins.note.failed"),
      };
    case "cancelled":
      return null;
  }
}

function Toggle({
  checked,
  disabled,
  onChange,
  label,
}: {
  checked: boolean;
  disabled?: boolean;
  onChange: (v: boolean) => void;
  label: string;
}) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-10 shrink-0 rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${checked ? "bg-ok" : "bg-ink-3/40"}`}
    >
      <span
        className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-all ${checked ? "left-[18px]" : "left-0.5"}`}
      />
    </button>
  );
}

/** "11. 07." (ko) or "11/07" (en) — when Claude Code last refreshed the local marketplace copy. */
function monthDay(iso: string | null): string | null {
  const d = iso ? new Date(iso) : null;
  if (!d || Number.isNaN(d.getTime())) return null;
  return new Intl.DateTimeFormat(currentLocale(), { month: "2-digit", day: "2-digit" }).format(d);
}

/** Item name without the "<plugin>:" prefix (full name stays in the tooltip). */
function shortName(plugin: string, name: string): string {
  return name.startsWith(`${plugin}:`) ? name.slice(plugin.length + 1) : name;
}

function day(iso: string | null): string {
  const t = iso ? Date.parse(iso) : NaN;
  return Number.isNaN(t) ? "-" : dateTime(t);
}

function counts(p: Plugin): string {
  const c = p.components;
  const parts = [
    c.skills.length && t("extensions.plugins.counts.skills", { count: c.skills.length }),
    c.commands.length && t("extensions.plugins.counts.commands", { count: c.commands.length }),
    c.agents.length && t("extensions.plugins.counts.agents", { count: c.agents.length }),
    c.hooks && t("extensions.plugins.counts.hooks"),
    p.mcp.length && `MCP ${p.mcp.length}`,
    c.lsp && "LSP",
  ].filter(Boolean);
  return parts.length
    ? parts.join(" · ")
    : c.unreadable
      ? t("extensions.plugins.counts.unreadable")
      : t("extensions.plugins.counts.none");
}

function UpdateCell({
  p,
  pending,
  busy,
  onUpdate,
}: {
  p: Plugin;
  pending: boolean;
  busy: boolean;
  onUpdate: () => void;
}) {
  const t = useT();
  const u = p.update;
  const label = pending
    ? t("extensions.plugins.update.updating")
    : u.state === "available"
      ? t("extensions.plugins.update.available", { version: u.latest ?? "" })
      : u.state === "latest"
        ? t("extensions.plugins.update.latest")
        : t("extensions.plugins.update.try");
  const at = monthDay(u.marketplaceUpdated);
  return (
    <div className="flex w-40 shrink-0 flex-col items-end gap-0.5">
      <Button
        variant={u.state === "available" ? "primary" : "quiet"}
        disabled={busy || !!p.locked || u.state === "latest"}
        onClick={onUpdate}
        size="sm"
      >
        {label}
      </Button>
      <span
        className="text-[11px] text-ink-3"
        title={t("extensions.plugins.update.basisTitle")}
      >
        {u.state === "unknown"
          ? t("extensions.plugins.update.unknown")
          : at
            ? t("extensions.plugins.update.basisAt", { date: at })
            : t("extensions.plugins.update.basis")}
      </span>
    </div>
  );
}

/** A live binding, refreshed on a language change (tests compare the title with it). */
export let NO_RECORD_TIP = t("extensions.plugins.uses.noneTitle");
subscribe(() => {
  NO_RECORD_TIP = t("extensions.plugins.uses.noneTitle");
});

/** Usage of a skill or command (D-147): "N회" or "기록 없음" — never read as on/off. */
export function Uses({ uses, lastUsed, className = "w-16" }: { uses: number; lastUsed: number | null; className?: string }) {
  const t = useT();
  return (
    <span className={`${className} shrink-0 text-right text-xs text-ink-3 tabular-nums`} title={uses ? (lastUsed ? t("extensions.plugins.uses.last", { date: dateTime(lastUsed) }) : undefined) : t("extensions.plugins.uses.noneTitle")}>
      {uses ? t("extensions.plugins.uses.count", { count: uses }) : t("extensions.plugins.uses.none")}
    </span>
  );
}

export function ItemList({
  plugin,
  title,
  items,
  prefix = "",
}: {
  plugin: string;
  title: string;
  items: PluginItem[];
  prefix?: string;
}) {
  if (items.length === 0) return null;
  return (
    <div>
      <h4 className="mb-1 text-xs font-semibold text-ink-2">
        {title} <span className="font-normal text-ink-3">{items.length}</span>
      </h4>
      <ul className="divide-y divide-line rounded-lg border border-line bg-paper">
        {items.map((i) => (
          <li
            key={i.name}
            className="flex items-baseline gap-3 px-3 py-1.5 text-[12px]"
          >
            <span
              className="w-56 shrink-0 truncate font-mono font-medium"
              title={`${prefix}${i.name}`}
            >
              {prefix}
              {shortName(plugin, i.name)}
            </span>
            <span
              className="min-w-0 flex-1 truncate text-ink-2"
              title={i.description ?? undefined}
            >
              {i.description ?? "-"}
            </span>
            {prefix !== "@" && <Uses uses={i.uses} lastUsed={i.lastUsed} className="w-20" />}
          </li>
        ))}
      </ul>
    </div>
  );
}

function Details({ p, items }: { p: Plugin; items?: ReactNode }) {
  const t = useT();
  const c = p.components;
  return (
    <div className="space-y-3 border-t border-line bg-sheet px-4 py-3 pl-14">
      {items === undefined ? (
        <>
          <ItemList plugin={p.name} title={t("extensions.kinds.skill")} items={c.skills} />
          <ItemList plugin={p.name} title={t("extensions.kinds.command")} items={c.commands} prefix="/" />
        </>
      ) : (
        items
      )}
      <ItemList plugin={p.name} title={t("extensions.plugins.details.agents")} items={c.agents} prefix="@" />
      <dl className="flex flex-wrap gap-x-6 gap-y-1 text-xs text-ink-3">
        {p.mcp.length > 0 && (
          <div>
            <dt className="inline">{t("extensions.plugins.details.mcpServers")} </dt>
            <dd className="inline text-ink-2">
              {p.mcp
                .map((m) => (m.kind ? `${m.name} (${transportLabel(m.kind)})` : m.name))
                .join(", ")}
            </dd>
          </div>
        )}
        {c.hooks && <div className="text-ink-2">{t("extensions.plugins.details.hooks")}</div>}
        {c.lsp && <div className="text-ink-2">{t("extensions.plugins.details.lsp")}</div>}
        {p.installedAt !== p.lastUpdated && (
          <div>
            <dt className="inline">{t("extensions.plugins.details.installed")} </dt>
            <dd className="inline text-ink-2">{day(p.installedAt)}</dd>
          </div>
        )}
      </dl>
      {c.unreadable && (
        <p className="text-xs text-ink-3">
          {t("extensions.plugins.details.unreadable")}
        </p>
      )}
    </div>
  );
}

/** Toggle/update with per-plugin notes, shared by the plugin rows of the 확장 page (nav-ia). */
export function usePluginActions() {
  const qc = useQueryClient();
  const [notes, setNotes] = useState<Record<string, Note>>({});
  const note = (id: string, n: Note | null) =>
    setNotes((m) => {
      const next = { ...m };
      if (n) next[id] = n;
      else delete next[id];
      return next;
    });
  const after = (p: Plugin, verb: "toggle" | "update", on?: boolean) => ({
    onSuccess: (o: PluginOutcome) => {
      note(p.id, noteFor(p, o, verb));
      // Keep the switch where the user put it until the re-read list arrives (no flicker back).
      if (o.kind === "done" && on !== undefined) qc.setQueryData<Plugin[]>(["plugins"], (l) => l?.map((x) => (x.id === p.id ? { ...x, enabled: on } : x)));
    },
    onError: (e: unknown) => note(p.id, { tone: "error", text: failure(e) }),
    onSettled: () => qc.invalidateQueries({ queryKey: ["plugins"] }),
  });
  const toggle = useMutation({ mutationFn: ({ p, on }: { p: Plugin; on: boolean }) => api.setPluginEnabled(p.id, on) });
  const update = useMutation({ mutationFn: (p: Plugin) => api.updatePlugin(p.id) });
  const busyId = toggle.isPending ? toggle.variables?.p.id : update.isPending ? update.variables?.id : undefined;
  return { notes, after, toggle, update, busyId };
}

export type PluginActions = ReturnType<typeof usePluginActions>;

/** One plugin in the 확장 list: expand for its skills, commands, agents, hooks and MCP; update and on/off at the right. */
/** `items`: what to show instead of the plain skill and command lists when expanded (확장 adds lock and copy). */
export function PluginRow({ p, actions, expanded, onExpand, badges, items }: { p: Plugin; actions: PluginActions; expanded: boolean; onExpand: () => void; badges?: ReactNode; items?: ReactNode }) {
  const t = useT();
  const { notes, after, toggle, update, busyId } = actions;
  const n = notes[p.id];
  const busy = busyId === p.id;
  return (
    <>
      <div className="flex items-center gap-3 px-4 py-3">
        <button aria-expanded={expanded} aria-label={t(expanded ? "extensions.plugins.row.collapse" : "extensions.plugins.row.expand", { name: p.name })} onClick={onExpand} className="flex min-w-0 flex-1 items-center gap-3 text-left">
          <ChevronRight size={14} className={`shrink-0 text-ink-3 transition-transform ${expanded ? "rotate-90" : ""}`} />
          <ConnectorIcon name={p.name} host={null} />
          <span className="min-w-0 flex-1">
            <span className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
              <span className={`truncate text-[13px] font-semibold ${p.enabled ? "" : "text-ink-3"}`}>{p.name}</span>
              <span className="truncate text-xs text-ink-3">@{p.marketplace}</span>
              <span className="font-mono text-[11px] text-ink-2">{p.version ?? "-"}</span>
              <Badge>{scopeLabel(p.scope)}</Badge>
              {p.projectEnabled && !p.enabled && (
                <Badge pill>{t("extensions.plugins.row.projectOn")}</Badge>
              )}
              {badges}
            </span>
            <span className="mt-0.5 block truncate text-xs text-ink-3" title={`${counts(p)} · ${t("extensions.plugins.row.updated", { date: day(p.lastUpdated) })}`}>
              {counts(p)} · {t("extensions.plugins.row.updated", { date: day(p.lastUpdated) })}
            </span>
          </span>
        </button>
        <UpdateCell p={p} pending={busy && update.isPending} busy={busyId !== undefined} onUpdate={() => update.mutate(p, after(p, "update"))} />
        <div className="flex w-14 shrink-0 flex-col items-center gap-0.5">
          <span className="flex items-center gap-1">
            {p.locked && <Lock size={11} className="text-ink-3" aria-hidden />}
            <Toggle
              label={t(p.enabled ? "extensions.plugins.row.turnOff" : "extensions.plugins.row.turnOn", { name: p.name })}
              checked={busy && toggle.isPending ? toggle.variables.on : p.enabled}
              disabled={!!p.locked || busyId !== undefined}
              onChange={(v) => toggle.mutate({ p, on: v }, after(p, "toggle", v))}
            />
          </span>
          <span className="text-[11px] text-ink-3">{p.enabled ? t("extensions.plugins.row.on") : t("extensions.plugins.row.off")}</span>
        </div>
      </div>
      {(p.locked || n) && (
        <div className="space-y-1 px-4 pb-3 pl-[76px] text-xs">
          {p.locked && (
            <p className="flex items-center gap-1 text-ink-3">
              <Lock size={11} /> {p.locked}
            </p>
          )}
          {n && (
            <p role={n.tone === "error" ? "alert" : "status"} className={n.tone === "ok" ? "text-ok" : n.tone === "error" ? "text-danger" : "text-ink"}>
              {shown(n.text)}
            </p>
          )}
        </div>
      )}
      {expanded && <Details p={p} items={items} />}
    </>
  );
}

/** One line under the plugin rows: what toggling and updating need. */
export function pluginHelp(): string {
  return t("extensions.plugins.help");
}
