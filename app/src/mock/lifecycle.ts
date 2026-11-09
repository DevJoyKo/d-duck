// Mock 리뷰 대기·정리 칸 (task-lifecycle) for design review, on top of the review tab's mock records:
// - 리뷰 대기: migrate-db (failed, listed), release-notes (stopped, listed), settings-layout and qa-lint-sweep (no longer
//   listed); total = the review badge.
// - 정리: lint-sweep (`?board=…` only: reviewed, listed; worktree left, no changes, PR #42 in the report, 로컬만), payment-refactor
//   (reviewed, no longer listed; worktree with 2 uncommitted changes), settings-polish (a terminal session started
//   outside D.Duck: "리뷰 없이 끝났어요").
// `?wrapup=many` adds 45 reviewed records to 정리 (40 cards, then "더 보기").
// Records the CLI no longer lists keep how they ended (`ended`): settings-layout done, payment-refactor failed (so 정리
// says "실패 · 리뷰했어요"), qa-lint-sweep never seen ended ("목록에서 사라진 작업").
// `?board=demo` turns this on (lint-sweep counts as reviewed); `?board=nolive` also drops every live session: the board still shows 리뷰 대기 and 정리.
import { isReviewed, pending, recs, since, wrapped } from "./review";
import { mockTaskReport } from "./report";
import { mockReviewReport } from "./review";
import { bt } from "./lang";

type Session = { key: string; id: string | null; status: string; name: string };
type Lane = "reviewWait" | "wrapup" | "done";

const param = (k: string) => (typeof location === "undefined" ? null : new URLSearchParams(location.search).get(k));
export const noLive = () => param("board") === "nolive";

/** Sessions without a D.Duck record that were wrapped up, by key. */
const wrappedKeys = new Set<string>();
/** Worktrees removed through "worktree 정리". */
const removed = new Set<string>();

const ended = (s: Session) => ["done", "failed", "stopped", "unknown"].includes(s.status);

function laneOf(s: Session): Lane | undefined {
  const r = s.id ? recs().find((x) => x.bgId === s.id) : undefined;
  if (r) {
    if (pending(r)) return "reviewWait";
    if (wrapped.has(r.bgId)) return "done";
    if (s.status === "failed" && !isReviewed(r.bgId)) return undefined;
    return "wrapup";
  }
  if (wrappedKeys.has(s.key)) return "done";
  return s.status === "failed" ? undefined : "wrapup";
}

export function mockLanes<T extends Record<string, object>>(sessions: Session[], tags: T): Record<string, object> {
  const out: Record<string, object> = { ...tags };
  for (const s of sessions.filter(ended)) {
    const lane = laneOf(s);
    if (lane) out[s.key] = { agent: null, taskType: null, ...(out[s.key] ?? {}), reviewed: s.id ? isReviewed(s.id) : false, lane };
  }
  return out;
}

const ENDED: Record<string, "done" | "failed" | "stopped"> = { "4b4b4b4b": "done", aa11bb22: "failed" };
const ENDED_OF: Record<string, "done" | "failed" | "stopped" | "unknown"> = { done: "done", failed: "failed", stopped: "stopped", unknown: "unknown" };
const manyWrapup = () => param("wrapup") === "many";
const MANY = Array.from({ length: 45 }, (_, i) => `d${String(i).padStart(7, "0")}`);

const NAMES: Record<string, string> = { "4b4b4b4b": "settings-layout", "5a5a5a5a": "qa-lint-sweep", aa11bb22: "payment-refactor", cc33dd44: "release-notes-1.2" };
const nameOf = (id: string) => NAMES[id] ?? (mockReviewReport(id) ?? mockTaskReport(id))?.delegated?.name ?? null;

export function lifecycleHandlers(sessions: () => Session[]): Record<string, (a: Record<string, unknown>) => unknown> {
  const listed = () => new Set(sessions().map((s) => s.id).filter(Boolean) as string[]);
  const endedOf = (id: string) => {
    const s = sessions().find((x) => x.id === id);
    return s ? (ENDED_OF[s.status] ?? null) : (ENDED[id] ?? null);
  };
  const card = (r: ReturnType<typeof recs>[number]) => ({ bgId: r.bgId, name: nameOf(r.bgId), folder: r.cwd.split("/").pop() ?? null, taskType: r.taskType, createdAt: r.createdAt, reviewed: isReviewed(r.bgId), listed: listed().has(r.bgId), ended: endedOf(r.bgId) });
  const many = () =>
    manyWrapup() ? MANY.filter((id) => !wrapped.has(id)).map((bgId, i) => ({ bgId, name: `cleanup-${i + 1}`, folder: "dev_team", taskType: "dev", createdAt: Date.now() - (i + 1) * 60_000, reviewed: true, listed: false, ended: "done" as const })) : [];
  return {
    board_wrapup: (a) => {
      const cap = Math.min(500, Math.max(40, Number(a.wrapupCards ?? 40)));
      const rw = recs().filter(pending);
      const wu = [...recs().filter((r) => isReviewed(r.bgId) && !wrapped.has(r.bgId) && r.createdAt >= since && r.running === null && !listed().has(r.bgId)).map(card), ...many()];
      return { reviewWait: { total: rw.length, items: rw.slice(0, 40).map(card) }, wrapup: { total: wu.length, items: wu.slice(0, cap) } };
    },
    wrap_task: (a) => {
      const s = sessions().find((x) => x.key === a.id || x.id === a.id);
      if (s && !ended(s)) throw "아직 정리할 수 없는 작업이에요";
      const bg = s?.id ?? String(a.id);
      if (manyWrapup() && MANY.includes(bg)) return (wrapped.add(bg), null);
      const r = recs().find((x) => x.bgId === bg);
      if (r) {
        if (pending(r)) throw "아직 정리할 수 없는 작업이에요";
        wrapped.add(bg);
        return null;
      }
      if (!s) throw "아직 정리할 수 없는 작업이에요";
      wrappedKeys.add(s.key);
      return null;
    },
    unwrap_task: (a) => {
      const s = sessions().find((x) => x.key === a.id || x.id === a.id);
      return wrapped.delete(s?.id ?? String(a.id)) || wrappedKeys.delete(s?.key ?? String(a.id));
    },
    wrapup_check: (a) => {
      const id = String(a.id);
      if (id === "9988aabb")
        return removed.has(id)
          ? { worktree: { kind: "gone" }, changes: null, pr: { number: 42, url: "https://github.com/example/dev-team/pull/42" }, guard: "local_only", refusal: "noWorktree" }
          : { worktree: { kind: "present", folder: "lint-sweep", branch: "dduck/lint-sweep" }, changes: 0, pr: { number: 42, url: "https://github.com/example/dev-team/pull/42" }, guard: "local_only", refusal: null };
      if (id === "aa11bb22") return { worktree: { kind: "present", folder: "payment-refactor", branch: "dduck/payment-refactor" }, changes: 2, pr: null, guard: "draft_pr", refusal: "changes" };
      return { worktree: null, changes: 0, pr: null, guard: null, refusal: "noWorktree" };
    },
    remove_worktree: (a) => {
      const id = String(a.id);
      if (id !== "9988aabb" || removed.has(id)) throw "지울 worktree가 없어요";
      // Stands in for the native confirm the backend shows.
      const ok = typeof window === "undefined" || window.confirm(bt("worktree를 지울까요?\n폴더: lint-sweep\n브랜치: dduck/lint-sweep\n무시된 파일 약 120개(빌드 결과 등)도 함께 지워져요\n브랜치와 커밋은 남겨요", "Delete the worktree?\nFolder: lint-sweep\nBranch: dduck/lint-sweep\nAbout 120 ignored files (build output and such) are deleted too\nThe branch and commits stay"));
      if (ok) removed.add(id);
      return ok;
    },
  };
}
