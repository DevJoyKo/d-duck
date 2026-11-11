// One model picker for the composer, the type detail and the agent detail (model-select design 7). A native select
// (keyboard and screen readers for free) plus "직접 입력". Every note under it is one line.
import { RefreshCw } from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useId, useState, type ReactNode } from "react";
import { useT } from "../i18n/react";
import { blockedText, costText, idOf, itemLabel, itemOf, modelApi, replacementText, validChoice, type ModelOptions } from "../lib/model";
import { fieldClass } from "./ui";

const CUSTOM = "__custom__";

export function defaultLabel(o: ModelOptions | undefined, t: ReturnType<typeof useT>): string {
  const d = o?.currentDefault;
  if (!d?.value) return t("common.model.defaultAccount");
  return t("common.model.defaultNow", { value: d.value, basis: t(`common.model.basis.${d.basis}`) });
}

export function ModelPicker({
  value,
  onChange,
  options,
  onRefresh,
  refreshing,
  defaultText,
  basePrice,
  disabled,
  label,
  children,
}: {
  value: string | null;
  onChange: (v: string | null) => void;
  options: ModelOptions | undefined;
  onRefresh?: () => void;
  refreshing?: boolean;
  /** First row: "기본값 (지금: … · 출처)". */
  defaultText: string;
  /** Output price of "the default here", for the cost line (design 6). */
  basePrice?: number;
  disabled?: boolean;
  label?: string;
  /** Extra one-line notes from the caller. */
  children?: ReactNode;
}) {
  const t = useT();
  const id = useId();
  const [typing, setTyping] = useState(false);
  const [typed, setTyped] = useState("");
  const item = itemOf(options, value);
  const listed = !!item;
  const typedBad = typed.trim() !== "" && !validChoice(typed.trim());
  // Warnings (blocked, replaced) read a step darker than plain notes; no yellow (one per screen).
  const lines: { text: string; warn?: boolean; title?: string }[] = [
    { text: item ? (blockedText(item) ?? "") : "", warn: true },
    { text: item && item.allow === "yes" ? (replacementText(item.replacement) ?? "") : "", warn: true },
    { text: item && !item.replacement ? (costText(item.price, basePrice) ?? "") : "", title: t("common.model.costBasis") },
    { text: item?.note ?? "" },
    { text: options && !options.cliRead ? t("common.model.cliUnread") : "" },
    { text: options?.allowlist === "unknown" ? t("common.model.managedUnread") : "", warn: true },
  ].filter((l) => !!l.text);
  return (
    <div className="space-y-1" data-model-picker>
      <div className="flex items-center justify-between">
        <label htmlFor={id} className="text-xs font-medium text-ink-2">
          {label ?? t("common.model.label")}
        </label>
        {onRefresh && (
          <button type="button" onClick={onRefresh} disabled={refreshing} className="flex items-center gap-1 text-[11px] text-ink-3 hover:text-ink-2 disabled:opacity-50">
            <RefreshCw size={11} aria-hidden className={refreshing ? "animate-spin" : ""} /> {t("common.model.refresh")}
          </button>
        )}
      </div>
      <select
        id={id}
        value={typing ? CUSTOM : (value ?? "")}
        disabled={disabled}
        onChange={(e) => {
          const v = e.target.value;
          if (v === CUSTOM) {
            setTyping(true);
            setTyped("");
            return;
          }
          setTyping(false);
          onChange(v || null);
        }}
        className={`${fieldClass} h-[38px]`}
      >
        <option value="">{defaultText}</option>
        {value && !listed && <option value={value}>{value}</option>}
        {options?.items.map((i) => {
          const why = blockedText(i);
          return (
            <option key={i.value} value={i.value} disabled={!!why}>
              {itemLabel(i)}
              {why ? ` — ${why}` : ""}
            </option>
          );
        })}
        <option value={CUSTOM} disabled={options?.allowlist === "unknown"}>
          {t("common.model.custom")}
        </option>
      </select>
      {typing && (
        <div className="flex gap-2">
          <input
            aria-label={t("common.model.customLabel")}
            value={typed}
            autoFocus
            onChange={(e) => setTyped(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.stopPropagation();
                setTyping(false);
              }
              if (e.key === "Enter") {
                e.preventDefault();
                if (validChoice(typed.trim())) {
                  onChange(typed.trim());
                  setTyping(false);
                }
              }
            }}
            placeholder="claude-opus-4-6"
            className={`${fieldClass} font-mono`}
          />
          <button
            type="button"
            disabled={!validChoice(typed.trim())}
            onClick={() => {
              onChange(typed.trim());
              setTyping(false);
            }}
            className="rounded-lg border border-line px-3 text-[13px] disabled:opacity-50"
          >
            {t("common.model.customApply")}
          </button>
        </div>
      )}
      {typing && typedBad && <p className="text-xs text-danger">{t("common.model.badFormat")}</p>}
      {lines.map((l) => (
        <p key={l.text} title={l.title} className={`text-xs ${l.warn ? "text-ink-2" : "text-ink-3"}`}>
          {l.text}
        </p>
      ))}
      {children}
    </div>
  );
}

/** Output price of what an item or the CLI default stands for. */
export const priceOf = (o: ModelOptions | undefined, v: string | null | undefined): number | undefined => {
  if (!v) return o?.currentDefault.price;
  const i = itemOf(o, v);
  return i?.price;
};

export { idOf };

/** The model list for a folder, cached for the session (D-138 pattern); "목록 새로 고침" re-runs the CLI help. */
export function useModelOptions(cwd: string | null) {
  const qc = useQueryClient();
  const key = ["model-options", cwd ?? ""];
  const q = useQuery({ queryKey: key, queryFn: () => modelApi.options(cwd), staleTime: 60_000, gcTime: Infinity, retry: false });
  const [refreshing, setRefreshing] = useState(false);
  const refresh = async () => {
    setRefreshing(true);
    try {
      qc.setQueryData(key, await modelApi.options(cwd, true));
    } finally {
      setRefreshing(false);
    }
  };
  return { data: q.data, refresh, refreshing };
}
