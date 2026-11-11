// One-line result notes of a plugin toggle or update, shared by the row and the batch status (plugin-bulk-update).
import { t } from "../i18n";
import type { Plugin, PluginOutcome, Shown } from "./api";

export type Note = { tone: "ok" | "warn" | "error"; text: Shown };

export function noteFor(
  p: Plugin,
  o: PluginOutcome,
  verb: "toggle" | "update",
): Note | null {
  switch (o.kind) {
    case "done":
      return {
        tone: "ok",
        text: t(verb === "update" ? "extensions.plugins.note.updated" : p.enabled ? "extensions.plugins.note.turnedOff" : "extensions.plugins.note.turnedOn"),
      };
    case "needsApproval":
      return {
        tone: "warn",
        text: t("extensions.plugins.note.needsApproval", { id: p.id }),
      };
    case "locked":
      return {
        tone: "warn",
        text: t("extensions.plugins.note.locked"),
      };
    case "unchanged":
      return {
        tone: "warn",
        text:
          verb === "update"
            ? t("extensions.plugins.note.unchangedUpdate")
            : t("extensions.plugins.note.unchangedToggle"),
      };
    case "timeout":
      return {
        tone: "error",
        text: t("extensions.plugins.note.timeout"),
      };
    case "failed":
      return {
        tone: "error",
        text: t("extensions.plugins.note.failed"),
      };
    case "cancelled":
      return null;
  }
}

