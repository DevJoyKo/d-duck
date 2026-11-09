// report-evidence on the mock backend: review cards carry "근거 N" / "근거 없음 N", and the Composer asks for a report
// for review/QA tasks ("근거를 남기려면 보고서가 필요해요").
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

let reportSetting = true;
vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return {
    invoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
      const v = await real.invoke(cmd, args);
      return cmd === "get_status" ? { ...(v as Record<string, unknown>), taskReport: reportSetting } : v;
    }),
  };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

async function boot(page: string, search = "") {
  localStorage.setItem("dduck.page", page);
  localStorage.setItem("dduck.tour.skip", "1");
  window.history.replaceState(null, "", `/${search}`);
  const { default: App } = await import("../App");
  render(<App />);
}

beforeEach(() => {
  vi.resetModules();
  reportSetting = true;
});
afterEach(() => {
  cleanup();
  localStorage.clear();
  window.history.replaceState(null, "", "/");
});

describe("근거 on cards and in the Composer", () => {
  it("a card shows 근거 N and 근거 없음 N, red for QA and amber for other types", async () => {
    await boot("review", "?evidence=1");
    const dev = await waitFor(() => {
      const c = document.querySelector('[data-review-card="e1e1e1e1"]') as HTMLElement | null;
      expect(c).toBeTruthy();
      return c!;
    });
    expect(dev.querySelector("[data-evidence-count]")?.textContent).toBe("근거 4");
    expect(dev.querySelector("[data-evidence-missing]")?.textContent).toBe("근거 없음 2");
    expect(dev.querySelector("[data-evidence-missing]")?.getAttribute("data-evidence-missing")).toBe("plain");
    // A criterion with failing tests is not counted as 충족 but named in danger tone.
    expect(dev.querySelector("[data-tests-failed]")?.textContent).toContain("테스트 실패 1");
    const qa = document.querySelector('[data-review-card="e3e3e3e3"]') as HTMLElement;
    expect(qa.querySelector("[data-evidence-count]")?.textContent).toBe("근거 1");
    expect(qa.querySelector("[data-evidence-missing]")?.getAttribute("data-evidence-missing")).toBe("strict");
    expect(within(qa).getByText(/테스트 리포트 없음/)).toBeTruthy();
    // A legacy (v1) card has no evidence badges.
    const lint = document.querySelector('[data-review-card="9988aabb"]') as HTMLElement;
    expect(lint.querySelector("[data-evidence-count]")).toBeNull();
  });

  it("QA with the report setting off: the report line asks for a report and stays checked", async () => {
    reportSetting = false;
    await boot("tasks");
    fireEvent.click((await screen.findAllByRole("button", { name: "새 작업" }))[0]);
    const types = await screen.findByRole("radiogroup", { name: "작업 유형" });
    const box = () => screen.getByLabelText(/이번 작업 보고서 남기기/) as HTMLInputElement;
    // A dev task follows the setting (off).
    fireEvent.click(await within(types).findByRole("radio", { name: "개발" }));
    expect(box().checked).toBe(false);
    expect(document.querySelector("[data-report-note]")?.textContent).not.toContain("근거를 남기려면");
    fireEvent.click(within(types).getByRole("radio", { name: "QA" }));
    expect(box().checked).toBe(true);
    expect(document.querySelector("[data-report-note]")?.textContent).toContain("근거를 남기려면 보고서가 필요해요");
    // Unchecking keeps the note (the task would start without evidence).
    fireEvent.click(box());
    expect(box().checked).toBe(false);
    expect(document.querySelector("[data-report-note]")?.textContent).toContain("근거를 남기려면 보고서가 필요해요");
  });
});
