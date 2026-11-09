import type { Item } from "../lib/api";
import { compact, pct, usd } from "../lib/format";
import { t } from "../i18n";
import { useT } from "../i18n/react";

/** Cost cell: full price, partial price (some models unpriced), or "단가 미등록" (usage-analytics: 비용 추정). */
export function costLabel(it: Item): string {
  if (it.priced && it.estCostUsd !== null) return usd(it.estCostUsd);
  if (!it.priced && it.estCostUsd !== null && it.estCostUsd > 0) return t("usage.dist.partlyUnpriced", { cost: usd(it.estCostUsd) });
  return t("usage.dist.unpriced");
}

/** The backend's catch-all row key (not user data, so its label is translated). */
const OTHER_BUCKET = "기타";

/** Ranked rows with a thin magnitude bar (one hue); skills also show the tool/slash split. */
export function DistributionList({ items, showVia = false, showCost = false, prefix = "" }: { items: Item[]; showVia?: boolean; showCost?: boolean; prefix?: string }) {
  const t = useT();
  if (items.length === 0) return <p className="py-2 text-[13px] text-ink-3">{t("usage.dist.empty")}</p>;
  const max = Math.max(...items.map((i) => i.count), 1);
  return (
    <ul className="space-y-2.5">
      {items.map((it) => {
        const label = it.key === OTHER_BUCKET ? t("usage.dist.otherBucket") : prefix + it.key;
        return (
        <li key={it.key}>
          <div className="flex items-baseline gap-2 text-[13px]">
            <span className="min-w-0 flex-1 truncate" title={label}>
              {label}
            </span>
            {showVia && (
              <span className="text-xs text-ink-3" data-testid="via">
                {t("usage.dist.via", { tool: it.viaTool, slash: it.viaSlash })}
              </span>
            )}
            {showCost && <span className="text-xs text-ink-3">{costLabel(it)}</span>}
            <span className="w-12 text-right font-medium">{compact(it.count)}</span>
            <span className="w-12 text-right text-xs text-ink-3">{pct(it.share)}</span>
          </div>
          <div className="mt-1 h-1 rounded-full bg-line">
            <div className="h-1 rounded-full bg-ink-2" style={{ width: `${Math.max(2, (it.count / max) * 100)}%` }} />
          </div>
        </li>
        );
      })}
    </ul>
  );
}
