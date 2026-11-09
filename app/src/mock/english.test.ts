// Mock English (english.ts): `?lang=en` answers with English example data and English backend text, so README and
// screen captures have no Korean outside the language name. Without it the mock is the Korean data it always was.
import { afterEach, describe, expect, it, vi } from "vitest";
import { delocalize, localize } from "./english";

const HANGUL = /[가-힣]/;
type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

function leaves(v: unknown, out: string[] = []): string[] {
  if (typeof v === "string") out.push(v);
  else if (Array.isArray(v)) v.forEach((x) => leaves(x, out));
  else if (v && typeof v === "object") Object.values(v).forEach((x) => leaves(x, out));
  return out;
}

async function boot(search: string): Promise<Invoke> {
  window.history.replaceState(null, "", `/${search}`);
  vi.resetModules();
  return (await import("./core")).invoke as Invoke;
}

afterEach(() => window.history.replaceState(null, "", "/"));

describe("localize", () => {
  it("changes nothing when both sides are Korean", () => {
    const v = { a: "문서 최신화", b: ["취소했어요"] };
    expect(localize(v, "ko", "ko")).toBe(v);
  });

  it("swaps backend sentences only as whole strings, and data also inside a longer text", () => {
    expect(localize(["취소했어요", "취소했어요 · 문서 최신화"], "en", "ko")).toEqual(["Cancelled", "취소했어요 · 문서 최신화"]);
    expect(localize("## 목표\n결제 화면 상태 관리를 정리한다", "ko", "en")).toBe("## Goal\nClean up the checkout screen's state management");
    expect(localize("---\n\n너는 qa-verifier 역할이다.\n", "ko", "en")).toBe("---\n\nYou are the qa-verifier agent.\n");
  });

  it("turns English arguments back into the Korean fixture they came from", () => {
    expect(delocalize({ name: "Docs refresh", other: "typed by hand" }, "en")).toEqual({ name: "문서 최신화", other: "typed by hand" });
    expect(delocalize({ name: "Docs refresh" }, "ko")).toEqual({ name: "Docs refresh" });
  });
});

describe("?lang=en", () => {
  it("answers the main screens' commands without Korean", async () => {
    const invoke = await boot("?lang=en");
    const calls: [string, Record<string, unknown>][] = [
      ["list_agents", {}],
      ["task_records", {}],
      ["review_inbox", { filter: "all", limit: 40 }],
      ["agent_catalog", { cwd: "/Users/me/Agents/dev_team" }],
      ["agent_activity", {}],
      ["team_config", {}],
      ["permission_scopes", { project: null }],
      ["explain_permission", { call: "Bash(git push origin main)" }],
      ["skill_catalog", {}],
      ["plugin_list", {}],
      ["authoring_info", {}],
      ["list_artifacts", {}],
      ["list_routines", {}],
      ["cloud_routines", {}],
      ["usage_insights_local", {}],
      ["usage_insights_last", {}],
      ["usage_sessions_page", { offset: 0 }],
      ["session_timeline", {}],
      ["session_raw", {}],
      ["instruction_files", {}],
      ["rules_list", { project: null }],
      ["limit_status", {}],
    ];
    const korean: string[] = [];
    for (const [cmd, args] of calls) for (const s of leaves(await invoke(cmd, args))) if (HANGUL.test(s)) korean.push(`${cmd}: ${s.slice(0, 60)}`);
    const records = (await invoke("task_records")) as { bgId: string }[];
    for (const r of records) for (const s of leaves(await invoke("task_report", { id: r.bgId }))) if (HANGUL.test(s)) korean.push(`task_report ${r.bgId}: ${s.slice(0, 60)}`);
    expect(korean).toEqual([]);
    // The language answer itself is unchanged.
    expect(await invoke("get_language")).toEqual({ choice: "en", resolved: "en" });
  });

  it("keeps the Korean data without it, and switches only backend text when the language changes", async () => {
    const invoke = await boot("");
    const routines = async () => (await invoke("list_routines")) as { name: string; scheduleText: string }[];
    expect((await routines())[0]).toMatchObject({ name: "아침 점검", scheduleText: "매일 09:00" });
    await invoke("set_language", { choice: "en" });
    expect((await routines())[0]).toMatchObject({ name: "아침 점검", scheduleText: "Daily 09:00" });
    await invoke("set_language", { choice: "ko" });
  });
});
