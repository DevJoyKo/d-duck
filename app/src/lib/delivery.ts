// 전달 범위 (delivery-guard, D-142·D-143): typed wrappers and the one-line copy. The webview only shows the scope and
// asks for ticks; Rust decides the scope again at every start, and unticking goes through a native confirmation.
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import { localized } from "./format";

export type DeliveryGuard = "draft_pr" | "local_only";

export interface LocalOnlyRow {
  id: string;
  name: string;
  parentDir: string | null;
  checked: boolean;
  missing: boolean;
  worktreeOf: string | null;
  offReason: string | null;
  /** Fixed code of `offReason` (`homeNotAllowed` or a project off code). */
  offCode?: string | null;
}

export interface GuardPreview {
  guard: DeliveryGuard;
  ticked: boolean;
  canTick: boolean;
  forcedWorktree: boolean;
}

export const deliveryApi = {
  localOnlyProjects: () => invoke<LocalOnlyRow[]>("local_only_projects"),
  /** Returns the checked state after the call (unticking may be declined in the native dialog). */
  setLocalOnly: (id: string, on: boolean) => invoke<boolean>("set_local_only", { id, on }),
  setLocalOnlyCwd: (cwd: string) => invoke<void>("set_local_only_cwd", { cwd }),
  preview: (cwd: string) => invoke<GuardPreview>("delivery_guard_preview", { cwd }),
};

export const GUARD_COPY = localized({
  title: "tasks.delivery.title",
  description: "tasks.delivery.description",
  cloud: "tasks.delivery.cloud",
  running: "tasks.delivery.running",
  worktree: "tasks.delivery.worktree",
  missing: "tasks.delivery.missing",
  chip: "tasks.delivery.chip",
  tickHere: "tasks.delivery.tickHere",
  forcedWorktree: "tasks.delivery.forcedWorktree",
  unreadable: "tasks.delivery.unreadable",
});

/** The Composer line before start. */
export function guardLine(g: DeliveryGuard): string {
  return t(g === "local_only" ? "tasks.delivery.lineLocal" : "tasks.delivery.lineDraft");
}

/** Post-check warnings (bits from Rust: 1 = push seen, 2 = default branch moved). */
export function alertLines(bits: number | null | undefined): string[] {
  if (!bits) return [];
  const out: string[] = [];
  if (bits & 1) out.push(t("tasks.delivery.alertPush"));
  if (bits & 2) out.push(t("tasks.delivery.alertBranch"));
  return out;
}

/** The report's 맡긴 내용 chip; null for older tasks without a record. */
export function guardChip(g: DeliveryGuard | null | undefined): string | null {
  if (g === "local_only") return t("tasks.delivery.chip");
  if (g === "draft_pr") return t("tasks.delivery.chipDraft");
  return null;
}
