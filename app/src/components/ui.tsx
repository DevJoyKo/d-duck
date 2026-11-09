// Small UI primitives shared by the pages. Kept deliberately few: a page header, a sheet, a section title,
// buttons, a segmented control, a select and an empty state.
import type { ButtonHTMLAttributes, ReactNode, SelectHTMLAttributes } from "react";
import { useT } from "../i18n/react";

/** `summary`: one quiet line of counts beside the title (no names or paths; not live, so it is not re-announced). */
export function PageHeader({ title, summary, children }: { title: string; summary?: string; children?: ReactNode }) {
  return (
    <header data-tauri-drag-region="deep" className="sticky top-0 z-10 flex min-h-14 flex-wrap items-center gap-3 border-b border-line bg-paper/90 px-8 py-2 backdrop-blur">
      <div className="mr-auto flex min-w-0 items-baseline gap-3">
        <h1 className="shrink-0 text-[20px] font-semibold tracking-tight">{title}</h1>
        {summary && (
          <p className="min-w-0 truncate text-[13px] text-ink-3" title={summary} data-page-summary>
            {summary}
          </p>
        )}
      </div>
      {children}
    </header>
  );
}

/** Page body under the header. The four tabs with a scope bar share it so the bar sits at the same spot (D-132). */
export function PageBody({ children }: { children: ReactNode }) {
  return <div className="mx-auto max-w-6xl space-y-4 px-8 py-6">{children}</div>;
}

/** One-line copy rule: the rest of an explanation folds under "자세히". */
export function MoreInfo({ children, className = "" }: { children: ReactNode; className?: string }) {
  const t = useT();
  return (
    <details className={`group text-xs text-ink-3 ${className}`}>
      <summary className="inline-flex cursor-pointer list-none items-center gap-1 rounded text-ink-3 select-none hover:text-ink-2 [&::-webkit-details-marker]:hidden">
        {t("common.actions.more")}<span aria-hidden className="transition-transform group-open:rotate-90">›</span>
      </summary>
      <p className="mt-1">{children}</p>
    </details>
  );
}

export function Sheet({ children, className = "" }: { children: ReactNode; className?: string }) {
  // Elevation, not borders (palette-refresh, D-176): e1 carries a 0.5px hairline plus a soft shadow.
  return <div className={`rounded-xl bg-sheet shadow-e1 ${className}`}>{children}</div>;
}

export function SectionTitle({ children, aside }: { children: ReactNode; aside?: ReactNode }) {
  return (
    <div className="mb-3 flex items-baseline gap-3">
      <h2 className="text-[13px] font-semibold text-ink">{children}</h2>
      {aside && <span className="text-xs text-ink-3">{aside}</span>}
    </div>
  );
}

/** `quiet` is the older name of `secondary` and stays as an alias; `plain` is text only, for low-weight actions. */
type Variant = "primary" | "secondary" | "quiet" | "plain" | "danger";

// Secondary is a flat ink tint, so buttons never compete with the elevated cards they sit on (A, D-176).
const SECONDARY = "bg-ink/6 text-ink hover:bg-ink/10";

/**
 * Height, padding and type size live here, not in `className` (palette-refresh 3.2): a second `h-*`, `px-*` or `text-*`
 * in className competes with these classes by stylesheet order, not by position, so the override silently lost.
 * md 32px/13px (default) · sm 28px/12px (rows) · compact 28px/12px with 8px sides (pagers, the board card's action
 * row, where two limit buttons share a 186px line) · xs 24px/12px (inline) · icon 32px pager · field 38px beside a field. className is for layout only (margins, flex, width).
 */
export type ButtonSize = "md" | "sm" | "xs" | "icon" | "compact" | "field";
const SIZES: Record<ButtonSize, string> = {
  md: "h-8 px-3 text-[13px]",
  sm: "h-7 px-2.5 text-[12px]",
  xs: "h-6 px-2 text-[12px]",
  icon: "h-8 px-2 text-[13px]",
  compact: "h-7 px-2 text-[12px]",
  field: "h-[38px] px-3 text-[13px]",
};

export function Button({ variant = "secondary", size = "md", className = "", ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; size?: ButtonSize }) {
  const styles: Record<Variant, string> = {
    primary: "bg-ink text-paper hover:opacity-90",
    secondary: SECONDARY,
    quiet: SECONDARY,
    plain: "text-ink-2 hover:bg-ink/6 hover:text-ink",
    danger: "text-danger ring-1 ring-danger/40 ring-inset hover:bg-danger/10",
  };
  return (
    <button
      {...props}
      className={`inline-flex shrink-0 items-center gap-1.5 rounded-lg font-medium whitespace-nowrap transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${SIZES[size]} ${styles[variant]} ${className}`}
    />
  );
}

export function Segmented<T extends string>({ value, options, onChange, label }: { value: T; options: { id: T; label: string }[]; onChange: (v: T) => void; label: string }) {
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className="inline-flex rounded-lg bg-ink/6 p-0.5"
      onKeyDown={(e) => {
        // Arrow keys move the selection, as in a native radio group.
        if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
        e.preventDefault();
        const i = options.findIndex((o) => o.id === value);
        const next = options[(i + (e.key === "ArrowRight" ? 1 : options.length - 1)) % options.length];
        onChange(next.id);
        (e.currentTarget.querySelector(`[data-id="${next.id}"]`) as HTMLElement | null)?.focus();
      }}
    >
      {options.map((o) => (
        <button
          key={o.id}
          data-id={o.id}
          role="radio"
          aria-checked={value === o.id}
          tabIndex={value === o.id ? 0 : -1}
          onClick={() => onChange(o.id)}
          className={`h-7 rounded-md px-2.5 text-[13px] transition-colors ${value === o.id ? "bg-sheet font-medium text-ink shadow-e1" : "text-ink-2 hover:text-ink"}`}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Select({ label, className = "", children, ...props }: SelectHTMLAttributes<HTMLSelectElement> & { label: string }) {
  return (
    <label className="inline-flex items-center gap-2 text-[13px] text-ink-2">
      {label}
      <select {...props} className={`h-8 max-w-44 rounded-lg border border-line bg-sheet px-2 text-ink ${className}`}>
        {children}
      </select>
    </label>
  );
}

export function Empty({ title, children, icon }: { title: string; children?: ReactNode; icon?: ReactNode }) {
  return (
    <div className="py-10 text-center">
      {icon && <div className="mb-2 flex justify-center">{icon}</div>}
      <p className="text-[13px] font-medium text-ink-2">{title}</p>
      {children && <div className="mt-1 text-xs text-ink-3">{children}</div>}
    </div>
  );
}

export const fieldClass = "w-full rounded-lg border border-line bg-sheet px-3 py-2 text-[13px] text-ink placeholder:text-ink-3";

export function Switch({ on, label, disabled, onClick, title }: { on: boolean; label: string; disabled?: boolean; onClick: () => void; title?: string }) {
  return (
    <button
      role="switch"
      aria-checked={on}
      aria-label={label}
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={`relative h-5 w-9 shrink-0 rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${on ? "bg-ok" : "bg-ink-3/40"}`}
    >
      <span className={`absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-sheet shadow transition-transform ${on ? "translate-x-4" : ""}`} />
    </button>
  );
}

/** `pill`: rounded status badge, for rows that also show square chips (rules scope patterns). */
export function Badge({ tone = "plain", children, title, pill }: { tone?: "plain" | "warn" | "danger" | "ok"; children: ReactNode; title?: string; pill?: boolean }) {
  const styles = {
    plain: "border-line text-ink-2",
    warn: "border-transparent bg-warn-soft text-warn",
    danger: "border-danger/40 text-danger",
    ok: "border-ok/40 text-ok",
  };
  return (
    <span title={title} className={`inline-flex h-5 items-center border text-[11px] font-medium whitespace-nowrap ${pill ? "gap-1 rounded-full px-2" : "rounded-md px-1.5"} ${styles[tone]}`}>
      {children}
    </span>
  );
}
