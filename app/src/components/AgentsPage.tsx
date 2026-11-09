import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { AlertTriangle, ArrowDown, ArrowUp, FileText, Lock, Settings, SquareTerminal, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { api, reasonIs, type Activity, type AgentDef, type AgentsPayload, failure, shown, type Shown } from "../lib/api";
import { blockedCount, overCapacity, reviewLoad, type ReviewCapacity } from "../lib/brief";
import { hasRisk, sourceText, statusLine } from "../lib/agents";
import { YELLOW } from "../lib/yellow";
import { DuckAvatar } from "./DuckAvatar";
import { DEFAULT_ROLE, loadOrder, moveInOrder, orderCards, roleOf, saveOrder, type OrderMode, type Role } from "../lib/roles";
import { Button, Empty, MoreInfo, PageHeader, Segmented, fieldClass } from "./ui";
import { AgentSettings } from "./AgentSettings";
import { useLanguage, useT } from "../i18n/react";
import { offState, takeAgentSettingsRequest, useAgentControls, useTeamConfig, type OffState } from "../lib/team";

interface CardModel {
  key: string;
  name: string;
  def: AgentDef | null;
  others: AgentDef[];
  activity?: Activity;
  subtitle: string;
  description: string | null;
  off?: OffState;
}

function Editor({ def, onClose }: { def: AgentDef; onClose: () => void }) {
  const t = useT();
  const qc = useQueryClient();
  const file = useQuery({ queryKey: ["agent-def", def.id], queryFn: () => api.agentDefinition(def.id), enabled: !!def.path, gcTime: 0 });
  const [text, setText] = useState<string | null>(null);
  const [msg, setMsg] = useState<{ tone: "ok" | "error"; text: Shown } | null>(null);
  const [saving, setSaving] = useState(false);
  const [conflict, setConflict] = useState(false);
  const value = text ?? file.data?.content ?? "";
  const dirty = text !== null && text !== file.data?.content;
  // Closing with unsaved edits asks first (Esc, backdrop, ×).
  const close = () => {
    if (!dirty || window.confirm(t("agents.editor.unsavedClose"))) onClose();
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

  return (
    <div className="fixed inset-0 z-30 flex justify-end bg-ink/20" onClick={close}>
      <div role="dialog" aria-modal="true" aria-label={t("agents.editor.dialog", { name: def.name })} onClick={(e) => e.stopPropagation()} className="flex h-full w-[min(720px,92vw)] flex-col border-l border-line bg-sheet shadow-e3">
        <div className="flex items-start gap-3 border-b border-line px-6 py-4">
          <div className="min-w-0 flex-1">
            <h2 className="truncate text-[14px] font-semibold">{def.name}</h2>
            <p className="mt-0.5 truncate font-mono text-xs text-ink-3" title={def.path ?? undefined}>
              {def.path ?? sourceText(def)}
            </p>
          </div>
          <button onClick={close} aria-label={t("common.actions.close")} className="rounded-md p-1 text-ink-3 hover:text-ink">
            <X size={18} />
          </button>
        </div>
        {!def.editable && (
          <p className="flex items-center gap-2 border-b border-line bg-paper px-6 py-2 text-xs text-ink-2">
            <Lock size={13} /> {def.readonlyReason ?? t("agents.editor.readOnly")}
          </p>
        )}
        {hasRisk(def) && (
          <p className="flex items-center gap-2 border-b border-line bg-warn-soft px-6 py-2 text-xs text-ink">
            <AlertTriangle size={13} /> {t("agents.editor.risk")}
          </p>
        )}
        <div className="min-h-0 flex-1 p-6">
          {!def.path ? (
            <div className="space-y-2 text-[13px] text-ink-2">
              <p>{def.description}</p>
              <p className="text-xs text-ink-3">{t("agents.editor.builtin")}</p>
            </div>
          ) : file.isError ? (
            <Empty title={t("agents.editor.openFailed")}>{String(file.error)}</Empty>
          ) : (
            <textarea
              aria-label={t("agents.editor.content")}
              value={value}
              readOnly={!def.editable}
              spellCheck={false}
              onChange={(e) => setText(e.target.value)}
              className={`${fieldClass} h-full resize-none font-mono text-[12px] leading-relaxed`}
            />
          )}
        </div>
        {def.path && (
          <div className="flex items-center gap-3 border-t border-line px-6 py-3">
            <p role="status" className={`min-w-0 flex-1 truncate text-xs ${msg?.tone === "error" ? "text-danger" : "text-ink-3"}`}>
              {(msg ? shown(msg.text) : null) ?? (def.editable ? t("agents.editor.saveNote") : "")}
            </p>
            {conflict && (
              <Button
                onClick={async () => {
                  // Keep the user's edit in the clipboard, then load the file as it is now.
                  try {
                    if (text) await navigator.clipboard.writeText(text);
                  } catch {
                    /* clipboard unavailable: the edit is lost only after the user confirmed below */
                  }
                  if (text && !window.confirm(t("agents.editor.copiedReload"))) return;
                  setText(null);
                  setConflict(false);
                  setMsg(null);
                  await file.refetch();
                }}
              >
                {t("agents.editor.reload")}
              </Button>
            )}
            {def.editable && (
              <>
                <Button onClick={() => setText(null)} disabled={!dirty || saving}>
                  {t("agents.editor.revert")}
                </Button>
                <Button
                  variant="primary"
                  disabled={!dirty || saving || !file.data}
                  onClick={async () => {
                    setSaving(true);
                    setMsg(null);
                    try {
                      await api.saveAgentDefinition(def.id, value, file.data!.token);
                      setMsg({ tone: "ok", text: t("agents.editor.saved") });
                      setText(null);
                      await qc.invalidateQueries({ queryKey: ["agent-def", def.id] });
                      qc.invalidateQueries({ queryKey: ["agent-catalog"] });
                    } catch (e) {
                      const m = e;
                      if (reasonIs(m, "cancelled")) setMsg({ tone: "ok", text: t("agents.editor.notSaved") });
                      else setMsg({ tone: "error", text: failure(m) });
                      setConflict(reasonIs(m, "changedOutside") || reasonIs(m, "listChanged"));
                    } finally {
                      setSaving(false);
                    }
                  }}
                >
                  {saving ? t("agents.editor.waiting") : t("common.actions.save")}
                </Button>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function AgentCard({
  c,
  role,
  onOpenTask,
  onEdit,
  onMove,
  first,
  last,
}: {
  c: CardModel;
  role: Role;
  onOpenTask: (key: string) => void;
  onEdit: (d: AgentDef) => void;
  /** Present while reordering in custom mode. */
  onMove?: (delta: -1 | 1) => void;
  first?: boolean;
  last?: boolean;
}) {
  const t = useT();
  const line = statusLine(c.activity);
  const busy = line.pose !== "idle";
  return (
    <li
      className={`flex flex-col rounded-xl bg-sheet p-4 transition-shadow ${c.off ? "opacity-60 grayscale" : ""} ${line.pose === "waiting" ? YELLOW.waitingCard : line.pose === "failed" ? "shadow-e1 ring-1 ring-danger/50" : "shadow-e1"}`}
    >
      {/* Name gets the full row (badges moved to the line under it: at 1440 a pill beside it cut the name, 3.2 review). */}
      <div className="flex items-start gap-2">
        <h3 className="min-w-0 flex-1 truncate text-[14px] font-semibold" title={c.name}>
          {c.name}
        </h3>
        {onMove && (
          <span className="flex shrink-0 gap-0.5">
            <button aria-label={t("agents.card.moveUp", { name: c.name })} disabled={first} onClick={() => onMove(-1)} className="rounded p-0.5 text-ink-3 hover:bg-paper hover:text-ink disabled:opacity-30">
              <ArrowUp size={14} />
            </button>
            <button aria-label={t("agents.card.moveDown", { name: c.name })} disabled={last} onClick={() => onMove(1)} className="rounded p-0.5 text-ink-3 hover:bg-paper hover:text-ink disabled:opacity-30">
              <ArrowDown size={14} />
            </button>
          </span>
        )}
        {c.def && hasRisk(c.def) && (
          <span title={t("agents.card.risk")} className="shrink-0 text-ink-3">
            <AlertTriangle size={14} />
          </span>
        )}
      </div>
      <p className="truncate text-[12px] text-ink-3">{c.subtitle}</p>
      {(c.key !== "__default" || c.off || c.def?.malformed) && (
        <div className="mt-1.5 flex min-w-0 flex-wrap items-center gap-1.5">
          {c.key !== "__default" && (
            <span className="flex shrink-0 items-center gap-1 rounded-full px-1.5 py-px text-[12px] font-medium text-ink-fixed" style={{ background: role.color.body }}>
              <span aria-hidden className="h-1.5 w-1.5 rounded-full" style={{ background: role.color.scarf }} />
              {role.label}
            </span>
          )}
          {c.off && (
            <span title={c.off === "off" ? t("agents.card.offHere") : t("agents.card.offElsewhere")} className="shrink-0 rounded-full bg-line px-2 py-px text-[12px] font-semibold text-ink-2">
              {t("agents.card.off")}
            </span>
          )}
          {c.def?.malformed && <span className="shrink-0 rounded-full bg-danger/15 px-2 py-px text-[12px] font-semibold text-danger">{t("agents.card.malformed")}</span>}
        </div>
      )}
      <div className="mt-2 flex flex-col items-center">
        <div className="grid h-[112px] w-[112px] place-items-center rounded-full bg-duck-stage">
          <DuckAvatar pose={line.pose} size={100} color={role.color} label={`${c.name}: ${line.head}`} />
        </div>
        <p className={`mt-1 text-[13px] font-semibold ${busy ? "text-ink" : "text-ink-3"}`}>{line.head}</p>
        <p className="h-4 max-w-full truncate text-[12px] text-ink-2" title={line.detail ?? undefined}>
          {line.detail ?? ""}
        </p>
      </div>
      <p className="mt-3 line-clamp-2 min-h-[2.6em] text-xs text-ink-2" title={c.description ?? undefined}>
        {c.description ?? t("agents.card.noDescription")}
      </p>
      {c.def && (c.def.model || c.def.tools.length > 0) && (
        <p className="mt-1.5 truncate text-[12px] text-ink-3">
          {[c.def.model, c.def.tools.length ? (c.def.tools.length > 4 ? t("agents.card.toolsMore", { tools: c.def.tools.slice(0, 4).join(", "), more: c.def.tools.length - 4 }) : t("agents.card.tools", { tools: c.def.tools.join(", ") })) : null].filter(Boolean).join(" · ")}
        </p>
      )}
      {c.others.length > 0 && <p className="mt-1 truncate text-[12px] text-ink-3">{t("agents.card.others", { list: c.others.map(sourceText).join(", ") })}</p>}
      <div className="mt-auto flex gap-1.5 pt-3">
        {c.activity?.sessionKey && (
          <Button className="flex-1 justify-center" onClick={() => onOpenTask(c.activity!.sessionKey!)}>
            <SquareTerminal size={14} />
            {t("agents.card.viewTasks")}
          </Button>
        )}
        {c.def && (
          <Button className="flex-1 justify-center" onClick={() => onEdit(c.def!)}>
            <FileText size={14} />
            {c.def.editable ? t("agents.card.edit") : t("agents.card.view")}
          </Button>
        )}
      </div>
    </li>
  );
}


export function AgentsPage({
  onOpenTask,
  onOpenBlocked,
  agents,
  capacity,
}: {
  onOpenTask: (key: string) => void;
  onOpenBlocked: () => void;
  agents?: AgentsPayload;
  capacity: ReviewCapacity;
}) {
  const t = useT();
  const { lang } = useLanguage();
  const qc = useQueryClient();
  const catalog = useQuery({ queryKey: ["agent-catalog", null], queryFn: () => api.agentCatalog(null), refetchInterval: 60_000 });
  const activity = useQuery({ queryKey: ["agent-activity"], queryFn: api.agentActivity, refetchInterval: 15_000 });
  const [editing, setEditing] = useState<AgentDef | null>(null);
  // Opened directly when the settings page sent the user here.
  const [settingsOpen, setSettingsOpen] = useState(takeAgentSettingsRequest);
  const [showHidden, setShowHidden] = useState(false);
  const controls = useAgentControls();
  const teamTypes = useTeamConfig().data?.config.types;
  const [order, setOrder] = useState(loadOrder);
  const [reordering, setReordering] = useState(false);
  const setMode = (mode: OrderMode) => {
    setOrder((o) => ({ ...o, mode }));
    saveOrder(mode, order.custom);
    if (mode === "abc") setReordering(false);
  };

  useEffect(() => {
    const sub = listen<Activity[]>("agent-activity", (e) => qc.setQueryData(["agent-activity"], e.payload));
    return () => {
      sub.then((un) => un());
    };
  }, [qc]);

  const cards = useMemo<CardModel[]>(() => {
    const defs = catalog.data ?? [];
    const acts = new Map((activity.data ?? []).map((a) => [a.agent, a]));
    const out: CardModel[] = [
      { key: "__default", name: t("agents.card.defaultName"), def: null, others: [], activity: acts.get(""), subtitle: t("agents.card.defaultSubtitle"), description: t("agents.card.defaultDescription") },
    ];
    const hidden = new Set(controls.data?.hidden ?? []);
    for (const d of defs.filter((x) => x.primary)) {
      // Hidden only here (D.Duck); Claude Code still uses it. "숨긴 에이전트 보기" brings them back.
      if (hidden.has(d.name) && !showHidden) continue;
      out.push({ key: d.id, name: d.name, def: d, others: defs.filter((x) => x.name === d.name && x.id !== d.id), activity: acts.get(d.name), subtitle: sourceText(d), description: d.description, off: offState(d, controls.data) });
    }
    const known = new Set([...out.map((c) => c.name), ...(showHidden ? [] : hidden)]);
    for (const a of activity.data ?? []) {
      if (a.agent && !known.has(a.agent)) out.push({ key: `x-${a.agent}`, name: a.agent, def: null, others: [], activity: a, subtitle: t("agents.card.otherSubtitle"), description: t("agents.card.otherDescription") });
    }
    return out;
    // `lang`: the built-in card labels follow a language change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [catalog.data, activity.data, controls.data, showHidden, lang]);
  const hiddenCount = (controls.data?.hidden ?? []).filter((n) => (catalog.data ?? []).some((d) => d.primary && d.name === n)).length;

  // Active agents first (user request), then ABC or the user's own order. While reordering, activity is ignored
  // so cards do not jump under the cursor.
  const ordered = useMemo(() => {
    const withActive = cards.map((c) => ({ ...c, active: !reordering && !!c.activity }));
    return orderCards(withActive, order.mode, order.custom);
  }, [cards, order, reordering]);
  const move = (name: string, delta: -1 | 1) => {
    const current = orderCards(cards.map((c) => ({ ...c, active: false })), "custom", order.custom)
      .filter((c) => c.key !== "__default")
      .map((c) => c.name);
    const custom = moveInOrder(current, name, delta);
    setOrder({ mode: "custom", custom });
    saveOrder("custom", custom);
  };

  const activityFailed = activity.isError;
  const blocked = blockedCount(agents);
  const load = reviewLoad(agents, teamTypes);
  const working = (activity.data ?? []).filter((a) => a.state === "working").reduce((n, a) => n + a.count, 0);
  const waiting = (activity.data ?? []).filter((a) => a.state === "waiting").reduce((n, a) => n + a.count, 0);

  return (
    <div>
      <PageHeader title={t("agents.page.title")}>
        {/* Standup order: what is blocked on you first (AG-07), then how much review you have taken on (AG-08). */}
        {blocked > 0 && (
          <Button variant="primary" onClick={onOpenBlocked}>
            {t("agents.page.blocked", { count: blocked })}
          </Button>
        )}
        <span className={`rounded-full bg-sheet px-2.5 py-1 text-xs shadow-e1 ${overCapacity(load, capacity) ? "font-medium text-warn" : "text-ink-2"}`} title={t("agents.page.capacityTitle")}>
          {capacity === "unlimited" ? t("agents.page.reviewWaiting", { load }) : t("agents.page.reviewCapacity", { load, capacity })}
        </span>
        <span className="text-xs text-ink-3">
          {activityFailed ? t("agents.page.activityFailed") : working > 0 || waiting > 0 ? t("agents.page.activity", { working, waiting }) : t("agents.page.allIdle")}
        </span>
        <button
          type="button"
          aria-label={t("agents.page.settings")}
          title={t("agents.page.settings")}
          onClick={() => setSettingsOpen(true)}
          className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-line bg-sheet px-2.5 text-[13px] text-ink-2 hover:border-ink-3 hover:text-ink"
        >
          <Settings size={15} />
          <span aria-hidden>{t("agents.page.settingsShort")}</span>
        </button>
      </PageHeader>
      <div className="mx-auto max-w-6xl px-8 py-6">
        <div className="mb-4 flex flex-wrap items-center gap-3">
          <Segmented
            label={t("agents.page.order")}
            value={order.mode}
            options={[
              { id: "abc", label: t("agents.page.orderAbc") },
              { id: "custom", label: t("agents.page.orderCustom") },
            ]}
            onChange={setMode}
          />
          {order.mode === "custom" && (
            <Button onClick={() => setReordering(!reordering)} aria-pressed={reordering}>
              {reordering ? t("agents.page.reorderDone") : t("agents.page.reorder")}
            </Button>
          )}
          <span className="text-xs text-ink-3">{reordering ? t("agents.page.activeFirstEditing") : t("agents.page.activeFirst")}</span>
          {hiddenCount > 0 && (
            <button type="button" onClick={() => setShowHidden(!showHidden)} aria-pressed={showHidden} className="ml-auto text-xs text-ink-2 underline underline-offset-2">
              {showHidden ? t("agents.page.hideAgain") : t("agents.page.showHidden", { count: hiddenCount })}
            </button>
          )}
        </div>
        {catalog.isError ? (
          <Empty title={t("agents.page.loadFailed")}>{String(catalog.error)}</Empty>
        ) : (
          <ul className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-4">
            {ordered.map((c, i) => (
              <AgentCard
                key={c.key}
                c={c}
                role={c.key === "__default" ? DEFAULT_ROLE : roleOf(c.name, c.description)}
                onOpenTask={onOpenTask}
                onEdit={setEditing}
                onMove={reordering && c.key !== "__default" ? (d) => move(c.name, d) : undefined}
                first={i <= 1}
                last={i === ordered.length - 1}
              />
            ))}
          </ul>
        )}
        <p className="mt-5 text-[12px] text-ink-3">{t("agents.page.sources")}</p>
        <MoreInfo className="mt-1 text-[12px]">
          {t("agents.page.sourcesMore")}
        </MoreInfo>
      </div>
      {editing && <Editor key={editing.id} def={editing} onClose={() => setEditing(null)} />}
      {settingsOpen && <AgentSettings catalog={catalog.data ?? []} onClose={() => setSettingsOpen(false)} />}
    </div>
  );
}
