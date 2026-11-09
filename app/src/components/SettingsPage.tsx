import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useEffect } from "react";
import { api, type Status } from "../lib/api";
import { Button, MoreInfo, PageHeader } from "./ui";
import { requestAgentSettings } from "../lib/team";
import { markDone, openTour } from "../lib/onboarding";
import { GUARD_COPY } from "../lib/delivery";
import { LocalOnlyList } from "./LocalOnlySection";
import { AuthSettingsRow } from "./LoginBits";
import { COPY } from "../lib/auth";
import { changeLanguage, useLanguage, useT } from "../i18n/react";
import { LANGUAGE_NAMES, parseChoice } from "../i18n/language";

function Row({ title, description, more, children }: { title: string; description?: ReactNode; /** Folded under 자세히 (one-line rule). */ more?: ReactNode; children: ReactNode }) {
  return (
    <div className="grid gap-3 border-b border-line py-5 last:border-0 md:grid-cols-[260px_1fr]">
      <div>
        <h3 className="text-[13px] font-semibold">{title}</h3>
        {description && <p className="mt-0.5 text-xs text-ink-3">{description}</p>}
        {more && <MoreInfo className="mt-1">{more}</MoreInfo>}
      </div>
      <div className="space-y-2">{children}</div>
    </div>
  );
}

function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-10 shrink-0 rounded-full transition-colors ${checked ? "bg-ink" : "bg-ink-3/40"}`}
    >
      <span className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-all ${checked ? "left-[18px]" : "left-0.5"}`} />
    </button>
  );
}

/** 언어: applies at once, without remounting the page, so open drafts keep their text. */
function LanguageRow() {
  const t = useT();
  const { choice } = useLanguage();
  return (
    <Row title={t("settings.language.title")} description={t("settings.language.description")} more={t("settings.language.more")}>
      <select
        aria-label={t("settings.language.title")}
        value={choice}
        onChange={(e) => void changeLanguage(parseChoice(e.target.value))}
        className="h-8 w-48 rounded-lg border border-line bg-sheet px-2 text-[13px]"
      >
        <option value="system">{t("common.language.system")}</option>
        <option value="ko" lang="ko">
          {LANGUAGE_NAMES.ko}
        </option>
        <option value="en" lang="en">
          {LANGUAGE_NAMES.en}
        </option>
      </select>
    </Row>
  );
}

export function SettingsPage({ onOpenAgentSettings, onOpenMcp }: { onOpenAgentSettings?: () => void; onOpenMcp?: () => void } = {}) {
  const t = useT();
  const qc = useQueryClient();
  const status = useQuery({ queryKey: ["status"], queryFn: api.status });
  const files = useQuery({ queryKey: ["usage", "files"], queryFn: api.ingestFiles });
  const refresh = () => qc.invalidateQueries({ queryKey: ["status"] });
  const pickCli = useMutation({ mutationFn: api.pickCliPath, onSettled: refresh });
  const resetCli = useMutation({ mutationFn: api.resetCliPath, onSettled: refresh });
  const pickDir = useMutation({ mutationFn: api.pickConfigDir, onSettled: refresh });
  const resetDir = useMutation({ mutationFn: api.resetConfigDir, onSettled: refresh });
  // Toggles update the cached status immediately, so a quick second click sends the opposite value, not the same one.
  const setStatusField = (field: "storeTitles" | "notifyWaiting" | "notifyFinished" | "remoteAlerts" | "taskReport" | "limitNotify" | "limitAutoResume", value: boolean) =>
    qc.setQueryData<Status>(["status"], (old) => (old ? { ...old, [field]: value } : old));
  const titles = useMutation({
    mutationFn: api.setStoreTitles,
    onMutate: (v: boolean) => setStatusField("storeTitles", v),
    onSettled: () => setTimeout(() => qc.invalidateQueries(), 500),
  });
  const notify = useMutation({
    mutationFn: (on: boolean) => api.setPref("notify_waiting", on ? "1" : "0"),
    onMutate: (v: boolean) => setStatusField("notifyWaiting", v),
    onSuccess: (_r, on) => on && markDone("notifyOn"),
    onSettled: refresh,
  });
  const limitNotify = useMutation({
    mutationFn: (on: boolean) => api.setPref("limit_notify", on ? "1" : "0"),
    onMutate: (v: boolean) => setStatusField("limitNotify", v),
    onSettled: refresh,
  });
  const limitAuto = useMutation({
    mutationFn: (on: boolean) => api.setPref("limit_auto_resume", on ? "1" : "0"),
    onMutate: (v: boolean) => setStatusField("limitAutoResume", v),
    onSettled: () => {
      refresh();
      qc.invalidateQueries({ queryKey: ["limits"] });
    },
  });
  const taskReport = useMutation({
    mutationFn: (on: boolean) => api.setPref("task_report", on ? "1" : "0"),
    onMutate: (v: boolean) => setStatusField("taskReport", v),
    onSettled: refresh,
  });
  const capacity = useMutation({
    mutationFn: (n: Status["reviewCapacity"]) => api.setPref("review_capacity", String(n)),
    onMutate: (n: Status["reviewCapacity"]) => qc.setQueryData<Status>(["status"], (old) => (old ? { ...old, reviewCapacity: n } : old)),
    onSettled: refresh,
  });
  const notifyDone = useMutation({
    mutationFn: (on: boolean) => api.setPref("notify_finished", on ? "1" : "0"),
    onMutate: (v: boolean) => setStatusField("notifyFinished", v),
    onSuccess: (_r, on) => on && markDone("notifyOn"),
    onSettled: refresh,
  });
  // Turning on goes through the backend's native confirmation; the cached value follows the real result.
  const remote = useMutation({ mutationFn: (on: boolean) => api.setRemoteAlerts(on), onSettled: refresh });
  const storePrompts = useMutation({ mutationFn: (on: boolean) => api.setStoreTaskPrompts(on), onSettled: refresh });
  const wipeHistory = useMutation({ mutationFn: api.deleteAllTaskRecords, onSettled: () => qc.invalidateQueries({ queryKey: ["task-records"] }) });
  const rebuild = useMutation({ mutationFn: api.rebuildUsage });
  const pickDefault = useMutation({ mutationFn: api.pickDefaultCwd, onSettled: refresh });
  const resetDefault = useMutation({ mutationFn: api.resetDefaultCwd, onSettled: refresh });
  const s = status.data;
  // Checklist "알림 켜기": ticks once the user sees or turns on a notification setting that is on.
  const notifyAnyOn = Boolean(s?.notifyWaiting || s?.notifyFinished);
  useEffect(() => {
    if (notifyAnyOn) markDone("notifyOn");
  }, [notifyAnyOn]);
  const skipped = (files.data ?? []).filter((f) => f.skippedLines > 0);

  return (
    <div>
      <PageHeader title={t("settings.title")} />
      <div className="mx-auto max-w-[880px] px-8 py-4">
        <LanguageRow />
        {onOpenAgentSettings && (
          <Row title={t("settings.agentSettings.title")} description={t("settings.agentSettings.description")} more={t("settings.agentSettings.more")}>
            <Button
              onClick={() => {
                requestAgentSettings();
                onOpenAgentSettings();
              }}
            >
              {t("settings.agentSettings.open")}
            </Button>
          </Row>
        )}
        <Row title={t("settings.tour.title")} description={t("settings.tour.description")} more={t("settings.tour.more")}>
          <div>
            <Button onClick={openTour}>{t("settings.tour.open")}</Button>
          </div>
        </Row>
        <Row title={t("settings.cli.title")} description={t("settings.cli.description")} more={t("settings.cli.more")}>
          <p className="font-mono text-xs break-all text-ink-2">{s?.cli ? s.cli.path : t("settings.cli.notFound")}</p>
          {s?.cli && (
            <p className="text-xs text-ink-3">
              {t("settings.cli.version", { version: s.cli.version })}
              {s.cli.supported ? "" : t("settings.cli.unverified")}
            </p>
          )}
          <div className="flex gap-2">
            <Button onClick={() => pickCli.mutate()}>{t("settings.cli.pick")}</Button>
            <Button onClick={() => resetCli.mutate()}>{t("settings.cli.auto")}</Button>
          </div>
          {pickCli.isError && <p className="text-xs text-danger">{String(pickCli.error)}</p>}
        </Row>
        {s?.cli && (
          <Row title={t("settings.login.title")} description={t("settings.login.description")} more={COPY.logout}>
            <AuthSettingsRow />
          </Row>
        )}
        <Row title={t("settings.defaultCwd.title")} description={t("settings.defaultCwd.description")} more={t("settings.defaultCwd.more")}>
          <p className="font-mono text-xs break-all text-ink-2">{s?.defaultCwd ?? <span className="font-sans text-ink-3">{t("settings.defaultCwd.none")}</span>}</p>
          <div className="flex gap-2">
            <Button onClick={() => pickDefault.mutate()}>{t("settings.defaultCwd.pick")}</Button>
            {s?.defaultCwd && <Button onClick={() => resetDefault.mutate()}>{t("settings.defaultCwd.clear")}</Button>}
          </div>
        </Row>
        <Row title={t("settings.configDir.title")} description={t("settings.configDir.description")} more={t("settings.configDir.more")}>
          <p className="font-mono text-xs break-all text-ink-2">
            {s?.configDir}
            {s && !s.configDirExplicit && <span className="ml-2 font-sans text-ink-3">{t("settings.configDir.isDefault")}</span>}
          </p>
          <div className="flex gap-2">
            <Button onClick={() => pickDir.mutate()}>{t("settings.configDir.pick")}</Button>
            <Button onClick={() => resetDir.mutate()}>{t("settings.configDir.reset")}</Button>
          </div>
          {pickDir.isError && <p className="text-xs text-danger">{String(pickDir.error)}</p>}
        </Row>
        <Row title={t("settings.storeTitles.title")} description={t("settings.storeTitles.description")} more={t("settings.storeTitles.more")}>
          <Toggle label={t("settings.storeTitles.title")} checked={s?.storeTitles ?? true} onChange={(v) => titles.mutate(v)} />
        </Row>
        <Row title={t("settings.notifyWaiting.title")} description={t("settings.notifyWaiting.description")}>
          <Toggle label={t("settings.notifyWaiting.title")} checked={s?.notifyWaiting ?? true} onChange={(v) => notify.mutate(v)} />
        </Row>
        <Row title={t("settings.notifyFinished.title")} description={t("settings.notifyFinished.description")} more={t("settings.notifyFinished.more")}>
          <Toggle label={t("settings.notifyFinished.title")} checked={s?.notifyFinished ?? true} onChange={(v) => notifyDone.mutate(v)} />
        </Row>
        <Row title={t("settings.limitNotify.title")} description={t("settings.limitNotify.description")} more={t("settings.limitNotify.more")}>
          <Toggle label={t("settings.limitNotify.title")} checked={s?.limitNotify ?? true} onChange={(v) => limitNotify.mutate(v)} />
        </Row>
        <Row
          title={t("settings.limitAuto.title")}
          description={t("settings.limitAuto.description")}
          more={s && !s.limitAutoShipped ? t("settings.limitAuto.notShipped") : t("settings.limitAuto.more")}
        >
          <Toggle label={t("settings.limitAuto.title")} checked={s?.limitAutoResume ?? true} onChange={(v) => limitAuto.mutate(v)} />
        </Row>
        <Row
          title={t("settings.remote.title")}
          description={t("settings.remote.description")}
          more={t("settings.remote.more")}
        >
          <Toggle label={t("settings.remote.toggle")} checked={s?.remoteAlerts ?? false} onChange={(v) => remote.mutate(v)} />
          {remote.isError && (
            <p role="alert" className="text-xs text-danger">
              {String(remote.error)}
            </p>
          )}
        </Row>
        <Row
          title={t("settings.capacity.title")}
          description={t("settings.capacity.description")}
          more={t("settings.capacity.more")}
        >
          <select
            aria-label={t("settings.capacity.title")}
            value={String(s?.reviewCapacity ?? 3)}
            onChange={(e) => capacity.mutate(e.target.value === "unlimited" ? "unlimited" : Number(e.target.value))}
            className="h-8 w-28 rounded-lg border border-line bg-sheet px-2 text-[13px]"
          >
            {Array.from({ length: 10 }, (_, i) => i + 1).map((n) => (
              <option key={n} value={n}>
                {t("settings.capacity.count", { count: n })}
              </option>
            ))}
            <option value="unlimited">{t("settings.capacity.unlimited")}</option>
          </select>
        </Row>
        <Row
          title={t("settings.storePrompts.title")}
          description={t("settings.storePrompts.description")}
          more={t("settings.storePrompts.more")}
        >
          <Toggle label={t("settings.storePrompts.title")} checked={s?.storeTaskPrompts ?? true} onChange={(v) => storePrompts.mutate(v)} />
          <div>
            <Button variant="danger" disabled={wipeHistory.isPending} onClick={() => wipeHistory.mutate()}>
              {t("settings.storePrompts.wipe")}
            </Button>
          </div>
        </Row>
        <Row title={t("settings.taskReport.title")} description={t("settings.taskReport.description")} more={t("settings.taskReport.more")}>
          <Toggle label={t("settings.taskReport.title")} checked={s?.taskReport ?? true} onChange={(v) => taskReport.mutate(v)} />
        </Row>
        <Row title={GUARD_COPY.title} description={GUARD_COPY.description} more={t("settings.localOnlyMore", { cloud: GUARD_COPY.cloud, worktree: GUARD_COPY.worktree })}>
          <LocalOnlyList />
        </Row>
        <Row title={t("settings.mcp.title")} description={t("settings.mcp.description")}>
          <div>
            <Button onClick={onOpenMcp}>{t("settings.mcp.open")}</Button>
          </div>
        </Row>
        <Row title={t("settings.history.title")} description={t("settings.history.files", { count: files.data?.length ?? 0 })}>
          <div>
            <Button disabled={rebuild.isPending} onClick={() => rebuild.mutate()}>
              {t("settings.history.rebuild")}
            </Button>
            {rebuild.isSuccess && <span className="ml-3 text-xs text-ink-3">{t("settings.history.rebuilding")}</span>}
          </div>
          {skipped.length === 0 ? (
            <p className="text-xs text-ink-3">{t("settings.history.noneSkipped")}</p>
          ) : (
            <ul className="space-y-1 text-xs">
              {skipped.map((f) => (
                <li key={f.path} className="flex gap-3">
                  <span className="min-w-0 flex-1 truncate font-mono text-ink-2" title={f.path}>
                    {f.path}
                  </span>
                  <span className="text-danger">{t("settings.history.skipped", { count: f.skippedLines })}</span>
                </li>
              ))}
            </ul>
          )}
        </Row>
      </div>
    </div>
  );
}
