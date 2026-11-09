// app-language (tasks.md 3.2) and report-copy-save review: the 리뷰 tab in English, and a double or triple click on a
// card (word or line selection) never opens the report.
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n/language";

vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return { invoke: vi.fn(real.invoke) };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

async function boot(search = "") {
  localStorage.setItem("dduck.page", "review");
  window.history.replaceState(null, "", `/${search}`);
  const { default: App } = await import("../App");
  render(<App />);
}

const card = () => document.querySelector('[data-review-card="9988aabb"]') as HTMLElement;
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

beforeEach(() => vi.resetModules());
afterEach(async () => {
  cleanup();
  await (await import("@tauri-apps/api/core")).invoke("set_language", { choice: "ko" });
  localStorage.clear();
  window.history.replaceState(null, "", "/");
  applyLanguage("ko");
});

describe("리뷰 tab", () => {
  it("a double or triple click selects words and does not open the report; a single click still does", async () => {
    await boot();
    await waitFor(() => expect(card()).toBeTruthy());
    const title = card().querySelector("[data-card-title]") as HTMLElement;
    fireEvent.click(title, { detail: 1 });
    fireEvent.click(title, { detail: 2 });
    fireEvent.doubleClick(title, { detail: 2 });
    fireEvent.click(title, { detail: 3 });
    await wait(400);
    expect(screen.queryByRole("dialog")).toBeNull();
    // A selection made during the wait cancels the pending open.
    const sel = vi.spyOn(window, "getSelection").mockReturnValueOnce({ toString: () => "" } as Selection).mockReturnValue({ toString: () => "린트" } as Selection);
    fireEvent.click(title, { detail: 1 });
    await wait(400);
    expect(screen.queryByRole("dialog")).toBeNull();
    sel.mockRestore();
    fireEvent.click(title, { detail: 1 });
    expect(await screen.findByRole("dialog")).toBeTruthy();
  });

  it("card chips are not selectable", async () => {
    await boot();
    await waitFor(() => expect(card()?.querySelector("[data-caution]")).toBeTruthy());
    expect(card().querySelector("[data-caution]")?.hasAttribute("data-no-select")).toBe(true);
  });

  it("in English, the page's own text has no Korean (user and agent text stays as written)", async () => {
    // The mock backend's stored choice is applied on start (its module outlives resetModules, so set it directly).
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("set_language", { choice: "en" });
    await boot();
    await waitFor(() => expect(card()).toBeTruthy());
    await waitFor(() => expect(document.querySelector("h1")?.textContent).toBe("Review"));
    expect(document.querySelector('[aria-checked="true"]')?.textContent).toContain("All projects");
    expect(card().textContent).toContain("Differs from brief 1 · Partly met 1 · Hard to undo 1 → Check");
    expect(card().textContent).toContain("Met 3 · Partly met 1");
    // Drop what the user or agent wrote, the role chips (agents) and delivery alerts (tasks: delivery.ts).
    const groups = Array.from(document.querySelectorAll("[data-review-group]")).map((g) => g.cloneNode(true) as HTMLElement);
    expect(groups.length).toBeGreaterThan(0);
    for (const g of groups) {
      g.querySelectorAll("[data-card-title], [data-preview] .text-ink, [data-preview] .text-ink-3, [data-attachment-line], [data-caution] .min-w-0, [data-delivery-alert], header").forEach((n) => n.remove());
      expect(g.textContent).not.toMatch(/[가-힣]/);
    }
  });
});
