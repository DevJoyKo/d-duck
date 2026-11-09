import { describe, expect, it } from "vitest";
import { moveInOrder, orderCards, roleOf } from "./roles";

describe("role colours", () => {
  it("same role, same colour; namespaced and Korean names work", () => {
    expect(roleOf("design-critic").id).toBe("review");
    expect(roleOf("dduck-reviewer").id).toBe("review");
    expect(roleOf("agent-skills:code-reviewer").color).toEqual(roleOf("design-critic").color);
    expect(roleOf("security-privacy-reviewer").id).toBe("security");
    expect(roleOf("qa-verifier").id).toBe("qa");
    expect(roleOf("agent-skills:test-engineer").id).toBe("qa");
    expect(roleOf("Explore").id).toBe("research");
    expect(roleOf("Plan").id).toBe("plan");
    expect(roleOf("helper", "릴리스 노트와 버전 태그를 정리한다").id).toBe("release");
    expect(roleOf("general-purpose", "researching complex questions").id).toBe("general");
    expect(roleOf("agent-skills:web-performance-auditor").id).toBe("dev");
    expect(roleOf("agent-skills:security-auditor").id).toBe("security");
    expect(roleOf("helper", "handles ownership and the latest docs").id).toBe("writer");
    expect(roleOf("frontend-dev").id).toBe("dev");
  });
});

describe("card order", () => {
  const card = (name: string, active = false) => ({ name, key: name === "기본 Claude" ? "__default" : name, active });
  it("puts active agents first, then ABC or custom order", () => {
    const cards = [card("zeta"), card("alpha"), card("기본 Claude"), card("mid", true)];
    expect(orderCards(cards, "abc", []).map((c) => c.name)).toEqual(["mid", "기본 Claude", "alpha", "zeta"]);
    expect(orderCards(cards, "custom", ["zeta", "alpha"]).map((c) => c.name)).toEqual(["mid", "기본 Claude", "zeta", "alpha"]);
  });
  it("moves one step and stops at the ends", () => {
    expect(moveInOrder(["a", "b", "c"], "c", -1)).toEqual(["a", "c", "b"]);
    expect(moveInOrder(["a", "b"], "a", -1)).toEqual(["a", "b"]);
  });
});
