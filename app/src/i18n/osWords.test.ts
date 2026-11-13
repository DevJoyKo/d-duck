// windows-support 4.2: catalogs name macOS places; on Windows the looked-up text names Windows ones. Values are not rewritten.
import { afterEach, describe, expect, it } from "vitest";
import { translate } from "./index";
import { setPlatformForTest } from "../lib/platform";

afterEach(() => setPlatformForTest(null));

describe("OS words", () => {
  it("leaves macOS text exactly as written", () => {
    expect(translate("ko", "tasks.scope.switch")).toBe("프로젝트 바꾸기 (⌘P)");
    expect(translate("en", "extensions.row.deleteTitle")).toBe("Moves it to Trash · You can restore it in Finder");
    expect(translate("ko", "settings.notifyFinished.description")).toContain("macOS 알림");
  });
  it("names Windows places and the Ctrl key on Windows", () => {
    setPlatformForTest({ os: "windows", gitBash: true });
    expect(translate("ko", "tasks.scope.switch")).toBe("프로젝트 바꾸기 (Ctrl+P)");
    expect(translate("en", "tasks.scope.switch")).toBe("Switch project (Ctrl+P)");
    expect(translate("en", "extensions.row.deleteTitle")).toBe("Moves it to Recycle Bin · You can restore it in File Explorer");
    expect(translate("ko", "extensions.row.deleteTitle")).toBe("휴지통으로 옮겨요 · 파일 탐색기에서 되살릴 수 있어요");
    expect(translate("ko", "settings.notifyFinished.description")).toContain("Windows 알림");
    expect(translate("en", "tasks.limits.autoNote")).toBe("Your PC needs to be awake");
    expect(translate("ko", "common.macOnly.chip")).toBe("Mac 전용");
    const note = translate("ko", "routines.list.note");
    expect(note).toContain("PC가 깨어 있을 때만");
    expect(note).toContain("이 PC의 앱 데이터");
    expect(note).not.toContain("맥");
    expect(translate("en", "routines.list.note")).toContain("the PC is awake");
    expect(translate("en", "routines.list.note")).toContain("this PC's app data");
    expect(translate("ko", "tour.cards.solo.point2")).toContain("맥락");
    expect(translate("en", "common.macOnly.chip")).toBe("Mac only");
  });
  it("never rewrites a filled-in value", () => {
    setPlatformForTest({ os: "windows", gitBash: true });
    expect(translate("en", "extensions.page.trashed", { name: "Finder macOS ⌘" })).toBe("Moved to Recycle Bin · Finder macOS ⌘");
  });
});
