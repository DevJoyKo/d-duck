// "적용 제안" in the news sheet (news-apply, D-208): the item's steps and how to check, its source address, a target
// project and "내 프로젝트에 적용", which opens a plan-mode draft in the task composer. Nothing starts from here.
import { useQuery } from "@tanstack/react-query";
import { useId, useState } from "react";
import type { NewsItem } from "../lib/api";
import type { Draft } from "../lib/brief";
import { newsTargets } from "../lib/news";
import { projectsApi } from "../lib/projects";
import { draftFromNews } from "../lib/refine";
import { useT } from "../i18n/react";
import { Button, fieldClass } from "./ui";

export function ApplyPart({ item, onDraft }: { item: NewsItem; onDraft?: (d: Draft) => void }) {
  const t = useT();
  const selectId = useId();
  // Same cache as the project bar, so the recent pick matches the other tabs.
  const known = useQuery({ queryKey: ["projects-known"], queryFn: projectsApi.known, staleTime: 30_000 });
  const { targets, fallback } = newsTargets(known.data);
  // `null` = follow the default (the first recent pick); "" = pick in the composer.
  const [picked, setPicked] = useState<string | null>(null);
  const target = picked ?? fallback;
  const steps = item.applySteps ?? [];
  return (
    <section data-news-detail="apply" className="space-y-3 rounded-lg border border-line bg-paper/60 p-3">
      <h3 className="text-[13px] font-semibold text-ink">{t("news.apply.title")}</h3>
      {steps.length > 0 ? (
        <ol className="list-decimal space-y-1 pl-5 text-[13px] leading-relaxed text-ink-2">
          {steps.map((s, i) => (
            <li key={i}>{s}</li>
          ))}
        </ol>
      ) : (
        <p className="text-[13px] text-ink-2">{t("news.apply.noSteps")}</p>
      )}
      {item.verify && <p className="text-[13px] text-ink-2">{t("news.apply.verify", { text: item.verify })}</p>}
      <p className="flex min-w-0 gap-1.5 text-xs text-ink-3">
        <span className="shrink-0">{t("news.apply.sourceLabel")}</span>
        <span className="min-w-0 font-mono break-all select-text">{item.sourceUrl}</span>
      </p>
      <div className="flex flex-wrap items-end gap-2">
        <label htmlFor={selectId} className="min-w-0 flex-1 space-y-1">
          <span className="block text-xs font-medium text-ink-2">{t("news.apply.target")}</span>
          <select id={selectId} value={target} onChange={(e) => setPicked(e.target.value)} className={`${fieldClass} h-[38px]`}>
            <option value="">{t("news.apply.targetNone")}</option>
            {targets.map((p) => (
              <option key={p.path} value={p.path} title={p.path}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <Button variant="primary" size="field" onClick={() => onDraft?.(draftFromNews(item, target))} disabled={!onDraft}>
          {t("news.apply.button")}
        </Button>
      </div>
      <p className="text-xs text-ink-3">{t("news.apply.note")}</p>
    </section>
  );
}
