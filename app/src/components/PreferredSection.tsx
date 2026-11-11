// "먼저 살펴볼 스킬·에이전트" in the team rules tab (preferred-skills, D-198): pick installed plugins, skills and agents.
// Prompt guidance only; the backend checks every item again and puts only installed ones into the start line.
// `defaults` is the bundled list (preferred-default: the mattpocock-skills plugin); an empty list is a choice of its own.
import { Plus, RotateCcw, Trash2, X } from "lucide-react";
import { useState } from "react";
import { useT } from "../i18n/react";
import { addable, folderName, installedOf, itemKey, KINDS, MAX_PREFERRED, toItem, usePreferredOptions, type PreferredItem } from "../lib/preferred";
import { Button, fieldClass } from "./ui";

export function PreferredSection({ value, defaults, onChange }: { value: PreferredItem[]; defaults: PreferredItem[]; onChange: (v: PreferredItem[]) => void }) {
  const tr = useT();
  const opts = usePreferredOptions();
  const [query, setQuery] = useState("");
  const [pick, setPick] = useState("");
  const full = value.length >= MAX_PREFERRED;
  const isDefault = JSON.stringify(value.map(itemKey)) === JSON.stringify(defaults.map(itemKey));
  const choices = addable(opts.data ?? [], value, query);
  const chosen = choices.find((o) => itemKey(o) === pick);
  const add = () => {
    if (!chosen || full) return;
    onChange([...value, toItem(chosen)]);
    setPick("");
    setQuery("");
  };
  return (
    <fieldset className="space-y-2 rounded-lg border border-line p-3" data-preferred>
      <legend className="px-1 text-xs font-medium text-ink-2">{tr("agents.preferred.title")}</legend>
      <p className="text-[11px] text-ink-3">{tr("agents.preferred.intro")}</p>
      <p className="text-[11px] text-ink-2">{tr("agents.preferred.honest")}</p>
      {value.length === 0 ? (
        <p className="text-[12px] text-ink-3">{tr("agents.preferred.empty")}</p>
      ) : (
        <ul className="space-y-1.5" aria-label={tr("agents.preferred.title")}>
          {value.map((item, i) => {
            const o = installedOf(item, opts.data);
            const folder = item.source === "project" ? folderName(item.scope) : null;
            return (
              <li key={itemKey(item)} className="flex items-start gap-2 rounded-md border border-line bg-sheet px-2.5 py-1.5">
                <span className="mt-0.5 min-w-[52px] shrink-0 rounded border border-line px-1.5 text-center text-[11px] text-ink-2">{tr(`agents.preferred.kind.${item.kind}`)}</span>
                <div className="min-w-0 flex-1">
                  <p className="flex flex-wrap items-center gap-x-2 text-[13px]">
                    <span className="font-medium break-all">{item.name}</span>
                    {folder && <span className="text-[11px] text-ink-3">{folder}</span>}
                    {o === null && <span className="text-[11px] text-ink-2">{tr("agents.preferred.notInstalled")}</span>}
                  </p>
                  {o && (o.kind === "plugin" ? <p className="truncate text-[11px] text-ink-3">{tr("agents.preferred.pluginMembers", { n: o.members.length })}</p> : o.description && <p className="truncate text-[11px] text-ink-3" title={o.description}>{o.description}</p>)}
                </div>
                <button type="button" aria-label={tr("agents.preferred.remove", { name: item.name })} onClick={() => onChange(value.filter((_, j) => j !== i))} className="rounded p-1 text-ink-3 hover:bg-danger/10 hover:text-danger">
                  <X size={13} />
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {opts.isPending ? (
        <p className="text-[11px] text-ink-3">{tr("agents.preferred.loading")}</p>
      ) : opts.isError ? (
        <p className="flex items-center gap-2 text-[11px] text-danger">
          {tr("agents.preferred.readFailed")}
          <Button size="sm" onClick={() => opts.refetch()}>
            {tr("agents.preferred.retry")}
          </Button>
        </p>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          <input type="search" aria-label={tr("agents.preferred.find")} placeholder={tr("agents.preferred.find")} value={query} onChange={(e) => setQuery(e.target.value)} className={`${fieldClass} w-auto min-w-[140px] flex-1 sm:max-w-[200px]`} />
          <select aria-label={tr("agents.preferred.pick")} value={chosen ? pick : ""} onChange={(e) => setPick(e.target.value)} disabled={full || choices.length === 0} className={`${fieldClass} w-auto min-w-[180px] flex-[2]`}>
            <option value="">{choices.length === 0 ? tr("agents.preferred.noMatch") : tr("agents.preferred.pickPlaceholder")}</option>
            {KINDS.map((k) => {
              const list = choices.filter((o) => o.kind === k);
              return (
                list.length > 0 && (
                  <optgroup key={k} label={tr(`agents.preferred.kind.${k}`)}>
                    {list.map((o) => (
                      <option key={itemKey(o)} value={itemKey(o)}>
                        {o.source === "project" ? `${o.name} · ${folderName(o.scope)}` : o.name}
                      </option>
                    ))}
                  </optgroup>
                )
              );
            })}
          </select>
          <Button onClick={add} disabled={!chosen || full}>
            <Plus size={14} /> {tr("agents.preferred.add")}
          </Button>
        </div>
      )}
      <div className="flex flex-wrap items-center gap-2 text-[11px] text-ink-3">
        <span className="tabular-nums">{tr("agents.preferred.count", { n: value.length, max: MAX_PREFERRED })}</span>
        {full && <span role="status">{tr("agents.preferred.max", { max: MAX_PREFERRED })}</span>}
        <span>{tr("agents.preferred.projectNote")}</span>
        {(!isDefault || value.length > 0) && (
          <span className="ml-auto flex items-center gap-1">
            {!isDefault && (
              <button type="button" className="flex items-center gap-1 rounded px-1.5 py-1 text-ink-3 hover:bg-paper hover:text-ink" onClick={() => onChange(defaults.map((d) => ({ ...d })))}>
                <RotateCcw size={12} aria-hidden /> {tr("agents.preferred.toDefault")}
              </button>
            )}
            {value.length > 0 && (
              <button type="button" className="flex items-center gap-1 rounded px-1.5 py-1 text-ink-3 hover:bg-paper hover:text-ink" onClick={() => onChange([])}>
                <Trash2 size={12} aria-hidden /> {tr("agents.preferred.clear")}
              </button>
            )}
          </span>
        )}
      </div>
    </fieldset>
  );
}
