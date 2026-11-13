// 확장 (nav-ia, D-112, D-130): skills, commands, workflows and plugins in one page. "전체" is grouped by kind with a
// header each, plugins last. Filter chips narrow the kind, a search matches name and description, each row says where
// it comes from (전역 · 프로젝트 · 내장 · claude.ai). What a plugin brings is listed only inside that plugin, where its
// locked skills and commands can be copied with "복사해서 내 것으로" (D-117). The scope bar reads
// [전역 | <project>] like the other tabs: 전역 keeps what is not tied to a project, the project side keeps the current
// project's items (D-132). Chip counts are row counts, the same basis as the section headers under that chip.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Copy, Folder, Lock, RefreshCw, Search } from "lucide-react";
import { useState, type ReactNode } from "react";
import { t } from "../i18n";
import { useT } from "../i18n/react";
import { api, type Plugin, type PluginItem, type Skill, failure, messageOf, shown, type Shown } from "../lib/api";
import { addApi, isNew, pendingReviews } from "../lib/addItems";
import { metaReasonText, registeredText } from "../lib/workflowCheck";
import type { ExtChip } from "../lib/nav";
import { AddMenu, AddSheet, type AddMode } from "./AddSheet";
import { AuthoringEditor } from "./AuthoringEditor";
import { pluginHelp, PluginRow, Uses, usePluginActions } from "./PluginsPage";
import { PluginBulkBar } from "./PluginBulk";
import { useBulk } from "../lib/pluginBulk";
import { useProject, useScope } from "./ProjectContext";
import { ScopeBar } from "./ScopeBar";
import { Badge, Button, Empty, PageBody, PageHeader, Segmented, Sheet, fieldClass } from "./ui";
import { splitPath } from "../lib/platform";

type Kind = Exclude<ExtChip, "all">;
const CHIPS: ExtChip[] = ["all", "skill", "command", "workflow", "plugin"];
const GROUP_ORDER: Kind[] = ["skill", "command", "workflow", "plugin"];

/** A skill or command a plugin brings, joined with its catalog row (editor id, lock) when there is one. */
export interface Inner {
  kind: "skill" | "command";
  name: string;
  description: string | null;
  uses: number;
  lastUsed: number | null;
  skill?: Skill;
}

export interface ExtRow {
  key: string;
  kind: Kind;
  name: string;
  description: string | null;
  /** 전역 · 프로젝트 · 내장 · claude.ai (or the plugin name when that plugin is not installed any more) */
  source: string;
  enabled: boolean;
  /** Belongs to the current project: its own `.claude`, or a plugin set at project scope or turned on there. */
  current: boolean;
  /** Not tied to a project: 전역·내장·claude.ai items and plugins installed for all projects. */
  global: boolean;
  /** Plugin item whose plugin is off. */
  pluginOff: boolean;
  lastUsed: number | null;
  skill?: Skill;
  plugin?: Plugin;
  /** Plugin rows: what it brings. */
  inner?: Inner[];
}

const pluginOf = (s: Skill) => (s.source === "plugin" && s.name.includes(":") ? s.name.split(":")[0] : null);
const shortName = (name: string) => (name.includes(":") ? name.slice(name.indexOf(":") + 1) : name);
const NAME_RE = /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/;
export function deletedNote(kind: Skill["kind"]): string {
  return t(`extensions.deleted.${kind}`);
}
/** What was deleted in this run, by catalog id: the catalog cannot tell a deleted item's usage row from a built-in
 * one, and lists that row as a skill (D-147). */
export interface DeletedItem {
  id: string;
  name: string;
  kind: Skill["kind"];
}
const deletedThisRun = new Map<string, DeletedItem>();

/** Usage-only rows (no definition left) of items deleted in this run get the deleted item's kind back. Rows that
 * still have a definition are never marked, so a same-named skill elsewhere stays as it is. Returns the skills and
 * the row keys to mark. */
export function withDeleted(skills: Skill[], deleted: DeletedItem[]): { skills: Skill[]; marked: Set<string> } {
  const marked = new Set<string>();
  const out = skills.map((s) => {
    if (s.defined || s.id || skills.some((o) => o.defined && o.name === s.name)) return s;
    const d = deleted.find((x) => x.name === s.name && x.kind !== "workflow");
    if (!d) return s;
    const t = { ...s, kind: d.kind };
    marked.add(`${t.kind}|${t.source}|${t.scope ?? ""}|${t.name}`);
    return t;
  });
  return { skills: out, marked };
}

/** Row buttons (D-147 design 2): own items "수정"·"삭제", read-only "보기" (+ copy for skills and commands), none
 * without a definition file. Only a convenience — Rust decides what may be saved or deleted. */
export function rowActions(s: Skill): {
  /** The button label ("수정" or "보기") in the current language. */
  edit: string | null;
  del: boolean;
  copy: boolean;
} {
  const view = t("extensions.row.view");
  if (!s.id) return { edit: null, del: false, copy: false };
  if (s.readonly) return { edit: view, del: false, copy: s.kind !== "workflow" };
  const own = s.source === "user" || s.source === "project";
  return { edit: own ? t("extensions.row.edit") : view, del: own, copy: false };
}

function sourceLabel(s: Skill): string {
  if (s.source === "plugin") return pluginOf(s) ?? t("extensions.source.plugin");
  if (s.source === "builtin") return t("extensions.source.builtin");
  if (s.source === "project") return t("extensions.source.project", { folder: (s.scope ? splitPath(s.scope).pop() : undefined) ?? "" });
  return s.readonly ? "claude.ai" : t("extensions.source.global");
}

const byUse = (a: ExtRow, b: ExtRow) => Number(b.enabled) - Number(a.enabled) || (b.lastUsed ?? 0) - (a.lastUsed ?? 0) || a.name.localeCompare(b.name);

/**
 * One list model (nav-ia design 1): on first, then most recently used, then name. A plugin's own skills and commands
 * live only in its `inner` list (no duplicate top-level row). `pluginOrder` keeps plugin rows where they were when the
 * page loaded, so toggling one does not move it (review LOW).
 */
export function extRows(skills: Skill[], plugins: Plugin[], currentPath: string | null, pluginOrder?: string[]): ExtRow[] {
  const installed = new Map(plugins.map((p) => [p.name, p]));
  const rows: ExtRow[] = [];
  for (const s of skills) {
    const plugin = pluginOf(s);
    if (plugin && installed.has(plugin)) continue;
    rows.push({
      key: `${s.kind}|${s.source}|${s.scope ?? ""}|${s.name}`,
      kind: s.kind,
      name: s.name,
      description: s.description,
      source: sourceLabel(s),
      enabled: true,
      // A parent folder's `.claude` applies to its subfolders too, as in Claude Code (D-169).
      current: s.source === "project" && !!currentPath && !!s.scope && (s.scope === currentPath || currentPath.startsWith(`${s.scope}/`)),
      global: s.source !== "project",
      pluginOff: false,
      lastUsed: s.lastUsed,
      skill: s,
    });
  }
  rows.sort(byUse);
  const pluginRows: ExtRow[] = plugins.map((p) => {
    const own = skills.filter((s) => pluginOf(s) === p.name);
    const find = (kind: Inner["kind"], name: string) => own.find((s) => s.kind === kind && s.name === name);
    const inner: Inner[] = [
      ...p.components.skills.map((i) => ({
        ...item("skill", i),
        skill: find("skill", i.name),
      })),
      ...p.components.commands.map((i) => ({
        ...item("command", i),
        skill: find("command", i.name),
      })),
    ];
    // Catalog rows the plugin listing missed (unreadable parts) still show inside it.
    for (const s of own)
      if ((s.kind === "skill" || s.kind === "command") && !inner.some((i) => i.kind === s.kind && i.name === s.name))
        inner.push({
          kind: s.kind,
          name: s.name,
          description: s.description,
          uses: s.uses,
          lastUsed: s.lastUsed,
          skill: s,
        });
    const on = p.enabled || p.projectEnabled;
    return {
      key: `plugin|${p.id}`,
      kind: "plugin",
      name: p.name,
      description: null,
      source: `@${p.marketplace}`,
      enabled: p.enabled,
      current: p.scope === "project" || p.scope === "local" || p.projectEnabled,
      global: p.scope !== "project" && p.scope !== "local",
      pluginOff: !on,
      lastUsed: null,
      plugin: p,
      inner,
    };
  });
  const at = (r: ExtRow) => {
    const i = pluginOrder?.indexOf(r.plugin?.id ?? "") ?? -1;
    return i < 0 ? Number.MAX_SAFE_INTEGER : i;
  };
  pluginRows.sort((a, b) => (pluginOrder ? at(a) - at(b) : 0) || byUse(a, b));
  return [...rows, ...pluginRows];
}

function item(kind: Inner["kind"], i: PluginItem): Inner {
  return {
    kind,
    name: i.name,
    description: i.description,
    uses: i.uses,
    lastUsed: i.lastUsed,
  };
}

const hit = (q: string, name: string, description: string | null) => !q || name.toLowerCase().includes(q) || (description ?? "").toLowerCase().includes(q);

/** A plugin's items that match the chip and the search (all of them when the plugin's own name matches). */
export function innerFor(r: ExtRow, chip: ExtChip, query: string): Inner[] {
  const q = query.trim().toLowerCase();
  const nameHit = !!q && r.name.toLowerCase().includes(q);
  return (r.inner ?? []).filter((i) => (chip === "all" || chip === "plugin" || i.kind === chip) && (nameHit || hit(q, i.name, i.description)));
}

export function filterRows(rows: ExtRow[], chip: ExtChip, query: string, side: "global" | "project"): ExtRow[] {
  const q = query.trim().toLowerCase();
  return rows.filter((r) => {
    if (side === "project" ? !r.current : !r.global) return false;
    if (r.kind !== "plugin") return (chip === "all" || r.kind === chip) && hit(q, r.name, r.description);
    // A plugin shows under 전체·플러그인, and under 스킬·커맨드 when it brings one that matches.
    if (chip === "workflow") return false;
    if (chip === "skill" || chip === "command") return innerFor(r, chip, query).length > 0;
    return hit(q, r.name, r.description) || innerFor(r, chip, query).length > 0;
  });
}

/** Chip counts with the scope and the search applied: the rows that chip shows, so they add up to its section headers
 * (a plugin that brings matching skills is one row; its own count is on the row). */
export function countFor(rows: ExtRow[], chip: ExtChip, query: string, side: "global" | "project"): number {
  return filterRows(rows, chip, query, side).length;
}

/** `frontend-design-custom`, or `-2`, `-3`, … when that name is taken. */
export function copyName(base: string, taken: string[]): string {
  const set = new Set(taken.map((t) => t.toLowerCase()));
  const first = `${base}-custom`.slice(0, 64);
  if (!set.has(first.toLowerCase())) return first;
  for (let i = 2; i < 1000; i += 1) if (!set.has(`${first}-${i}`.toLowerCase())) return `${first}-${i}`;
  return first;
}

/** Where the copy lands, shown under the form: `~/.claude/skills/<이름>` or the project folder. */
export function copyTarget(kind: Skill["kind"], dest: "user" | "project", name: string, projectPath: string | null): string {
  const n = name || t("extensions.copy.namePlaceholder");
  const leaf = kind === "command" ? `commands/${n}.md` : `skills/${n}`;
  return dest === "project" && projectPath ? `${projectPath.replace(/\/$/, "")}/.claude/${leaf}` : `~/.claude/${leaf}`;
}

function CopyPanel({ s, all, onDone, onCancel }: { s: Skill; all: Skill[]; onDone: (id: string | null, name: string) => void; onCancel: () => void }) {
  const t = useT();
  const { project, projectId } = useScope("folder");
  const [dest, setDest] = useState<"user" | "project">("user");
  const taken = all.filter((x) => x.kind === s.kind && (dest === "user" ? x.source === "user" : x.source === "project" && x.scope === project?.path)).map((x) => x.name);
  const [name, setName] = useState(() => copyName(shortName(s.name), taken));
  const qc = useQueryClient();
  const clash = taken.some((t) => t.toLowerCase() === name.toLowerCase());
  const bad = !NAME_RE.test(name);
  const copy = useMutation({
    mutationFn: () => api.copyItem(s.id ?? "", dest, name, dest === "project" ? projectId : null),
    onSuccess: (r) => {
      qc.invalidateQueries({ queryKey: ["skills"] });
      qc.invalidateQueries({ queryKey: ["authoring-info"] });
      onDone(r.id, name);
    },
  });
  return (
    <div
      role="group"
      aria-label={t("extensions.copy.title")}
      className="mx-4 mb-3 space-y-2 rounded-lg border border-line bg-paper p-3 text-xs"
      onKeyDown={(e) => {
        if (e.key === "Escape" && !e.nativeEvent.isComposing) onCancel();
      }}
    >
      <div className="flex flex-wrap items-center gap-2">
        <Segmented
          label={t("extensions.copy.dest")}
          value={dest}
          onChange={setDest}
          options={[{ id: "user", label: t("extensions.copy.global") }, ...(projectId && project ? [{ id: "project" as const, label: t("extensions.copy.project", { name: project.name }) }] : [])]}
        />
        <label className="flex min-w-0 flex-1 basis-56 items-center gap-2 whitespace-nowrap text-ink-2">
          {t("extensions.copy.name")}
          <input value={name} onChange={(e) => setName(e.target.value.trim())} aria-label={t("extensions.copy.newName")} className={`${fieldClass} h-8 min-w-0 flex-1 py-0 font-mono`} />
        </label>
      </div>
      <p className="truncate font-mono text-[11px] text-ink-2" title={copyTarget(s.kind, dest, name, project?.path ?? null)}>
        → {copyTarget(s.kind, dest, name, dest === "project" ? (project?.path ?? null) : null)}
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <p className={`min-w-0 flex-1 ${bad || clash ? "text-danger" : "text-ink-3"}`}>
          {bad ? t("extensions.copy.badName") : clash ? t("extensions.copy.clash") : dest === "project" ? t("extensions.copy.projectNote") : t("extensions.copy.folderNote")}
        </p>
        <Button onClick={onCancel}>{t("common.actions.cancel")}</Button>
        <Button variant="primary" disabled={bad || clash || copy.isPending} onClick={() => copy.mutate()}>
          {copy.isPending ? t("extensions.copy.copying") : t("extensions.copy.copy")}
        </Button>
      </div>
      {copy.isError && <p className="text-danger">{String(copy.error)}</p>}
    </div>
  );
}

function CopyButton({ onClick }: { onClick: () => void }) {
  const t = useT();
  return (
    <Button size="compact" onClick={onClick} title={t("extensions.copy.buttonTitle")}>
      <Copy size={12} /> {t("extensions.copy.title")}
    </Button>
  );
}

type Deleting = { busy: boolean; onDelete: () => void };

function ItemLine({ r, fresh, deleted, del, onEdit, onCopy }: { r: ExtRow; fresh: boolean; deleted: boolean; del: Deleting; onEdit: () => void; onCopy: () => void }) {
  const t = useT();
  const s = r.skill as Skill;
  const label = s.kind === "command" ? `/${s.name}` : s.name;
  const a = rowActions(s);
  // Claude Code skips this workflow file (D-183): a mark and the reason replace the description.
  const unreadable = s.kind === "workflow" && s.ccReadable === false;
  return (
    <div className={`flex items-start gap-3 px-4 py-2.5 ${r.current ? "border-l-2 border-l-ink bg-paper/60 pl-[14px]" : ""}`}>
      <span className="min-w-0 flex-1">
        <span className="flex flex-wrap items-center gap-1.5">
          {s.id ? (
            <button className="truncate text-left font-mono text-[12px] font-medium hover:underline" title={`${s.name} — ${a.edit ?? t("extensions.row.view")}`} onClick={onEdit}>
              {label}
            </button>
          ) : (
            <span className="truncate font-mono text-[12px] font-medium">{label}</span>
          )}
          {s.readonly && <Lock size={11} className="shrink-0 text-ink-3" aria-label={t("extensions.row.locked")} />}
          <Badge>{s.copiedFrom ? t("extensions.row.copiedFrom", { source: r.source, from: s.copiedFrom.split(":").pop() ?? "" }) : deleted && !s.defined ? t("extensions.row.recordOnly") : r.source}</Badge>
          {r.current && (
            <Badge>
              <Folder size={10} className="mr-1" />
              {t("extensions.row.thisProject")}
            </Badge>
          )}
          {fresh && (
            <Badge pill tone="ok">
              {t("extensions.row.fresh")}
            </Badge>
          )}
          {unreadable && <Badge tone="danger">{t("extensions.wfcheck.unreadable")}</Badge>}
          {!unreadable && s.registeredName && <Badge>{t("extensions.wfcheck.registeredBadge")}</Badge>}
        </span>
        {unreadable ? (
          <span className="mt-0.5 line-clamp-1 block text-xs text-danger">{s.reason ? metaReasonText(s.reason, s.name) : "-"}</span>
        ) : (
          <span className="mt-0.5 line-clamp-1 block text-xs text-ink-2">{s.description ?? (s.defined ? "-" : deleted ? deletedNote(s.kind) : t("extensions.row.noDefinition"))}</span>
        )}
        {s.registeredName && <span className="mt-0.5 line-clamp-1 block text-xs text-ink-3">{registeredText(s.registeredName)}</span>}
      </span>
      {/* Fixed slots, so 수정/Edit, 삭제/Delete (or the copy button) and the use count each line up in one column
          down the list; a row without one keeps the gap (design review). */}
      <span className="flex shrink-0 items-center gap-2" data-row-actions>
        <span className="flex w-[4.5rem] justify-end">
          {a.edit && (
            <Button size="compact" onClick={onEdit} className="w-full justify-center">
              {a.edit}
            </Button>
          )}
        </span>
        <span className="flex w-[7.5rem] justify-end">
          {a.del ? (
            <Button variant="danger" size="compact" disabled={del.busy} onClick={del.onDelete} title={t("extensions.row.deleteTitle")} className="w-[4.5rem] justify-center">
              {del.busy ? t("extensions.row.deleting") : t("extensions.row.delete")}
            </Button>
          ) : (
            a.copy && <CopyButton onClick={onCopy} />
          )}
        </span>
        <span className="flex w-16 justify-end">{s.kind !== "workflow" && <Uses uses={s.uses} lastUsed={s.lastUsed} />}</span>
      </span>
    </div>
  );
}

/** One skill or command inside an expanded plugin: locked, viewable, copyable. "꺼짐" sits apart from the usage. */
export function InnerLine({ plugin, i, off, onEdit, onCopy }: { plugin: string; i: Inner; off: boolean; onEdit: () => void; onCopy: () => void }) {
  const t = useT();
  const label = `${i.kind === "command" ? "/" : ""}${i.name.startsWith(`${plugin}:`) ? i.name.slice(plugin.length + 1) : i.name}`;
  const s = i.skill;
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1 px-3 py-1.5 text-[12px]">
      {/* Only the name and description fade when the plugin is off; the chip and the usage keep full contrast. */}
      <span className={`flex w-40 shrink-0 items-center gap-1.5 lg:w-52 ${off ? "opacity-60" : ""}`}>
        {s?.id ? (
          <button className="truncate text-left font-mono font-medium hover:underline" title={`${i.name} — ${s.readonly ? t("extensions.row.view") : t("extensions.row.edit")}`} onClick={onEdit}>
            {label}
          </button>
        ) : (
          <span className="truncate font-mono font-medium" title={i.name}>
            {label}
          </span>
        )}
        <Lock size={11} className="shrink-0 text-ink-3" aria-label={t("extensions.row.locked")} />
      </span>
      <span className={`min-w-0 flex-1 basis-32 truncate text-ink-2 ${off ? "opacity-60" : ""}`} title={i.description ?? undefined}>
        {i.description ?? "-"}
      </span>
      <span className="ml-auto flex shrink-0 items-center gap-3">
        {s?.id && (
          <Button size="compact" onClick={onEdit}>
            {t("extensions.row.view")}
          </Button>
        )}
        {s?.id && s.readonly && <CopyButton onClick={onCopy} />}
        {off && (
          <Badge pill title={t("extensions.row.pluginOffTitle")}>
            {t("extensions.row.off")}
          </Badge>
        )}
        <Uses uses={i.uses} lastUsed={i.lastUsed} />
      </span>
    </div>
  );
}

export function ExtensionsPage({ initialChip }: { initialChip?: ExtChip | null }) {
  const t = useT();
  const [chip, setChip] = useState<ExtChip>(initialChip ?? "all");
  const [query, setQuery] = useState("");
  const [add, setAdd] = useState<AddMode | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [open, setOpen] = useState<Set<string>>(new Set());
  const [copying, setCopying] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  /** Windows: deleted but not seen in the Recycle Bin (9.1), shown apart from the ok-toned status line. */
  const [trashWarn, setTrashWarn] = useState<string | null>(null);
  const [trashError, setTrashError] = useState<{
    key: string;
    text: Shown;
  } | null>(null);
  const [deleted, setDeleted] = useState<DeletedItem[]>(() => [...deletedThisRun.values()]);
  const [now] = useState(() => Date.now());
  const qc = useQueryClient();
  const trash = useMutation({
    mutationFn: (v: { key: string; s: Skill; unsaved: boolean }) => api.trashExtensionItem(v.s.id ?? "", v.unsaved),
    onMutate: () => {
      setTrashError(null);
      setCopied(null);
      setTrashWarn(null);
    },
    onSuccess: (r, { s }) => {
      if (!r.done) return;
      const id = s.id ?? "";
      deletedThisRun.set(id, { id, name: s.name, kind: s.kind });
      setDeleted([...deletedThisRun.values()]);
      qc.removeQueries({ queryKey: ["authoring-item", id] });
      for (const k of ["skills", "authoring-info", "loadout-options", "agents"]) qc.invalidateQueries({ queryKey: [k] });
      setEditing((e) => (e === id ? null : e));
      setCopying(null);
      setCopied(t("extensions.page.trashed", { name: s.name }));
      setTrashWarn(r.unconfirmed ? t("common.trash.unconfirmed") : null);
    },
    onError: (e, { key }) => setTrashError({ key, text: failure(e) }),
  });
  const delFor = (key: string, s: Skill | undefined): Deleting => ({
    busy: trash.isPending && trash.variables?.key === key,
    onDelete: () => s && trash.mutate({ key, s, unsaved: false }),
  });
  const errorLine = (key: string): ReactNode =>
    trashError?.key === key ? (
      <p role="alert" className="px-4 pb-2 text-xs text-danger">
        {shown(trashError.text)}
      </p>
    ) : null;
  const skills = useQuery({ queryKey: ["skills"], queryFn: api.skillCatalog });
  const editingSkill = skills.data?.find((x) => x.id === editing);
  const plugins = useQuery({ queryKey: ["plugins"], queryFn: api.plugins });
  const info = useQuery({ queryKey: ["authoring-info"], queryFn: addApi.info });
  const jobs = useQuery({
    queryKey: ["authoring-jobs"],
    queryFn: addApi.jobs,
    refetchInterval: 15_000,
  });
  const actions = usePluginActions();
  const bulk = useBulk();
  const { current } = useProject();
  const [only, setOnly] = useState<"global" | "project">("global");
  const scope = only === "project" && current && !current.offReason ? "project" : "global";
  // Plugin order as first loaded on this visit: a toggle re-reads the list but the row stays put.
  const [order, setOrder] = useState<string[] | null>(null);
  if (plugins.data && !order) setOrder(extRows([], plugins.data, null).map((r) => r.plugin?.id ?? ""));
  const view = withDeleted(skills.data ?? [], deleted);
  const all = extRows(view.skills, plugins.data ?? [], current?.path ?? null, order ?? undefined);
  const onlyCurrent = scope === "project";
  const rows = filterRows(all, chip, query, scope);
  const refreshing = skills.isFetching || plugins.isFetching;
  // In the global view, project items are hidden: say how many, so an empty list is not mistaken for "none".
  const hiddenProject = scope === "global" ? all.filter((r) => r.skill?.source === "project" && (chip === "all" || r.kind === chip) && hit(query.trim().toLowerCase(), r.name, r.description)).length : 0;
  const groups = GROUP_ORDER.map((k) => [k, rows.filter((r) => r.kind === k)] as const).filter(([, rs]) => rs.length > 0);
  const searching = query.trim().length > 0;
  const loading = skills.isPending || plugins.isPending;
  const error = skills.error ?? plugins.error;
  const toggleOpen = (id: string) =>
    setOpen((s) => {
      const next = new Set(s);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const startCopy = (key: string) => {
    setCopied(null);
    setTrashWarn(null);
    setTrashError(null);
    setCopying(copying === key ? null : key);
  };
  const copyPanel = (key: string, s: Skill | undefined): ReactNode =>
    copying === key && s ? (
      <CopyPanel
        s={s}
        all={skills.data ?? []}
        onCancel={() => setCopying(null)}
        onDone={(id, name) => {
          setCopying(null);
          setCopied(t("extensions.page.copied", { name }));
          if (id) setEditing(id);
        }}
      />
    ) : null;
  const renderRow = (r: ExtRow) => {
    if (r.plugin) {
      const p = r.plugin;
      const inner = innerFor(r, chip, query);
      // While searching, a plugin whose items match opens by itself to show them (a click still folds it).
      const expanded = open.has(p.id) !== (searching && inner.length > 0);
      return (
        <PluginRow
          p={p}
          actions={actions}
          expanded={expanded}
          onExpand={() => toggleOpen(p.id)}
          badges={
            r.current ? (
              <Badge>
                <Folder size={10} className="mr-1" />
                {t("extensions.row.thisProject")}
              </Badge>
            ) : undefined
          }
          items={
            inner.length > 0 ? (
              <div>
                <h4 className="mb-1 text-xs font-semibold text-ink-2">
                  {t("extensions.row.innerTitle")} <span className="font-normal text-ink-3">{inner.length}</span>
                  <span className="ml-2 font-normal text-ink-3">{t("extensions.row.innerLocked")}</span>
                </h4>
                <ul className="divide-y divide-line rounded-lg border border-line bg-paper">
                  {inner.map((i) => {
                    const key = `${p.id}|${i.kind}|${i.name}`;
                    return (
                      <li key={key}>
                        <InnerLine plugin={p.name} i={i} off={r.pluginOff} onEdit={() => setEditing(i.skill?.id ?? null)} onCopy={() => startCopy(key)} />
                        {copyPanel(key, i.skill)}
                      </li>
                    );
                  })}
                </ul>
              </div>
            ) : null
          }
        />
      );
    }
    return (
      <>
        <ItemLine
          r={r}
          fresh={!!r.skill && (r.skill.source === "user" || r.skill.source === "project") && isNew(info.data?.made, r.kind, r.name, r.skill.source === "project" ? r.skill.scope : null, now)}
          deleted={view.marked.has(r.key)}
          del={delFor(r.key, r.skill)}
          onEdit={() => {
            setTrashError(null);
            setEditing(r.skill?.id ?? null);
          }}
          onCopy={() => startCopy(r.key)}
        />
        {errorLine(r.key)}
        {copyPanel(r.key, r.skill)}
      </>
    );
  };
  return (
    <div>
      <PageHeader title={t("extensions.page.title")}>
        <label className="relative w-60">
          <span className="sr-only">{t("extensions.page.searchLabel")}</span>
          <Search size={14} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-ink-3" />
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("extensions.page.searchPlaceholder")} className={`${fieldClass} h-8 py-0 pl-8`} />
        </label>
        <Button
          disabled={refreshing}
          title={t("extensions.page.refreshTitle")}
          onClick={() => {
            qc.invalidateQueries({ queryKey: ["skills"] });
            qc.invalidateQueries({ queryKey: ["plugins"] });
            qc.invalidateQueries({ queryKey: ["authoring-info"] });
          }}
        >
          <RefreshCw size={14} className={refreshing ? "animate-spin" : ""} />
          {t("extensions.page.refresh")}
        </Button>
        <AddMenu onPick={setAdd} pending={pendingReviews(jobs.data, info.data?.made)} />
      </PageHeader>
      <PageBody>
        <ScopeBar need="folder" value={only} onChange={setOnly} />
        <div role="group" aria-label={t("extensions.page.kindGroup")} className="flex flex-wrap items-center gap-1.5">
          {CHIPS.map((c) => (
            <button
              key={c}
              aria-pressed={chip === c}
              onClick={() => setChip(c)}
              className={`inline-flex h-7 items-center gap-1 rounded-full border px-3 text-[13px] transition-colors ${chip === c ? "border-ink bg-ink font-medium text-paper" : "border-line bg-sheet text-ink-2 hover:text-ink"}`}
            >
              {t(`extensions.kinds.${c}`)}
              <span className={`text-[11px] tabular-nums ${chip === c ? "text-paper/70" : "text-ink-3"}`}>{countFor(all, c, query, scope)}</span>
            </button>
          ))}
          <span className="ml-2 text-xs text-ink-3">
            {chip === "plugin" ? pluginHelp() : chip === "workflow" ? t("extensions.page.workflowHelp") : t("extensions.page.lockedHelp")}
          </span>
        </div>
        {chip === "workflow" && info.data?.workflowsDisabled && (
          <p className="rounded-lg border border-warn/30 bg-warn-soft px-3 py-2 text-xs text-ink">{t("extensions.page.workflowsDisabled")}</p>
        )}
        {(chip === "all" || chip === "plugin" || bulk.phase !== "idle" || bulk.counts) && plugins.data && (
          <PluginBulkBar plugins={order ? [...plugins.data].sort((a, b) => order.indexOf(a.id) - order.indexOf(b.id)) : plugins.data} busy={actions.busyId !== undefined} onSettled={() => qc.invalidateQueries({ queryKey: ["plugins"] })} />
        )}
        {hiddenProject > 0 && <p className="text-xs text-ink-3">{t("extensions.page.hiddenProject", { count: hiddenProject })}</p>}
        {copied && (
          <p role="status" className="text-xs text-ok">
            {copied}
          </p>
        )}
        {trashWarn && (
          <p role="status" data-tone="warn" className="rounded-lg border border-warn/30 bg-warn-soft px-3 py-2 text-xs text-ink">
            {trashWarn}
          </p>
        )}
        {error ? (
          <Sheet>
            <Empty title={t("extensions.page.loadFailed")}>{messageOf(error)}</Empty>
          </Sheet>
        ) : groups.length === 0 ? (
          <Sheet>
            <Empty title={loading ? t("extensions.page.loading") : onlyCurrent && !searching ? t("extensions.page.emptyProject") : t("extensions.page.emptyMatch")}>
              {!loading && t("extensions.page.emptyHint")}
            </Empty>
          </Sheet>
        ) : (
          groups.map(([k, rs]) => (
            <Sheet key={k} className="overflow-hidden">
              <h2 className="border-b border-line px-4 py-2 text-xs font-semibold text-ink-2">
                {t(`extensions.kinds.${k}`)} <span className="font-normal text-ink-3">{rs.length}</span>
              </h2>
              <ul className="divide-y divide-line">
                {rs.map((r) => (
                  <li key={r.key}>{renderRow(r)}</li>
                ))}
              </ul>
            </Sheet>
          ))
        )}
      </PageBody>
      {add && <AddSheet mode={add} onMode={setAdd} onClose={() => setAdd(null)} kind={chip === "command" || chip === "workflow" ? chip : "skill"} />}
      {editing && (
        <AuthoringEditor
          id={editing}
          onClose={() => setEditing(null)}
          deleting={trash.isPending && trash.variables?.key === `editor|${editing}`}
          deleteError={trashError?.key === `editor|${editing}` ? shown(trashError.text) : null}
          onDelete={
            editingSkill && rowActions(editingSkill).del
              ? (unsaved) =>
                  trash.mutate({
                    key: `editor|${editing}`,
                    s: editingSkill,
                    unsaved,
                  })
              : undefined
          }
        />
      )}
    </div>
  );
}
