// Edit one skill (SKILL.md), command (.md) or workflow (.js). Rust decides read-only (plugin, claude.ai sync, links),
// refuses a save when the file changed meanwhile, and confirms natively when the text can run commands.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Lock, X } from "lucide-react";
import { useCallback, useRef, useState } from "react";
import { useT } from "../i18n/react";
import { addApi, kindLabel } from "../lib/addItems";
import { discardText } from "../lib/rules";
import { metaBlocks, metaReasonText, registeredText, useWorkflowCheck, type MetaCheck } from "../lib/workflowCheck";
import { useModal } from "../lib/useModal";
import { useDirty } from "./ProjectContext";
import { Button, fieldClass } from "./ui";

/** `onDelete` (D-147) is given only for the user's own items; the parent runs the delete and closes the editor. */
export function AuthoringEditor({
  id,
  onClose,
  onDelete,
  deleting = false,
  deleteError = null,
}: {
  id: string;
  onClose: () => void;
  onDelete?: (unsaved: boolean) => void;
  deleting?: boolean;
  deleteError?: string | null;
}) {
  const t = useT();
  const qc = useQueryClient();
  const item = useQuery({ queryKey: ["authoring-item", id], queryFn: () => addApi.read(id), staleTime: 0 });
  const [text, setText] = useState<string | null>(null);
  const d = item.data;
  const value = text ?? d?.content ?? "";
  const dirty = d != null && text != null && text !== d.content;
  // Workflows: the same Rust verdict as the save path, 400 ms after the last edit (D-183).
  const wf = useWorkflowCheck(value, d?.name ?? "", d?.kind === "workflow" && !d.readonly);
  // An existing file may keep a different meta.name: saved, with the name Claude Code registers (design 8).
  const blocked = metaBlocks(wf.check, true);
  // Unsaved text guards the project switch too, and the backdrop, Esc and X ask before throwing it away.
  useDirty(`authoring:${id}`, dirty);
  const close = useCallback(() => {
    if (!dirty || window.confirm(discardText())) onClose();
  }, [dirty, onClose]);
  // Focus in, Tab inside, Escape through `close`, focus back to the row (sheet-focus).
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeBtn = useRef<HTMLButtonElement>(null);
  useModal(dialogRef, close, { initial: closeBtn });
  const save = useMutation({
    mutationFn: () => (d ? addApi.save(id, value, d.token) : Promise.reject()),
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: ["authoring-item", id] });
      qc.invalidateQueries({ queryKey: ["skills"] });
      setText(null);
    },
  });
  const title = d ? (d.kind === "command" ? `/${d.name}` : d.name) : t("extensions.editor.loading");
  return (
    <div className="fixed inset-0 z-30 flex justify-end bg-ink/20" onClick={close}>
      <div ref={dialogRef} role="dialog" aria-modal="true" aria-label={t("extensions.editor.label", { title })} onClick={(e) => e.stopPropagation()} className="flex h-full w-[min(760px,100vw)] flex-col border-l border-line bg-sheet shadow-e3">
        <div className="flex items-center gap-3 border-b border-line px-5 py-3">
          <div className="mr-auto min-w-0">
            <p className="text-xs text-ink-3">
              {d ? `${kindLabel(d.kind)} · ${d.location}` : ""}
            </p>
            <h2 className="truncate font-mono text-[15px] font-semibold">{title}</h2>
          </div>
          <button ref={closeBtn} aria-label={t("common.actions.close")} onClick={close} className="rounded-md p-1 text-ink-3 hover:text-ink">
            <X size={16} />
          </button>
        </div>
        <div className="min-h-0 flex-1 space-y-3 overflow-auto px-5 py-4">
          {item.isError && <p className="text-[13px] text-danger">{String(item.error)}</p>}
          {d?.readonly && (
            <p className="flex items-center gap-1.5 rounded-lg border border-line bg-paper px-3 py-2 text-xs text-ink-2">
              <Lock size={12} /> {t("extensions.editor.readonly", { reason: d.readonly })}
            </p>
          )}
          {d && d.warnings.length > 0 && (
            <ul className="list-disc space-y-0.5 rounded-lg border border-warn/30 bg-warn-soft py-2 pr-3 pl-7 text-xs text-ink">
              {d.warnings.map((w) => (
                <li key={w} className={w.startsWith("hooks") ? "font-medium text-danger" : undefined}>
                  {w}
                </li>
              ))}
            </ul>
          )}
          {d && (
            <textarea
              aria-label={t("extensions.editor.content")}
              value={value}
              readOnly={!!d.readonly}
              onChange={(e) => setText(e.target.value)}
              spellCheck={false}
              rows={24}
              className={`${fieldClass} font-mono text-[12px] leading-relaxed ${d.readonly ? "bg-paper text-ink-2" : ""}`}
            />
          )}
          {d?.kind === "workflow" && !d.readonly && <WorkflowCheckNote check={wf.check} name={d.name} existing />}
          {d && d.extraFiles.length > 0 && <p className="text-xs text-ink-3">{t("extensions.editor.extraFiles", { files: d.extraFiles.join(", ") })}</p>}
          {save.isError && <p className="text-[13px] text-danger">{String(save.error)}</p>}
          {deleteError && (
            <p role="alert" className="text-[13px] text-danger">
              {deleteError}
            </p>
          )}
          {save.isSuccess && !dirty && <p className="text-[13px] text-ok">{t("extensions.editor.saved")}</p>}
        </div>
        {d && !d.readonly && (
          <div className="flex items-center gap-2 border-t border-line px-5 py-3">
            {onDelete && (
              <Button variant="danger" disabled={deleting || save.isPending} onClick={() => onDelete(dirty)} title={t("extensions.row.deleteTitle")}>
                {deleting ? t("extensions.row.deleting") : t("extensions.row.delete")}
              </Button>
            )}
            <span className="mr-auto text-xs text-ink-3">{dirty ? t("extensions.editor.dirty") : t("extensions.editor.clean")}</span>
            <Button disabled={!dirty || save.isPending} onClick={() => setText(null)}>
              {t("extensions.editor.revert")}
            </Button>
            <Button variant="primary" disabled={!dirty || blocked || save.isPending} onClick={() => save.mutate()}>
              {save.isPending ? t("extensions.editor.saving") : t("common.actions.save")}
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}

/** The live meta verdict under a workflow's text: the reason, how to fix it (and the line) when Claude Code would
 * skip the file, and a standing note that only meta is checked. For an `existing` file a different meta.name is a
 * warning naming what Claude Code registers, not a refusal. */
export function WorkflowCheckNote({ check, name, existing = false }: { check: MetaCheck | null; name: string; existing?: boolean }) {
  const t = useT();
  const renamed = existing && check?.reason === "nameMismatch";
  return (
    <div className="space-y-0.5 text-xs" aria-live="polite">
      {renamed && <p className="text-ink-2">{check.registeredName ? registeredText(check.registeredName) : t("extensions.wfcheck.registeredMeta")}</p>}
      {check && !check.ok && check.reason && !renamed && (
        <p className="text-danger">
          {metaReasonText(check.reason, name)}
          {check.line != null && check.reason !== "nameMismatch" ? ` · ${t("extensions.wfcheck.line", { line: check.line })}` : ""}
        </p>
      )}
      {check?.ok && <p className={check.blankDescription ? "text-ink-2" : "text-ok"}>{check.blankDescription ? t("extensions.wfcheck.blankDescription") : t("extensions.wfcheck.ok")}</p>}
      <p className="text-ink-3">{t("extensions.wfcheck.bodyNote")}</p>
    </div>
  );
}
