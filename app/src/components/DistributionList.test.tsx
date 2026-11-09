import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Item } from "../lib/api";
import { DistributionList } from "./DistributionList";

const item = (key: string, count: number, share: number, extra: Partial<Item> = {}): Item => ({
  key,
  count,
  share,
  viaTool: 0,
  viaSlash: 0,
  tokens: null,
  estCostUsd: null,
  priced: true,
  ...extra,
});

describe("DistributionList", () => {
  it("shows count and share for each item, including the 기타 bucket", () => {
    render(<DistributionList items={[item("Bash", 60, 60), item("Edit", 30, 30), item("기타", 10, 10)]} />);
    expect(screen.getByText("Bash")).toBeTruthy();
    expect(screen.getByText("60.0%")).toBeTruthy();
    expect(screen.getByText("기타")).toBeTruthy();
  });

  it("splits skill calls by tool and slash", () => {
    render(<DistributionList showVia items={[item("opsx:propose", 3, 100, { viaTool: 1, viaSlash: 2 })]} />);
    expect(screen.getByTestId("via").textContent).toBe("도구 1 · 슬래시 2");
  });

  it("marks unpriced models exactly as the backend sends them (cost null)", () => {
    render(<DistributionList showCost items={[item("mystery-model", 1, 100, { estCostUsd: null, priced: false })]} />);
    expect(screen.getByText("단가 미등록")).toBeTruthy();
  });

  it("marks partially priced projects", () => {
    render(<DistributionList showCost items={[item("proj", 2, 100, { estCostUsd: 1.5, priced: false })]} />);
    expect(screen.getByText("$1.50 + 단가 미등록 포함")).toBeTruthy();
  });

  it("renders markup in keys as plain text", () => {
    const { container } = render(<DistributionList items={[item("<img src=x onerror=alert(1)>", 1, 100)]} />);
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText("<img src=x onerror=alert(1)>")).toBeTruthy();
  });

  it("explains an empty list", () => {
    render(<DistributionList items={[]} />);
    expect(screen.getByText("기록이 없어요")).toBeTruthy();
  });
});
