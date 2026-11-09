// The one-line count next to the Tasks title (palette-refresh 2.3): numbers only, never a task name or path.
import { t } from "../i18n";

export function boardSummary({ waiting, review }: { waiting: number; review: number }): string {
  return [waiting > 0 && t("tasks.summary.waiting", { count: waiting }), review > 0 && t("tasks.summary.review", { count: review })].filter(Boolean).join(", ");
}
