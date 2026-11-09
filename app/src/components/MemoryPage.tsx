// Memory tab (memory-tab, D-091): Claude Code auto memory per project — view, edit, turn single memories off
// (file kept in `.off/`), the global auto-memory switch, and the CLAUDE.md files that apply (read-only).
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, ExternalLink, FileText, Link2, Lock } from "lucide-react";
import { useRef, useState } from "react";
import { confirmLeave } from "../lib/projects";
import { useDirty, useScope } from "./ProjectContext";
import { ScopeBar } from "./ScopeBar";
import { dateTime } from "../lib/format";
import { t } from "../i18n";
import { tx, useT } from "../i18n/react";
import {
  KINDS,
  memoryKindLabel,
  memoryApi,
  previewIndexLine,
  secretText,
  sizeText,
  type InstructionFile,
  type MemoryDoc,
  type MemoryEntry,
  type MemoryKind,
  type MemoryList,
} from "../lib/memory";
import { RulesSection } from "./RulesPanel";
import { Badge, Button, Empty, PageBody, PageHeader, Segmented, Sheet, Switch, fieldClass } from "./ui";

type Segment = "memory" | "rules" | "instructions";

function kindLabel(k: string | null): string {
  return k && (KINDS as string[]).includes(k) ? memoryKindLabel(k as MemoryKind) : (k ?? t("extensions.memory.noKind"));
}

function errText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

function Warnings({ secrets, instructionLike }: { secrets: MemoryEntry["secrets"]; instructionLike: number[] }) {
  const t = useT();
  if (!secrets.length && !instructionLike.length) return null;
  return (
    <div className="space-y-1 text-xs">
      {secrets.length > 0 && (
        <p className="flex items-start gap-1.5 text-danger">
          <AlertTriangle size={13} className="mt-0.5 shrink-0" />
          <span>{t("extensions.memory.warnings.secrets", { hits: secretText(secrets) })}</span>
        </p>
      )}
      {instructionLike.length > 0 && (
        <p className="flex items-start gap-1.5 text-ink-2">
          <AlertTriangle size={13} className="mt-0.5 shrink-0 text-ink-3" />
          <span>{t("extensions.memory.warnings.instructionLike", { lines: instructionLike.join(", ") })}</span>
        </p>
      )}
    </div>
  );
}

function AutoMemoryBar({ list }: { list?: MemoryList }) {
  const t = useT();
  const qc = useQueryClient();
  const auto = useQuery({ queryKey: ["auto-memory"], queryFn: memoryApi.autoMemory });
  const toggle = useMutation({
    mutationFn: (enabled: boolean) => memoryApi.setAutoMemory(enabled, auto.data?.token ?? ""),
    onSettled: () => qc.invalidateQueries({ queryKey: ["auto-memory"] }),
  });
  const a = auto.data;
  const overrides = list?.projectSettings ?? [];
  const locked = !a || !!a.readonlyReason || toggle.isPending;
  return (
    <Sheet className="flex flex-wrap items-start gap-x-4 gap-y-2 px-4 py-3 ring-1 ring-ink-3/40">
      <div className="min-w-0 flex-1">
        <p className="text-[13px] font-semibold">{t("extensions.memory.auto.title")}</p>
        <p className="mt-0.5 text-xs text-ink-3" title={t("extensions.memory.auto.settingTitle", { path: a?.path ?? "settings.json" })}>
          {t("extensions.memory.auto.description")}
        </p>
        {overrides.map(([file, v]) => (
          <p key={file} className="mt-1 text-xs text-ink-2">
            {t(v ? "extensions.memory.auto.overrideOn" : "extensions.memory.auto.overrideOff", { file })}
          </p>
        ))}
        {a?.customDir && <p className="mt-1 text-xs text-ink-2">autoMemoryDirectory: {a.customDir}</p>}
        {a?.readonlyReason && <p className="mt-1 text-xs text-danger">{a.readonlyReason}</p>}
        {toggle.isError && <p className="mt-1 text-xs text-danger">{errText(toggle.error)}</p>}
      </div>
      <div title={t("extensions.memory.auto.settingTitle", { path: a?.path ?? "settings.json" })} className={locked ? "pointer-events-none opacity-50" : ""}>
        <Segmented
          label={t("extensions.memory.auto.label")}
          value={a?.enabled === false ? "off" : "on"}
          onChange={(v) => {
            if ((v === "on") !== a?.enabled) toggle.mutate(v === "on");
          }}
          options={[
            { id: "on", label: t("extensions.memory.auto.allOn") },
            { id: "off", label: t("extensions.memory.auto.allOff") },
          ]}
        />
      </div>
    </Sheet>
  );
}

function warningText(e: { secrets: MemoryEntry["secrets"]; instructionLike: number[]; indexProblem: string | null }): string | null {
  const parts = [
    e.secrets.length > 0 ? t("extensions.memory.warnings.secretsShort", { hits: secretText(e.secrets) }) : null,
    e.instructionLike.length > 0 ? t("extensions.memory.warnings.instructionShort", { lines: e.instructionLike.join(", ") }) : null,
    e.indexProblem,
  ].filter(Boolean);
  return parts.length ? parts.join("\n") : null;
}

function EntryRow({ e, selected, onSelect, onToggle, busy }: { e: MemoryEntry; selected: boolean; onSelect: () => void; onToggle: () => void; busy: boolean }) {
  const t = useT();
  const warn = warningText(e);
  return (
    <div className={`flex items-start gap-3 px-4 py-3 ${selected ? "bg-paper" : ""}`}>
      <button onClick={onSelect} aria-current={selected ? "true" : undefined} className="min-w-0 flex-1 text-left">
        <span className="flex flex-wrap items-center gap-1.5">
          <span className={`text-[13px] font-medium ${e.enabled ? "text-ink" : "text-ink-3"}`}>{e.name ?? e.file}</span>
          <Badge>{kindLabel(e.kind)}</Badge>
          {e.fresh && <Badge tone="warn" title={t("extensions.memory.entry.freshTitle")}>{t("extensions.memory.entry.fresh")}</Badge>}
          {warn && (
            <span title={warn} aria-label={t("extensions.memory.warnings.label", { text: warn })} className="inline-flex text-danger">
              <AlertTriangle size={13} />
            </span>
          )}
          {e.enabled && e.beyondLimit && <Badge tone="warn">{t("extensions.memory.entry.beyondLimit")}</Badge>}
          {e.enabled && !e.inIndex && <Badge title={t("extensions.memory.entry.notInIndexTitle")}>{t("extensions.memory.entry.notInIndex")}</Badge>}
          {e.readonlyReason && (
            <span title={e.readonlyReason} className="text-ink-3">
              <Lock size={12} />
            </span>
          )}
        </span>
        {e.description && <span className="mt-0.5 line-clamp-2 block text-xs text-ink-2">{e.description}</span>}
        <span className="mt-0.5 block text-[11px] text-ink-3">
          {e.enabled ? e.file : <span title={t("extensions.memory.entry.keptTitle", { file: e.file })}>{t("extensions.memory.entry.notUsed")}</span>} · {t("extensions.memory.entry.modified", { date: dateTime(e.mtimeMs) })}
        </span>
      </button>
      <Switch on={e.enabled} label={t(e.enabled ? "extensions.memory.entry.turnOff" : "extensions.memory.entry.turnOn", { name: e.name ?? e.file })} disabled={busy || !!e.readonlyReason || !!e.indexProblem} onClick={onToggle} />
    </div>
  );
}

function TurnOnPreview({ e, onConfirm, onCancel, busy }: { e: MemoryEntry; onConfirm: () => void; onCancel: () => void; busy: boolean }) {
  const t = useT();
  return (
    <div className="mx-4 mb-3 rounded-lg border border-line bg-paper p-3 text-xs">
      {e.inIndex ? (
        <p className="text-ink-2">{t("extensions.memory.turnOn.alreadyIndexed")}</p>
      ) : (
        <>
          <p className="text-ink-2">{t("extensions.memory.turnOn.willIndex")}</p>
          <p className="mt-0.5 text-ink-3">{t("extensions.memory.turnOn.order")}</p>
          <details className="mt-1">
            <summary className="cursor-pointer text-ink-3">{t("extensions.memory.turnOn.showLine")}</summary>
            <code className="mt-1.5 block rounded-md bg-sheet px-2 py-1.5 font-mono break-all text-ink">{e.onLine}</code>
          </details>
        </>
      )}
      <Warnings secrets={e.secrets} instructionLike={e.instructionLike} />
      <div className="mt-2 flex gap-2">
        <Button variant="primary" disabled={busy} onClick={onConfirm}>
          {t("extensions.memory.turnOn.confirm")}
        </Button>
        <Button onClick={onCancel}>{t("common.actions.cancel")}</Button>
      </div>
    </div>
  );
}

function Editor({ project, entry, indexToken, onGone }: { project: string; entry: MemoryEntry; indexToken: string; onGone: () => void }) {
  const t = useT();
  const qc = useQueryClient();
  const doc = useQuery({ queryKey: ["memory-doc", project, entry.file], queryFn: () => memoryApi.read(project, entry.file), staleTime: Infinity });
  if (doc.isError) return <p className="p-4 text-xs text-danger">{errText(doc.error)}</p>;
  if (!doc.data) return <p className="p-4 text-xs text-ink-3">{t("extensions.memory.editor.loading")}</p>;
  const reload = () => qc.invalidateQueries({ queryKey: ["memory-doc", project, entry.file] });
  return (
    <>
      {entry.token !== doc.data.token && (
        <div className="m-4 mb-0 flex flex-wrap items-center gap-2 rounded-lg bg-warn-soft px-3 py-2 text-xs text-ink">
          <span>{t("extensions.memory.editor.changed")}</span>
          <Button onClick={reload}>{t("extensions.memory.editor.reload")}</Button>
        </div>
      )}
      <EditorForm
        key={doc.data.token}
        project={project}
        doc={doc.data}
        indexToken={indexToken}
        onSaved={() => {
          qc.invalidateQueries({ queryKey: ["memory-entries", project] });
          reload();
        }}
        onReload={reload}
        onGone={onGone}
      />
    </>
  );
}

function EditorForm({ project, doc, indexToken, onSaved, onReload, onGone }: { project: string; doc: MemoryDoc; indexToken: string; onSaved: () => void; onReload: () => void; onGone: () => void }) {
  const t = useT();
  const qc = useQueryClient();
  const [name, setName] = useState(doc.name);
  const [description, setDescription] = useState(doc.description);
  const [kind, setKind] = useState(doc.kind);
  const [body, setBody] = useState(doc.body);
  const [note, setNote] = useState<string | null>(null);
  const dirty = name !== doc.name || description !== doc.description || kind !== doc.kind || body !== doc.body;
  useDirty("memory", dirty);
  const readonly = !!doc.readonlyReason;
  const save = useMutation({
    mutationFn: () => memoryApi.save(project, doc.file, doc.token, indexToken, { name, description, kind, body }),
    onSuccess: (n) => {
      setNote(n ?? t("extensions.memory.editor.saved"));
      onSaved();
    },
  });
  const trash = useMutation({
    mutationFn: () => memoryApi.trash(project, doc.file, doc.token, indexToken),
    onSuccess: (done) => {
      if (done) {
        qc.invalidateQueries({ queryKey: ["memory-entries", project] });
        qc.invalidateQueries({ queryKey: ["memory-projects"] });
        onGone();
      }
    },
  });
  const preview = doc.enabled ? previewIndexLine(doc.indexLine, description) : null;
  const err = save.error ?? trash.error;
  return (
    <div className="space-y-3 p-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <FileText size={14} className="self-center text-ink-3" />
        <span className="font-mono text-xs break-all text-ink-2">{doc.enabled ? doc.file : `.off/${doc.file}`}</span>
        {!doc.enabled && <Badge title={t("extensions.memory.entry.keptTitle", { file: doc.file })}>{t("extensions.memory.entry.notUsed")}</Badge>}
      </div>
      {doc.readonlyReason && <p className="text-xs text-danger">{doc.readonlyReason}</p>}
      {doc.indexProblem && <p className="text-xs text-danger">{doc.indexProblem}</p>}
      {!doc.fieldsEditable && !readonly && (
        <p className="text-xs text-ink-3">{doc.hasFrontmatter ? t("extensions.memory.editor.fmUnparsed") : t("extensions.memory.editor.fmMissing")}</p>
      )}
      <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_10rem]">
        <label className="block text-xs text-ink-2">
          {t("extensions.memory.editor.name")}
          <input value={name} onChange={(e) => setName(e.target.value)} disabled={readonly || !doc.fieldsEditable} className={`${fieldClass} mt-1`} />
        </label>
        <label className="block text-xs text-ink-2">
          {t("extensions.memory.editor.kind")}
          <select value={kind} onChange={(e) => setKind(e.target.value)} disabled={readonly || !doc.fieldsEditable} className={`${fieldClass} mt-1 h-[38px]`}>
            {!KINDS.includes(kind as MemoryKind) && <option value={kind}>{kind || t("extensions.memory.kindNone")}</option>}
            {KINDS.map((k) => (
              <option key={k} value={k}>
                {memoryKindLabel(k)} ({k})
              </option>
            ))}
          </select>
        </label>
      </div>
      <label className="block text-xs text-ink-2">
        {t("extensions.memory.editor.description")} <span className="text-ink-3">{t("extensions.memory.editor.descriptionHint")}</span>
        <input value={description} onChange={(e) => setDescription(e.target.value)} disabled={readonly || !doc.fieldsEditable} className={`${fieldClass} mt-1`} />
      </label>
      <label className="block text-xs text-ink-2">
        {t("extensions.memory.editor.body")}
        <textarea value={body} onChange={(e) => setBody(e.target.value)} disabled={readonly} rows={12} spellCheck={false} className={`${fieldClass} mt-1 font-mono text-[12px] leading-relaxed`} />
      </label>
      <Warnings secrets={doc.secrets} instructionLike={doc.instructionLike} />
      <div className="rounded-lg border border-line bg-paper p-3 text-xs">
        {doc.enabled ? (
          preview ? (
            <>
              <p className="text-ink-2">{t("extensions.memory.editor.preview")}</p>
              <code className="mt-1.5 block font-mono break-all text-ink">{preview}</code>
            </>
          ) : (
            <p className="text-ink-2">{t("extensions.memory.editor.noLine")}</p>
          )
        ) : (
          <>
            <p className="text-ink-2">{t("extensions.memory.editor.offLine")}</p>
            <code className="mt-1.5 block font-mono break-all text-ink">{doc.onLine}</code>
          </>
        )}
      </div>
      {err && (
        <div className="flex flex-wrap items-center gap-2 text-xs text-danger">
          <span>{errText(err)}</span>
          <Button
            onClick={() => {
              qc.invalidateQueries({ queryKey: ["memory-entries", project] });
              onReload();
            }}
          >
            {t("extensions.memory.editor.reload")}
          </Button>
        </div>
      )}
      {note && !err && <p className="text-xs text-ok">{note}</p>}
      <div className="flex flex-wrap gap-2">
        <Button variant="primary" disabled={!dirty || readonly || save.isPending} onClick={() => save.mutate()}>
          {t("common.actions.save")}
        </Button>
        <Button
          disabled={!dirty}
          onClick={() => {
            setName(doc.name);
            setDescription(doc.description);
            setKind(doc.kind);
            setBody(doc.body);
          }}
        >
          {t("extensions.memory.editor.revert")}
        </Button>
        <Button variant="danger" className="ml-auto" disabled={readonly || trash.isPending} onClick={() => trash.mutate()}>
          {t("extensions.memory.editor.trash")}
        </Button>
      </div>
    </div>
  );
}

const keyOf = (e: MemoryEntry) => `${e.enabled ? "on" : "off"}:${e.file}`;

function MemorySection({ project, list, loading, globalOff }: { project: string; list?: MemoryList; loading: boolean; globalOff: boolean }) {
  const t = useT();
  const qc = useQueryClient();
  const editorRef = useRef<HTMLDivElement>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [pendingOn, setPendingOn] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const toggle = useMutation({
    mutationFn: (e: MemoryEntry) => memoryApi.setEnabled(project, e.file, !e.enabled, e.token, list?.indexToken ?? ""),
    onSuccess: (n) => setNote(n),
    onSettled: () => {
      setPendingOn(null);
      qc.invalidateQueries({ queryKey: ["memory-entries", project] });
      qc.invalidateQueries({ queryKey: ["memory-projects"] });
    },
  });
  const openIndex = useMutation({ mutationFn: () => memoryApi.openIndex(project) });
  if (!list) return <p className="text-xs text-ink-3">{loading ? t("extensions.memory.editor.loading") : t("extensions.memory.section.readFailed")}</p>;
  const entries = list.entries;
  const current = entries.find((e) => keyOf(e) === selected) ?? null;
  const select = (e: MemoryEntry) => {
    setSelected(keyOf(e));
    // Stacked layout (under 1100px): bring the editor into view.
    if (window.innerWidth < 1100) setTimeout(() => editorRef.current?.scrollIntoView({ behavior: "smooth", block: "start" }), 50);
  };
  return (
    <div className="space-y-3">
      <div className={`flex flex-wrap items-center gap-x-3 gap-y-1 rounded-lg px-3 py-2 text-xs ${list.overLimit ? "bg-warn-soft text-ink" : "text-ink-3"}`}>
        {list.indexExists ? (
          <span>
            {t("extensions.memory.section.index", { count: list.indexLines, size: sizeText(list.indexBytes) })}
            {list.overLimit ? t("extensions.memory.section.overLimit") : t("extensions.memory.section.withinLimit")}
          </span>
        ) : (
          <span>{t("extensions.memory.section.noIndex")}</span>
        )}
        {list.indexError && <span className="text-danger">{list.indexError}</span>}
        {list.indexExists && (
          <button onClick={() => openIndex.mutate()} className="ml-auto inline-flex items-center gap-1 text-ink-2 underline-offset-2 hover:underline">
            <ExternalLink size={12} /> {t("extensions.memory.section.openIndex")}
          </button>
        )}
      </div>
      {(toggle.error || openIndex.error) && <p className="text-xs text-danger">{errText(toggle.error ?? openIndex.error)}</p>}
      {note && !toggle.error && <p className="text-xs text-ink-2">{note}</p>}
      {globalOff && (
        <p className="rounded-lg border border-line px-3 py-2 text-xs text-ink-2">{t("extensions.memory.section.globalOff")}</p>
      )}
      {entries.length === 0 ? (
        <Sheet>
          <Empty title={t("extensions.memory.section.emptyTitle")}>{t("extensions.memory.section.emptyBody")}</Empty>
        </Sheet>
      ) : (
        <div className={`grid items-start gap-4 min-[1100px]:grid-cols-[minmax(0,1fr)_minmax(0,1.15fr)] ${globalOff ? "opacity-60" : ""}`}>
          <Sheet className="divide-y divide-line overflow-hidden">
            {entries.map((e) => (
              <div key={keyOf(e)}>
                <EntryRow
                  e={e}
                  selected={keyOf(e) === selected}
                  busy={toggle.isPending}
                  onSelect={() => select(e)}
                  onToggle={() => {
                    setNote(null);
                    if (e.enabled) toggle.mutate(e);
                    else setPendingOn(pendingOn === e.file ? null : e.file);
                  }}
                />
                {pendingOn === e.file && !e.enabled && <TurnOnPreview e={e} busy={toggle.isPending} onConfirm={() => toggle.mutate(e)} onCancel={() => setPendingOn(null)} />}
              </div>
            ))}
            {list.orphans.length > 0 && (
              <div className="px-4 py-3">
                <p className="text-xs font-medium text-ink-2">{t("extensions.memory.section.orphans")}</p>
                <ul className="mt-1 space-y-0.5 text-[11px] text-ink-3">
                  {list.orphans.map((o) => (
                    <li key={o.line}>
                      {t("extensions.memory.section.orphanLine", { line: String(o.line), title: o.title, target: o.target, problem: o.problem })}
                      {o.beyondLimit ? t("extensions.memory.section.orphanBeyond") : ""}
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </Sheet>
          <Sheet>
            <div ref={editorRef} className="scroll-mt-16" />
            {current ? (
              <Editor key={keyOf(current)} project={project} entry={current} indexToken={list.indexToken} onGone={() => setSelected(null)} />
            ) : (
              <p className="p-6 text-center text-xs text-ink-3">{t("extensions.memory.section.pick")}</p>
            )}
          </Sheet>
        </div>
      )}
    </div>
  );
}

function InstructionRow({ project, f }: { project: string | null; f: InstructionFile }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const openFile = useMutation({ mutationFn: () => memoryApi.openInstructionFile(project, f.id) });
  return (
    <Sheet className="p-4">
      <div className="flex flex-wrap items-center gap-2">
        <Badge>{f.scopeCode && ["global", "project", "local", "parent"].includes(f.scopeCode) ? t(`extensions.memory.instructions.scope.${f.scopeCode as "global"}`) : f.scope}</Badge>
        <span className="min-w-0 flex-1 font-mono text-xs break-all text-ink">{f.path}</span>
        {!f.link && (
          <Button onClick={() => openFile.mutate()} disabled={openFile.isPending}>
            <ExternalLink size={13} /> {t("extensions.memory.instructions.openEditor")}
          </Button>
        )}
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-1.5 text-[11px] text-ink-3">
        {f.size !== null && <span>{sizeText(f.size)}</span>}
        {f.gitTracked === true && <Badge tone="ok">{t("extensions.memory.instructions.gitTracked")}</Badge>}
        {f.gitTracked === false && <Badge>{t("extensions.memory.instructions.gitUntracked")}</Badge>}
        {f.link && (
          <Badge title={t("extensions.memory.instructions.linkTitle")}>
            <Link2 size={11} className="mr-1" />
            {t("extensions.memory.instructions.link")}
          </Badge>
        )}
        {f.secrets.length > 0 && <Badge tone="danger">{t("extensions.memory.instructions.secrets")}</Badge>}
        {f.instructionLike.length > 0 && <Badge tone="danger">{t("extensions.memory.instructions.instructionLike")}</Badge>}
        {f.content !== null && (
          <button onClick={() => setOpen(!open)} aria-expanded={open} className="ml-auto text-ink-2 underline-offset-2 hover:underline">
            {open ? t("extensions.memory.instructions.collapse") : t("extensions.memory.instructions.expand")}
          </button>
        )}
      </div>
      {(f.secrets.length > 0 || f.instructionLike.length > 0) && (
        <div className="mt-2">
          <Warnings secrets={f.secrets} instructionLike={f.instructionLike} />
        </div>
      )}
      {openFile.isError && <p className="mt-1 text-xs text-danger">{errText(openFile.error)}</p>}
      {open && f.content !== null && <pre className="mt-3 max-h-80 overflow-auto rounded-lg bg-paper p-3 font-mono text-[12px] leading-relaxed whitespace-pre-wrap text-ink-2">{f.content}</pre>}
    </Sheet>
  );
}

function InstructionSection({ project, onRules }: { project: string | null; onRules: () => void }) {
  const t = useT();
  const files = useQuery({ queryKey: ["instruction-files", project], queryFn: () => memoryApi.instructionFiles(project) });
  return (
    <div className="space-y-3">
      <p className="text-xs text-ink-3">
        {project ? t("extensions.memory.instructions.introProject") : t("extensions.memory.instructions.introGlobal")}
      </p>
      <p className="text-xs text-ink-3">
        {tx("extensions.memory.instructions.conflict", {
          link: (
            <button onClick={onRules} className="text-ink-2 underline underline-offset-2">
              {t("extensions.memory.instructions.showRules")}
            </button>
          ),
        })}
      </p>
      {files.isError && <p className="text-xs text-danger">{errText(files.error)}</p>}
      {files.data?.length === 0 && (
        <Sheet>
          <Empty title={t("extensions.memory.instructions.emptyTitle")}>{t("extensions.memory.instructions.emptyBody")}</Empty>
        </Sheet>
      )}
      {files.data?.map((f) => (
        <InstructionRow key={f.id} project={project} f={f} />
      ))}
    </div>
  );
}

export function MemoryPage() {
  const t = useT();
  const [segment, setSegment] = useState<Segment>("memory");
  // Memory works for any picked project (home and folder-less ones too); rules and CLAUDE.md need a real folder.
  const need = segment === "memory" ? "memory" : "folder";
  const { scope, project, projectId } = useScope(need);
  const folderId = useScope("folder").projectId;
  const memId = scope === "project" ? projectId : null;
  const auto = useQuery({ queryKey: ["auto-memory"], queryFn: memoryApi.autoMemory });
  const list = useQuery({
    queryKey: ["memory-entries", memId],
    queryFn: () => memoryApi.entries(memId ?? ""),
    enabled: !!memId && !!project?.memory,
    refetchInterval: 15_000,
  });
  return (
    <div>
      <PageHeader title={t("extensions.memory.title")} />
      <PageBody>
        <ScopeBar need={need} />
        <AutoMemoryBar list={list.data} />
        {scope === "project" && project && (
          <p className="text-xs text-ink-3">{project.path ? <span className="font-mono text-ink-2">{project.path}</span> : t("extensions.memory.page.noFolder")}</p>
        )}
        <Segmented
          label={t("extensions.memory.page.view")}
          value={segment}
          onChange={(s) => confirmLeave() && setSegment(s)}
          options={[
            { id: "memory", label: t("extensions.memory.page.memory") },
            { id: "rules", label: t("extensions.memory.page.rules") },
            { id: "instructions", label: t("extensions.memory.page.instructions") },
          ]}
        />
        {segment === "rules" ? (
          // The current project's settings still decide whether a global rule is off, so its id goes along.
          <RulesSection key={`${scope}:${folderId ?? "-"}`} project={folderId} hasPath={!!folderId} scope={scope} />
        ) : segment === "instructions" ? (
          <InstructionSection key={memId ?? "-"} project={scope === "project" ? projectId : null} onRules={() => setSegment("rules")} />
        ) : !memId ? (
          <Sheet>
            <Empty title={t("extensions.memory.page.perProjectTitle")}>{t("extensions.memory.page.perProjectBody")}</Empty>
          </Sheet>
        ) : !project?.memory ? (
          <Sheet>
            <Empty title={t("extensions.memory.page.noMemoryTitle")}>{t("extensions.memory.page.noMemoryBody")}</Empty>
          </Sheet>
        ) : (
          <MemorySection key={memId} project={memId} list={list.data} loading={list.isLoading} globalOff={auto.data?.enabled === false} />
        )}
      </PageBody>
    </div>
  );
}
