// Duck drawing (D-177, refined "pebble" duck A): four states plus neutral, three detail levels by size, colours through
// CSS variables, the role item kept (D-128), and the working pose leaving out the belt buckle (D-129·D-132).
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { ALL_ROLES, DEFAULT_ROLE } from "../lib/roles";
import { DUCK_COLORS, DuckAvatar, PLAIN_DUCK, detailFor } from "./DuckAvatar";

afterEach(cleanup);

const belt = DUCK_COLORS.find((c) => c.item === "belt")!;
const buckle = (root: HTMLElement) => root.querySelector("[data-duck-buckle]");
const POSES = ["working", "waiting", "failed", "done", "idle"] as const;
/** Every inline style in the drawing, joined (fills and strokes live there). */
const styles = (root: Element) => [...root.querySelectorAll("[style]")].map((e) => e.getAttribute("style")).join(" ");

describe("duck avatar", () => {
  it("draws the belt but no buckle in the working pose", () => {
    const { container } = render(<DuckAvatar pose="working" color={belt} label="개발 오리" />);
    expect(container.querySelector('[data-duck-item="belt"]')).toBeTruthy();
    expect(buckle(container)).toBeNull();
  });

  it("keeps the buckle in the other poses", () => {
    for (const pose of ["waiting", "done", "failed", "idle"] as const) {
      const { container, unmount } = render(<DuckAvatar pose={pose} color={belt} label="개발 오리" />);
      expect(buckle(container), pose).toBeTruthy();
      unmount();
    }
  });

  it("gives every coloured role its item, including the tutor's beanie", () => {
    for (const role of ALL_ROLES) {
      const { container, unmount } = render(<DuckAvatar pose="done" color={role.color} label={role.id} />);
      expect(container.querySelector("[data-duck-item]"), role.id).toBeTruthy();
      unmount();
    }
    const { container } = render(<DuckAvatar pose="done" color={DEFAULT_ROLE.color} label="기본" />);
    expect(container.querySelector("[data-duck-item]")).toBeNull();
  });

  it("marks each state with its own sign; yellow only in the waiting bubble", () => {
    const marks: Record<string, string | null> = { working: "laptop", waiting: "waiting", failed: "failed", done: null, idle: null };
    for (const pose of POSES) {
      const { container, unmount } = render(<DuckAvatar pose={pose} color={belt} label="x" />);
      const mark = container.querySelector("[data-duck-mark]");
      expect(mark?.getAttribute("data-duck-mark") ?? null, pose).toBe(marks[pose]);
      if (pose === "idle") expect(container.querySelectorAll("text").length).toBe(2);
      unmount();
    }
  });

  it("picks the detail level from the size (full ≥ 56, small ≥ 28, tiny below)", () => {
    expect([100, 56, 52, 32, 28, 20].map(detailFor)).toEqual(["full", "full", "small", "small", "small", "tiny"]);
    const { container } = render(<DuckAvatar pose="waiting" size={20} color={belt} label="x" />);
    const svg = container.querySelector("svg")!;
    expect(svg.getAttribute("data-detail")).toBe("tiny");
    expect(svg.getAttribute("viewBox")).toBe("12 14 96 96");
  });

  it("paints through CSS variables with the role colour as fallback, without outlines or navy", () => {
    const { container } = render(<DuckAvatar pose="done" color={belt} label="x" />);
    const css = styles(container);
    expect(css).toContain(`var(--duck-body, ${belt.body})`);
    expect(css).toContain("var(--duck-acc,");
    expect(css.toLowerCase()).not.toMatch(/#222854|rgb\(34, 40, 84\)/);
    // No outline: the coloured body ellipse carries no stroke.
    expect(container.querySelector('ellipse[stroke-width="1.7"]')).toBeNull();
  });

  it("keeps a light rim on the plain duck at small sizes so it holds on white sheets", () => {
    const { container } = render(<DuckAvatar pose="done" size={20} color={PLAIN_DUCK} label="x" />);
    expect(styles(container)).toContain("stroke: rgb(201, 204, 214)");
  });
});
