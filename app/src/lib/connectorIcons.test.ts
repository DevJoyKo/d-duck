import { describe, expect, it } from "vitest";
import { iconFor, serviceName } from "./connectorIcons";

const title = (name: string, host: string | null = null) => iconFor(name, host)?.title ?? null;

describe("connector icons", () => {
  it("matches the real connector names", () => {
    expect(title("claude.ai Gmail", "gmailmcp.googleapis.com")).toBe("Gmail");
    expect(title("claude.ai Google Calendar", "calendarmcp.googleapis.com")).toBe("Google Calendar");
    expect(title("claude.ai Google Drive", "drivemcp.googleapis.com")).toBe("Google Drive");
    expect(title("claude.ai Atlassian (2)", "mcp.atlassian.com")).toBe("Atlassian");
    expect(title("plugin:cloudflare:cloudflare")).toBe("Cloudflare");
    expect(title("plugin:expo:expo", "mcp.expo.dev")).toBe("Expo");
    expect(title("notebooklm-mcp")).toBe("NotebookLM");
    expect(title("linear", "mcp.linear.app")).toBe("Linear");
  });

  it("does not guess other vendors' products", () => {
    expect(title("outlook-calendar")).toBeNull();
    expect(title("onedrive")).toBeNull();
    expect(title("files", "drivers.example.com")).toBeNull();
    expect(title("linear-algebra")).toBeNull();
    expect(title("expo-something")).toBeNull();
    expect(title("claude.ai Slack", "mcp.slack.com")).toBeNull();
  });

  it("strips prefixes for the letter badge", () => {
    expect(serviceName("claude.ai Claude Docs")).toBe("Claude Docs");
    expect(serviceName("plugin:cloudflare:cloudflare")).toBe("cloudflare");
  });
});
