// New background task, tech-lead style (D-061, D-063):
// type preset → role/permission/effort, a structured brief, sensitive-area cap, optional worktree, review capacity.
import { useMutation, useQuery } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { ChevronDown, FolderOpen, Lock, ShieldAlert, X } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { api, type AgentsPayload, type Status } from "../lib/api";
import { personasFor, sourceText } from "../lib/agents";
import { composeBrief, EFFORTS, EMPTY_BRIEF, overCapacity as isOverCapacity, reviewLoad, type Brief, type Draft, type EffortId, type TaskTypeId } from "../lib/brief";
import { isOffAnywhere, isReviewType, MODE_LABEL, useAgentControls, useTeamConfig, writesFiles } from "../lib/team";
import { cliErrorDetail, cliErrorMessage, DANGEROUS_MODES, localized, validateTaskName } from "../lib/format";
import { Button, fieldClass, Sheet } from "./ui";
import { markDone } from "../lib/onboarding";
import { ADVICE_GOALS, HOME_ONLY_CLAUDE, looksLikeHome } from "../lib/refine";
import { BriefRefiner } from "./BriefRefiner";
import { isEvidenceStrictType } from "../lib/report";
import { DeliveryGuardLine } from "./LocalOnlySection";
import { deliveryApi, GUARD_COPY } from "../lib/delivery";
import { ExpiryLine, GateLine } from "./LoginBits";
import { isGateError } from "../lib/auth";
import { START_TASK_KEY, StartingCard, newStartId, useStartingCards, type StartVars } from "./StartingCards";
import { useT } from "../i18n/react";
import { forgetComposer, keepComposer, recallComposer } from "../lib/composerMemory";

const REPO_REASON: Record<string, string> = localized({
  notRepo: "tasks.composer.repo.notRepo",
  homeRoot: "tasks.composer.repo.homeRoot",
  noCommits: "tasks.composer.repo.noCommits",
});

/** Task type chip (radio in the composer, a still copy in the tour). */
export const typeChipClass = (on: boolean) => `h-7 rounded-full border px-3 text-[12px] transition-colors ${on ? "border-ink bg-ink font-medium text-paper" : "border-line bg-sheet text-ink-2 hover:border-ink-3"}`;

function Field({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <label className="block space-y-1">
      <span className="text-xs font-medium text-ink-2">{label}</span>
      {children}
      {hint && <span className="block text-xs text-ink-3">{hint}</span>}
    </label>
  );
}

export function Composer({ status, agents, draft, onClose: close }: { status?: Status; agents?: AgentsPayload; draft: Draft | null; onClose: () => void }) {
  const t = useT();
  // Unsent input survives a trip to another page (memory only, see composerMemory).
  const memoryKey = JSON.stringify(draft ?? {});
  const [kept] = useState(() => recallComposer(memoryKey));
  const [cwd, setCwd] = useState(kept?.cwd || draft?.cwd || status?.defaultCwd || "");
  const [taskType, setTaskType] = useState<TaskTypeId | null>(kept ? kept.taskType : (draft?.taskType ?? null));
  const [brief, setBrief] = useState<Brief>(kept?.brief ?? draft?.brief ?? EMPTY_BRIEF);
  const [name, setName] = useState(kept?.name ?? draft?.name ?? "");
  // `null` = follow the type preset; a value = the user changed it and it sticks.
  const [modeChoice, setModeChoice] = useState<string | null>(kept ? kept.modeChoice : (draft?.permissionMode ?? null));
  const [effortChoice, setEffortChoice] = useState<EffortId | null>(kept?.effortChoice ?? null);
  const [sensitive, setSensitive] = useState(kept?.sensitive ?? false);
  // `null` = follow the type default (dev → on in a safe repository, AG-16).
  const [worktreeChoice, setWorktreeChoice] = useState<boolean | null>(kept?.worktreeChoice ?? null);
  const [persona, setPersona] = useState(kept?.persona ?? "");
  // Follows the Settings switch unless changed here (D-067).
  const [remoteChoice, setRemoteChoice] = useState<boolean | null>(kept?.remoteChoice ?? null);
  // "이번 작업 보고서": follows the Settings switch unless changed here (decision-report).
  const [reportChoice, setReportChoice] = useState<boolean | null>(kept?.reportChoice ?? null);
  // "리셋되면 자동으로 이어서 하기": follows the Settings switch unless changed here (usage-limit-resume).
  const [limitChoice, setLimitChoice] = useState<boolean | null>(kept?.limitChoice ?? null);
  const closed = useRef(false);
  useEffect(() => {
    if (!closed.current) keepComposer(memoryKey, { cwd, taskType, brief, name, modeChoice, effortChoice, sensitive, worktreeChoice, persona, remoteChoice, reportChoice, limitChoice });
  }, [memoryKey, cwd, taskType, brief, name, modeChoice, effortChoice, sensitive, worktreeChoice, persona, remoteChoice, reportChoice, limitChoice]);
  // Closed on purpose or started: the kept input goes.
  const onClose = () => {
    closed.current = true;
    forgetComposer();
    close();
  };
  const [showPreview, setShowPreview] = useState(false);
  // Over review capacity: the first submit only shows the warning; the second one starts (AG-08).
  const [ackCapacity, setAckCapacity] = useState(false);
  const [awaitingTrust, setAwaitingTrust] = useState(false);
  useEffect(() => {
    const un = listen<string>("task-start-status", (e) => setAwaitingTrust(e.payload === "awaiting_trust"));
    return () => {
      un.then((f) => f());
    };
  }, []);

  // Types come from the editable team config (D-081); the backend re-merges and re-checks it on start.
  const team = useTeamConfig();
  const controls = useAgentControls();
  const types = team.data?.config.types;
  const typeOptions = (types ?? []).filter((t) => !t.hidden || t.key === taskType);
  const preset = (taskType && types?.find((t) => t.key === taskType)) || null;
  const roleLabel = preset ? (preset.agent ? t("tasks.composer.myAgent", { name: preset.agent }) : preset.roleLabel) : "";
  const roleOff = !!preset && isOffAnywhere(preset.agent ?? preset.role, controls.data);
  const allModes = status?.permissionModes ?? ["manual"];
  // Drafts from usage insights: Claude's advice, or any insight run in the whole home folder, plan only (the
  // backend enforces the same).
  const origin = draft?.origin ?? null;
  const atHome = !!origin && looksLikeHome(cwd);
  const planOnly = origin === "insight-claude" || atHome;
  const modes = (sensitive ? ["plan", "manual"] : preset ? preset.modes : allModes).filter((m) => allModes.includes(m) && (!planOnly || m === "plan"));
  const mode = modeChoice && modes.includes(modeChoice) ? modeChoice : preset && modes.includes(preset.mode) ? preset.mode : modes.includes("manual") ? "manual" : modes[0];
  const effortBase = effortChoice ?? ((preset?.effort as EffortId | undefined) ?? null);
  const effort: EffortId | null = sensitive ? (effortBase === "max" ? "max" : "xhigh") : effortBase;

  const catalog = useQuery({ queryKey: ["agent-catalog", cwd], queryFn: () => api.agentCatalog(cwd || null), staleTime: 30_000 });
  // Agents turned off with `Agent(<name>)` are not offered (the backend refuses them too).
  const personas = personasFor(catalog.data ?? []).filter((d) => !isOffAnywhere(d.name, controls.data));
  const chosen = personas.find((d) => d.name === persona);
  const agent = chosen?.name ?? "";
  const risky = !!chosen && (chosen.risk.hooks || chosen.risk.mcpServers || chosen.risk.dangerMode || chosen.risk.unreadable);

  const repo = useQuery({ queryKey: ["repo", cwd], queryFn: () => api.repoCheck(cwd), enabled: !!cwd, staleTime: 30_000 });
  const repoOk = repo.data?.kind === "ok";
  // The default branch is checked out: the backend starts in a worktree anyway (delivery-guard), so the box shows it.
  const guard = useQuery({ queryKey: ["delivery-guard", cwd], queryFn: () => deliveryApi.preview(cwd), enabled: cwd.startsWith("/"), staleTime: 10_000, retry: false });
  const forcedWorktree = repoOk && !atHome && !origin && !!guard.data?.forcedWorktree;
  const useWorktree = forcedWorktree || (repoOk && !atHome && (worktreeChoice ?? taskType === "dev"));

  const remoteBlocked = sensitive || !!preset?.strict || DANGEROUS_MODES.has(mode) || planOnly;
  const remoteControl = !!status?.remoteAlerts && !remoteBlocked && (remoteChoice ?? true);
  // Phone-controlled tasks get no report folder (the backend skips it too).
  // Review and QA leave evidence in the report: with the setting off it is still on for this task unless unchecked
  // here (report-evidence, D-152 (7)).
  const evidenceStrict = isEvidenceStrictType(taskType);
  const report = !remoteControl && (reportChoice ?? (evidenceStrict ? true : (status?.taskReport ?? true)));
  const reportNeeded = evidenceStrict && !remoteControl && (status?.taskReport === false || !report);
  // Deploy and phone-controlled tasks never resume on their own; dangerous modes are asked in the native dialog.
  const limitBlocked = taskType === "deploy" ? t("tasks.composer.limitDeploy") : remoteControl ? t("tasks.composer.limitRemote") : !status?.limitAutoResume ? t("tasks.composer.settingsOff") : null;
  const limitResume = !limitBlocked && (limitChoice ?? true);
  const capacity = status?.reviewCapacity ?? 3;
  const load = reviewLoad(agents, types);
  const overCapacity = isReviewType(types, taskType) && isOverCapacity(load, capacity);

  // In the home folder the task may only touch the matching file under ~/.claude.
  const effective = atHome && !brief.constraints.includes(HOME_ONLY_CLAUDE) ? { ...brief, constraints: [brief.constraints.trim(), HOME_ONLY_CLAUDE].filter(Boolean).join("\n") } : brief;
  const prompt = composeBrief(effective);
  const startRefine = useRef<(() => void) | null>(null);
  const submitRefine = useRef<(() => boolean) | null>(null);
  const fields = useRef<Partial<Record<keyof Brief, HTMLTextAreaElement | null>>>({});
  const focusField = (k: keyof Brief) => {
    const el = fields.current[k];
    el?.scrollIntoView({ block: "center", behavior: "smooth" });
    el?.focus();
  };
  const refineBlocked = !status?.cli ? t("tasks.composer.noCli") : !brief.goal.trim() ? (origin === "insight-claude" || draft?.goalExamples?.length ? t("tasks.composer.goalOrExample") : t("tasks.composer.goalFirst")) : null;
  const nameError = validateTaskName(name);
  const start = useMutation({
    // The label rides along so the "시작하는 중" card can name the task (StartingCards, D-159).
    mutationKey: START_TASK_KEY,
    // The label is read back from the mutation cache; the start id lets the card cancel it (D-161).
    mutationFn: ({ startId }: StartVars) =>
      api.startTask({ cwd, prompt, name: name || null, permissionMode: mode || null, agent: agent || null, taskType, effort, sensitive, worktree: useWorktree, remoteControl, brief: effective, origin, skipReport: !report, wantReport: report && evidenceStrict && status?.taskReport === false, limitOptOut: !limitBlocked && limitChoice === false }, startId),
    onSuccess: () => {
      markDone("firstTask");
      setBrief(EMPTY_BRIEF);
      setName("");
      setTaskType(null);
      setModeChoice(null);
      setEffortChoice(null);
      setSensitive(false);
      setWorktreeChoice(null);
      setAckCapacity(false);
      // The composer closes once the task started; the board's "시작하는 중" card takes over (D-164).
      onClose();
    },
    onSettled: () => setAwaitingTrust(false),
  });
  // Strict types refuse personas that can write or run commands, not only risky ones (QA review).
  const strictWrites = !!preset?.strict && !!chosen && writesFiles(chosen);
  const strictRisky = (risky && !!preset?.strict) || strictWrites;
  const disabled = !status?.cli || !cwd || !brief.goal.trim() || !!nameError || start.isPending || (sensitive && risky) || strictRisky || (roleOff && !agent);
  // Why 작업 시작 is off, one line in the footer next to the button.
  const blocked = !status?.cli
    ? t("tasks.composer.blocked.noCli")
    : !cwd
      ? t("tasks.composer.blocked.noCwd")
      : !brief.goal.trim()
        ? t("tasks.composer.blocked.noGoal")
        : nameError || (sensitive && risky) || strictRisky || (roleOff && !agent)
          ? t("tasks.composer.blocked.check")
          : null;
  const starting = useStartingCards(agents?.sessions);
  const label = (name.trim() || brief.goal.trim().split("\n")[0]).slice(0, 60);
  const set = (k: keyof Brief) => (e: React.ChangeEvent<HTMLTextAreaElement>) => setBrief({ ...brief, [k]: e.target.value });

  return (
    <Sheet className="p-5">
      <form
        className="space-y-4"
        onKeyDown={(e) => {
          // ⌘↵ anywhere in the form answers the open refine questions (design review HIGH). It never starts the task.
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && submitRefine.current) {
            e.preventDefault();
            submitRefine.current();
          }
        }}
        onSubmit={(e) => {
          e.preventDefault();
          if (disabled) return;
          if (overCapacity && !ackCapacity) {
            setAckCapacity(true);
            return;
          }
          start.mutate({ label, startId: newStartId() });
        }}
      >
        <div className="flex items-center">
          <h2 className="flex-1 text-[14px] font-semibold">
            {t("tasks.composer.title")}
            {draft?.from && (
              <span className="ml-2 rounded-full bg-paper px-2 py-0.5 text-[11px] font-medium text-ink-2">{origin ? t("tasks.composer.source", { from: draft.from }) : t("tasks.composer.followUpOf", { from: draft.from })}</span>
            )}
          </h2>
          <button type="button" onClick={onClose} aria-label={t("common.actions.close")} className="rounded-md p-1 text-ink-3 hover:text-ink">
            <X size={16} />
          </button>
        </div>

        <Field label={t("tasks.composer.cwd.label")}>
          <div className="flex gap-2">
            <input value={cwd} onChange={(e) => setCwd(e.target.value)} placeholder="/Users/me/project" className={`${fieldClass} font-mono`} />
            <Button
              type="button"
              onClick={async () => {
                const picked = await api.pickDirectory();
                if (picked) setCwd(picked);
              }}
            >
              <FolderOpen size={14} />
              {t("tasks.composer.cwd.pick")}
            </Button>
          </div>
          {!cwd && draft?.cwdHint && <span className="block text-xs text-ink-2">{draft.cwdHint}</span>}
          {atHome && (
            <span role="alert" className="block text-xs text-danger">
              {t("tasks.composer.cwd.home", { rule: HOME_ONLY_CLAUDE })}
            </span>
          )}
          {status?.defaultCwd && cwd !== status.defaultCwd && (
            <button type="button" onClick={() => setCwd(status.defaultCwd as string)} className="text-xs text-ink-3 underline underline-offset-2">
              {t("tasks.composer.cwd.reset")}
            </button>
          )}
        </Field>

        <div className="space-y-1.5">
          <span className="text-xs font-medium text-ink-2">{t("tasks.composer.type.label")}</span>
          <div role="radiogroup" aria-label={t("tasks.composer.type.label")} className="flex flex-wrap gap-1.5">
            {typeOptions.map((t) => (
              <button
                key={t.key}
                type="button"
                role="radio"
                aria-checked={taskType === t.key}
                onClick={() => {
                  setTaskType(taskType === t.key ? null : (t.key as TaskTypeId));
                  // The new type decides permissions and isolation; a hand-picked effort stays.
                  setModeChoice(null);
                  setWorktreeChoice(null);
                }}
                className={typeChipClass(taskType === t.key)}
              >
                {t.label}
              </button>
            ))}
          </div>
          {preset && (
            <p className="text-xs text-ink-3">
              {agent
                ? t("tasks.composer.type.persona", { name: agent })
                : t("tasks.composer.type.role", { role: roleLabel, note: preset.note })}
            </p>
          )}
          {roleOff && !agent && <p className="text-xs text-danger">{t("tasks.composer.type.roleOff")}</p>}
          {team.isError && <p className="text-xs text-ink-3">{t("tasks.composer.type.teamError")}</p>}
          {taskType && types && !preset && <p className="text-xs text-danger">{t("tasks.composer.type.deleted", { type: taskType })}</p>}
        </div>

        {draft?.refineHint && (
          <div className="flex flex-wrap items-center gap-2 rounded-lg border border-warn/30 bg-warn-soft px-3 py-2 text-xs text-ink">
            <span className="min-w-0 flex-1">
              {draft.fromTour
                ? t("tasks.composer.refineHint.tour")
                : origin === "insight-claude"
                  ? t("tasks.composer.refineHint.advice")
                  : t("tasks.composer.refineHint.insight")}
            </span>
            {refineBlocked && <span className="text-ink-2">{refineBlocked}</span>}
            <Button
              type="button"
              variant="primary"
              size="sm"
              disabled={!!refineBlocked}
              onClick={() => {
                document.getElementById("brief-refiner")?.scrollIntoView({ block: "start", behavior: "smooth" });
                startRefine.current?.();
              }}
            >
              {t("tasks.composer.refineHint.action")}
            </Button>
          </div>
        )}

        <div className="space-y-3 rounded-lg border border-line bg-paper/60 p-3">
          {draft?.advice && (
            <figure className="space-y-1">
              <figcaption className="text-xs font-medium text-ink-2">{t("tasks.composer.adviceCaption")}</figcaption>
              <blockquote className="rounded-md border-l-2 border-line bg-sheet px-3 py-2 text-[13px] text-ink-2 select-text">{draft.advice}</blockquote>
              <span className="flex flex-wrap gap-1.5" role="group" aria-label={t("tasks.composer.goalExamples")}>
                {ADVICE_GOALS.map((g) => (
                  <button
                    key={g}
                    type="button"
                    onClick={() => setBrief({ ...brief, goal: g })}
                    aria-pressed={brief.goal === g}
                    className={`h-7 rounded-full border px-2.5 text-[12px] ${brief.goal === g ? "border-ink bg-ink text-paper" : "border-line bg-sheet text-ink-2 hover:border-ink-3"}`}
                  >
                    {g}
                  </button>
                ))}
              </span>
            </figure>
          )}
          {!draft?.advice && !!draft?.goalExamples?.length && (
            <span className="flex flex-wrap gap-1.5" role="group" aria-label={t("tasks.composer.goalExamples")}>
              {draft.goalExamples.map((g) => (
                <button
                  key={g}
                  type="button"
                  onClick={() => setBrief({ ...brief, goal: g })}
                  aria-pressed={brief.goal === g}
                  className={`h-7 rounded-full border px-2.5 text-[12px] ${brief.goal === g ? "border-ink bg-ink text-paper" : "border-line bg-sheet text-ink-2 hover:border-ink-3"}`}
                >
                  {g}
                </button>
              ))}
            </span>
          )}
          <Field label={t("tasks.composer.brief.goal")}>
            <textarea
              ref={(el) => {
                fields.current.goal = el;
              }}
              value={brief.goal}
              onChange={set("goal")}
              rows={2}
              placeholder={origin === "insight-claude" ? t("tasks.composer.brief.goalAdvicePlaceholder") : t("tasks.composer.brief.goalPlaceholder")}
              className={fieldClass}
            />
          </Field>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label={t("tasks.composer.brief.outOfScope")}>
              <textarea ref={(el) => void (fields.current.outOfScope = el)} value={brief.outOfScope} onChange={set("outOfScope")} rows={2} placeholder={t("tasks.composer.brief.outOfScopePlaceholder")} className={fieldClass} />
            </Field>
            <Field label={t("tasks.composer.brief.constraints")}>
              <textarea ref={(el) => void (fields.current.constraints = el)} value={brief.constraints} onChange={set("constraints")} rows={2} placeholder={t("tasks.composer.brief.constraintsPlaceholder")} className={fieldClass} />
            </Field>
          </div>
          <Field label={t("tasks.composer.brief.done")}>
            <textarea ref={(el) => void (fields.current.done = el)} value={brief.done} onChange={set("done")} rows={2} placeholder={t("tasks.composer.brief.donePlaceholder")} className={fieldClass} />
          </Field>
          <button type="button" onClick={() => setShowPreview(!showPreview)} aria-expanded={showPreview} className="flex items-center gap-1 text-xs text-ink-2">
            <ChevronDown size={12} className={showPreview ? "rotate-180" : ""} /> {t("tasks.composer.brief.preview")}
          </button>
          {showPreview && <pre className="max-h-48 overflow-auto rounded-md border border-line bg-sheet p-2 text-[11px] whitespace-pre-wrap text-ink-2">{prompt || t("tasks.composer.brief.previewEmpty")}</pre>}
        </div>

        <BriefRefiner
          brief={brief}
          taskType={taskType}
          strict={!!preset?.strict}
          cliReady={!!status?.cli}
          hint={draft?.refineHint}
          localNames={draft?.localNames ?? []}
          maxRounds={team.data?.config.refine?.rounds ?? 4}
          onApply={setBrief}
          startRef={startRefine}
          submitRef={submitRefine}
          onFocusField={focusField}
        />

        <label className="flex items-start gap-2 text-[13px]">
          <input type="checkbox" checked={sensitive} onChange={(e) => setSensitive(e.target.checked)} className="mt-0.5" />
          <span>
            <span className="font-medium">{t("tasks.composer.sensitive.title")}</span>
            <span className="block text-xs text-ink-3">{t("tasks.composer.sensitive.description")}</span>
          </span>
        </label>

        <div className="grid gap-3 sm:grid-cols-3">
          <Field label={t("tasks.composer.persona.label")} hint={chosen?.description ? <span className="line-clamp-2">{chosen.description}</span> : preset ? t("tasks.composer.persona.hint") : undefined}>
            <select value={agent} onChange={(e) => setPersona(e.target.value)} className={`${fieldClass} h-[38px]`}>
              <option value="">{preset ? t("tasks.composer.persona.typeDefault", { role: roleLabel }) : t("tasks.composer.persona.claudeDefault")}</option>
              {personas.map((d) => (
                <option key={d.id} value={d.name}>
                  {d.name} — {sourceText(d)}
                </option>
              ))}
            </select>
          </Field>
          <Field label={t("tasks.composer.mode")}>
            <select value={mode} onChange={(e) => setModeChoice(e.target.value)} className={`${fieldClass} h-[38px]`}>
              {modes.map((m) => (
                <option key={m} value={m}>
                  {MODE_LABEL[m] ?? m}
                  {preset?.mode === m ? t("tasks.composer.recommended") : ""}
                </option>
              ))}
            </select>
          </Field>
          <Field label={t("tasks.composer.effort.label")}>
            <select value={effort ?? ""} onChange={(e) => setEffortChoice((e.target.value || null) as EffortId | null)} className={`${fieldClass} h-[38px]`}>
              {!sensitive && <option value="">{t("tasks.composer.effort.cliDefault")}</option>}
              {EFFORTS.filter((x) => !sensitive || x.id === "xhigh" || x.id === "max").map((x) => (
                <option key={x.id} value={x.id}>
                  {x.label}
                  {preset?.effort === x.id ? t("tasks.composer.recommended") : ""}
                </option>
              ))}
            </select>
          </Field>
        </div>

        <div className="grid gap-3 sm:grid-cols-[1fr_auto] sm:items-end">
          <Field label={t("tasks.composer.name")}>
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder="fix-flaky-test" className={fieldClass} />
          </Field>
          <label className={`flex h-[38px] items-center gap-2 text-[13px] ${repoOk && !forcedWorktree ? "" : "text-ink-3"}`} title={forcedWorktree ? GUARD_COPY.forcedWorktree : repoOk ? undefined : REPO_REASON[repo.data?.kind ?? "notRepo"]}>
            <input type="checkbox" disabled={!repoOk || forcedWorktree} checked={useWorktree} onChange={(e) => setWorktreeChoice(e.target.checked)} />
            {t("tasks.composer.worktree.label")}
            {forcedWorktree && <Lock size={12} strokeWidth={2} aria-hidden />}
          </label>
        </div>
        {forcedWorktree && <p className="-mt-2 text-xs text-ink-3">{GUARD_COPY.forcedWorktree}</p>}
        {!repoOk && cwd && repo.data && <p className="-mt-2 text-xs text-ink-3">{REPO_REASON[repo.data.kind]}</p>}
        <label className={`flex items-center gap-2 text-[13px] ${status?.remoteAlerts && !remoteBlocked ? "" : "text-ink-3"}`}>
          <input
            type="checkbox"
            disabled={!status?.remoteAlerts || remoteBlocked}
            checked={remoteControl}
            onChange={(e) => setRemoteChoice(e.target.checked)}
          />
          {t("tasks.composer.remote.label")}
          <span className="text-xs text-ink-3">
            {!status?.remoteAlerts ? t("tasks.composer.settingsOff") : remoteBlocked ? t("tasks.composer.remote.blocked") : t("tasks.composer.remote.on")}
          </span>
        </label>
        <label className={`flex items-center gap-2 text-[13px] ${remoteControl ? "text-ink-3" : ""}`}>
          <input type="checkbox" disabled={remoteControl} checked={report} onChange={(e) => setReportChoice(e.target.checked)} />
          {t("tasks.composer.report.label")}
          <span className={`text-xs ${reportNeeded ? "text-ink-2" : "text-ink-3"}`} data-report-note>
            {remoteControl ? (evidenceStrict ? t("tasks.composer.report.remoteEvidence") : t("tasks.composer.report.remote")) : reportNeeded ? t("tasks.composer.report.needed") : t("tasks.composer.report.on")}
          </span>
        </label>
        <label className={`flex items-center gap-2 text-[13px] ${limitBlocked ? "text-ink-3" : ""}`}>
          <input type="checkbox" disabled={!!limitBlocked} checked={limitResume} onChange={(e) => setLimitChoice(e.target.checked)} />
          {t("tasks.composer.limit.label")}
          <span className="text-xs text-ink-3">
            {limitBlocked ?? (DANGEROUS_MODES.has(mode) ? t("tasks.composer.limit.danger") : t("tasks.composer.limit.usage"))}
          </span>
        </label>
        {useWorktree && (
          <p className="-mt-2 text-xs text-ink-3">
            {t("tasks.composer.worktree.help")}
          </p>
        )}

        {risky && (
          <p className={`flex items-start gap-1.5 text-xs ${sensitive ? "text-danger" : "text-ink-2"}`}>
            <ShieldAlert size={13} className="mt-0.5 shrink-0" />
            {sensitive
              ? t("tasks.composer.risk.sensitive")
              : strictRisky
                ? t("tasks.composer.risk.strict")
                : t("tasks.composer.risk.confirm")}
          </p>
        )}
        {strictWrites && !risky && (
          <p className="flex items-start gap-1.5 text-xs text-danger">
            <ShieldAlert size={13} className="mt-0.5 shrink-0" />
            {t("tasks.composer.risk.strictWrites")}
          </p>
        )}
        {DANGEROUS_MODES.has(mode) && <p className="text-xs text-danger">{t("tasks.composer.risk.dangerMode")}</p>}
        {nameError && <p className="text-xs text-danger">{nameError}</p>}
        {overCapacity && ackCapacity && (
          <p role="alert" className="rounded-lg border border-warn/30 bg-warn-soft px-3 py-2 text-xs text-ink">
            {t("tasks.composer.capacityWarn", { load, capacity })}
          </p>
        )}

        <DeliveryGuardLine cwd={cwd} worktree={useWorktree} />
        <GateLine force={start.isError ? isGateError(start.error) : null} />
        <ExpiryLine />

        {/* Sticky so the start button stays in view in a long brief. Opaque, and everything about starting (why it is
            off, progress, errors) lives inside it so nothing scrolls through or under it (D-159). */}
        <div data-composer-footer className="sticky bottom-0 z-10 -mx-5 -mb-5 space-y-2 rounded-b-xl border-t border-line bg-sheet px-5 py-3 shadow-[0_-8px_16px_-12px_color-mix(in_srgb,var(--color-scrim)_18%,transparent)]">
          {awaitingTrust && start.isPending && (
            <p className="rounded-lg border border-line bg-paper px-3 py-2 text-xs text-ink-2">
              {t("tasks.composer.trust")}
            </p>
          )}
          {starting.map((c) => (
            <StartingCard key={c.key} c={c} />
          ))}
          {start.isError && !isGateError(start.error) && (
            <div role="alert" className="text-xs text-danger">
              <p>{cliErrorMessage(start.error)}</p>
              {cliErrorDetail(start.error) && <pre className="mt-1 max-h-24 overflow-auto rounded-lg bg-danger/5 p-2 whitespace-pre-wrap">{cliErrorDetail(start.error)}</pre>}
            </div>
          )}
          <div className="flex items-center gap-3">
            <p className="min-w-0 flex-1 truncate text-xs text-ink-3" data-start-hint>
              {start.isSuccess && starting.length === 0
                ? taskType === "dev" && mode === "plan"
                  ? t("tasks.composer.startedPlan")
                  : t("tasks.composer.startedRunning")
                : (blocked ?? (overCapacity && !ackCapacity ? t("tasks.composer.capacityHint", { load, capacity }) : ""))}
            </p>
            <Button type="submit" variant="primary" disabled={disabled} size="field" className="shrink-0">
              {start.isPending ? t("tasks.composer.starting") : overCapacity && ackCapacity ? t("tasks.composer.startAnyway") : t("tasks.composer.start")}
            </Button>
          </div>
        </div>
      </form>
    </Sheet>
  );
}
