// "Mac 전용" (windows-support 4.3): shown on Windows next to a control the preview disables because its protection is
// verified on macOS only. The backend refuses the same actions (`MacOnly`), so this is the explanation, not the guard.
import { useT } from "../i18n/react";

export function MacOnlyChip() {
  const t = useT();
  return (
    <span className="inline-flex shrink-0 items-center rounded-md border border-line px-1.5 text-[10px] text-ink-2" data-mac-only>
      {t("common.macOnly.chip")}
    </span>
  );
}

/** The chip plus its one-line reason. */
export function MacOnlyNote({ text }: { text: string }) {
  return (
    <p className="flex flex-wrap items-center gap-1.5 text-xs text-ink-3">
      <MacOnlyChip />
      <span>{text}</span>
    </p>
  );
}
