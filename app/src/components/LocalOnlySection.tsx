// "원격 push 막기" (delivery-guard, D-143): one checkbox per project. Ticking saves at once; unticking asks in a native
// dialog (Rust), so the list only changes after the backend answers (no optimistic update for the less safe side).
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, GitPullRequestDraft, Lock } from "lucide-react";
import { deliveryApi, GUARD_COPY, guardLine, type LocalOnlyRow } from "../lib/delivery";
import { offReasonText } from "../lib/projects";
import { Button } from "./ui";
import { translate } from "../i18n";
import { useT } from "../i18n/react";

export function LocalOnlyList() {
  const t = useT();
  const qc = useQueryClient();
  const q = useQuery({ queryKey: ["local-only"], queryFn: deliveryApi.localOnlyProjects });
  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["local-only"] });
    void qc.invalidateQueries({ queryKey: ["projects-known"] });
    void qc.invalidateQueries({ queryKey: ["delivery-guard"] });
  };
  const set = useMutation({ mutationFn: ({ id, on }: { id: string; on: boolean }) => deliveryApi.setLocalOnly(id, on), onSettled: refresh });
  if (q.isError) return <p className="text-xs text-danger">{GUARD_COPY.unreadable}</p>;
  const rows = q.data ?? [];
  const top = rows.filter((r) => !r.worktreeOf);
  const kids = (id: string) => rows.filter((r) => r.worktreeOf === id);
  return (
    <div className="space-y-1.5">
      {rows.length === 0 && !q.isLoading && <p className="text-xs text-ink-3">{t("tasks.delivery.noProjects")}</p>}
      <ul className="space-y-0.5" aria-label={GUARD_COPY.title}>
        {top.map((r) => (
          <ProjectRow key={r.id} r={r} worktrees={kids(r.id).length} busy={set.isPending} onChange={(on) => set.mutate({ id: r.id, on })} />
        ))}
      </ul>
      <p className="text-xs text-ink-3">{GUARD_COPY.running}</p>
    </div>
  );
}

function ProjectRow({ r, worktrees, busy, onChange }: { r: LocalOnlyRow; worktrees: number; busy: boolean; onChange: (on: boolean) => void }) {
  const t = useT();
  const disabled = busy || (!!r.offReason && !r.checked);
  const id = `local-only-${r.id}`;
  return (
    <li className="flex items-start gap-2.5 rounded-lg px-1.5 py-1.5 hover:bg-paper">
      <span className="relative mt-0.5 inline-flex h-4 w-4 shrink-0">
        <input
          id={id}
          type="checkbox"
          checked={r.checked}
          disabled={disabled}
          onChange={(e) => onChange(e.target.checked)}
          className="peer h-4 w-4 cursor-pointer appearance-none rounded border border-ink-3/60 bg-paper checked:border-ink checked:bg-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink disabled:cursor-not-allowed disabled:opacity-50"
        />
        <Check size={12} strokeWidth={3} className="pointer-events-none absolute top-0.5 left-0.5 hidden text-paper peer-checked:block" aria-hidden />
      </span>
      <label htmlFor={id} className={`min-w-0 flex-1 ${disabled ? "cursor-not-allowed" : "cursor-pointer"}`}>
        <span className="flex items-center gap-1.5 text-[13px]">
          <span className={`truncate ${r.missing ? "text-ink-2" : ""}`}>{r.name}</span>
          {r.checked && (
            <span className="inline-flex shrink-0 items-center gap-0.5 rounded-md border border-line px-1.5 text-[10px] text-ink-2">
              <Lock size={9} strokeWidth={2.2} aria-hidden />
              {GUARD_COPY.chip}
            </span>
          )}
        </span>
        <span className="block truncate text-[11px] text-ink-3">
          {r.missing ? `${GUARD_COPY.missing} · ` : r.offReason && !r.checked ? `${offReasonText(r.offCode, r.offReason)} · ` : ""}
          {r.parentDir ?? ""}
          {worktrees > 0 ? t("tasks.delivery.worktrees", { count: worktrees }) : ""}
        </span>
      </label>
    </li>
  );
}

/** Composer: the scope this folder starts with (the same lookup as the start), plus "이 프로젝트는 로컬만". */
export function DeliveryGuardLine({ cwd, worktree }: { cwd: string; /** The composer will start in a worktree anyway. */ worktree: boolean }) {
  useT();
  const qc = useQueryClient();
  const enabled = cwd.startsWith("/");
  const q = useQuery({ queryKey: ["delivery-guard", cwd], queryFn: () => deliveryApi.preview(cwd), enabled, staleTime: 10_000, retry: false });
  const tick = useMutation({
    mutationFn: () => deliveryApi.setLocalOnlyCwd(cwd),
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: ["delivery-guard"] });
      void qc.invalidateQueries({ queryKey: ["local-only"] });
      void qc.invalidateQueries({ queryKey: ["projects-known"] });
    },
  });
  if (!enabled) return null;
  if (q.isError) {
    const msg = String(q.error);
    // The backend still sends this one as Korean text (app-language moves it to a code later).
    return msg.includes(GUARD_COPY.unreadable) || msg.includes(translate("ko", "tasks.delivery.unreadable")) ? <p className="text-xs text-danger">{GUARD_COPY.unreadable}</p> : null;
  }
  const p = q.data;
  if (!p) return null;
  const local = p.guard === "local_only";
  return (
    <div className="space-y-1" data-testid="delivery-guard">
      <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-ink-2">
        <span className="inline-flex items-center gap-1.5">
          {local ? <Lock size={12} strokeWidth={2} aria-hidden /> : <GitPullRequestDraft size={12} strokeWidth={2} aria-hidden />}
          {guardLine(p.guard)}
        </span>
        {p.canTick && (
          <Button type="button" disabled={tick.isPending} onClick={() => tick.mutate()} size="sm">
            {GUARD_COPY.tickHere}
          </Button>
        )}
      </p>
      {/* The worktree box above shows a forced worktree; this line only covers a hidden box. */}
      {p.forcedWorktree && !worktree && <p className="text-xs text-ink-3">{GUARD_COPY.forcedWorktree}</p>}
      {tick.isError && <p className="text-xs text-danger">{String(tick.error)}</p>}
    </div>
  );
}
