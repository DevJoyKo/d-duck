// Per-tab project (project-switcher, D-127; replaces D-111's app-wide pick). The provider wraps the page area and
// knows which tab is open: permissions, memory, extensions and MCP each keep their own project (Rust settings
// `page_projects`, falling back to the most recent pick), and their own [전역 | 프로젝트] scope. Other pages get no
// project at all. The picker lives in each tab's ScopeBar; ⌘P opens the open tab's picker.
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { confirmLeave, offFor, pageProjectId, projectPageOf, projectsApi, setDirty, type KnownProject, type ProjectPage, type ScopeNeed } from "../lib/projects";

export type Scope = "global" | "project";

interface ProjectCtx {
  /** The open tab when it picks a project; null elsewhere. */
  page: ProjectPage | null;
  list: KnownProject[];
  recent: string[];
  current: KnownProject | null;
  scope: Scope;
  setScope: (s: Scope) => void;
  /** Asks first when an editor has unsaved changes. Resolves false when the user kept editing. */
  select: (id: string | null) => Promise<boolean>;
  pickerOpen: boolean;
  setPickerOpen: (open: boolean) => void;
}

const Ctx = createContext<ProjectCtx | null>(null);

export function ProjectProvider({ page: rawPage, children }: { page: string; children: ReactNode }) {
  const qc = useQueryClient();
  const page = projectPageOf(rawPage);
  const q = useQuery({ queryKey: ["projects-known"], queryFn: projectsApi.known, staleTime: 30_000 });
  const [scopes, setScopes] = useState<Partial<Record<ProjectPage, Scope>>>({});
  const [pickerOpen, setPickerOpen] = useState(false);
  const list = useMemo(() => q.data?.projects ?? [], [q.data]);
  const currentId = pageProjectId(q.data, page);
  const current = list.find((p) => p.id === currentId) ?? null;
  const scope = page ? (scopes[page] ?? "project") : "global";
  // A picker left open does not follow to another tab.
  const [pickerPage, setPickerPage] = useState(page);
  if (pickerPage !== page) {
    setPickerPage(page);
    setPickerOpen(false);
  }
  const select = async (id: string | null) => {
    if (!page) return true;
    const same = id === currentId;
    if (!same && !confirmLeave()) return false;
    if (!same || !(page in (q.data?.pages ?? {}))) await projectsApi.setPage(page, id);
    setScopes((m) => ({ ...m, [page]: id ? "project" : "global" }));
    await qc.invalidateQueries({ queryKey: ["projects-known"] });
    return true;
  };
  const setScope = (s: Scope) => {
    if (page && confirmLeave()) setScopes((m) => ({ ...m, [page]: s }));
  };
  // ⌘P / Ctrl+P opens this tab's picker (only on tabs that have one).
  useEffect(() => {
    if (!page) return;
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "p") {
        e.preventDefault();
        setPickerOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [page]);
  const value = { page, list, recent: q.data?.recent ?? [], current, scope: current ? scope : "global", setScope, select, pickerOpen, setPickerOpen } satisfies ProjectCtx;
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

const FALLBACK: ProjectCtx = { page: null, list: [], recent: [], current: null, scope: "global", setScope: () => undefined, select: async () => true, pickerOpen: false, setPickerOpen: () => undefined };

export function useProject(): ProjectCtx {
  return useContext(Ctx) ?? FALLBACK;
}

/**
 * The scope a screen should use. `off` is why the project side cannot be used here (home, unknown folder); the screen
 * then shows the global side and the scope bar shows the reason.
 */
export function useScope(need: ScopeNeed): { scope: Scope; project: KnownProject | null; projectId: string | null; off: string | null } {
  const { scope, current } = useProject();
  const off = offFor(current, need);
  const usable = current && !off ? current : null;
  return { scope: scope === "project" && usable ? "project" : "global", project: current, projectId: usable?.id ?? null, off };
}

/** Registers an editor's unsaved state with the app-wide guard while it is mounted. */
export function useDirty(key: string, dirty: boolean) {
  useEffect(() => {
    setDirty(key, dirty);
  }, [key, dirty]);
  useEffect(() => () => setDirty(key, false), [key]);
}
