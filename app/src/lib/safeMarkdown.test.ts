import { describe, expect, it } from "vitest";
import { parseInline, parseMarkdown, safeLink, stripUnsafe } from "./safeMarkdown";

describe("safe markdown", () => {
  it("keeps raw HTML as text and drops images", () => {
    const b = parseMarkdown('<img src=x onerror=alert(1)> hi ![tracker](https://evil.example/p.png)\n\n<script>x</script>');
    expect(b).toEqual([
      { type: "p", inl: [{ t: "text", v: "<img src=x onerror=alert(1)> hi [이미지: tracker]" }] },
      { type: "p", inl: [{ t: "text", v: "<script>x</script>" }] },
    ]);
  });

  it("links only https with a host and shows the host", () => {
    expect(parseInline("[가격](https://docs.claude.com/en/pricing)")).toEqual([{ t: "link", label: "가격", url: "https://docs.claude.com/en/pricing", host: "docs.claude.com" }]);
    for (const bad of ["javascript:alert(1)", "http://docs.claude.com", "https://u:p@docs.claude.com/x", "data:text/html,x"]) {
      expect(parseInline(`[a](${bad})`).some((i) => i.t === "link"), bad).toBe(false);
    }
    expect(safeLink("https://user@x.com")).toBeNull();
  });

  it("replaces URL-looking labels with the real host", () => {
    for (const label of ["https://docs.claude.com", "docs.claude.com/pricing", "www.anthropic.com", "anthropic.com"]) {
      expect(parseInline(`[${label}](https://evil.example/x)`)).toEqual([{ t: "link", label: "evil.example", url: "https://evil.example/x", host: "evil.example" }]);
    }
    expect(parseInline("[가격 v1.2](https://docs.claude.com/p)")[0]).toMatchObject({ label: "가격 v1.2" });
  });

  it("does not autolink bare URLs", () => {
    expect(parseInline("see https://docs.claude.com now")).toEqual([{ t: "text", v: "see https://docs.claude.com now" }]);
  });

  it("strips bidi, zero-width and control characters", () => {
    expect(stripUnsafe("a‮b​c\u0007d\ne")).toBe("abcd\ne");
    const b = parseMarkdown("## 비용‮ 줄이기");
    expect(b[0]).toEqual({ type: "h", level: 2, inl: [{ t: "text", v: "비용 줄이기" }] });
  });

  it("parses headings, lists, bold, code and fences", () => {
    const b = parseMarkdown("# T\n- **캐시** 비율 `12%`\n- 둘\n1. 하나\n```\n<b>raw</b>\n```");
    expect(b.map((x) => x.type)).toEqual(["h", "ul", "ol", "pre"]);
    expect(b[1]).toEqual({ type: "ul", items: [[{ t: "b", v: "캐시" }, { t: "text", v: " 비율 " }, { t: "code", v: "12%" }], [{ t: "text", v: "둘" }]] });
    expect(b[3]).toEqual({ type: "pre", text: "<b>raw</b>" });
  });

  it("caps very long input", () => {
    expect(new TextEncoder().encode(stripUnsafe("가".repeat(40_000))).length).toBeLessThanOrEqual(64 * 1024);
  });

  it("draws pipe tables with alignment and inline cells", () => {
    const md = "| 후보 | 비용 | 운영 |\n|:--|--:|:-:|\n| A | **낮음** | 쉬움 |\n| B | 중간 | `k8s` |\n| C | 높음 | 어려움 |\n| D | x | y |\n| E | 1 \\| 2 | z |";
    const [t] = parseMarkdown(md);
    expect(t.type).toBe("table");
    if (t.type !== "table") return;
    expect(t.head).toHaveLength(3);
    expect(t.rows).toHaveLength(5);
    expect(t.align).toEqual(["left", "right", "center"]);
    expect(t.rows[0][1]).toEqual([{ t: "b", v: "낮음" }]);
    expect(t.rows[4][1]).toEqual([{ t: "text", v: "1 | 2" }]);
    // Raw HTML inside a cell stays text.
    const [h] = parseMarkdown("| a |\n|---|\n| <img src=x onerror=1> |");
    expect(h.type === "table" && h.rows[0][0]).toEqual([{ t: "text", v: "<img src=x onerror=1>" }]);
  });

  it("keeps a 50 KB document to the end and numbers image lines", () => {
    const line = "가나다 라마바 사아자\n\n";
    const big = line.repeat(Math.ceil((50 * 1024) / new TextEncoder().encode(line).length)) + "끝 문단";
    const b = parseMarkdown(big);
    expect(b[b.length - 1]).toEqual({ type: "p", inl: [{ t: "text", v: "끝 문단" }] });
    const imgs = parseMarkdown("![화면](a.png)\n\n본문 ![x](b.png)\n\n![둘째](b.png)");
    expect(imgs.filter((x) => x.type === "img")).toEqual([
      { type: "img", alt: "화면", name: "a.png", n: 0, k: 0 },
      { type: "img", alt: "둘째", name: "b.png", n: 1, k: 0 },
    ]);
  });
});
