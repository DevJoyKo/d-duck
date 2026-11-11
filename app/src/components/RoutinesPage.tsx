import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { AlertTriangle, CalendarClock, CheckCircle2, ExternalLink, FolderOpen, History, Pencil, Play, Plus, Trash2 } from "lucide-react";
import { Fragment, useEffect, useState } from "react";
import { api, type CloudLastRun, type CloudList, type CloudRoutine, type CloudRunLog, type CloudRuns, type CloudSchedule, type RoutineInput, type RoutineMode, type RoutineSummary, type Schedule, type Status, failure, later, messageOf, shown, type Shown } from "../lib/api";
import { type CloudDraft, cloudDiff, cloudScheduleText, draftOf, fromLocalInput, pastOnce, toLocalInput, validateCloudDraft, zoneLabel } from "../lib/cloudRoutines";
import { dateTime, shortPath } from "../lib/format";
import { ROUTINE_MODES, modeHint, modeLabel, relativeNext, runStatusText, runTone, scheduleText, validateRoutineForm, weekdayNames } from "../lib/routines";
import { Button, Empty, PageHeader, SectionTitle, Segmented, Sheet, fieldClass } from "./ui";
import { gateFor, isGateError, useAuth } from "../lib/auth";
import { GateLine } from "./LoginBits";
import { t } from "../i18n";
import { currentLocale } from "../i18n/language";
import { useT } from "../i18n/react";

type Draft = RoutineInput & { id: number | null };

const blank = (cwd: string | null): Draft => ({
  id: null,
  name: "",
  cwd: cwd ?? "",
  prompt: "",
  permissionMode: "plan",
  schedule: { kind: "daily", hour: 9, minute: 0 },
});

const TONE: Record<ReturnType<typeof runTone>, string> = {
  attention: "bg-warn-soft text-ink",
  active: "bg-ink/10 text-ink",
  danger: "bg-danger/10 text-danger",
  quiet: "bg-line/60 text-ink-2",
};

function Chip({ status, error }: { status: string; error: string | null }) {
  // A block-level box so a long failure reason ends in "…" inside a fixed table column; the full text is the title.
  const text = runStatusText(status, error);
  return (
    <span title={text} className={`inline-block max-w-full truncate rounded-full px-2 py-0.5 align-middle text-[11px] font-medium ${TONE[runTone(status)]}`}>
      {text}
    </span>
  );
}

function Toggle({ on, label, onChange }: { on: boolean; label: string; onChange: (v: boolean) => void }) {
  return (
    <button
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={(e) => {
        e.stopPropagation();
        onChange(!on);
      }}
      className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${on ? "bg-ink" : "bg-line"}`}
    >
      <span className={`absolute top-0.5 h-4 w-4 rounded-full bg-sheet shadow transition-all ${on ? "left-[18px]" : "left-0.5"}`} />
    </button>
  );
}

function ScheduleInput({ value, onChange }: { value: Schedule; onChange: (s: Schedule) => void }) {
  const t = useT();
  // Interval has no clock time; switching back to daily/weekly starts at 09:00.
  const h = value.kind === "interval" ? 9 : value.hour;
  const m = value.kind === "interval" ? 0 : value.minute;
  const setTime = (hour: number, minute: number) => {
    if (value.kind === "daily") onChange({ kind: "daily", hour, minute });
    if (value.kind === "weekly") onChange({ ...value, hour, minute });
  };
  return (
    <div className="space-y-2.5">
      <Segmented
        label={t("routines.scheduleInput.kind")}
        value={value.kind}
        options={[
          { id: "daily", label: t("routines.scheduleInput.daily") },
          { id: "weekly", label: t("routines.scheduleInput.weekly") },
          { id: "interval", label: t("routines.scheduleInput.interval") },
        ]}
        onChange={(k) => onChange(k === "daily" ? { kind: "daily", hour: h, minute: m } : k === "weekly" ? { kind: "weekly", weekdays: 0b11111, hour: h, minute: m } : { kind: "interval", hours: 6 })}
      />
      {value.kind === "interval" ? (
        <label className="flex items-center gap-2 text-[13px] text-ink-2">
          <input type="number" min={1} max={24} value={value.hours} onChange={(e) => onChange({ kind: "interval", hours: Number(e.target.value) })} className={`${fieldClass} w-20`} aria-label={t("routines.scheduleInput.intervalLabel")} />
          {t("routines.scheduleInput.intervalSuffix")}
        </label>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          {value.kind === "weekly" && (
            <div role="group" aria-label={t("routines.scheduleInput.weekdays")} className="flex gap-1">
              {weekdayNames().map((d, i) => {
                const on = (value.weekdays & (1 << i)) !== 0;
                return (
                  <button
                    key={i}
                    aria-pressed={on}
                    onClick={() =>
                      onChange({
                        ...value,
                        weekdays: value.weekdays ^ (1 << i),
                      })
                    }
                    className={`h-8 w-8 rounded-lg border text-[13px] ${on ? "border-ink bg-ink text-paper" : "border-line bg-sheet text-ink-2 hover:border-ink-3"}`}
                  >
                    {d}
                  </button>
                );
              })}
            </div>
          )}
          {/* Two 24-hour selects, same as the cloud form: no AM/PM segment, and no half-typed value can reach the schedule. */}
          <div role="group" aria-label={t("routines.scheduleInput.time")}>
            <TimeSelect hour={h} minute={m} onChange={setTime} />
          </div>
        </div>
      )}
    </div>
  );
}

function Form({ draft, onDone, onCancel }: { draft: Draft; onDone: (id: number) => void; onCancel: () => void }) {
  const t = useT();
  const [f, setF] = useState<Draft>(draft);
  const [error, setError] = useState<Shown | null>(null);
  const [saving, setSaving] = useState(false);
  const trusted = useQuery({
    queryKey: ["trusted", f.cwd],
    queryFn: () => api.folderTrusted(f.cwd),
    enabled: !!f.cwd,
  });
  const mode = ROUTINE_MODES.find((m) => m.id === f.permissionMode);

  // Editing loads the full prompt on demand; the list only knows its length (D-050).
  useEffect(() => {
    if (draft.id == null) return;
    let live = true;
    api.routinePrompt(draft.id).then((p) => live && p != null && setF((x) => ({ ...x, prompt: p })));
    return () => {
      live = false;
    };
  }, [draft.id]);

  const save = async () => {
    // Worded when drawn, so a language change rewords a problem already on screen.
    if (validateRoutineForm(f)) return setError(later(() => validateRoutineForm(f) ?? ""));
    setError(null);
    setSaving(true);
    try {
      const { id, ...input } = f;
      onDone(await api.saveRoutine(id, input));
    } catch (e) {
      setError(failure(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Sheet className="space-y-5 p-6">
      <SectionTitle aside={t("routines.form.approveNote")}>
        <span className="whitespace-nowrap">{f.id == null ? t("routines.form.newTitle") : t("routines.form.editTitle")}</span>
      </SectionTitle>
      <label className="block space-y-1.5">
        <span className="text-xs font-medium text-ink-2">{t("routines.form.name")}</span>
        <input value={f.name} maxLength={64} onChange={(e) => setF({ ...f, name: e.target.value })} placeholder={t("routines.form.namePlaceholder")} className={fieldClass} />
      </label>
      <div className="space-y-1.5">
        <span className="text-xs font-medium text-ink-2">{t("routines.form.cwd")}</span>
        <div className="flex gap-2">
          <div className={`${fieldClass} truncate font-mono text-xs ${f.cwd ? "" : "text-ink-3"}`} title={f.cwd}>
            {f.cwd || t("routines.form.cwdPlaceholder")}
          </div>
          <Button
            onClick={async () => {
              const p = await api.pickDirectory();
              if (p) setF({ ...f, cwd: p });
            }}
          >
            <FolderOpen size={14} />
            {t("routines.form.pick")}
          </Button>
        </div>
        {f.cwd && trusted.data === false && (
          <div role="alert" className="flex items-start gap-2 rounded-lg bg-warn-soft px-3 py-2 text-xs text-ink">
            <AlertTriangle size={14} className="mt-0.5 shrink-0" />
            <span className="flex-1">{t("routines.form.untrusted")}</span>
          </div>
        )}
      </div>
      <label className="block space-y-1.5">
        <span className="text-xs font-medium text-ink-2">{t("routines.form.prompt")}</span>
        <textarea value={f.prompt} onChange={(e) => setF({ ...f, prompt: e.target.value })} rows={5} placeholder={t("routines.form.promptPlaceholder")} className={`${fieldClass} resize-y`} />
      </label>
      <div className="space-y-1.5">
        <span className="text-xs font-medium text-ink-2">{t("routines.form.schedule")}</span>
        <ScheduleInput value={f.schedule} onChange={(schedule) => setF({ ...f, schedule })} />
      </div>
      <div className="space-y-1.5">
        <span className="text-xs font-medium text-ink-2">{t("routines.form.permission")}</span>
        <select value={f.permissionMode} onChange={(e) => setF({ ...f, permissionMode: e.target.value as RoutineMode })} className={`${fieldClass} h-9 py-0`}>
          {ROUTINE_MODES.map((m) => (
            <option key={m.id} value={m.id}>
              {modeLabel(m.id)}
            </option>
          ))}
        </select>
        <p className={`text-xs ${mode?.warn ? "text-danger" : "text-ink-3"}`}>{mode && modeHint(mode.id)}</p>
      </div>
      {error && (
        <p role="alert" className="text-xs text-danger">
          {shown(error)}
        </p>
      )}
      <div className="flex justify-end gap-2">
        <Button onClick={onCancel}>{t("common.actions.cancel")}</Button>
        <Button variant="primary" disabled={saving} onClick={save}>
          {saving ? t("routines.form.saving") : t("common.actions.save")}
        </Button>
      </div>
    </Sheet>
  );
}

function Detail({ r, onEdit, onOpenTask, onDeleted }: { r: RoutineSummary; onEdit: () => void; onOpenTask: (bgId: string) => void; onDeleted: () => void }) {
  const t = useT();
  const qc = useQueryClient();
  const runs = useQuery({
    queryKey: ["routine-runs", r.id],
    queryFn: () => api.routineRuns(r.id),
    refetchInterval: 15_000,
  });
  const trusted = useQuery({
    queryKey: ["trusted", r.cwd],
    queryFn: () => api.folderTrusted(r.cwd),
  });
  const [msg, setMsg] = useState<Shown | null>(null);
  const [gate, setGate] = useState<"login" | "claudeAi" | null>(null);
  // claude-login: gated up front like the cloud button; a backend refusal (stale state) shows the same line.
  const authView = useAuth().view;
  const runGate = gate ?? gateFor(authView, "model");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["routines"] });
    qc.invalidateQueries({ queryKey: ["routine-runs", r.id] });
  };

  return (
    <div className="space-y-4">
      <Sheet className="p-6">
        <div className="flex flex-wrap items-start gap-3">
          <div className="min-w-0 flex-1 basis-64">
            <h2 className="line-clamp-2 wrap-anywhere text-[15px] font-semibold" title={r.name}>
              {r.name}
            </h2>
            <p className="mt-1 break-keep text-xs text-ink-3">
              {scheduleText(r.schedule)} · {modeLabel(r.permissionMode)} · {t("routines.detail.promptLen", { count: r.promptLen })}
            </p>
            <p className="mt-0.5 truncate font-mono text-xs text-ink-3" title={r.cwd}>
              {r.cwd}
            </p>
          </div>
          <div className="flex shrink-0 gap-2">
            <Button onClick={onEdit}>
              <Pencil size={14} />
              {t("routines.detail.edit")}
            </Button>
            <Button
              variant="primary"
              disabled={r.active || runGate !== null}
              title={r.active ? t("routines.detail.alreadyRunning") : undefined}
              onClick={async () => {
                setMsg(null);
                setGate(null);
                try {
                  await api.runRoutineNow(r.id);
                  setMsg(later(() => t("routines.detail.started")));
                } catch (e) {
                  setGate(isGateError(e));
                  if (!isGateError(e)) setMsg(failure(e));
                }
                refresh();
              }}
            >
              <Play size={14} />
              {r.active ? t("routines.detail.running") : t("routines.detail.runNow")}
            </Button>
          </div>
        </div>
        <dl className="mt-4 grid grid-cols-[repeat(auto-fit,minmax(10rem,1fr))] gap-3 text-xs">
          <div className="rounded-lg bg-paper px-3 py-2">
            <dt className="text-ink-3">{t("routines.detail.nextRun")}</dt>
            <dd className="mt-0.5 font-medium text-ink tabular-nums">
              {r.enabled && r.nextRunAt ? (
                <>
                  <span className="whitespace-nowrap">{dateTime(r.nextRunAt)}</span> <span className="whitespace-nowrap">({relativeNext(r.nextRunAt)})</span>
                </>
              ) : (
                t("routines.detail.off")
              )}
            </dd>
          </div>
          <div className="rounded-lg bg-paper px-3 py-2">
            <dt className="text-ink-3">{t("routines.detail.lastResult")}</dt>
            <dd className="mt-0.5 font-medium text-ink">{r.lastStatus ? runStatusText(r.lastStatus, null) : t("routines.detail.none")}</dd>
          </div>
        </dl>
        {trusted.data === false && (
          <div role="alert" className="mt-3 flex items-center gap-2 rounded-lg bg-warn-soft px-3 py-2 text-xs">
            <AlertTriangle size={14} className="shrink-0" />
            <span className="flex-1">{t("routines.detail.untrusted")}</span>
            <Button
              onClick={async () => {
                try {
                  await api.trustRoutineFolder(r.id);
                  setMsg(later(() => t("routines.detail.trustHint")));
                } catch (e) {
                  setMsg(failure(e));
                }
              }}
            >
              {t("routines.detail.trust")}
            </Button>
          </div>
        )}
        <GateLine className="mt-3" force={runGate} more />
        {msg && (
          <p role="status" className="mt-3 text-xs text-ink-2">
            {shown(msg)}
          </p>
        )}
      </Sheet>

      <Sheet className="p-6">
        <div className="mb-3 flex flex-wrap items-baseline gap-x-3 gap-y-1">
          <h2 className="whitespace-nowrap text-[13px] font-semibold text-ink">{t("routines.detail.history")}</h2>
          <span className="break-keep text-xs text-ink-3">{t("routines.detail.historyNote")}</span>
        </div>
        {runs.isError ? (
          <Empty title={t("routines.detail.historyFailed")}>{messageOf(runs.error)}</Empty>
        ) : (runs.data ?? []).length === 0 ? (
          <Empty title={t("routines.detail.historyEmpty")} />
        ) : (
          <div className="overflow-x-auto">
            {/* Fixed layout: column widths come from the header, so a long failure reason truncates instead of
                widening the table (palette-refresh 2.5, D-168). */}
            <table className="w-full min-w-[26rem] table-fixed text-left text-xs">
              <thead className="text-ink-3">
                <tr className="border-b border-line">
                  <th className="w-[32%] truncate py-2 pr-4 font-medium">{t("routines.detail.scheduledAt")}</th>
                  <th className="truncate py-2 pr-4 font-medium">{t("routines.detail.status")}</th>
                  <th className="w-[32%] truncate py-2 pr-4 font-medium">{t("routines.detail.endedAt")}</th>
                  <th className="w-24 py-2" />
                </tr>
              </thead>
              <tbody className="divide-y divide-line">
                {runs.data!.map((run) => (
                  <tr key={run.id}>
                    <td className="truncate py-2 pr-4 tabular-nums">{dateTime(run.startedAt ?? run.scheduledFor)}</td>
                    <td className="py-2 pr-4">
                      <Chip status={run.status} error={run.error} />
                    </td>
                    <td className="truncate py-2 pr-4 text-ink-3 tabular-nums">{run.endedAt ? dateTime(run.endedAt) : "-"}</td>
                    <td className="truncate py-2 text-right">
                      {run.bgId && (run.status === "running" || run.status === "waiting") && (
                        <button className="text-xs font-medium text-ink underline-offset-2 hover:underline" onClick={() => onOpenTask(run.bgId!)}>
                          {t("routines.detail.openTask")}
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Sheet>

      <div className="flex justify-end">
        {confirmDelete ? (
          <div className="flex items-center gap-2 text-xs text-ink-2">
            {t("routines.detail.deleteWarn")}
            <Button onClick={() => setConfirmDelete(false)}>{t("common.actions.cancel")}</Button>
            <Button
              variant="danger"
              onClick={async () => {
                try {
                  await api.deleteRoutine(r.id);
                  onDeleted();
                } catch (e) {
                  setMsg(failure(e));
                }
              }}
            >
              {t("routines.detail.deleteConfirm")}
            </Button>
          </div>
        ) : (
          <Button variant="danger" onClick={() => setConfirmDelete(true)}>
            <Trash2 size={14} />
            {t("routines.detail.delete")}
          </Button>
        )}
      </div>
    </div>
  );
}

const pad2 = (n: number) => String(n).padStart(2, "0");

/** 24-hour time as two selects, so it reads like the list (no 오전/오후 from the OS locale). */
function TimeSelect({ hour, minute, onChange }: { hour: number; minute: number; onChange: (hour: number, minute: number) => void }) {
  const t = useT();
  const cls = `${fieldClass} w-18! px-2!`;
  return (
    <div className="flex items-center gap-1 text-[13px]">
      <select aria-label={t("routines.scheduleInput.hour")} value={hour} onChange={(e) => onChange(Number(e.target.value), minute)} className={cls}>
        {Array.from({ length: 24 }, (_, h) => (
          <option key={h} value={h}>
            {pad2(h)}
          </option>
        ))}
      </select>
      :
      <select aria-label={t("routines.scheduleInput.minute")} value={minute} onChange={(e) => onChange(hour, Number(e.target.value))} className={cls}>
        {Array.from({ length: 60 }, (_, m) => (
          <option key={m} value={m}>
            {pad2(m)}
          </option>
        ))}
      </select>
    </div>
  );
}

function CloudScheduleInput({ value, now, onChange }: { value: CloudSchedule; now: number; onChange: (s: CloudSchedule) => void }) {
  const t = useT();
  if (value.kind === "once") {
    const d = new Date(value.at);
    const set = (date: string, hour: number, minute: number) => {
      const at = fromLocalInput(`${date}T${pad2(hour)}:${pad2(minute)}`);
      if (at != null) onChange({ kind: "once", at });
    };
    const date = toLocalInput(value.at).slice(0, 10);
    return (
      <div className="flex flex-wrap items-center gap-2">
        <input type="date" value={date} min={toLocalInput(now).slice(0, 10)} onChange={(e) => e.target.value && set(e.target.value, d.getHours(), d.getMinutes())} className={`${fieldClass} w-44!`} aria-label={t("routines.cloudEdit.date")} />
        <TimeSelect hour={d.getHours()} minute={d.getMinutes()} onChange={(h, m) => set(date, h, m)} />
      </div>
    );
  }
  return (
    <div className="space-y-2">
      <Segmented
        label={t("routines.cloudEdit.repeat")}
        value={value.kind}
        options={[
          { id: "daily", label: t("routines.scheduleInput.daily") },
          { id: "weekly", label: t("routines.scheduleInput.weekly") },
        ]}
        onChange={(k) =>
          onChange(
            k === "daily"
              ? { kind: "daily", hour: value.hour, minute: value.minute }
              : {
                  kind: "weekly",
                  weekdays: value.kind === "weekly" ? value.weekdays : 0b11111,
                  hour: value.hour,
                  minute: value.minute,
                },
          )
        }
      />
      <div className="flex flex-wrap items-center gap-2">
        {value.kind === "weekly" && (
          <div role="group" aria-label={t("routines.scheduleInput.weekdays")} className="flex gap-1">
            {weekdayNames().map((d, i) => {
              const on = (value.weekdays & (1 << i)) !== 0;
              return (
                <button
                  key={i}
                  aria-pressed={on}
                  onClick={() => onChange({ ...value, weekdays: value.weekdays ^ (1 << i) })}
                  className={`h-8 w-8 rounded-lg border text-[13px] ${on ? "border-ink bg-ink text-paper" : "border-line bg-sheet text-ink-2 hover:border-ink-3"}`}
                >
                  {d}
                </button>
              );
            })}
          </div>
        )}
        <TimeSelect hour={value.hour} minute={value.minute} onChange={(hour, minute) => onChange({ ...value, hour, minute })} />
      </div>
    </div>
  );
}

// Matched against Rust's plain-string errors in either app language (`Fail::ChangedElsewhere`, `…Reload` lines in
// i18n.rs; copyRules.test.ts checks both sides still match).
export const CONFLICT_TEXTS = ["다른 곳에서 바뀌었어요", "It changed somewhere else"];
export const RELOAD_TEXTS = ["다시 불러오", "Load it again"];
const hasAny = (e: string, words: string[]) => words.some((w) => e.includes(w));
/** Server conflict text → the copy the user sees (the draft is lost on reload, so say so). */
const shownError = (e: string) => (hasAny(e, CONFLICT_TEXTS) ? t("routines.cloudEdit.conflict") : e);

function CloudEditor({ r, listedAt, onSaved, onCancel, onReload }: { r: CloudRoutine; listedAt: number; onSaved: (r: CloudRoutine) => void; onCancel: () => void; onReload: () => void }) {
  const t = useT();
  const [d, setD] = useState<CloudDraft>(() => draftOf(r));
  const [now] = useState(() => Date.now());
  const [touched, setTouched] = useState(false);
  const [error, setError] = useState<Shown | null>(null);
  const [saving, setSaving] = useState(false);
  // A switched-off one-time routine whose time has passed: it can only be switched on together with a new time.
  const wasPast = !r.enabled && pastOnce(r.schedule, now);
  const stillPast = pastOnce(d.schedule, now);
  const setSchedule = (schedule: CloudSchedule) =>
    setD({
      ...d,
      schedule,
      enabled: wasPast && !touched ? !pastOnce(schedule, now) : d.enabled,
    });
  const save = async () => {
    const checkedAt = Date.now();
    if (validateCloudDraft(d, checkedAt)) return setError(later(() => validateCloudDraft(d, checkedAt) ?? ""));
    const edit = cloudDiff(r, d);
    if (Object.keys(edit).length === 0) return setError(later(() => t("routines.cloudEdit.noChange")));
    setError(null);
    setSaving(true);
    try {
      const res = await api.updateCloudRoutine(r, edit, listedAt);
      if (res.outcome === "done") onSaved(res.routine);
    } catch (e) {
      setError(failure(e));
    } finally {
      setSaving(false);
    }
  };
  return (
    <div className="space-y-4 rounded-xl border border-line bg-paper p-4">
      <label className="block space-y-1.5">
        <span className="text-xs font-medium text-ink-2">{t("routines.cloudEdit.name")}</span>
        <input value={d.name} maxLength={80} onChange={(e) => setD({ ...d, name: e.target.value })} className={fieldClass} />
      </label>
      <div className="space-y-1.5">
        <span className="text-xs font-medium text-ink-2">{t("routines.cloudEdit.schedule", { zone: zoneLabel() })}</span>
        {d.schedule ? (
          <CloudScheduleInput value={d.schedule} now={now} onChange={setSchedule} />
        ) : (
          <div className="flex flex-wrap items-center gap-2 text-[13px]">
            <code className="rounded bg-sheet px-2 py-1 font-mono text-xs">{r.cron}</code>
            <span className="text-xs text-ink-3">{t("routines.cloudEdit.cronOnly")}</span>
            <Button onClick={() => api.openCloudRoutine(r.id).catch(() => undefined)}>
              <ExternalLink size={14} />
              {t("routines.cloudEdit.editWeb")}
            </Button>
          </div>
        )}
      </div>
      {wasPast && (
        <div className="flex items-center gap-2 text-[13px]">
          <Toggle
            on={d.enabled}
            label={t("routines.cloudEdit.enableNew")}
            onChange={(v) => {
              if (stillPast) return;
              setTouched(true);
              setD({ ...d, enabled: v });
            }}
          />
          <span className={stillPast ? "text-ink-3" : ""}>{stillPast ? t("routines.cloudEdit.pickFuture") : t("routines.cloudEdit.enableNew")}</span>
        </div>
      )}
      {error && (
        <div role="alert" className="flex flex-wrap items-center gap-2 text-xs text-danger">
          <AlertTriangle size={14} className="shrink-0" />
          <span>{shownError(shown(error))}</span>
          {hasAny(shown(error), RELOAD_TEXTS) && <Button onClick={onReload}>{t("routines.cloudEdit.reload")}</Button>}
        </div>
      )}
      <div className="flex flex-wrap items-center gap-2">
        <button className="text-xs text-ink-3 underline-offset-2 hover:text-ink hover:underline" onClick={() => api.openCloudRoutine(r.id).catch(() => undefined)}>
          {t("routines.cloudEdit.promptWeb")}
        </button>
        <div className="ml-auto flex gap-2">
          <Button onClick={onCancel} disabled={saving}>
            {t("common.actions.cancel")}
          </Button>
          <Button variant="primary" disabled={saving} onClick={save}>
            {saving ? t("routines.cloudEdit.sending") : t("common.actions.save")}
          </Button>
        </div>
      </div>
      <p className="text-[11px] text-ink-3">{t("routines.cloudEdit.costNote")}</p>
    </div>
  );
}

/** Last fire of a cloud routine as one line (D-166). Failure reasons are API codes, shown readable. */
export function lastRunText(r: CloudLastRun): {
  text: string;
  tone: "ok" | "danger" | "wait" | "muted";
} {
  const when = r.firedAt ? ` · ${dateTime(r.firedAt)}` : "";
  switch (r.status) {
    case "succeeded":
      return { text: `${t("routines.lastRun.succeeded")}${when}`, tone: "ok" };
    case "failed":
      return {
        text: `${r.failureReason ? t("routines.lastRun.failedWith", { reason: r.failureReason.toLowerCase().replace(/_/g, " ") }) : t("routines.lastRun.failed")}${when}`,
        tone: "danger",
      };
    case "running":
      return { text: `${t("routines.lastRun.running")}${when}`, tone: "wait" };
    default:
      return { text: `${t("routines.lastRun.unknown")}${when}`, tone: "muted" };
  }
}

const RUN_TONE: Record<string, string> = {
  ok: "text-ok",
  danger: "text-danger",
  wait: "text-ink-2",
  muted: "text-ink-3",
};

/** Run history of one cloud routine and the log of a chosen run (D-166). Remote content is shown as plain text. */
function CloudRunsPanel({ r, onClose }: { r: CloudRoutine; onClose: () => void }) {
  const t = useT();
  // One CLI run per open (no polling, no retry): the list is fetched when the panel opens, and again by "다시 불러오기" after a failed load.
  const list = useQuery<CloudRuns>({
    queryKey: ["cloud-runs", r.id],
    queryFn: () => api.cloudRoutineRuns(r.id),
    retry: false,
    staleTime: Infinity,
    gcTime: 0,
  });
  const runs = list.data ?? null;
  const [log, setLog] = useState<CloudRunLog | null>(null);
  const [loadingLog, setLoadingLog] = useState<string | null>(null);
  const [logError, setLogError] = useState<Shown | null>(null);
  const error = logError ? shown(logError) : list.error ? messageOf(list.error) : null;
  const loading: "runs" | string | null = list.isFetching ? "runs" : loadingLog;
  const loadRuns = () => {
    setLogError(null);
    list.refetch();
  };
  const openLog = async (sessionId: string) => {
    setLoadingLog(sessionId);
    setLogError(null);
    try {
      setLog(await api.cloudRunLog(sessionId));
    } catch (e) {
      setLogError(failure(e));
    } finally {
      setLoadingLog(null);
    }
  };
  const lastId = r.lastRun?.sessionId ?? null;
  return (
    <div className="max-w-full min-w-0 rounded-lg border border-line bg-sheet p-3" aria-label={t("routines.runs.label", { name: r.name })}>
      <div className="flex items-center gap-2">
        <span className="shrink-0 text-xs font-semibold">{t("routines.runs.title")}</span>
        <div className="ml-auto flex shrink-0 gap-2">
          {/* The panel loads once per open; reloading is offered only after a failed load (the cloud list above has its own). */}
          {(list.isError || loading === "runs") && (
            <Button disabled={loading != null} onClick={loadRuns}>
              {loading === "runs" ? t("routines.runs.loading") : t("routines.runs.reload")}
            </Button>
          )}
          <Button onClick={onClose}>{t("common.actions.close")}</Button>
        </div>
      </div>
      {/* Wraps instead of truncating: "logs aren't saved" is a promise, not a hint (3.2 review). */}
      <p className="mt-1 text-[12px] text-ink-3">{t("routines.runs.note")}</p>
      {error && (
        <p role="alert" className="mt-2 text-xs text-danger">
          {error}
        </p>
      )}
      {runs &&
        (runs.runs.length === 0 ? (
          <p className="mt-2 text-xs text-ink-3">{t("routines.runs.empty")}</p>
        ) : (
          <table className="mt-2 w-full table-fixed text-left text-xs">
            <thead className="text-ink-3">
              <tr className="border-b border-line">
                <th className="truncate py-1.5 pr-3 font-medium">{t("routines.runs.started")}</th>
                <th className="truncate py-1.5 pr-3 font-medium">{t("routines.runs.result")}</th>
                <th className="truncate py-1.5 pr-3 font-medium">{t("routines.runs.lastEvent")}</th>
                <th className="w-40 py-1.5" />
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {runs.runs.map((run) => {
                const fromLog =
                  log?.sessionId === run.id && log.outcome
                    ? log.outcome === "succeeded"
                      ? lastRunText({
                          status: "succeeded",
                          failureReason: null,
                          firedAt: null,
                          finishedAt: null,
                          sessionId: null,
                        })
                      : lastRunText({
                          status: "failed",
                          failureReason: null,
                          firedAt: null,
                          finishedAt: null,
                          sessionId: null,
                        })
                    : null;
                const fromList = run.id === lastId && r.lastRun ? lastRunText({ ...r.lastRun, firedAt: null }) : null;
                const result = fromLog ?? fromList;
                return (
                  <tr key={run.id}>
                    <td className="truncate py-1.5 pr-3 tabular-nums">{run.createdAt ? dateTime(run.createdAt) : "-"}</td>
                    <td className={`truncate py-1.5 pr-3 ${result ? RUN_TONE[result.tone] : "text-ink-3"}`} title={result ? result.text : t("routines.runs.fromLog")}>
                      {result ? result.text : t("routines.runs.fromLog")}
                    </td>
                    <td className="truncate py-1.5 pr-3 tabular-nums text-ink-3">{run.lastEventAt ? dateTime(run.lastEventAt) : "-"}</td>
                    <td className="py-1.5 text-right whitespace-nowrap">
                      <Button disabled={loading != null} aria-expanded={log?.sessionId === run.id} onClick={() => (log?.sessionId === run.id ? setLog(null) : openLog(run.id))}>
                        {loading === run.id ? t("routines.runs.loading") : log?.sessionId === run.id ? t("routines.runs.hideLog") : t("routines.runs.showLog")}
                      </Button>
                      {run.url && (
                        <a className="ml-2 text-[11px] text-ink-3 underline-offset-2 hover:text-ink hover:underline" href={run.url} target="_blank" rel="noreferrer">
                          claude.ai
                        </a>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        ))}
      {runs?.hasMore && <p className="mt-1 text-[11px] text-ink-3">{t("routines.runs.older")}</p>}
      {log && (
        <div className="mt-3">
          <div className="flex items-center gap-2 text-xs">
            <span className="font-semibold">{t("routines.runs.log")}</span>
            {log.outcome && <span className={log.outcome === "succeeded" ? "text-ok" : "text-danger"}>{log.outcome === "succeeded" ? t("routines.lastRun.succeeded") : t("routines.lastRun.failed")}</span>}
            {!log.outcome && <span className="text-ink-3">{t("routines.runs.noResult")}</span>}
            <span className="text-[11px] text-ink-3">{t("routines.runs.remote")}</span>
          </div>
          {/* Plain text only: the run log is remote data and is never rendered as HTML. */}
          <ol className="mt-1 max-h-96 overflow-auto rounded border border-line bg-paper p-2 font-mono text-[11px] leading-relaxed">
            {log.events.map((e, i) => (
              <li key={i} className={`whitespace-pre-wrap wrap-anywhere ${e.kind === "result" ? (log.outcome === "failed" ? "text-danger" : "text-ok") : e.kind.startsWith("env") ? "text-ink-3" : ""}`}>
                <span className="text-ink-3 tabular-nums">{e.at ? new Date(e.at).toLocaleTimeString(currentLocale()) : ""}</span> <span className="font-semibold">{e.kind}</span> {e.text}
              </li>
            ))}
          </ol>
          {log.hasOlder && <p className="mt-1 text-[11px] text-ink-3">{t("routines.runs.recentOnly")}</p>}
        </div>
      )}
    </div>
  );
}

function CloudSection() {
  const t = useT();
  const [data, setData] = useState<{ list: CloudList; at: number } | null>(null);
  const [error, setError] = useState<Shown | null>(null);
  const [gate, setGate] = useState<"login" | "claudeAi" | null>(null);
  const [loading, setLoading] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);
  const [history, setHistory] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<{
    id: string;
    text: Shown;
    error?: boolean;
  } | null>(null);
  // The saved notice fades after ~5 s; errors stay until the next action.
  useEffect(() => {
    if (!notice || notice.error) return;
    const t = setTimeout(() => setNotice((n) => (n === notice ? null : n)), 5000);
    return () => clearTimeout(t);
  }, [notice]);
  const load = async () => {
    setLoading(true);
    setError(null);
    setEditing(null);
    setNotice(null);
    try {
      setGate(null);
      setData({ list: await api.cloudRoutines(), at: Date.now() });
    } catch (e) {
      setGate(isGateError(e));
      if (!isGateError(e)) setError(failure(e));
    } finally {
      setLoading(false);
    }
  };
  const saved = (r: CloudRoutine) => {
    setData(
      (x) =>
        x && {
          ...x,
          list: {
            ...x.list,
            routines: x.list.routines.map((o) => (o.id === r.id ? r : o)),
          },
        },
    );
    setEditing(null);
    setNotice({
      id: r.id,
      text: later(() => (r.enabled && r.nextRunAt ? t("routines.cloud.saved", { time: dateTime(r.nextRunAt) }) : t("routines.cloud.savedOff"))),
    });
  };
  const quickToggle = async (r: CloudRoutine, on: boolean, listedAt: number) => {
    setBusy(r.id);
    setNotice(null);
    try {
      const res = await api.updateCloudRoutine(r, { enabled: on }, listedAt);
      if (res.outcome === "done") saved(res.routine);
    } catch (e) {
      setNotice({
        id: r.id,
        text: failure(e),
        error: true,
      });
    } finally {
      setBusy(null);
    }
  };
  // claude-login: without a claude.ai login the list can never load, so the button is off and says why.
  const authView = useAuth().view;
  const authGate = gate ?? gateFor(authView, "claudeAi");
  const state = (r: CloudRoutine) =>
    t(r.endedReason === "run_once_fired" ? "routines.cloud.state.endedOnce" : r.endedReason ? "routines.cloud.state.ended" : r.enabled ? "routines.cloud.state.on" : "routines.cloud.state.off");
  const listTip = `${t("routines.cloud.listTip")}${data?.list.costUsd != null ? t("routines.cloud.lastCost", { cost: `$${data.list.costUsd.toFixed(4)}` }) : ""}`;
  return (
    <Sheet className="p-5">
      <div className="flex flex-wrap items-center gap-3">
        <SectionTitle aside={t("routines.cloud.aside")}>
          <span className="whitespace-nowrap">{t("routines.cloud.title")}</span>
        </SectionTitle>
        <div className="ml-auto flex items-center gap-2">
          {data && <span className="text-[11px] text-ink-3 tabular-nums">{t("routines.cloud.asOf", { time: dateTime(data.at) })}</span>}
          <Button onClick={() => api.openCloudRoutines().catch(() => undefined)}>
            <ExternalLink size={14} />
            {t("routines.cloud.openWeb")}
          </Button>
          <Button variant={data ? "secondary" : "primary"} disabled={loading || busy != null || authGate !== null} onClick={load} title={authGate ? undefined : listTip}>
            {loading ? t("routines.cloud.loading") : data ? t("routines.cloud.reload") : t("routines.cloud.load")}
          </Button>
        </div>
      </div>
      {authGate && <GateLine need="claudeAi" force={authGate} more className="mt-3 justify-end" />}
      {error && (
        <p role="alert" className="mt-3 text-xs text-danger">
          {t("routines.cloud.loadFailed", { error: shown(error) })}
        </p>
      )}
      {data &&
        (data.list.routines.length === 0 ? (
          <Empty title={t("routines.cloud.empty")} />
        ) : (
          <table className="mt-3 w-full text-left text-xs">
            <thead className="text-ink-3">
              <tr className="border-b border-line">
                <th className="py-2 font-medium">{t("routines.cloud.name")}</th>
                <th className="py-2 font-medium">{t("routines.cloud.status")}</th>
                <th className="py-2 font-medium">{t("routines.cloud.schedule")}</th>
                <th className="py-2 font-medium">{t("routines.cloud.nextRun")}</th>
                <th className="py-2" />
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {data.list.routines.map((r) => {
                const lockedOn = !r.enabled && pastOnce(r.schedule, data.at);
                const mine = notice?.id === r.id ? notice : null;
                return (
                  <Fragment key={r.id}>
                    <tr className={r.enabled ? "" : "text-ink-3"}>
                      <td className="min-w-36 py-2 pr-3 font-medium">
                        {r.name}
                        {r.localOnly && <span className="mt-0.5 block text-[11px] font-normal text-ink-3">{t("routines.cloud.localOnly")}</span>}
                        {r.lastRun && <span className={`mt-0.5 block text-[11px] font-normal ${RUN_TONE[lastRunText(r.lastRun).tone]}`}>{t("routines.cloud.recent", { text: lastRunText(r.lastRun).text })}</span>}
                        {mine && !mine.error && (
                          <span role="status" className="mt-0.5 flex items-center gap-1 text-[11px] font-normal text-ok">
                            <CheckCircle2 size={12} />
                            {shown(mine.text)}
                          </span>
                        )}
                      </td>
                      <td className="py-2 pr-3">
                        <div className="flex items-center gap-2 whitespace-nowrap">
                          <Toggle
                            on={r.enabled}
                            label={t(r.enabled ? "routines.toggle.off" : "routines.toggle.on", { name: r.name })}
                            onChange={(v) => {
                              if (v && r.localOnly) {
                                setNotice({
                                  id: r.id,
                                  text: t("routines.cloud.localOnlyRefused"),
                                  error: true,
                                });
                                return;
                              }
                              if (v && lockedOn) {
                                setNotice({
                                  id: r.id,
                                  text: t("routines.cloud.pastRefused"),
                                  error: true,
                                });
                                return;
                              }
                              if (busy == null) quickToggle(r, v, data.at);
                            }}
                          />
                          <span>{busy === r.id ? t("routines.cloud.sending") : state(r)}</span>
                        </div>
                      </td>
                      <td className={`min-w-28 py-2 pr-3 ${r.schedule ? "" : "font-mono"}`}>
                        {cloudScheduleText(r)}
                        {!r.schedule && r.cron && <span className="ml-1.5 font-sans text-[11px] text-ink-3">{t("routines.cloud.webOnly")}</span>}
                      </td>
                      <td className="py-2 pr-3 whitespace-nowrap tabular-nums">
                        {r.enabled && r.nextRunAt ? (
                          <>
                            {dateTime(r.nextRunAt)}
                            <span className="hidden xl:inline"> ({relativeNext(r.nextRunAt)})</span>
                          </>
                        ) : (
                          "-"
                        )}
                      </td>
                      <td className="py-2">
                        <div className="flex flex-wrap justify-end gap-1.5">
                          <Button disabled={busy != null} aria-expanded={history === r.id} onClick={() => setHistory(history === r.id ? null : r.id)}>
                            <History size={13} />
                            {t("routines.cloud.history")}
                          </Button>
                          <Button disabled={busy != null} aria-expanded={editing === r.id} onClick={() => setEditing(editing === r.id ? null : r.id)}>
                            <Pencil size={13} />
                            {t("routines.cloud.edit")}
                          </Button>
                        </div>
                      </td>
                    </tr>
                    {mine?.error && editing !== r.id && (
                      <tr>
                        <td colSpan={5} className="pb-2">
                          <p role="alert" className="flex items-center gap-2 text-xs text-danger">
                            {shownError(shown(mine.text))}
                            {hasAny(shown(mine.text), RELOAD_TEXTS) && <Button onClick={load}>{t("routines.cloud.reload")}</Button>}
                          </p>
                        </td>
                      </tr>
                    )}
                    {history === r.id && (
                      <tr>
                        <td colSpan={5} className="pb-3">
                          <CloudRunsPanel r={r} onClose={() => setHistory(null)} />
                        </td>
                      </tr>
                    )}
                    {editing === r.id && (
                      <tr>
                        <td colSpan={5} className="pb-3">
                          <CloudEditor r={r} listedAt={data.at} onSaved={saved} onCancel={() => setEditing(null)} onReload={load} />
                        </td>
                      </tr>
                    )}
                  </Fragment>
                );
              })}
            </tbody>
          </table>
        ))}
      {data?.list.hasMore && <p className="mt-2 text-[11px] text-ink-3">{t("routines.cloud.more")}</p>}
    </Sheet>
  );
}

export function RoutinesPage({ status, onOpenTask }: { status?: Status; onOpenTask: (bgId: string) => void }) {
  const t = useT();
  const qc = useQueryClient();
  const list = useQuery({
    queryKey: ["routines"],
    queryFn: api.routines,
    refetchInterval: 30_000,
  });
  const [selected, setSelected] = useState<number | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const routines = list.data ?? [];
  const current = routines.find((r) => r.id === selected) ?? routines[0] ?? null;

  useEffect(() => {
    const sub = listen("routines-updated", () => {
      qc.invalidateQueries({ queryKey: ["routines"] });
      qc.invalidateQueries({ queryKey: ["routine-runs"] });
    });
    return () => {
      sub.then((un) => un());
    };
  }, [qc]);

  return (
    <div>
      <PageHeader title={t("routines.title")}>
        <Button variant="primary" disabled={routines.length >= 20} onClick={() => setDraft(blank(status?.defaultCwd ?? null))}>
          <Plus size={14} />
          {t("routines.new")}
        </Button>
      </PageHeader>
      <div className="mx-auto max-w-6xl px-8 pt-6">
        <CloudSection />
      </div>
      {/* One column under lg: at the 900px minimum the detail column was 232px wide and its run table scrolled (2.5). */}
      <div className="mx-auto grid max-w-6xl grid-cols-1 gap-5 px-8 py-6 lg:grid-cols-[minmax(240px,320px)_1fr]">
        <aside className="space-y-3">
          <Sheet className="overflow-hidden">
            {list.isError ? (
              <Empty title={t("routines.list.loadFailed")}>{String(list.error)}</Empty>
            ) : list.isPending ? (
              <Empty title={t("routines.list.loading")} />
            ) : routines.length === 0 ? (
              <Empty title={t("routines.list.empty")}>{t("routines.list.emptyBody")}</Empty>
            ) : (
              <ul className="divide-y divide-line">
                {routines.map((r) => (
                  <li key={r.id}>
                    <div
                      role="button"
                      tabIndex={0}
                      aria-current={current?.id === r.id && !draft ? "true" : undefined}
                      onClick={() => {
                        setSelected(r.id);
                        setDraft(null);
                      }}
                      onKeyDown={(e) => {
                        // Keys on the switch inside the row belong to the switch (QA F2).
                        if (e.target !== e.currentTarget) return;
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          setSelected(r.id);
                          setDraft(null);
                        }
                      }}
                      className={`flex cursor-pointer items-center gap-3 px-4 py-3 ${current?.id === r.id && !draft ? "bg-paper" : "hover:bg-paper/60"}`}
                    >
                      <CalendarClock size={16} className={r.enabled ? "text-ink-2" : "text-ink-3/60"} />
                      <div className="min-w-0 flex-1">
                        <p className={`truncate text-[13px] font-medium ${r.enabled ? "text-ink" : "text-ink-3"}`} title={r.name}>
                          {r.name}
                        </p>
                        <p className="truncate text-xs text-ink-3" title={`${scheduleText(r.schedule)} · ${r.cwd}`}>
                          {scheduleText(r.schedule)} · {shortPath(r.cwd)}
                        </p>
                        {r.active && <p className="mt-0.5 text-[11px] font-medium text-ink-2">{t("routines.list.running")}</p>}
                      </div>
                      <Toggle
                        on={r.enabled}
                        label={t(r.enabled ? "routines.toggle.off" : "routines.toggle.on", { name: r.name })}
                        onChange={async (v) => {
                          await api.setRoutineEnabled(r.id, v).catch(() => undefined);
                          qc.invalidateQueries({ queryKey: ["routines"] });
                        }}
                      />
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </Sheet>
          <p className="px-1 text-[11px] leading-relaxed text-ink-3">
            {t("routines.list.note")}
          </p>
        </aside>
        <section className="min-w-0">
          {draft ? (
            <Form
              key={draft.id ?? "new"}
              draft={draft}
              onCancel={() => setDraft(null)}
              onDone={(id) => {
                setDraft(null);
                setSelected(id);
                qc.invalidateQueries({ queryKey: ["routines"] });
              }}
            />
          ) : current ? (
            <Detail
              key={current.id}
              r={current}
              onOpenTask={onOpenTask}
              onEdit={() =>
                setDraft({
                  id: current.id,
                  name: current.name,
                  cwd: current.cwd,
                  prompt: "",
                  permissionMode: current.permissionMode,
                  schedule: current.schedule,
                })
              }
              onDeleted={() => {
                setSelected(null);
                qc.invalidateQueries({ queryKey: ["routines"] });
              }}
            />
          ) : (
            <Sheet className="p-10">
              <Empty title={t("routines.list.first")}>{t("routines.list.firstBody")}</Empty>
            </Sheet>
          )}
        </section>
      </div>
    </div>
  );
}
