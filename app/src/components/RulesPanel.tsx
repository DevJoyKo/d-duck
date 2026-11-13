// Rules segment of the memory tab (rules-editor, D-106): global and project `.claude/rules/` in one list, new
// rule from a template, body + scope chips editor, off/on through `claudeMdExcludes`, Trash. One line of help each.
import { useIsFetching, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, ChevronDown, FileText, GitCommitHorizontal, Link2, Plus, Search, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { secretText, sizeText } from "../lib/memory";
import { t } from "../i18n";
import { tx, useT } from "../i18n/react";
import {
  alwaysChip,
  offText,
  byFolder,
  confirmDiscard,
  filterRules,
  freeName,
  globWarnings,
  patternProblem,
  projectFolder,
  ruleFileName,
  rulesApi,
  scopeChips,
  scopeTitle,
  summaryText,
  validRuleName,
  type RuleDoc,
  type RuleGroup,
  type RuleRow,
  type RuleScope,
  type RuleTemplate,
} from "../lib/rules";
import { setDirty as setAppDirty } from "../lib/projects";
import { DuckAvatar } from "./DuckAvatar";
import { Badge, Button, Empty, Sheet, Switch, fieldClass } from "./ui";

/** Group title and help line, read at render time so they follow the language. */
function groupText(scope: RuleScope): { title: string; line: string } {
  return scope === "global"
    ? { title: t("extensions.rules.group.globalTitle"), line: t("extensions.rules.group.globalLine") }
    : { title: t("extensions.rules.group.projectTitle"), line: t("extensions.rules.group.projectLine") };
}

function errText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

/** Scope pattern chip: square, mono, grey. `muted` (dashed) for "항상". */
function Chip({ children, muted, onRemove }: { children: string; muted?: boolean; onRemove?: () => void }) {
  const t = useT();
  return (
    <span
      title={scopeTitle(children)}
      className={`inline-flex h-5 max-w-full items-center gap-1 rounded-md border px-1.5 font-mono text-[11px] ${muted ? "border-dashed border-line text-ink-3" : "border-line bg-line/60 text-ink-2"}`}
    >
      <span className="truncate">{children}</span>
      {onRemove && (
        <button onClick={onRemove} aria-label={t("extensions.rules.removePattern", { pattern: children })} className="text-ink-3 hover:text-ink">
          <X size={12} />
        </button>
      )}
    </span>
  );
}

/** Status badges of a rule (pills, unlike the square scope chips), most important first. */
function RuleBadges({ r, inEditor }: { r: RuleRow; inEditor?: boolean }) {
  const t = useT();
  return (
    <>
      {r.offBy === "other" && (
        <Badge pill title={t("extensions.rules.otherOff")}>
          {t("extensions.rules.badges.otherOff")}
        </Badge>
      )}
      {r.offBy === "app" && (
        <Badge pill title={offText()}>
          {t("extensions.rules.badges.off")}
        </Badge>
      )}
      {r.fm === "broken" && !inEditor && (
        <Badge pill tone="danger" title={t("extensions.rules.badges.brokenTitle")}>
          {t("extensions.rules.badges.broken")}
        </Badge>
      )}
      {r.fm === "complex" && !inEditor && (
        <Badge pill title={t("extensions.rules.badges.complexTitle")}>
          {t("extensions.rules.badges.complex")}
        </Badge>
      )}
      {r.link && !r.dirLink && (
        <Badge pill title={t("extensions.rules.badges.sharedLinkTitle")}>
          <Link2 size={12} />
          {t("extensions.rules.badges.sharedLink")}
        </Badge>
      )}
      {r.dirLink && (
        <Badge pill title={t("extensions.rules.badges.linkFolderTitle")}>
          <Link2 size={12} />
          {t("extensions.rules.badges.linkFolder")}
        </Badge>
      )}
      {r.linkOutside && (
        <Badge pill tone="warn" title={t("extensions.rules.badges.approvalTitle")}>
          {t("extensions.rules.badges.approval")}
        </Badge>
      )}
    </>
  );
}

/** Small row icons: committed to the repo, secrets, pattern warnings. */
function RuleIcons({ r }: { r: RuleRow }) {
  const t = useT();
  const tracked = t("extensions.rules.tracked");
  return (
    <>
      {r.gitTracked === true && (
        <span title={tracked} aria-label={tracked} className="inline-flex text-ink-3">
          <GitCommitHorizontal size={14} />
        </span>
      )}
      {r.hasSecrets && (
        <span title={t("extensions.rules.icons.secretsTitle")} aria-label={t("extensions.rules.icons.secretsLabel")} className="inline-flex text-danger">
          <AlertTriangle size={14} />
        </span>
      )}
      {r.warnings.length > 0 && (
        <span title={r.warnings.join("\n")} aria-label={t("extensions.rules.icons.warnLabel", { list: r.warnings.join(", ") })} className="inline-flex text-ink-3">
          <AlertTriangle size={14} />
        </span>
      )}
    </>
  );
}

function RuleLine({ r, selected, busy, onSelect, onToggle }: { r: RuleRow; selected: boolean; busy: boolean; onSelect: () => void; onToggle: () => void }) {
  const t = useT();
  const chips = scopeChips(r);
  const always = alwaysChip();
  return (
    <div className={`flex items-start gap-3 px-4 py-3 ${selected ? "bg-paper" : ""}`}>
      <button onClick={onSelect} aria-current={selected ? "true" : undefined} disabled={r.dirLink} className="min-w-0 flex-1 text-left disabled:cursor-default">
        <span className="flex flex-wrap items-center gap-1.5">
          <span className={`text-[13px] font-medium ${r.enabled ? "text-ink" : "text-ink-3"}`}>{r.name}</span>
          {chips.map((c) => (
            <Chip key={c} muted={c === always}>
              {c}
            </Chip>
          ))}
          <RuleBadges r={r} />
          <RuleIcons r={r} />
        </span>
      </button>
      <Switch
        on={r.enabled}
        label={t(r.enabled ? "extensions.rules.turnOff" : "extensions.rules.turnOn", { name: r.rel })}
        title={r.offBy === "other" ? t("extensions.rules.otherOff") : offText()}
        disabled={busy || r.dirLink || r.offBy === "other"}
        onClick={onToggle}
      />
    </div>
  );
}

function Group({ g, filter, selected, busy, onSelect, onToggle, onNew }: { g: RuleGroup; filter: string; selected: string | null; busy: boolean; onSelect: (r: RuleRow) => void; onToggle: (r: RuleRow) => void; onNew: () => void }) {
  const t = useT();
  const text = groupText(g.scope);
  const rules = filterRules(g.rules, filter);
  const folder = g.scope === "project" ? projectFolder(g.dir) : null;
  const tracked = g.rules.filter((r) => r.gitTracked === true).length;
  const off = !!g.readonlyReason && g.rules.length === 0;
  return (
    <Sheet className={`overflow-hidden ${off ? "opacity-60" : ""}`}>
      <div className="border-b border-line px-4 py-3">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <p className="text-[13px] font-semibold">
            {text.title}
            {folder && <span className="font-normal text-ink-2"> · {folder}</span>}
          </p>
          {tracked > 0 && (
            <span className="ml-auto inline-flex items-center gap-1 text-[11px] text-ink-3" title={t("extensions.rules.trackedTitle")}>
              <GitCommitHorizontal size={14} />
              {t("extensions.rules.trackedCount", { count: tracked })}
            </span>
          )}
        </div>
        {g.readonlyReason ? <p className="text-xs text-ink-2">{g.readonlyReason}</p> : <p className="text-xs text-ink-3">{text.line}</p>}
        {g.rules.length > 0 && <p className="mt-0.5 text-[11px] text-ink-3">{summaryText(g)}</p>}
        {g.settingsReadonly && <p className="mt-0.5 text-xs text-danger">{g.settingsReadonly}</p>}
      </div>
      {off ? null : g.rules.length === 0 ? (
        <p className="px-4 py-4 text-xs text-ink-3">
          {tx("extensions.rules.emptyGroup", {
            link: (
              <button onClick={onNew} disabled={!!g.readonlyReason} className="text-ink-2 underline underline-offset-2 disabled:no-underline disabled:opacity-50">
                {t("extensions.rules.newRule")}
              </button>
            ),
          })}
        </p>
      ) : rules.length === 0 ? (
        <p className="px-4 py-4 text-xs text-ink-3">{t("extensions.rules.noMatch")}</p>
      ) : (
        <div className="divide-y divide-line">
          {byFolder(rules).map(([folder, list]) => {
            const lines = list.map((r) => <RuleLine key={r.id} r={r} selected={r.id === selected} busy={busy} onSelect={() => onSelect(r)} onToggle={() => onToggle(r)} />);
            return folder === "" ? (
              <div key="" className="divide-y divide-line">
                {lines}
              </div>
            ) : (
              <details key={folder} open className="group/f">
                <summary className="flex cursor-pointer list-none items-center gap-1 px-4 py-1.5 font-mono text-[11px] text-ink-3 [&::-webkit-details-marker]:hidden">
                  <ChevronDown size={14} className="-rotate-90 transition-transform group-open/f:rotate-0" />
                  {folder}/
                </summary>
                <div className="divide-y divide-line">{lines}</div>
              </details>
            );
          })}
        </div>
      )}
      {g.truncated && <p className="border-t border-line px-4 py-2 text-[11px] text-ink-3">{t("extensions.rules.truncated")}</p>}
    </Sheet>
  );
}

/** Scope chips input: Enter adds, × removes; empty = always. */
function ScopeInput({ paths, onChange, disabled }: { paths: string[]; onChange: (p: string[]) => void; disabled: boolean }) {
  const t = useT();
  const [draft, setDraft] = useState("");
  const problem = draft ? patternProblem(draft) : null;
  const add = () => {
    if (!draft || problem || paths.includes(draft)) return;
    onChange([...paths, draft]);
    setDraft("");
  };
  return (
    <div className="text-xs text-ink-2">
      {t("extensions.rules.scope.label")} <span className="text-ink-3">{t("extensions.rules.scope.hint")}</span>
      <div className={`mt-1 flex min-h-9 flex-wrap items-center gap-1.5 rounded-lg border border-line bg-sheet px-2 py-1.5 ${disabled ? "opacity-60" : ""}`}>
        {paths.length === 0 && <Chip muted>{alwaysChip()}</Chip>}
        {paths.map((p) => (
          <Chip key={p} onRemove={disabled ? undefined : () => onChange(paths.filter((x) => x !== p))}>
            {p}
          </Chip>
        ))}
        {!disabled && (
          <input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.nativeEvent.isComposing) {
                e.preventDefault();
                add();
              }
            }}
            onBlur={add}
            placeholder={t("extensions.rules.scope.placeholder")}
            aria-label={t("extensions.rules.scope.add")}
            className="min-w-[10rem] flex-1 bg-transparent font-mono text-[12px] text-ink outline-none placeholder:font-sans placeholder:text-ink-3"
          />
        )}
      </div>
      {problem && <p className="mt-1 text-danger">{problem}</p>}
      {globWarnings(paths).map((w) => (
        <p key={w} className="mt-1 flex items-start gap-1 text-ink-2">
          <AlertTriangle size={14} className="mt-px shrink-0 text-ink-3" />
          {w}
        </p>
      ))}
    </div>
  );
}

function EditorForm({
  project,
  doc,
  saved,
  onSaved,
  onEdited,
  onReload,
  onGone,
  onDirty,
}: {
  project: string | null;
  doc: RuleDoc;
  saved: boolean;
  onSaved: () => void;
  onEdited: () => void;
  onReload: () => void;
  onGone: (unconfirmed?: boolean) => void;
  onDirty: (d: boolean) => void;
}) {
  const t = useT();
  const [body, setBody] = useState(doc.body);
  const [paths, setPaths] = useState(doc.paths);
  const pathsChanged = paths.join("\n") !== doc.paths.join("\n");
  const dirty = body !== doc.body || pathsChanged;
  const readonly = !!doc.readonlyReason;
  useEffect(() => onDirty(dirty), [dirty, onDirty]);
  useEffect(() => () => onDirty(false), [onDirty]);
  const save = useMutation({
    mutationFn: () => rulesApi.save(project, { id: doc.id, token: doc.token, body, ...(doc.pathsEditable ? { paths } : {}) }),
    onSuccess: onSaved,
  });
  const trash = useMutation({ mutationFn: () => rulesApi.trash(project, doc.id, doc.token), onSuccess: (r) => r.done && onGone(r.unconfirmed) });
  const err = save.error ?? trash.error;
  const edit = <T,>(set: (v: T) => void) => (v: T) => {
    set(v);
    onEdited();
  };
  return (
    <div className="space-y-3 p-4">
      <div className="flex flex-wrap items-center gap-2">
        <FileText size={14} className="text-ink-3" />
        <span className="font-mono text-xs break-all text-ink-2">{doc.rel}</span>
        {doc.size !== null && <span className="text-[11px] text-ink-3">{sizeText(doc.size)}</span>}
        <RuleBadges r={doc} inEditor />
        <RuleIcons r={doc} />
      </div>
      {doc.offBy === "app" && <p className="text-xs text-ink-2">{offText()}</p>}
      {doc.link && doc.linkTarget && (
        <p className="flex min-w-0 items-center gap-1.5 text-xs text-ink-3">
          <Link2 size={14} className="shrink-0" />
          <span className="font-mono break-all text-ink-2">{doc.linkTarget}</span>
        </p>
      )}
      {doc.readonlyReason && <p className="text-xs text-ink-2">{doc.readonlyReason}</p>}
      {doc.fm === "complex" && <p className="text-xs text-ink-3">{t("extensions.rules.editor.complex")}</p>}
      {doc.fm === "broken" && <p className="text-xs text-danger">{t("extensions.rules.editor.broken")}</p>}
      {readonly ? (
        doc.link &&
        doc.body && (
          <>
            {doc.paths.length > 0 && (
              <div className="flex flex-wrap items-center gap-1.5 text-xs text-ink-2">
                {t("extensions.rules.scope.label")}
                {doc.paths.map((p) => (
                  <Chip key={p}>{p}</Chip>
                ))}
              </div>
            )}
            <label className="block text-xs text-ink-2">
              {t("extensions.rules.editor.body")} <span className="text-ink-3">{t("extensions.rules.editor.readOnly")}</span>
              <textarea value={doc.body} readOnly rows={12} spellCheck={false} className={`${fieldClass} mt-1 font-mono text-[12px] leading-relaxed text-ink-2`} />
            </label>
          </>
        )
      ) : (
        <>
          <ScopeInput paths={paths} onChange={edit(setPaths)} disabled={!doc.pathsEditable} />
          <label className="block text-xs text-ink-2">
            {t("extensions.rules.editor.body")}
            <textarea value={body} onChange={(e) => edit(setBody)(e.target.value)} rows={12} spellCheck={false} className={`${fieldClass} mt-1 font-mono text-[12px] leading-relaxed`} />
          </label>
        </>
      )}
      {doc.secrets.length > 0 && (
        <p className="flex items-start gap-1.5 text-xs text-danger">
          <AlertTriangle size={14} className="mt-px shrink-0" />
          {t("extensions.rules.editor.secrets", { hits: secretText(doc.secrets) })}
          {doc.gitTracked ? t("extensions.rules.editor.committed") : ""}
        </p>
      )}
      {doc.instructionLike.length > 0 && <p className="text-xs text-ink-2">{t("extensions.rules.editor.instructionLike", { lines: doc.instructionLike.join(", ") })}</p>}
      {err && (
        <div className="flex flex-wrap items-center gap-2 text-xs text-danger">
          <span>{errText(err)}</span>
          <Button onClick={onReload}>{t("extensions.rules.editor.reload")}</Button>
        </div>
      )}
      {saved && !dirty && !err && (
        <p role="status" className="text-xs text-ok">
          {t("extensions.rules.editor.saved")}
        </p>
      )}
      {!readonly && (
        <div className="flex flex-wrap gap-2">
          <Button variant="primary" disabled={!dirty || save.isPending} onClick={() => save.mutate()}>
            {t("common.actions.save")}
          </Button>
          <Button
            disabled={!dirty}
            onClick={() => {
              setBody(doc.body);
              setPaths(doc.paths);
            }}
          >
            {t("extensions.rules.editor.revert")}
          </Button>
          <Button variant="danger" className="ml-auto" disabled={trash.isPending} onClick={() => trash.mutate()}>
            {t("extensions.rules.editor.trash")}
          </Button>
        </div>
      )}
    </div>
  );
}

function Editor({ project, row, onGone, onDirty, guard, onReady, focusTick }: { project: string | null; row: RuleRow; onGone: (unconfirmed?: boolean) => void; onDirty: (d: boolean) => void; guard: () => boolean; onReady: () => void; focusTick: number }) {
  const t = useT();
  const qc = useQueryClient();
  const [saved, setSaved] = useState(false);
  const doc = useQuery({ queryKey: ["rule-doc", project, row.id], queryFn: () => rulesApi.read(project, row.id), staleTime: Infinity });
  const fetching = useIsFetching({ queryKey: ["rules", project] }) + useIsFetching({ queryKey: ["rule-doc", project, row.id] }) > 0;
  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["rules", project] });
    qc.invalidateQueries({ queryKey: ["rule-doc", project, row.id] });
  };
  const reload = () => {
    if (!guard()) return;
    setSaved(false);
    refresh();
  };
  // Scroll only once the text is there: scrolling over "불러오는 중…" left 73px of editor in view (review H2).
  const ready = !!doc.data;
  const onReadyRef = useRef(onReady);
  useEffect(() => {
    onReadyRef.current = onReady;
  }, [onReady]);
  useEffect(() => {
    if (ready) onReadyRef.current();
  }, [ready, focusTick]);
  if (doc.isError) return <p className="p-4 text-xs text-danger">{errText(doc.error)}</p>;
  if (!doc.data) return <p className="p-4 text-xs text-ink-3">{t("extensions.rules.editor.loading")}</p>;
  return (
    <>
      {row.token !== doc.data.token && !fetching && (
        <div className="m-4 mb-0 flex flex-wrap items-center gap-2 rounded-lg border border-line bg-paper px-3 py-2 text-xs text-ink">
          <span>{t("extensions.rules.editor.changed")}</span>
          <Button onClick={reload}>{t("extensions.rules.editor.reload")}</Button>
        </div>
      )}
      <EditorForm
        key={doc.data.token}
        project={project}
        doc={doc.data}
        saved={saved}
        onSaved={() => {
          setSaved(true);
          refresh();
        }}
        onEdited={() => setSaved(false)}
        onReload={reload}
        onGone={onGone}
        onDirty={onDirty}
      />
    </>
  );
}

function NewRule({
  project,
  fixedScope,
  scopes,
  taken,
  templates,
  onCreated,
  onCancel,
  onDirty,
}: {
  project: string | null;
  /** The page's [전역 | 프로젝트] choice (project-switcher): no scope select in the form. */
  fixedScope?: RuleScope;
  scopes: RuleScope[];
  taken: Record<RuleScope, string[]>;
  templates: RuleTemplate[];
  onCreated: (id: string) => void;
  onCancel: () => void;
  onDirty: (d: boolean) => void;
}) {
  const t = useT();
  const [chosen, setScope] = useState<RuleScope>(scopes.includes("project") ? "project" : "global");
  const scope = fixedScope ?? chosen;
  const [tpl, setTpl] = useState<string>("");
  const [name, setName] = useState("");
  const [paths, setPaths] = useState<string[]>([]);
  const [body, setBody] = useState("");
  const file = ruleFileName(name);
  const nameOk = validRuleName(file);
  const exists = nameOk && taken[scope].some((t) => t.toLowerCase() === file.toLowerCase());
  const dirty = !!(name || body || paths.length);
  useEffect(() => onDirty(dirty), [dirty, onDirty]);
  useEffect(() => () => onDirty(false), [onDirty]);
  const pick = (id: string) => {
    // A template replaces name, scope and body: ask first when something is typed (review M2).
    if (!confirmDiscard(dirty)) return;
    setTpl(id);
    const t = templates.find((x) => x.id === id);
    setName(t ? freeName(t.name, taken[scope]) : "");
    setPaths(t?.paths ?? []);
    setBody(t?.body ?? "");
  };
  const create = useMutation({ mutationFn: () => rulesApi.save(project, { scope, name: file, body, paths }), onSuccess: onCreated });
  const suggestion = exists ? freeName(name.replace(/\.md$/, ""), taken[scope]) : null;
  return (
    <div
      className="space-y-3 p-4"
      onKeyDown={(e) => {
        if (e.key === "Escape" && !e.nativeEvent.isComposing) {
          e.preventDefault();
          onCancel();
        }
      }}
    >
      <p className="text-[13px] font-semibold">{t("extensions.rules.create.title")}</p>
      <div className="grid gap-3 sm:grid-cols-2">
        {fixedScope ? (
          <p className="block text-xs text-ink-2">
            {t("extensions.rules.create.scope")}
            <span className="mt-1 flex h-8 items-center text-[13px] text-ink">{groupText(fixedScope).title}</span>
          </p>
        ) : (
          <label className="block text-xs text-ink-2">
            {t("extensions.rules.create.scope")}
            <select value={scope} onChange={(e) => setScope(e.target.value as RuleScope)} className={`${fieldClass} mt-1 h-8 py-0`}>
              {scopes.map((s) => (
                <option key={s} value={s}>
                  {groupText(s).title}
                </option>
              ))}
            </select>
          </label>
        )}
        <label className="block text-xs text-ink-2">
          {t("extensions.rules.create.start")}
          <select value={tpl} onChange={(e) => pick(e.target.value)} className={`${fieldClass} mt-1 h-8 py-0`}>
            <option value="">{t("extensions.rules.create.blank")}</option>
            {templates.map((t) => (
              <option key={t.id} value={t.id}>
                {t.label}
              </option>
            ))}
          </select>
        </label>
      </div>
      <label className="block text-xs text-ink-2">
        {t("extensions.rules.create.fileName")} <span className="text-ink-3">{t("extensions.rules.create.fileNameHint")}</span>
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={t("extensions.rules.create.fileNamePlaceholder")}
          className={`${fieldClass} mt-1 h-8 py-0 font-mono placeholder:font-sans`}
        />
      </label>
      {name && !nameOk && <p className="text-xs text-danger">{t("extensions.rules.create.badName")}</p>}
      {suggestion && (
        <p className="text-xs text-danger">
          {tx("extensions.rules.create.taken", {
            suggestion: (
              <button onClick={() => setName(suggestion)} className="font-mono text-ink-2 underline underline-offset-2">
                {suggestion}
              </button>
            ),
          })}
        </p>
      )}
      <ScopeInput paths={paths} onChange={setPaths} disabled={false} />
      <label className="block text-xs text-ink-2">
        {t("extensions.rules.editor.body")}
        <textarea value={body} onChange={(e) => setBody(e.target.value)} rows={8} spellCheck={false} className={`${fieldClass} mt-1 font-mono text-[12px] leading-relaxed`} />
      </label>
      {scope === "project" && <p className="text-xs text-ink-3">{t("extensions.rules.create.projectNote")}</p>}
      {create.isError && <p className="text-xs text-danger">{errText(create.error)}</p>}
      <div className="flex gap-2">
        <Button variant="primary" disabled={!nameOk || exists || create.isPending} onClick={() => create.mutate()}>
          {t("extensions.rules.create.submit")}
        </Button>
        <Button onClick={onCancel}>{t("common.actions.cancel")}</Button>
      </div>
    </div>
  );
}

function EditorColumn({ children, innerRef }: { children: ReactNode; innerRef: RefObject<HTMLDivElement | null> }) {
  const t = useT();
  return (
    <div className="min-[1100px]:sticky min-[1100px]:top-16 min-[1100px]:max-h-[calc(100vh-5rem)] min-[1100px]:overflow-y-auto min-[1100px]:rounded-xl">
      <Sheet>
        <div ref={innerRef} tabIndex={-1} aria-label={t("extensions.rules.editor.label")} className="min-h-[24rem] scroll-mt-16 rounded-xl outline-none">
          {children}
        </div>
      </Sheet>
    </div>
  );
}

/**
 * `onDirtyChange` tells the page about unsaved edits so switching project or segment can ask first.
 */
export function RulesSection({ project, hasPath, onDirtyChange, scope }: { project: string | null; hasPath: boolean; onDirtyChange?: (dirty: boolean) => void; scope?: RuleScope }) {
  const t = useT();
  const qc = useQueryClient();
  const editorRef = useRef<HTMLDivElement>(null);
  const dirtyRef = useRef(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [goneNote, setGoneNote] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [filter, setFilter] = useState("");
  const [focusTick, setFocusTick] = useState(0);
  const list = useQuery({ queryKey: ["rules", project], queryFn: () => rulesApi.list(project), refetchInterval: 15_000 });
  const toggle = useMutation({
    mutationFn: ({ r, g }: { r: RuleRow; g: RuleGroup }) => rulesApi.setEnabled(project, r.id, !r.enabled, g.settingsToken),
    onSettled: () => qc.invalidateQueries({ queryKey: ["rules", project] }),
  });
  const onDirtyRef = useRef(onDirtyChange);
  useEffect(() => {
    onDirtyRef.current = onDirtyChange;
  }, [onDirtyChange]);
  const setDirty = useCallback((d: boolean) => {
    dirtyRef.current = d;
    // App-wide guard: sidebar navigation and project switching ask too (review H1).
    setAppDirty("rules", d);
    onDirtyRef.current?.(d);
  }, []);
  useEffect(() => () => setAppDirty("rules", false), []);
  const guard = () => confirmDiscard(dirtyRef.current);
  useEffect(() => {
    if (!focusTick) return;
    const el = editorRef.current;
    if (!el) return;
    el.focus({ preventScroll: true });
    // A new-rule form is there at once; an existing rule scrolls when its text has loaded (onReady).
    if (creating && window.innerWidth < 1100) el.scrollIntoView?.({ behavior: "smooth", block: "start" });
    // Only on a new focus request.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusTick]);
  const scrollToEditor = useCallback(() => {
    if (window.innerWidth < 1100) editorRef.current?.scrollIntoView?.({ behavior: "smooth", block: "start" });
  }, []);
  const data = list.data;
  const groups = [data?.global, data?.project].filter((g): g is RuleGroup => !!g && (!scope || g.scope === scope));
  const all = groups.flatMap((g) => g.rules);
  const current = all.find((r) => r.id === selected) ?? null;
  const scopes = groups.filter((g) => !g.readonlyReason).map((g) => g.scope);
  const taken: Record<RuleScope, string[]> = { global: data?.global?.rules.map((r) => r.rel) ?? [], project: data?.project?.rules.map((r) => r.rel) ?? [] };
  const startNew = () => {
    if (creating || !guard()) return;
    setGoneNote(null);
    setSelected(null);
    setCreating(true);
    setFocusTick((t) => t + 1);
  };
  const cancelNew = () => {
    if (!guard()) return;
    setCreating(false);
  };
  if (list.isError) return <p className="text-xs text-danger">{errText(list.error)}</p>;
  if (!data) return <p className="text-xs text-ink-3">{t("extensions.rules.editor.loading")}</p>;
  const none = all.length === 0;
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <label className="relative min-w-[12rem] flex-1 sm:max-w-xs">
          <Search size={14} className="absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
          <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder={t("extensions.rules.list.searchPlaceholder")} aria-label={t("extensions.rules.list.searchLabel")} className={`${fieldClass} h-8 py-0 pl-8`} />
        </label>
        <Button variant="primary" disabled={scopes.length === 0} onClick={startNew}>
          <Plus size={14} /> {t("extensions.rules.newRule")}
        </Button>
      </div>
      {toggle.isError && <p className="text-xs text-danger">{errText(toggle.error)}</p>}
      {goneNote && (
        <p role="status" data-tone="warn" className="rounded-lg border border-warn/30 bg-warn-soft px-3 py-2 text-xs text-ink">
          {goneNote}
        </p>
      )}
      <div className="grid items-start gap-4 min-[1100px]:grid-cols-[minmax(0,1fr)_minmax(0,1.15fr)]">
        <div className="space-y-3">
          {groups.map((g) => (
            <Group
              key={g.scope}
              g={g}
              filter={filter}
              selected={selected}
              busy={toggle.isPending}
              onSelect={(r) => {
                if (r.id === selected && !creating) {
                  setFocusTick((t) => t + 1);
                  return;
                }
                if (!guard()) return;
                setGoneNote(null);
                setCreating(false);
                setSelected(r.id);
                setFocusTick((t) => t + 1);
              }}
              onToggle={(r) => toggle.mutate({ r, g })}
              onNew={startNew}
            />
          ))}
          {!data.project && scope !== "global" && (
            <Sheet className="px-4 py-3 opacity-60">
              <p className="text-[13px] font-semibold">{t("extensions.rules.list.projectTitle")}</p>
              <p className="text-xs text-ink-3">{hasPath ? t("extensions.rules.list.projectUnreadable") : t("extensions.rules.list.projectMissing")}</p>
            </Sheet>
          )}
        </div>
        <EditorColumn innerRef={editorRef}>
          {creating ? (
            <NewRule
              project={project}
              fixedScope={scope}
              scopes={scopes}
              taken={taken}
              templates={data.templates}
              onCancel={cancelNew}
              onDirty={setDirty}
              onCreated={(id) => {
                setDirty(false);
                setCreating(false);
                setSelected(id);
                qc.invalidateQueries({ queryKey: ["rules", project] });
              }}
            />
          ) : current ? (
            <Editor
              key={current.id}
              project={project}
              row={current}
              guard={guard}
              onDirty={setDirty}
              onReady={scrollToEditor}
              focusTick={focusTick}
              onGone={(unconfirmed) => {
                setSelected(null);
                setGoneNote(unconfirmed ? t("common.trash.unconfirmed") : null);
                qc.invalidateQueries({ queryKey: ["rules", project] });
              }}
            />
          ) : none ? (
            <Empty title={t("extensions.rules.list.emptyTitle")} icon={<DuckAvatar pose="idle" size={48} />}>
              {t("extensions.rules.list.emptyBody")}
            </Empty>
          ) : (
            <Empty title={t("extensions.rules.list.pickTitle")} />
          )}
        </EditorColumn>
      </div>
    </div>
  );
}
