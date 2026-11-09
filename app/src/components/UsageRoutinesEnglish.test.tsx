// tasks.md 3.5/3.6: usage insights, artifacts and routines render without Korean in English (user data aside).
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n/language";
import type { ArtifactRow, Finding, LocalReport } from "../lib/api";

const invoke = vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

const auth = await import("../lib/auth");
const { ArtifactsPage } = await import("./ArtifactsPage");
const { RoutinesPage, lastRunText } = await import("./RoutinesPage");
const { UsageInsights } = await import("./UsageInsights");
const { DistributionList } = await import("./DistributionList");
const { relativeNext, runStatusText, scheduleText, validateRoutineForm } = await import("../lib/routines");
const { cloudScheduleText, validateCloudDraft } = await import("../lib/cloudRoutines");
const { reasonText } = await import("../lib/artifactCards");

const HANGUL = /[가-힣]/;
/** Text left after removing the given user-data strings. */
const without = (text: string, data: string[]) => data.reduce((s, d) => s.split(d).join(""), text);

const artifacts: ArtifactRow[] = Array.from({ length: 12 }, (_, i) => ({
  artifactId: `id-${i + 1}`,
  url: `https://claude.ai/artifact/id-${i + 1}`,
  title: `Dashboard ${i + 1}`,
  kind: i === 1 ? "typed" : "page",
  seq: 2,
  firstSeenTs: 1,
  lastTs: Date.UTC(2026, 10, 9, 11, 19),
  projectName: i === 0 ? null : "dev_team",
}));

const finding: Finding = { id: "f1", level: "high", title: "Cache misses", evidence: "40%", effect: "less cost", action: "Pin the model", detail: "more", link: "settings", localNames: [] };
const report: LocalReport = { enough: true, responses: 120, findings: [finding], notChecked: [], priceDate: "2026-10-01", cliVersion: "2.1.0" };

const routine = {
  id: 1,
  name: "아침 점검",
  cwd: "/Users/me/w",
  promptLen: 42,
  permissionMode: "manual",
  schedule: { kind: "weekly", weekdays: 0b10001, hour: 14, minute: 30 },
  scheduleText: "매주 월·금 14:30",
  enabled: true,
  nextRunAt: Date.now() + 5 * 3_600_000,
  lastStatus: "start_failed",
  active: false,
};

const wrap = (ui: React.ReactNode) => render(<QueryClientProvider client={new QueryClient()}>{ui}</QueryClientProvider>);

beforeEach(() => {
  applyLanguage("en");
  auth.resetAuthForTests();
  auth.setAuthForTests({ view: { login: "loggedIn", method: "claudeAi", connection: null, plan: null, claudeAi: true, expiresSoon: false } });
  invoke.mockReset();
  invoke.mockImplementation(async (cmd) => {
    switch (cmd) {
      case "list_artifacts":
        return artifacts;
      case "list_routines":
        return [routine];
      case "routine_runs":
        return [{ id: 9, scheduledFor: Date.now() - 3_600_000, startedAt: Date.now() - 3_600_000, endedAt: null, status: "skipped", error: "busy", bgId: null }];
      case "folder_trusted":
        return false;
      case "usage_insights_local":
        return report;
      case "usage_insights_preview":
        return "models: 3";
      case "usage_insights_last":
        return { text: "## Cache\n- Use a smaller model", atMs: Date.UTC(2026, 10, 1), costUsd: 0.12, stale: true };
      default:
        throw "missing";
    }
  });
});
afterEach(() => {
  cleanup();
  applyLanguage("ko");
});

describe("English text, usage/artifacts/routines", () => {
  it("lib sentences", () => {
    expect(scheduleText({ kind: "weekly", weekdays: 0b10001, hour: 14, minute: 30 })).toBe("Weekly on Mon, Fri at 14:30");
    expect(scheduleText({ kind: "interval", hours: 1 })).toBe("Every hour");
    expect(runStatusText("start_failed", "untrustedWorkspace")).toBe("Couldn't start (untrusted folder)");
    expect(relativeNext(5 * 3_600_000, 0)).toBe("in 5 hours");
    expect(validateRoutineForm({ name: "", cwd: "", prompt: "", schedule: { kind: "daily", hour: 9, minute: 0 } })).toBe("Enter a name");
    expect(cloudScheduleText({ cron: null, schedule: { kind: "once", at: new Date(2026, 10, 12, 9, 5).getTime() } })).toBe("Once · 11/12/2026 09:05");
    expect(validateCloudDraft({ name: "", enabled: false, schedule: null }, 0)).toBe("Names must be 1–80 characters");
    expect(lastRunText({ status: "failed", failureReason: "ENVIRONMENT_NOT_FOUND", firedAt: null, finishedAt: null, sessionId: null }).text).toBe("Failed (environment not found)");
    expect(reasonText("tooLarge")).toBe("The source is larger than 2 MB");
  });

  it("distribution list", () => {
    const { container } = render(
      <DistributionList showVia showCost items={[{ key: "기타", count: 3, share: 10, viaTool: 1, viaSlash: 2, priced: false, estCostUsd: null } as never]} />,
    );
    expect(container.textContent).toContain("Other");
    expect(container.textContent).toContain("Tool 1 · slash 2");
    expect(container.textContent).not.toMatch(HANGUL);
  });

  it("artifacts page", async () => {
    const { container } = wrap(<ArtifactsPage />);
    await screen.findByText("1 / 2");
    expect(screen.getByRole("button", { name: "Next page" })).toBeTruthy();
    expect(screen.getAllByText("Unknown project", { exact: false }).length).toBe(1);
    expect(container.textContent).not.toMatch(HANGUL);
  });

  it("usage insights, opened, with stored advice", async () => {
    const { container } = wrap(<UsageInsights q={{ from: null, to: null, projects: [], models: [] } as never} onNavigate={() => undefined} onDraft={() => undefined} />);
    fireEvent.click(await screen.findByRole("button", { name: /Usage insights/ }));
    await screen.findByText("Cache misses");
    expect(screen.getByText("Important")).toBeTruthy();
    expect(screen.getByText(/Fix in Settings tab/)).toBeTruthy();
    expect(screen.getByText("Stale · over 30 days")).toBeTruthy();
    expect(container.textContent).not.toMatch(HANGUL);
  });

  it("routines page and the new routine sheet", async () => {
    const { container } = wrap(<RoutinesPage onOpenTask={() => undefined} />);
    await screen.findByText("Skipped (previous run still going)");
    expect(screen.getAllByText(/Weekly on Mon, Fri at 14:30/).length).toBeGreaterThan(0);
    expect(without(container.textContent ?? "", [routine.name])).not.toMatch(HANGUL);
    fireEvent.click(screen.getByRole("button", { name: /New routine/ }));
    await screen.findByText("Plan only", { selector: "option" });
    expect(screen.getByPlaceholderText("e.g. Morning check")).toBeTruthy();
    expect(without(container.textContent ?? "", [routine.name])).not.toMatch(HANGUL);
  });
});
