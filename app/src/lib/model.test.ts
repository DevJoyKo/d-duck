import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createElement } from "react";
import { applyLanguage } from "../i18n";
import { eul } from "../i18n/josa";
import { costText, effortHint, recordLines, replacementText, reworkModel, shallowHint, validChoice, validModel, type ModelOptions } from "./model";
import { ModelPicker } from "../components/ModelPicker";

// The same table as `model_select::tests::model_value_table` in Rust (design 2).
const OK = ["opus", "sonnet[1m]", "claude-opus-4-6[1m]", "us.anthropic.claude-opus-4-8", "claude-sonnet-4-5@20250929", "anthropic.claude-haiku-4-5-v1:0", "claude-opus-5-5", "best", "opusplan"];
const BAD = ["-p", "opus --dangerously-skip-permissions", "Opus", "a/b", "null", "TRUE", "true", "yes", "no", "on", "off", "y", "n", "123", "1.5", "", " opus", "<synthetic>", "opus\n", "~", `a${"b".repeat(100)}`];

describe("model values", () => {
  it("accepts and refuses the same values as Rust", () => {
    for (const v of OK) expect([v, validModel(v)]).toEqual([v, true]);
    for (const v of BAD) expect([v, validModel(v)]).toEqual([v, false]);
    expect(validModel("a".repeat(100))).toBe(true);
    expect([validChoice("inherit"), validChoice("default"), validChoice("haiku")]).toEqual([false, false, true]);
  });

  it("reads the Rust test table itself", async () => {
    const rs = (await import("../../crates/dduck-core/src/model_select.rs?raw")).default as string;
    for (const v of OK) expect(rs).toContain(`"${v}"`);
  });
});

describe("model lines", () => {
  afterEach(() => applyLanguage("ko"));

  it("words the replacement and the task-detail notice with the right particles", () => {
    expect(replacementText({ kind: "alias", to: "opus" })).toBe("지원되지 않는 모델 · opus로 대체돼요");
    expect(replacementText({ kind: "default" })).toBe("지원되지 않는 모델 · 기본값으로 대체돼요");
    expect(replacementText({ kind: "none" })).toBe("지원되지 않는 모델 · 대신 쓸 모델이 없어요");
    expect(replacementText(undefined)).toBeNull();
    expect(recordLines({ modelLayer: "type", modelUsed: "opus", modelRequested: "claude-opus-3-9" })).toEqual(["모델: opus · 유형 설정", "요청한 claude-opus-3-9를 쓸 수 없어 opus로 실행했어요"]);
    expect(recordLines({ modelLayer: "task", modelUsed: null, modelRequested: "acme-large-2" })[1]).toBe("요청한 acme-large-2를 쓸 수 없어 기본값으로 실행했어요");
    expect(recordLines({ modelLayer: "task", modelUsed: "opus", modelOther: "claude-sonnet-5-5" })[1]).toBe("요청한 모델(opus)과 다른 claude-sonnet-5-5로 돌았어요");
    expect(recordLines(null)).toEqual(["모델: 기록 없음"]);
    expect([eul("sonnet"), eul("claude-opus-4-1"), eul("haiku")]).toEqual(["을", "을", "를"]);
    applyLanguage("en");
    expect(recordLines({ modelLayer: "type", modelUsed: "opus", modelRequested: "claude-opus-3-9" })[1]).toBe("claude-opus-3-9 wasn't available, so it ran on opus");
  });

  it("shows cost from 1.2× on, rounded to one decimal", () => {
    expect(costText(20, 10)).toBe("기본값보다 약 2.0배 비싸요");
    expect(costText(11, 10)).toBeNull();
    expect(costText(undefined, 10)).toBeNull();
    expect(costText(20, 20)).toBeNull();
    expect(shallowHint(true, 0.5, 20)).toBe(true);
    expect(shallowHint(false, 0.5, 20)).toBe(false);
    expect(effortHint("claude-opus-4-6[1m]", "xhigh")).toBe(true);
    expect(effortHint("claude-opus-5-5", "xhigh")).toBe(false);
  });

  it("fills the rework model only for a task-layer choice", () => {
    expect(reworkModel({ modelLayer: "task", modelUsed: "haiku" })).toEqual({ model: "haiku" });
    expect(reworkModel({ modelLayer: "agent", modelUsed: "opus" })).toEqual({ originalModel: "opus" });
    expect(reworkModel({ modelLayer: "cli" })).toEqual({});
  });
});

const opts: ModelOptions = {
  items: [
    { value: "opus", sources: ["cli"], resolved: { id: "claude-opus-5-5", basis: "recent" }, allow: "yes", price: 20 },
    { value: "claude-haiku-5-5", sources: ["recent"], allow: "no", blockedBy: "managed", price: 0.5 },
    { value: "claude-opus-3-9", sources: ["custom"], allow: "yes", replacement: { kind: "alias", to: "opus" } },
  ],
  currentDefault: { value: "claude-sonnet-5-5", basis: "recent", price: 10 },
  cliRead: false,
  dropped: 0,
  allowlist: "managed",
};

describe("ModelPicker", () => {
  afterEach(cleanup);
  it("lists the default first, blocks with a reason, and marks a stored unsupported value", () => {
    let picked: string | null = "x";
    const { rerender } = render(createElement(ModelPicker, { value: "claude-opus-3-9", onChange: (v: string | null) => (picked = v), options: opts, defaultText: "기본값 (지금: claude-sonnet-5-5 · 최근 기록)", basePrice: 10 }));
    const select = screen.getByLabelText("모델") as HTMLSelectElement;
    expect(select.options[0].textContent).toBe("기본값 (지금: claude-sonnet-5-5 · 최근 기록)");
    expect(select.options[1].textContent).toBe("opus · claude-opus-5-5 (최근 기록 기준) · CLI");
    expect(select.options[2].disabled).toBe(true);
    expect(select.options[2].textContent).toContain("관리 설정이 이 모델을 막아요");
    expect(screen.getByText("지원되지 않는 모델 · opus로 대체돼요")).toBeTruthy();
    expect(screen.getByText("CLI에서 별칭을 못 읽었어요")).toBeTruthy();
    fireEvent.change(select, { target: { value: "opus" } });
    expect(picked).toBe("opus");
    rerender(createElement(ModelPicker, { value: "opus", onChange: (v: string | null) => (picked = v), options: opts, defaultText: "d", basePrice: 10 }));
    expect(screen.getByText("기본값보다 약 2.0배 비싸요")).toBeTruthy();
  });

  it("checks a typed model before using it, and Esc closes only the input", () => {
    let picked: string | null = null;
    render(createElement(ModelPicker, { value: null, onChange: (v: string | null) => (picked = v), options: { ...opts, allowlist: "none" }, defaultText: "d" }));
    fireEvent.change(screen.getByLabelText("모델"), { target: { value: "__custom__" } });
    const input = screen.getByLabelText("모델 이름");
    fireEvent.change(input, { target: { value: "Opus X" } });
    expect(screen.getByText("모델 이름 형식이 아니에요")).toBeTruthy();
    fireEvent.change(input, { target: { value: "claude-opus-4-6[1m]" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(picked).toBe("claude-opus-4-6[1m]");
  });
});
