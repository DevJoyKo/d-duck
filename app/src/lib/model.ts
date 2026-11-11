// Model choice (model-select, D-153·D-154·D-197). The same value rules as `dduck_core::model_select` (shared table
// test), the shapes `model_options` returns, and the one-line texts the pickers and the task detail show.
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import { eul, ro } from "../i18n/josa";

const MODEL_RE = /^[a-z0-9][a-z0-9._:@-]{0,99}(\[1m\])?$/;
const YAML_WORDS = new Set(["null", "true", "false", "yes", "no", "on", "off", "y", "n"]);

/** A value D.Duck may pass as `--model=<v>` (design 2). Mirrors `model_select::valid_model`. */
export function validModel(s: string): boolean {
  if (!MODEL_RE.test(s)) return false;
  const bare = s.endsWith("[1m]") ? s.slice(0, -4) : s;
  return /[a-z]/.test(bare) && !YAML_WORDS.has(bare);
}

/** A value the user may choose for a task, a type or an agent: `default`/`inherit` mean "no model". */
export const validChoice = (s: string) => validModel(s) && s !== "default" && s !== "inherit";

export type ModelSource = "cli" | "settings" | "recent" | "custom";
export type Allow = "yes" | "no" | "unknown";
export type AllowKind = "none" | "managed" | "settings" | "unknown";
export type Replacement = { kind: "alias"; to: string } | { kind: "default" } | { kind: "none" };
export type ModelLayer = "task" | "agent" | "type" | "cli";

export interface ModelItem {
  value: string;
  sources: ModelSource[];
  resolved?: { id: string; basis: "settings" | "recent" };
  allow: Allow;
  blockedBy?: AllowKind;
  note?: string;
  price?: number;
  /** Only for a stored value that is no longer supported (design 11). */
  replacement?: Replacement;
}

export interface ModelOptions {
  items: ModelItem[];
  currentDefault: { value: string | null; basis: "env" | "settings" | "recent" | "account"; price?: number };
  cliRead: boolean;
  dropped: number;
  allowlist: AllowKind;
}

export const modelApi = {
  options: (cwd: string | null, refresh = false) => invoke<ModelOptions>("model_options", { cwd, refresh }),
  saveAgent: (id: string, model: string | null, token: string) => invoke<void>("save_agent_model", { id, project: null, model, token }),
};

export const itemOf = (o: ModelOptions | undefined, v: string | null | undefined) => (v ? o?.items.find((i) => i.value === v) : undefined);

/** The id an item stands for (an alias's known id, else the value). */
export const idOf = (i: ModelItem) => i.resolved?.id ?? i.value;

/** One option row: `opus · claude-opus-5-5 (최근 기록 기준) · CLI·최근 사용`. */
export function itemLabel(i: ModelItem): string {
  const parts = [i.value];
  if (i.resolved) parts.push(i.resolved.basis === "recent" ? t("common.model.resolvedRecent", { id: i.resolved.id }) : i.resolved.id);
  parts.push(i.sources.map((s) => t(`common.model.source.${s}`)).join("·"));
  return parts.join(" · ");
}

/** Why an item can't be chosen, or `null`. */
export function blockedText(i: ModelItem): string | null {
  if (i.allow === "yes") return null;
  if (i.allow === "unknown" || i.blockedBy === "unknown") return t("common.model.blocked.unknown");
  return i.blockedBy === "managed" ? t("common.model.blocked.managed") : t("common.model.blocked.settings");
}

/** "지원되지 않는 모델 · opus로 대체돼요" for a stored unsupported value (agent files included since spike 1.1). */
export function replacementText(r: Replacement | undefined): string | null {
  if (!r) return null;
  if (r.kind === "alias") return t("common.model.unsupportedAlias", { to: r.to, ro: ro(r.to) });
  if (r.kind === "default") return t("common.model.unsupportedDefault");
  return t("common.model.unsupportedNone");
}

/** "기본값보다 토큰당 약 N배 비싸요" when the chosen output price is ≥ 1.2× the default's (design 6). */
export function costText(chosen: number | undefined, base: number | undefined): string | null {
  if (chosen == null || base == null || base <= 0) return null;
  const r = chosen / base;
  return r >= 1.2 ? t("common.model.cost", { n: (Math.round(r * 10) / 10).toFixed(1) }) : null;
}

/** A strict or sensitive task on a cheaper model than the default (design 4). */
export const shallowHint = (strict: boolean, chosen: number | undefined, base: number | undefined) => strict && chosen != null && base != null && chosen < base;

/** 4.6 ids have no xhigh (design 8): one id-prefix rule only. */
export const effortHint = (id: string | null | undefined, effort: string | null | undefined) => !!id && /-4-6(\b|$|\[|-|@)/.test(id) && (effort === "xhigh" || effort === "max");

/** The task detail's model lines (design 9·11). */
export interface ModelRecord {
  modelLayer?: string | null;
  modelUsed?: string | null;
  modelRequested?: string | null;
  modelOther?: string | null;
}

export function recordLines(r: ModelRecord | null | undefined): string[] {
  if (!r?.modelLayer) return [t("common.model.noRecord")];
  const layer = t(`common.model.layer.${r.modelLayer as ModelLayer}`);
  const out = [r.modelUsed ? t("common.model.line", { value: r.modelUsed, layer }) : t("common.model.lineDefault", { layer })];
  if (r.modelRequested) {
    out.push(
      r.modelUsed
        ? t("common.model.fellBack", { requested: r.modelRequested, eul: eul(r.modelRequested), used: r.modelUsed, ro: ro(r.modelUsed) })
        : t("common.model.fellBackDefault", { requested: r.modelRequested, eul: eul(r.modelRequested) }),
    );
  }
  if (r.modelOther && r.modelUsed) out.push(t("common.model.ranOther", { requested: r.modelUsed, actual: r.modelOther, ro: ro(r.modelOther) }));
  return out;
}

/** "다시 맡기기" (design 10): the task's model fills the composer only when it was chosen for that task. */
export function reworkModel(r: ModelRecord | null | undefined): { model?: string; originalModel?: string } {
  if (!r?.modelUsed) return {};
  return r.modelLayer === "task" ? { model: r.modelUsed } : { originalModel: r.modelUsed };
}
