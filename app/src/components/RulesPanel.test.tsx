import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import rawTemplates from "../../crates/dduck-core/data/authoring_templates.json";
import { pickLang } from "../lib/bundled";
import { byFolder, confirmDiscard, filterRules, freeName, globWarnings, patternProblem, projectFolder, ruleFileName, scopeChips, scopeTitle, summaryText, validRuleName, type RuleGroup, type RuleRow } from "../lib/rules";

// Bundled text fields are {ko, en} pairs (app-language); this file uses the Korean side.
const templates = pickLang(rawTemplates);

const row = (rel: string, extra: Partial<RuleRow> = {}): RuleRow => ({
  id: `r-${rel}`,
  scope: "project",
  rel,
  name: rel.split("/").pop()!.replace(/\.md$/, ""),
  folder: rel.includes("/") ? rel.split("/")[0] : null,
  paths: [],
  fm: "none",
  enabled: true,
  offBy: null,
  gitTracked: null,
  link: false,
  dirLink: false,
  linkOutside: false,
  linkTarget: null,
  size: 10,
  token: `t-${rel}`,
  readonlyReason: null,
  hasSecrets: false,
  warnings: [],
  ...extra,
});

const group = (scope: "global" | "project", rules: RuleRow[]): RuleGroup => ({
  scope,
  dir: `/x/${scope}`,
  exists: true,
  rules,
  truncated: false,
  settingsPath: "/x/settings.json",
  settingsToken: "s",
  settingsReadonly: null,
  alwaysCount: 0,
  alwaysBytes: 0,
  readonlyReason: null,
});

const bodies = new Map<string, string>();
let listData = { global: group("global", [row("korean.md", { scope: "global" })]), project: group("project", [] as RuleRow[]), templates: templates.rules };

vi.mock("../lib/rules", async (orig) => {
  const real = await orig<typeof import("../lib/rules")>();
  return {
    ...real,
    rulesApi: {
      list: vi.fn(async () => listData),
      read: vi.fn(async (_p: string | null, id: string) => {
        const r = [...listData.global.rules, ...(listData.project?.rules ?? [])].find((x) => x.id === id)!;
        const body = bodies.get(id) ?? (r.link ? "# 공유 본문\n" : "# 본문\n");
        return { ...r, token: bodies.has(id) ? `${r.token}-2` : r.token, body, pathsEditable: !r.readonlyReason && (r.fm === "none" || r.fm === "ok"), secrets: [], instructionLike: [] };
      }),
      save: vi.fn(async (_p: string | null, s: { id?: string; body: string }) => {
        if (s.id) bodies.set(s.id, s.body);
        return s.id ?? "r-new";
      }),
      setEnabled: vi.fn(async () => undefined),
      trash: vi.fn(async () => ({ done: false, unconfirmed: false })),
    },
  };
});

const { RulesSection } = await import("./RulesPanel");
const { rulesApi } = await import("../lib/rules");

function mount(project: string | null = "p") {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <RulesSection project={project} hasPath={!!project} />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.restoreAllMocks();
  bodies.clear();
});

describe("rule helpers", () => {
  it("validates new file names like Rust", () => {
    expect(["api.md", "frontend/react-hooks.md"].every(validRuleName)).toBe(true);
    expect(["../x.md", "a/b/c.md", "가.md", "a b.md", ".x.md"].some(validRuleName)).toBe(false);
    expect(ruleFileName(" api ")).toBe("api.md");
    expect(ruleFileName("a.md")).toBe("a.md");
  });

  it("warns on bad brackets, outside paths and big brace expansions, not on a leading /", () => {
    expect(globWarnings(["/*.md", "src/**/*.{ts,tsx}", "photos \\[2024/**"])).toEqual([]);
    expect(globWarnings(["photos [2024/**"])).toHaveLength(1);
    expect(globWarnings(["~/x", "../y", "C:/z"])).toHaveLength(3);
    const big = "{0,1,2,3,4,5,6,7,8,9}{0,1,2,3,4,5,6,7,8,9}{0,1,2,3,4,5,6,7,8,9}{a,b}";
    expect(globWarnings([big]).some((w) => w.includes("1,000"))).toBe(true);
  });

  it("refuses unsafe patterns before sending", () => {
    expect(patternProblem("src/**")).toBeNull();
    for (const bad of ["a\nb", "a\u2028b", "a\u202eb", " x", ""]) expect(patternProblem(bad)).not.toBeNull();
  });

  it("labels scope, groups by folder and filters", () => {
    expect(scopeChips(row("a.md"))).toEqual(["항상"]);
    expect(scopeChips(row("a.md", { fm: "broken", paths: [] }))).toEqual(["항상"]);
    expect(scopeChips(row("a.md", { paths: ["src/**"] }))).toEqual(["src/**"]);
    expect(scopeChips(row("a.md", { link: true, paths: ["x/**"] }))).toEqual(["x/**"]);
    expect(scopeChips(row("a.md", { fm: "complex", paths: [] }))).toEqual([]);
    expect(scopeChips(row("a", { dirLink: true }))).toEqual([]);
    const rules = [row("b/x.md"), row("top.md"), row("a/y.md", { paths: ["src/api/**"] })];
    expect(byFolder(rules).map(([f]) => f)).toEqual(["", "a", "b"]);
    expect(filterRules(rules, "API").map((r) => r.rel)).toEqual(["a/y.md"]);
    expect(summaryText({ alwaysCount: 2, alwaysBytes: 61 })).toBe("항상 적용 2개 · 합계 61B · 매 대화마다 읽어요");
    expect(projectFolder("/Users/me/Agents/dev_team/.claude/rules")).toBe("dev_team");
  });

  it("explains scope chips in plain words", () => {
    expect(scopeTitle("**/*.{ts,tsx}")).toBe("TS·TSX 파일을 다룰 때만");
    expect(scopeTitle("src/api/**/*.ts")).toBe("src/api 폴더의 TS 파일을 다룰 때만");
    expect(scopeTitle("docs/**")).toBe("docs 폴더 파일을 다룰 때만");
    expect(scopeTitle("README.md")).toBe("README.md 파일을 다룰 때만");
    expect(scopeTitle("항상")).toBe("매 대화마다 읽어요");
    expect(scopeTitle("a?b[c]")).toBe("이 패턴에 맞는 파일을 다룰 때만");
  });

  it("suggests a free name next to existing rules", () => {
    expect(freeName("api", ["api.md"])).toBe("api-2");
    expect(freeName("api", ["api.md", "api-2.md"])).toBe("api-3");
    expect(freeName("api", ["testing.md"])).toBe("api");
  });

  it("asks before dropping unsaved changes", () => {
    const ask = vi.fn(() => false);
    expect(confirmDiscard(false, ask)).toBe(true);
    expect(ask).not.toHaveBeenCalled();
    expect(confirmDiscard(true, ask)).toBe(false);
    expect(ask).toHaveBeenCalledWith("저장하지 않은 변경이 있어요. 버릴까요?");
  });

  it("ships three short rule templates", () => {
    expect(templates.rules).toHaveLength(3);
    for (const t of templates.rules) {
      expect(validRuleName(`${t.name}.md`)).toBe(true);
      expect(t.body.trimEnd().split("\n").length).toBeLessThanOrEqual(5);
    }
    expect(templates.rules.find((t) => t.label === "API 규칙")?.paths).toEqual(["src/api/**/*.ts"]);
  });
});

describe("RulesSection", () => {
  it("shows global even without a project, and the empty project hint", async () => {
    listData = { ...listData, project: group("project", []) };
    mount();
    expect(await screen.findByText("korean")).toBeTruthy();
    expect(screen.getByText("모든 프로젝트에 적용 · 패턴은 Claude를 시작한 폴더 기준이에요")).toBeTruthy();
    expect(screen.getByText(/아직 규칙이 없어요/)).toBeTruthy();
    cleanup();
    listData = { ...listData, project: null as unknown as RuleGroup };
    mount(null);
    expect(await screen.findByText("korean")).toBeTruthy();
    expect(screen.getByText("프로젝트 폴더를 찾지 못해 전역만 보여요")).toBeTruthy();
  });

  it("creates a path-scoped rule from the API template", async () => {
    listData = { ...listData, project: group("project", []) };
    mount();
    fireEvent.click((await screen.findAllByRole("button", { name: /새 규칙/ }))[0]);
    fireEvent.change(screen.getByLabelText("시작"), { target: { value: "api" } });
    expect(screen.getByText("src/api/**/*.ts")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "만들기" }));
    await waitFor(() => expect(rulesApi.save).toHaveBeenCalled());
    expect(vi.mocked(rulesApi.save).mock.calls[0][1]).toMatchObject({ scope: "project", name: "api.md", paths: ["src/api/**/*.ts"] });
  });

  it("adds a scope chip with Enter and saves body + paths", async () => {
    listData = { ...listData, project: group("project", [row("testing.md", { gitTracked: true })]) };
    mount();
    fireEvent.click(await screen.findByText("testing"));
    expect(screen.getByText("저장소에 커밋되는 파일 1개")).toBeTruthy();
    expect(screen.getAllByLabelText("저장소에 커밋되는 파일").length).toBeGreaterThan(0);
    const input = await screen.findByLabelText("적용 범위 패턴 추가");
    fireEvent.change(input, { target: { value: "**/*.test.ts" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() => expect(rulesApi.save).toHaveBeenCalled());
    expect(vi.mocked(rulesApi.save).mock.calls[0][1]).toEqual({ id: "r-testing.md", token: "t-testing.md", body: "# 본문\n", paths: ["**/*.test.ts"] });
  });

  it("an unconfirmed Recycle Bin move refreshes like a move and says so (windows-support 9.1)", async () => {
    listData = { ...listData, project: group("project", [row("testing.md")]) };
    vi.mocked(rulesApi.trash).mockResolvedValueOnce({ done: true, unconfirmed: true });
    mount();
    fireEvent.click(await screen.findByText("testing"));
    const lists = vi.mocked(rulesApi.list).mock.calls.length;
    fireEvent.click(await screen.findByRole("button", { name: "휴지통으로" }));
    expect(await screen.findByText("지웠지만 휴지통에서 찾지 못했어요 · 영구 삭제됐을 수 있어요")).toBeTruthy();
    await waitFor(() => expect(vi.mocked(rulesApi.list).mock.calls.length).toBeGreaterThan(lists));
    expect(screen.queryByRole("button", { name: "휴지통으로" })).toBeNull();
  });

  it("a cancelled rule trash changes nothing", async () => {
    listData = { ...listData, project: group("project", [row("testing.md")]) };
    mount();
    fireEvent.click(await screen.findByText("testing"));
    fireEvent.click(await screen.findByRole("button", { name: "휴지통으로" }));
    await waitFor(() => expect(rulesApi.trash).toHaveBeenCalled());
    expect(screen.getByRole("button", { name: "휴지통으로" })).toBeTruthy();
    expect(screen.queryByText("지웠지만 휴지통에서 찾지 못했어요 · 영구 삭제됐을 수 있어요")).toBeNull();
  });

  it("shows a conflict and offers reload", async () => {
    listData = { ...listData, project: group("project", [row("testing.md")]) };
    vi.mocked(rulesApi.save).mockRejectedValueOnce("파일이 바뀌었어요");
    mount();
    fireEvent.click(await screen.findByText("testing"));
    fireEvent.change(await screen.findByRole("textbox", { name: "본문" }), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    expect(await screen.findByText("파일이 바뀌었어요")).toBeTruthy();
    // Reloading would drop the edit: asks first.
    const ask = vi.spyOn(window, "confirm").mockReturnValue(false);
    fireEvent.click(screen.getByRole("button", { name: "다시 불러오기" }));
    expect(ask).toHaveBeenCalledWith("저장하지 않은 변경이 있어요. 버릴까요?");
    expect((screen.getByRole("textbox", { name: "본문" }) as HTMLTextAreaElement).value).toBe("x");
  });

  it("shows 저장했어요 after a save and moves focus to the editor", async () => {
    listData = { ...listData, project: group("project", [row("testing.md")]) };
    mount();
    fireEvent.click(await screen.findByText("testing"));
    const box = await screen.findByRole("textbox", { name: "본문" });
    await waitFor(() => expect(document.activeElement?.getAttribute("aria-label")).toBe("규칙 편집"));
    fireEvent.change(box, { target: { value: "새 본문\n" } });
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    expect(await screen.findByText("저장했어요")).toBeTruthy();
    expect((screen.getByRole("textbox", { name: "본문" }) as HTMLTextAreaElement).value).toBe("새 본문\n");
  });

  it("asks before switching rule or opening 새 규칙 with unsaved edits", async () => {
    listData = { ...listData, project: group("project", [row("testing.md"), row("api.md")]) };
    const dirty = vi.fn();
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <RulesSection project="p" hasPath onDirtyChange={dirty} />
      </QueryClientProvider>,
    );
    fireEvent.click(await screen.findByText("testing"));
    fireEvent.change(await screen.findByRole("textbox", { name: "본문" }), { target: { value: "바꿈" } });
    expect(dirty).toHaveBeenLastCalledWith(true);
    const ask = vi.spyOn(window, "confirm").mockReturnValue(false);
    fireEvent.click(screen.getByText("api"));
    fireEvent.click(screen.getAllByRole("button", { name: /새 규칙/ })[0]);
    expect(ask).toHaveBeenCalledTimes(2);
    expect((screen.getByRole("textbox", { name: "본문" }) as HTMLTextAreaElement).value).toBe("바꿈");
    ask.mockReturnValue(true);
    fireEvent.click(screen.getAllByRole("button", { name: /새 규칙/ })[0]);
    expect(await screen.findByText("파일 이름", { exact: false })).toBeTruthy();
    expect(dirty).toHaveBeenLastCalledWith(false);
    // Esc cancels an untouched 새 규칙 without asking.
    ask.mockClear();
    fireEvent.keyDown(screen.getByLabelText("시작"), { key: "Escape" });
    expect(ask).not.toHaveBeenCalled();
    expect(screen.queryByLabelText("시작")).toBeNull();
  });

  it("suggests api-2 when the template name is taken", async () => {
    listData = { ...listData, project: group("project", [row("api.md")]) };
    mount();
    fireEvent.click((await screen.findAllByRole("button", { name: /새 규칙/ }))[0]);
    fireEvent.change(screen.getByLabelText("시작"), { target: { value: "api" } });
    expect((screen.getByPlaceholderText("api 또는 frontend/react") as HTMLInputElement).value).toBe("api-2");
    fireEvent.change(screen.getByPlaceholderText("api 또는 frontend/react"), { target: { value: "api" } });
    expect(screen.getByText(/같은 이름의 규칙이 있어요/)).toBeTruthy();
    expect((screen.getByRole("button", { name: "만들기" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("disables the project group for the home folder", async () => {
    listData = { ...listData, project: { ...group("project", []), dir: "/Users/me/.claude/rules", exists: false, readonlyReason: "홈 폴더는 전역과 같아요" } };
    mount();
    expect(await screen.findByText("홈 폴더는 전역과 같아요")).toBeTruthy();
    expect(screen.queryByText(/아직 규칙이 없어요 ·/)).toBeNull();
    fireEvent.click(screen.getAllByRole("button", { name: /새 규칙/ })[0]);
    expect((screen.getByLabelText("범위") as HTMLSelectElement).options.length).toBe(1);
  });

  it("toggles with the settings token; other-pattern and linked rules are handled", async () => {
    listData = {
      ...listData,
      project: group("project", [
        row("testing.md"),
        row("legacy.md", { enabled: false, offBy: "other" }),
        row("old.md", { enabled: false, offBy: "app" }),
        row("security.md", { link: true, linkTarget: "/shared/security.md", readonlyReason: "공유 링크 · 원본에서 편집해 주세요" }),
      ]),
    };
    mount();
    fireEvent.click(await screen.findByRole("switch", { name: "testing.md 끄기" }));
    await waitFor(() => expect(rulesApi.setEnabled).toHaveBeenCalledWith("p", "r-testing.md", false, "s"));
    expect((screen.getByRole("switch", { name: "legacy.md 켜기" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText("다른 설정에서 꺼짐")).toBeTruthy();
    expect(screen.getByRole("switch", { name: "old.md 켜기" }).getAttribute("title")).toBe("파일은 그대로 · 이 컴퓨터 설정에서만 제외돼요");
    expect((screen.getByRole("switch", { name: "security.md 끄기" }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByText("old"));
    expect(await screen.findByText("파일은 그대로 · 이 컴퓨터 설정에서만 제외돼요")).toBeTruthy();
    fireEvent.click(screen.getByText("security"));
    expect(await screen.findByText("공유 링크 · 원본에서 편집해 주세요")).toBeTruthy();
    expect(screen.getByText("/shared/security.md")).toBeTruthy();
    const body = screen.getByRole("textbox", { name: /본문/ }) as HTMLTextAreaElement;
    expect(body.readOnly && body.value === "# 공유 본문\n").toBe(true);
    for (const b of ["휴지통으로", "저장", "되돌리기"]) expect(screen.queryByRole("button", { name: b })).toBeNull();
  });
});
