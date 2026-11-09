// Live workflow meta check (workflow-meta-check, D-183): 400 ms after the last edit the webview asks Rust
// (`check_workflow_meta`, a pure command) and shows the same verdict the save path will give. No TS-side parsing,
// no eval or new Function.
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../i18n";
import { ro } from "../i18n/josa";

export type MetaReason =
  | "notFirstStatement"
  | "parseError"
  | "notLiteral"
  | "templateInterpolation"
  | "unsupportedUnary"
  | "sparseArray"
  | "reservedKey"
  | "unsupportedKey"
  | "missingName"
  | "emptyName"
  | "missingDescription"
  | "emptyDescription"
  | "nameMismatch"
  | "tooComplex";

export interface MetaCheck {
  ok: boolean;
  reason: MetaReason | null;
  line: number | null;
  blankDescription: boolean;
  /** `nameMismatch` only: the meta.name Claude Code registers (display-cleaned by Rust). */
  registeredName?: string;
}

export const CHECK_DELAY_MS = 400;

export const checkWorkflowMeta = (text: string, name: string) => invoke<MetaCheck>("check_workflow_meta", { text, name });

/** The reason and how to fix it as one line in the current language; `name` fills the name-mismatch line. */
export function metaReasonText(reason: MetaReason, name: string): string {
  return reason === "nameMismatch" ? t("extensions.wfcheck.nameMismatch", { name, ro: ro(name) }) : t(`extensions.wfcheck.reason.${reason}`);
}

/** "Claude Code registers it as <name>" with the right particle. */
export const registeredText = (name: string) => t("extensions.wfcheck.registered", { name, ro: ro(name) });

/**
 * What the verdict means for saving. Creating needs meta.name to match the name field; saving an existing file only
 * warns which name Claude Code registers (design 8), so a name mismatch does not block it there.
 */
export function metaBlocks(check: MetaCheck | null, existing: boolean): boolean {
  if (!check || check.ok) return false;
  return !(existing && check.reason === "nameMismatch");
}

/**
 * The latest verdict for `text`/`name`, or null while disabled or before the first answer. `current` is false while
 * a newer edit waits for its answer (the old verdict stays on screen meanwhile).
 */
export function useWorkflowCheck(text: string, name: string, enabled: boolean): { check: MetaCheck | null; current: boolean } {
  const [state, setState] = useState<{ key: string; check: MetaCheck } | null>(null);
  const key = `${name}\u0000${text}`;
  useEffect(() => {
    if (!enabled) return;
    let live = true;
    const id = setTimeout(() => {
      checkWorkflowMeta(text, name).then(
        (check) => live && setState({ key, check }),
        () => live && setState(null),
      );
    }, CHECK_DELAY_MS);
    return () => {
      live = false;
      clearTimeout(id);
    };
  }, [enabled, text, name, key]);
  if (!enabled) return { check: null, current: false };
  return { check: state?.check ?? null, current: state?.key === key };
}
