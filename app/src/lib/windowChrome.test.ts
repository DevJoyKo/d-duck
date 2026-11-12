// Double-clicking the window's top area zooms the window like other macOS apps (D-146). Tauri's drag script calls
// `internal_toggle_maximize`, which needs its permission; the page header is a deep drag region so double-clicking its
// title text works too, while its buttons stay clickable (the drag script skips clickable elements).
import { describe, expect, it } from "vitest";
import capability from "../../src-tauri/capabilities/default.json";
import uiSource from "../components/ui.tsx?raw";
import sidebarSource from "../components/Sidebar.tsx?raw";

describe("window chrome", () => {
  it("allows the double-click zoom and keeps the permission list small", () => {
    expect(capability.permissions).toContain("core:window:allow-internal-toggle-maximize");
    expect(capability.permissions).toHaveLength(4);
  });
  it("makes the whole page header a drag region", () => {
    expect(uiSource).toContain('<header data-tauri-drag-region="deep"');
  });
  // windows-support 4.4: the traffic-light strip is macOS only; Windows keeps its native title bar (DOM checks for both
  // OSes are in components/WindowsPreview.test.tsx).
  it("keeps the sidebar drag strip and its padding to macOS", () => {
    expect(sidebarSource).toContain('{isMac() && <div data-tauri-drag-region className="absolute top-0 left-0 h-10 w-[216px]" />}');
    expect(sidebarSource).toContain('${isMac() ? "pt-11" : "pt-4"}');
  });
});
