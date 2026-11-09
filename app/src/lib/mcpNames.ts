// MCP labels shared by the MCP tab and the plugin details (nav-ia review LOW): one transport map, and a readable name
// for plugin servers (`plugin:<plugin>:<server>` → `<server>`; the full id stays for status matching and tooltips).

import { t } from "../i18n";

/** Protocol names stay as written; the two words are read at call time so they follow the language. */
const FIXED: Record<string, string> = { http: "HTTP", sse: "SSE", ws: "WebSocket" };

/** Transport label; unknown values fall back to "기타" rather than the raw string. */
export function transportLabel(kind: string | null | undefined): string | null {
  if (!kind) return null;
  const k = kind.toLowerCase();
  if (k === "stdio") return t("extensions.mcp.transport.stdio");
  return FIXED[k] ?? t("extensions.mcp.transport.fallback");
}

/** `plugin:expo:expo` → `expo`, `plugin:kit:db` → `db`; other names unchanged. */
export function mcpDisplayName(name: string): string {
  const m = /^plugin:[^:]+:(.+)$/i.exec(name);
  return m ? m[1] : name;
}
