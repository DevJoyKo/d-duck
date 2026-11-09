// Mock-mode data for "원격 push 막기" (delivery-guard). Design review only: the real list, path checks, repository roots
// and the native confirmation live in Rust (delivery_guard.rs, delivery_cmds.rs).
import { mockLocalOnly as isLocal, mockProjects, mockTicked as ticked } from "./projects";

export const deliveryHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  local_only_projects: () =>
    mockProjects
      .filter((p) => p.path)
      .map((p) => ({
        id: p.id,
        name: p.name,
        parentDir: p.parentDir,
        checked: ticked.has(p.id),
        missing: p.folder === "missing",
        worktreeOf: p.worktreeOf,
        offReason: p.isHome ? "홈 폴더는 고를 수 없어요" : null,
        offCode: p.isHome ? "homeNotAllowed" : null,
      })),
  set_local_only: (a) => {
    const id = String(a.id ?? "");
    if (!mockProjects.some((p) => p.id === id)) throw "목록에 없는 프로젝트예요";
    if (a.on) ticked.add(id);
    // The real app asks in a native dialog first; the mock accepts.
    else ticked.delete(id);
    return ticked.has(id);
  },
  set_local_only_cwd: (a) => {
    const p = mockProjects.find((x) => x.path === a.cwd);
    if (p) ticked.add(p.id);
  },
  delivery_guard_preview: (a) => {
    const p = mockProjects.find((x) => x.path && String(a.cwd ?? "").startsWith(x.path));
    const local = p ? isLocal(p.id) : false;
    return { guard: local ? "local_only" : "draft_pr", ticked: p ? ticked.has(p.id) : false, canTick: !local && !p?.isHome,
      // Same condition as the backend: only where a worktree can be made (mock repo_check says "ok" for side-project).
      forcedWorktree: String(a.cwd ?? "").includes("side-project") };
  },
};
