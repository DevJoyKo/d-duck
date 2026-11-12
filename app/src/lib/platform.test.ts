// windows-support 4.1: path and shortcut helpers per OS. macOS is the default and must behave as before.
import { afterEach, describe, expect, it, vi } from "vitest";
import { baseName, isAbsPath, isMac, isWindows, loadPlatform, modKey, platform, setPlatformForTest, shortcut, shortPath, splitPath } from "./platform";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

afterEach(() => setPlatformForTest(null));

describe("platform on macOS (default)", () => {
  it("starts as macOS with Git Bash irrelevant", () => {
    expect(isMac()).toBe(true);
    expect(platform()).toEqual({ os: "macos", gitBash: true });
  });
  it("keeps the / rules", () => {
    expect(isAbsPath("/Users/me/app")).toBe(true);
    expect(isAbsPath("C:\\Users\\me")).toBe(false);
    expect(isAbsPath("~/app")).toBe(false);
    expect(splitPath("/Users/me/a\\b")).toEqual(["Users", "me", "a\\b"]);
    expect(baseName("/Users/me/dev_team/")).toBe("dev_team");
    expect(shortPath("/Users/me/work/app", "/Users/me")).toBe("~/work/app");
    expect(shortPath("/Users/meow/app", "/Users/me")).toBe("/Users/meow/app");
    expect(modKey()).toBe("⌘");
    expect(shortcut("↵")).toBe("⌘↵");
  });
});

describe("platform on Windows", () => {
  it("treats drive and UNC paths as absolute, not / or relative ones", () => {
    setPlatformForTest({ os: "windows", gitBash: true });
    expect(isWindows()).toBe(true);
    expect(isAbsPath("C:\\Users\\me\\work\\app")).toBe(true);
    expect(isAbsPath("d:/work")).toBe(true);
    expect(isAbsPath("\\\\server\\share\\x")).toBe(true);
    expect(isAbsPath("C:work")).toBe(false);
    expect(isAbsPath("work\\app")).toBe(false);
    expect(isAbsPath("/Users/me")).toBe(false);
  });
  it("splits on both slashes and shows the user folder as ~ regardless of letter case", () => {
    setPlatformForTest({ os: "windows", gitBash: true });
    expect(splitPath("C:\\Users\\me/work\\app\\")).toEqual(["C:", "Users", "me", "work", "app"]);
    expect(baseName("C:\\Users\\me\\work\\app")).toBe("app");
    expect(shortPath("c:\\users\\ME\\work\\app", "C:\\Users\\me")).toBe("~\\work\\app");
    expect(shortPath("C:\\Users\\me", "C:\\Users\\me\\")).toBe("~");
    expect(shortPath("C:\\Users\\meow\\app", "C:\\Users\\me")).toBe("C:\\Users\\meow\\app");
  });
  it("labels shortcuts with Ctrl", () => {
    setPlatformForTest({ os: "windows", gitBash: true });
    expect(modKey()).toBe("Ctrl");
    expect(shortcut("↵")).toBe("Ctrl+Enter");
    expect(shortcut("P")).toBe("Ctrl+P");
  });
});

describe("loadPlatform", () => {
  it("reads the Rust command once and keeps macOS when it fails or sends junk", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const mock = vi.mocked(invoke);
    mock.mockResolvedValueOnce({ os: "windows", gitBash: false });
    await loadPlatform();
    await loadPlatform();
    expect(mock).toHaveBeenCalledTimes(1);
    expect(mock).toHaveBeenCalledWith("platform");
    expect(platform()).toEqual({ os: "windows", gitBash: false });

    setPlatformForTest(null);
    mock.mockRejectedValueOnce(new Error("no command"));
    await loadPlatform();
    expect(isMac()).toBe(true);

    setPlatformForTest(null);
    mock.mockResolvedValueOnce({ os: "plan9" });
    await loadPlatform();
    expect(platform()).toEqual({ os: "macos", gitBash: true });
  });
});
