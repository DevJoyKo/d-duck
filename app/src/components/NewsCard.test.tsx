// news-apply 2.2: the key-fact card and the adopt tag on a news row and in the sheet head.
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { NewsItem } from "../lib/api";
import { NewsThumb, isCommandFact } from "./NewsCard";

const item = (extra: Partial<NewsItem> = {}): NewsItem => ({
  id: "2.1.323-diff",
  title: "Live /diff panel",
  version: "2.1.323",
  date: "2026-11-10",
  category: "ide",
  summary: "s",
  what: "w",
  whenToUse: "u",
  bestPractice: "b",
  sourceUrl: "https://code.claude.com/docs/en/changelog",
  firstSeenMs: 0,
  seen: false,
  ...extra,
});

afterEach(cleanup);

const fact = (c: HTMLElement) => c.querySelector("[data-news-fact]") as HTMLElement | null;
const tag = (c: HTMLElement) => c.querySelector("[data-news-adopt]") as HTMLElement | null;

describe("news key-fact card", () => {
  it("shows a command big in monospace with the 바로 됨 tag", () => {
    const { container } = render(<NewsThumb item={item({ keyFact: "/diff", adopt: "update" })} />);
    expect(fact(container)?.textContent).toBe("/diff");
    expect(fact(container)?.className).toContain("font-mono");
    expect(tag(container)?.textContent).toBe("바로 됨");
    expect(container.querySelector("[data-news-card]")?.getAttribute("style") ?? "").not.toContain("url(");
    expect(container.querySelector("img")).toBeNull();
  });

  it("shows a number or a name in the normal face with its tag", () => {
    const { container } = render(<NewsThumb item={item({ keyFact: "1M 컨텍스트", adopt: "opt_in" })} />);
    expect(fact(container)?.textContent).toBe("1M 컨텍스트");
    expect(fact(container)?.className).not.toContain("font-mono");
    expect(tag(container)?.textContent).toBe("골라 쓰기");
    cleanup();
    const name = render(<NewsThumb item={item({ keyFact: "maxEffortLevel", adopt: "config" })} />).container;
    expect(tag(name)?.textContent).toBe("설정 필요");
  });

  it("knows a command by a leading slash or `claude `", () => {
    expect([isCommandFact("/diff"), isCommandFact("claude plugin eval"), isCommandFact("1M 컨텍스트"), isCommandFact("claudeMd"), isCommandFact("modelPicker")]).toEqual([true, true, false, false, false]);
  });

  it("shows a long fact on up to two lines then an ellipsis on the row, keeps the whole text in title, and wraps in the sheet", () => {
    const long = "가".repeat(24);
    const { container } = render(<NewsThumb item={item({ keyFact: long, adopt: "update" })} />);
    const f = fact(container)!;
    expect(f.className).toContain("line-clamp-2");
    expect(f.className).toContain("wrap-break-word");
    expect(f.getAttribute("title")).toBe(long);
    expect(container.querySelector("[data-news-card]")?.className).toContain("w-[118px]");
    cleanup();
    const sheet = render(<NewsThumb item={item({ keyFact: long, adopt: "update" })} size="sheet" />).container;
    expect(fact(sheet)?.className).not.toContain("line-clamp");
    expect(fact(sheet)?.textContent).toBe(long);
    expect(sheet.querySelector("[data-news-card]")?.className).toContain("w-[132px]");
  });

  it("falls back to the category card without a key fact, and adds the tag when adopt is there", () => {
    const old = render(<NewsThumb item={item({ keyFact: null, adopt: null })} />).container;
    expect(old.querySelector("[data-news-category]")?.className).toContain("w-[118px]");
    expect(fact(old)).toBeNull();
    expect(tag(old)).toBeNull();
    cleanup();
    // An item stored before v18 has no apply keys at all.
    const legacy = render(<NewsThumb item={item()} />).container;
    expect(legacy.querySelector("[data-news-category]")).not.toBeNull();
    cleanup();
    const adoptOnly = render(<NewsThumb item={item({ keyFact: null, adopt: "update" })} />).container;
    expect(adoptOnly.querySelector("[data-news-category]")).not.toBeNull();
    expect(tag(adoptOnly)?.textContent).toBe("바로 됨");
  });

  it("never uses the duck yellow (the config tag is amber)", () => {
    const { container } = render(<NewsThumb item={item({ keyFact: "maxEffortLevel", adopt: "config" })} />);
    const classes = Array.from(container.querySelectorAll("*")).map((e) => e.getAttribute("class") ?? "");
    expect(classes.filter((c) => /(?:bg|ring|text|border)-duck/.test(c))).toEqual([]);
    expect(tag(container)?.className).toContain("text-warn");
  });
});
