// extension-edit-delete (D-147): "기록 없음" instead of "안 씀", "꺼짐" chip, 수정·보기·삭제 buttons and the Trash flow,
// on the mock backend (the real rules and the native dialog live in Rust: ext_trash.rs, ext_items_cmds.rs).
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Skill } from "../lib/api";

vi.mock("@tauri-apps/api/core", async () => {
  const real = await vi.importActual<typeof import("../mock/core")>("../mock/core");
  return { invoke: vi.fn(real.invoke) };
});
vi.mock("@tauri-apps/api/event", () => import("../mock/event"));

type Invoke = ReturnType<typeof vi.fn> & ((c: string, a?: Record<string, unknown>) => Promise<unknown>);
let real: (c: string, a?: Record<string, unknown>) => Promise<unknown>;

async function boot(): Promise<Invoke> {
  localStorage.setItem("dduck.page", "extensions");
  const core = await import("@tauri-apps/api/core");
  const { default: App } = await import("../App");
  render(<App />);
  return core.invoke as unknown as Invoke;
}

/** The list item of a top-level row by its visible name. */
async function rowOf(name: string): Promise<HTMLElement> {
  const label = await screen.findByRole("button", { name }, { timeout: 3000 });
  return label.closest("li") as HTMLElement;
}

beforeEach(async () => {
  vi.resetModules();
  const core = (await import("@tauri-apps/api/core")).invoke as unknown as Invoke;
  core.mockClear();
  real = (await vi.importActual<typeof import("../mock/core")>("../mock/core")).invoke as typeof real;
  core.mockImplementation(real);
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  localStorage.clear();
});

const sk = (extra: Partial<Skill>): Skill => ({ name: "x", source: "user", scope: null, kind: "skill", description: null, uses: 0, lastUsed: null, defined: true, id: "s-x", readonly: null, ...extra });

describe("row buttons (design 2)", async () => {
  const { rowActions } = await import("./ExtensionsPage");
  it("own items edit and delete, copies included; read-only items view (+ copy); usage-only rows have none", () => {
    expect(rowActions(sk({}))).toEqual({ edit: "수정", del: true, copy: false });
    expect(rowActions(sk({ copiedFrom: "frontend-design:frontend-design" }))).toEqual({ edit: "수정", del: true, copy: false });
    expect(rowActions(sk({ source: "project", kind: "workflow" }))).toEqual({ edit: "수정", del: true, copy: false });
    expect(rowActions(sk({ source: "plugin", readonly: "플러그인 항목은 고칠 수 없어요" }))).toEqual({ edit: "보기", del: false, copy: true });
    expect(rowActions(sk({ readonly: "claude.ai에서 동기화된 항목은 고칠 수 없어요" }))).toEqual({ edit: "보기", del: false, copy: true });
    expect(rowActions(sk({ source: "builtin", id: null, defined: false }))).toEqual({ edit: null, del: false, copy: false });
  });
});

describe("deleted rows (review 3, 5)", async () => {
  const { withDeleted } = await import("./ExtensionsPage");
  it("marks only usage-only rows and gives them the deleted kind; a same-named defined item stays", () => {
    const usage = sk({ name: "standup", source: "builtin", id: null, defined: false, uses: 2 });
    const r = withDeleted([usage], [{ id: "s-a", name: "standup", kind: "command" }]);
    expect(r.skills[0].kind).toBe("command");
    expect([...r.marked]).toEqual(["command|builtin||standup"]);
    const still = sk({ name: "standup", id: "s-b", uses: 2 });
    const r2 = withDeleted([still], [{ id: "s-a", name: "standup", kind: "skill" }]);
    expect(r2.skills[0]).toBe(still);
    expect(r2.marked.size).toBe(0);
    expect(withDeleted([usage], []).marked.size).toBe(0);
  });
});

describe("usage label and the off chip", async () => {
  const { InnerLine } = await import("./ExtensionsPage");
  const { ItemList, NO_RECORD_TIP } = await import("./PluginsPage");
  it("PluginsPage ItemList says 기록 없음 with the tooltip, or N회", () => {
    render(<ItemList plugin="kit" title="스킬" items={[{ name: "kit:a", description: null, uses: 0, lastUsed: null }, { name: "kit:b", description: null, uses: 3, lastUsed: 1_700_000_000_000 }]} />);
    expect(screen.getByText("기록 없음").getAttribute("title")).toBe(NO_RECORD_TIP);
    expect(screen.getByText("3회").getAttribute("title")).toMatch(/^마지막 /);
    expect(screen.queryByText("안 씀")).toBeNull();
  });

  it("an item of an off plugin shows 꺼짐 before the usage; an on plugin's does not", () => {
    const i = { kind: "skill" as const, name: "expo:eas-update", description: null, uses: 0, lastUsed: null, skill: sk({ name: "expo:eas-update", source: "plugin", readonly: "플러그인 항목은 고칠 수 없어요" }) };
    const { container, rerender } = render(<InnerLine plugin="expo" i={i} off onEdit={() => {}} onCopy={() => {}} />);
    const chip = screen.getByText("꺼짐");
    const uses = screen.getByText("기록 없음");
    expect(chip.compareDocumentPosition(uses) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // Only the name and description fade: the chip and the usage keep full contrast (review HIGH 1).
    expect(chip.closest(".opacity-60")).toBeNull();
    expect(uses.closest(".opacity-60")).toBeNull();
    expect(screen.getByText("eas-update").closest(".opacity-60")).toBeTruthy();
    expect(screen.getByRole("button", { name: "보기" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "수정" })).toBeNull();
    expect(screen.queryByRole("button", { name: "삭제" })).toBeNull();
    rerender(<InnerLine plugin="expo" i={i} off={false} onEdit={() => {}} onCopy={() => {}} />);
    expect(within(container).queryByText("꺼짐")).toBeNull();
  });
});

describe("확장 tab: delete", () => {
  it("shows 기록 없음 and the buttons per row", async () => {
    await boot();
    const pr = await rowOf("pr-writer");
    expect(within(pr).getByText("기록 없음").getAttribute("title")).toBe("수집한 기록에서 부른 적이 없어요 · 켜져 있는지와는 달라요");
    expect(within(pr).getByRole("button", { name: "수정" })).toBeTruthy();
    expect(within(pr).getByRole("button", { name: "삭제" })).toBeTruthy();
    const copy = await rowOf("design-review");
    expect(within(copy).getByRole("button", { name: "삭제" })).toBeTruthy();
    expect(within(copy).queryByRole("button", { name: /복사해서 내 것으로/ })).toBeNull();
    const docx = await rowOf("anthropic-skills:docx");
    expect(within(docx).getByRole("button", { name: "보기" })).toBeTruthy();
    expect(within(docx).getByRole("button", { name: /복사해서 내 것으로/ })).toBeTruthy();
    expect(within(docx).queryByRole("button", { name: "삭제" })).toBeNull();
    const builtin = screen.getByText("update-config").closest("li") as HTMLElement;
    expect(within(builtin).queryAllByRole("button")).toEqual([]);
    expect(screen.queryByText("안 씀")).toBeNull();
  });

  it("moves an item to the Trash; a command with usage leaves a usage-only row in the command group", async () => {
    const invoke = await boot();
    const commands = () => screen.getByRole("heading", { level: 2, name: /^커맨드/ }).closest("section, div") as HTMLElement;
    fireEvent.click(within(await rowOf("/standup")).getByRole("button", { name: "삭제" }));
    expect(await screen.findByRole("status")).toHaveProperty("textContent", "휴지통으로 옮겼어요 · standup");
    expect(invoke).toHaveBeenCalledWith("trash_extension_item", { id: "s-standupcommand", unsaved: false });
    expect(await screen.findByText("지운 커맨드예요 · 기록만 남아 있어요")).toBeTruthy();
    expect(screen.queryByText("지운 스킬이에요 · 기록만 남아 있어요")).toBeNull();
    expect(screen.queryByRole("button", { name: "/standup" })).toBeNull();
    const left = screen.getByText("지운 커맨드예요 · 기록만 남아 있어요").closest("li") as HTMLElement;
    expect(commands().contains(left)).toBe(true);
    expect(within(left).getByText("/standup")).toBeTruthy();
    expect(within(left).getByText("2회")).toBeTruthy();
    expect(within(left).getByText("기록만")).toBeTruthy();
    expect(within(left).queryAllByRole("button")).toEqual([]);
    expect(screen.getByRole("button", { name: /^커맨드/ }).textContent).toBe("커맨드3");
  });

  it("a cancelled dialog changes nothing and shows no error", async () => {
    const invoke = await boot();
    invoke.mockImplementation((c, a) => (c === "trash_extension_item" ? Promise.resolve(false) : real(c, a)));
    fireEvent.click(within(await rowOf("pr-writer")).getByRole("button", { name: "삭제" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("trash_extension_item", { id: "s-prwriterskill", unsaved: false }));
    expect(await rowOf("pr-writer")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("a failed move shows one line under that row and keeps the item", async () => {
    const invoke = await boot();
    invoke.mockImplementation((c, a) => (c === "trash_extension_item" ? Promise.reject("휴지통으로 옮기지 못했어요") : real(c, a)));
    const pr = await rowOf("pr-writer");
    fireEvent.click(within(pr).getByRole("button", { name: "삭제" }));
    expect((await within(pr).findByRole("alert")).textContent).toBe("휴지통으로 옮기지 못했어요");
    expect(await rowOf("pr-writer")).toBeTruthy();
  });

  it("deletes from the editor and closes it; unsaved changes are passed to the dialog", async () => {
    const invoke = await boot();
    fireEvent.click(within(await rowOf("triage")).getByRole("button", { name: "수정" }));
    const box = (await screen.findByLabelText("내용")) as HTMLTextAreaElement;
    fireEvent.change(box, { target: { value: `${box.value}\n// edit` } });
    const dialog = screen.getByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "삭제" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(invoke).toHaveBeenCalledWith("trash_extension_item", { id: "s-triageworkflow", unsaved: true });
    await waitFor(() => expect(screen.queryByRole("button", { name: "triage" })).toBeNull());
  });

  it("read-only items open the editor without a delete button", async () => {
    await boot();
    fireEvent.click(within(await rowOf("anthropic-skills:docx")).getByRole("button", { name: "보기" }));
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByLabelText("내용");
    expect(within(dialog).queryByRole("button", { name: "삭제" })).toBeNull();
  });
});
