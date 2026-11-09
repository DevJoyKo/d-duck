import { QueryClient, QueryClientProvider, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { api, type AgentsPayload, type IngestStatus } from "./lib/api";
import { blockedCount, type Draft } from "./lib/brief";
import { AgentsPage } from "./components/AgentsPage";
import { ArtifactsPage } from "./components/ArtifactsPage";
import { RoutinesPage } from "./components/RoutinesPage";
import { ExtensionsPage } from "./components/ExtensionsPage";
import { McpPage } from "./components/McpPage";
import { PermissionsPage } from "./components/PermissionsPage";
import { MemoryPage } from "./components/MemoryPage";
import { SettingsPage } from "./components/SettingsPage";
import { Sidebar } from "./components/Sidebar";
import { normalizePage, type ExtChip, type NavTarget, type Page } from "./lib/nav";
import { TasksPage } from "./components/TasksPage";
import { ReviewPage } from "./components/ReviewPage";
import { UsagePage } from "./components/UsagePage";
import { TourDeck, TourToast } from "./components/TourDeck";
import { StartChecklist } from "./components/StartChecklist";
import { closeTour, dismissToast, loadOnboarding, openTour, useOnboarding } from "./lib/onboarding";
import { checklistVisible, tourDraft } from "./lib/tour";
import { ProjectProvider } from "./components/ProjectContext";
import { anyDirty, confirmLeave } from "./lib/projects";
import { loadAuth } from "./lib/auth";
import { LoginBanner, LoginSheet } from "./components/LoginBits";
import { LanguageProvider } from "./i18n/react";
import { t } from "./i18n";

const PAGE_KEY = "dduck.page";

/** Remembered page; old ids (`skills`, `plugins`) open 확장 with that chip (nav-ia normalizePage). */
function loadPage(): { page: Page; chip: ExtChip | null } {
  try {
    // `?page=` overrides the remembered page (used for design review screenshots in mock mode).
    const p = new URLSearchParams(window.location.search).get("page") ?? localStorage.getItem(PAGE_KEY);
    return normalizePage(p) ?? { page: "agents", chip: null };
  } catch {
    return { page: "agents", chip: null };
  }
}

function savePage(p: Page) {
  try {
    localStorage.setItem(PAGE_KEY, p);
  } catch {
    // Storage unavailable: the page simply is not remembered.
  }
}

const client = new QueryClient({ defaultOptions: { queries: { refetchOnWindowFocus: false, staleTime: 5_000 } } });

function Shell() {
  const [initial] = useState(loadPage);
  const [page, setPage] = useState<Page>(initial.page);
  const [extChip, setExtChip] = useState<ExtChip | null>(initial.chip);
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  // Composer state lives here so the agents page and task cards can open it prefilled. undefined = closed.
  const [draft, setDraft] = useState<Draft | null | undefined>(undefined);
  const [ingest, setIngest] = useState<IngestStatus | null>(null);
  const qc = useQueryClient();
  const status = useQuery({ queryKey: ["status"], queryFn: api.status });
  const agents = useQuery({ queryKey: ["agents"], queryFn: api.agents, refetchInterval: 15_000 });
  // Badge = the "확인 필요" column: waiting for input, or failed without a report to review (task-lifecycle).
  const waiting = blockedCount(agents.data);

  const onboarding = useOnboarding();
  useEffect(() => {
    loadOnboarding();
    void loadAuth();
  }, []);

  // Leaving a page with an unsaved editor asks first (one registry with project switching).
  const navigate = (target: NavTarget) => {
    const n = normalizePage(target);
    if (!n) return;
    if (n.page !== page && !confirmLeave()) return;
    setPage(n.page);
    setExtChip(n.chip);
    savePage(n.page);
  };
  useEffect(() => {
    // Reloading the webview would drop edits. Closing the window only hides it (Rust), so edits survive that.
    const onUnload = (e: BeforeUnloadEvent) => {
      if (anyDirty()) e.preventDefault();
    };
    window.addEventListener("beforeunload", onUnload);
    return () => window.removeEventListener("beforeunload", onUnload);
  }, []);
  // "작업으로 만들기" from usage insights: open the task composer prefilled. Nothing runs or is sent until the user
  // asks; an open composer is replaced only after a confirmation (same as other prefilled drafts).
  const openDraft = (d: Draft) => {
    if (draft !== undefined && !window.confirm(t("common.draft.replace"))) return;
    setDraft(d);
    navigate("tasks");
  };

  useEffect(() => {
    const subs = [
      listen("usage-updated", () => qc.invalidateQueries({ queryKey: ["usage"] })),
      listen("limits-updated", () => {
        void qc.invalidateQueries({ queryKey: ["limits"] });
        // The resume-pending refusal on an open 정리 checklist follows the limit rows (D-191).
        void qc.invalidateQueries({ queryKey: ["wrapup-check"] });
      }),
      listen<AgentsPayload>("agents-updated", (e) => qc.setQueryData(["agents"], e.payload)),
      listen<IngestStatus>("ingest-status", (e) => setIngest(e.payload)),
      listen<string>("select-session", (e) => {
        if (!confirmLeave()) return;
        setPage("tasks");
        savePage("tasks");
        setSelectedSession(e.payload);
      }),
    ];
    // "작업 보드에서 보기" from the add sheet (skill-authoring): open the task board on that session.
    const openTask = (e: Event) => {
      if (!confirmLeave()) return;
      setPage("tasks");
      savePage("tasks");
      setSelectedSession(String((e as CustomEvent).detail));
    };
    window.addEventListener("dduck-open-task", openTask);
    return () => {
      subs.forEach((p) => p.then((un) => un()));
      window.removeEventListener("dduck-open-task", openTask);
    };
  }, [qc]);

  return (
    <ProjectProvider page={page}>
      <div className="flex h-full" inert={onboarding.deck !== null}>
        <Sidebar
          page={page}
          onNavigate={navigate}
          waiting={waiting}
          reviewPending={agents.data?.reviewPending}
          status={status.data}
          ingest={ingest}
          checklist={!onboarding.deck && onboarding.state && checklistVisible(onboarding.state) ? <StartChecklist state={onboarding.state} onNavigate={navigate} /> : null}
        />
        <main className="min-w-0 flex-1 overflow-auto">
          <LoginBanner cliMissing={status.data !== undefined && !status.data.cli} />
          {page === "usage" && <UsagePage onNavigate={navigate} onDraft={openDraft} />}
          {page === "tasks" && <TasksPage status={status.data} agents={agents.data} selected={selectedSession} onSelect={setSelectedSession} draft={draft} onDraft={setDraft} onOpenReview={() => navigate("review")} />}
          {page === "agents" && (
            <AgentsPage
              agents={agents.data}
              capacity={status.data?.reviewCapacity ?? 3}
              onOpenBlocked={() => {
                window.location.hash = "review";
                navigate("tasks");
              }}
              onOpenTask={(key) => {
                navigate("tasks");
                setSelectedSession(key);
              }}
            />
          )}
          {page === "review" && <ReviewPage agents={agents.data} onNavigate={navigate} />}
          {page === "artifacts" && <ArtifactsPage />}
          {page === "routines" && (
            <RoutinesPage
              status={status.data}
              onOpenTask={(bgId) => {
                const s = agents.data?.sessions.find((x) => x.id === bgId);
                navigate("tasks");
                if (s) setSelectedSession(s.key);
              }}
            />
          )}
          {page === "extensions" && <ExtensionsPage key={extChip ?? "all"} initialChip={extChip} />}
          {page === "mcp" && <McpPage />}
          {page === "permissions" && <PermissionsPage />}
          {page === "memory" && <MemoryPage />}
          {page === "settings" && <SettingsPage onOpenAgentSettings={() => navigate("agents")} onOpenMcp={() => navigate("mcp")} />}
        </main>
      </div>
      {onboarding.deck && (
        <TourDeck
          cards={onboarding.deck}
          status={status.data}
          onClose={closeTour}
          onNavigate={navigate}
          onStart={() => {
            // Opens the composer only: empty goal, example chips, refine highlighted. Nothing is sent or started.
            closeTour("done");
            openDraft(tourDraft(status.data?.defaultCwd ?? ""));
          }}
        />
      )}
      {onboarding.toast && !onboarding.deck && <TourToast onOpen={openTour} onClose={dismissToast} />}
      <LoginSheet />
    </ProjectProvider>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={client}>
      <LanguageProvider>
        <Shell />
      </LanguageProvider>
    </QueryClientProvider>
  );
}
