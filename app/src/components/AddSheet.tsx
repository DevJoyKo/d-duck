// "추가 ▾" menu and sheet shared by the skills and plugins tabs (install-from-link, skill-authoring).
// Link → preview → install; direct create from a bundled template; "Claude에게 부탁하기" with result review.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, ChevronDown, Copy, FileCode2, Link2, PenLine, Sparkles, X } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { api, type AgentsPayload, failure, messageOf, shown, type Shown } from "../lib/api";
import {
  addApi,
  fillTemplate,
  kindLabel,
  kindNoun,
  nameError,
  shortSha,
  targetPath,
  type AuthorKind,
  type Job,
  type LinkPreview,
  type PluginInstall,
  type Produced,
  type SkillRisk,
} from "../lib/addItems";
import { dateTime } from "../lib/format";
import { t } from "../i18n";
import { tx, useT } from "../i18n/react";
import { Button, Segmented, fieldClass } from "./ui";
import { isGateError } from "../lib/auth";
import { GateLine } from "./LoginBits";
import { useScope } from "./ProjectContext";
import { WorkflowCheckNote } from "./AuthoringEditor";
import { metaBlocks, useWorkflowCheck } from "../lib/workflowCheck";

export type AddMode = "link" | "create" | "ask";
const MODES: AddMode[] = ["link", "create", "ask"];
const KINDS: AuthorKind[] = ["skill", "command", "workflow"];

export function AddMenu({ onPick, pending = 0 }: { onPick: (m: AddMode) => void; pending?: number }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);
  const items: { id: AddMode; icon: ReactNode; title: string; hint: string }[] = [
    { id: "link", icon: <Link2 size={14} />, title: t("extensions.add.modes.link"), hint: t("extensions.add.menu.linkHint") },
    { id: "create", icon: <PenLine size={14} />, title: t("extensions.add.modes.create"), hint: t("extensions.add.menu.createHint") },
    { id: "ask", icon: <Sparkles size={14} />, title: pending > 0 ? t("extensions.add.menu.askPending", { count: pending }) : t("extensions.add.modes.ask"), hint: t("extensions.add.menu.askHint") },
  ];
  return (
    <div ref={ref} className="relative">
      <Button variant="primary" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((v) => !v)}>
        {t("extensions.add.button")}
        {pending > 0 && (
          <span className="rounded-full bg-sheet px-1.5 text-[11px] font-semibold text-ink" title={t("extensions.add.pendingTitle")}>
            {t("extensions.add.pending", { count: pending })}
          </span>
        )}
        <ChevronDown size={13} />
      </Button>
      {open && (
        <div role="menu" className="absolute right-0 z-20 mt-1 w-72 overflow-hidden rounded-xl bg-sheet shadow-e3 py-1">
          {items.map((i) => (
            <button
              key={i.id}
              role="menuitem"
              onClick={() => {
                setOpen(false);
                onPick(i.id);
              }}
              className="flex w-full items-start gap-2.5 px-3 py-2 text-left hover:bg-paper"
            >
              <span className="mt-0.5 text-ink-2">{i.icon}</span>
              <span>
                <span className="block text-[13px] font-medium">{i.title}</span>
                <span className="block text-xs text-ink-3">{i.hint}</span>
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function Box({ tone, children }: { tone: "info" | "warn" | "danger" | "ok"; children: ReactNode }) {
  const cls = {
    info: "border-line bg-paper text-ink-2",
    warn: "border-warn/30 bg-warn-soft text-ink",
    danger: "border-danger/40 bg-danger/5 text-danger",
    ok: "border-ok/40 bg-ok/5 text-ok",
  }[tone];
  return <div className={`rounded-lg border px-3 py-2 text-xs leading-relaxed ${cls}`}>{children}</div>;
}

function CopyLine({ label, text }: { label: string; text: string }) {
  const t = useT();
  const [done, setDone] = useState(false);
  return (
    <div className="mt-1.5">
      <span className="text-xs text-ink-2">{label}</span>
      <div className="mt-0.5 flex items-center gap-2 rounded-md border border-line bg-sheet px-2 py-1">
        <code className="min-w-0 flex-1 truncate font-mono text-[12px] text-ink">{text}</code>
        <button
          className="inline-flex items-center gap-1 text-xs text-ink-2 hover:text-ink"
          onClick={() => {
            navigator.clipboard?.writeText(text).then(() => setDone(true), () => setDone(false));
          }}
        >
          <Copy size={12} /> {done ? t("extensions.add.copyDone") : t("extensions.add.copy")}
        </button>
      </div>
    </div>
  );
}

export function RiskBox({ risk, executables = 0 }: { risk: SkillRisk; executables?: number }) {
  const t = useT();
  const any = risk.allowedTools || risk.hooks || risk.shellLines > 0 || executables > 0;
  if (!any) return <Box tone="info">{t("extensions.add.risk.none")}</Box>;
  const red = risk.hooks || executables > 0;
  return (
    <Box tone={red ? "danger" : "warn"}>
      <p className="mb-1 flex items-center gap-1.5 font-semibold">
        <AlertTriangle size={13} /> {t("extensions.add.risk.title")}
      </p>
      <ul className="list-disc space-y-0.5 pl-5">
        {risk.hooks && <li className="text-danger">{t("extensions.add.risk.hooks")}</li>}
        {executables > 0 && <li className="text-danger">{t("extensions.add.risk.executables", { count: executables })}</li>}
        {risk.allowedTools && (
          <li className="text-ink">
            {tx("extensions.add.risk.allowedTools", { tools: <code className="font-mono">{risk.allowedTools}</code> })}
          </li>
        )}
        {risk.shellLines > 0 && <li className="text-ink">{t("extensions.add.risk.shellLines", { count: risk.shellLines })}</li>}
      </ul>
    </Box>
  );
}

const MARK = /(⟦U\+[0-9A-F]{4,6}⟧)/;
function markTitle(m: string): string {
  const code = parseInt(m.slice(3, -1), 16);
  return (code >= 0x202a && code <= 0x202e) || (code >= 0x2066 && code <= 0x2069) || code === 0x200e || code === 0x200f || code === 0x061c ? t("extensions.add.risk.bidi") : t("extensions.add.risk.invisible");
}

function Pre({ text, max = "max-h-72" }: { text: string; max?: string }) {
  return (
    <pre className={`${max} overflow-auto rounded-lg border border-line bg-paper p-3 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap text-ink`}>
      {text.split(MARK).map((part, i) =>
        MARK.test(part) ? (
          <abbr key={i} title={markTitle(part)} className="rounded bg-danger/15 px-0.5 text-danger no-underline">
            {part}
          </abbr>
        ) : (
          part
        ),
      )}
    </pre>
  );
}

// ---------- link ----------

type LinkDone = { tone: "ok" | "warn" | "danger"; title?: string; text: Shown; approval?: string | null; remove?: string | null; cancelled?: boolean };

function pluginResult(r: PluginInstall, plugin: string): LinkDone {
  const status = t(r.marketplaceAdded ? "extensions.add.result.addedNotInstalled" : "extensions.add.result.notInstalled", { plugin });
  switch (r.outcome.kind) {
    case "done":
      return { tone: "ok", text: t("extensions.add.result.done", { plugin }) };
    case "needsApproval":
      return { tone: "warn", title: status, text: t("extensions.add.result.needsApproval"), approval: r.approvalCommand, remove: r.removeCommand };
    case "cancelled":
      return { tone: "warn", text: t("extensions.add.result.cancelled"), cancelled: true };
    case "timeout":
      return { tone: "danger", title: status, text: t("extensions.add.result.timeout"), remove: r.removeCommand };
    case "unchanged":
      return { tone: "warn", title: status, text: t("extensions.add.result.unchanged"), remove: r.removeCommand };
    default:
      return { tone: "danger", title: status, text: t("extensions.add.result.failed"), remove: r.removeCommand };
  }
}

function LinkPane({ initial }: { initial?: string }) {
  const t = useT();
  const qc = useQueryClient();
  const [input, setInput] = useState(initial ?? "");
  const [preview, setPreview] = useState<LinkPreview | null>(null);
  const [pick, setPick] = useState<string | null>(null);
  const [done, setDone] = useState<LinkDone | null>(null);
  const [used, setUsed] = useState(false);
  const inspect = useMutation({
    mutationFn: () => addApi.inspectLink(input),
    onMutate: () => {
      setDone(null);
      setUsed(false);
      if (preview) addApi.discardLink(preview.token);
      setPreview(null);
    },
    onSuccess: (p) => {
      setPreview(p);
      setPick(p.kind === "marketplace" ? (p.plugins[0]?.name ?? null) : null);
    },
  });
  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["plugins"] });
    qc.invalidateQueries({ queryKey: ["skills"] });
    qc.invalidateQueries({ queryKey: ["authoring-info"] });
  };
  const install = useMutation({
    mutationFn: async (): Promise<LinkDone> => {
      if (!preview) throw new Error("no preview");
      if (preview.kind === "skill") {
        const name = await addApi.installSkill(preview.token);
        return { tone: "ok", text: t("extensions.add.result.skillInstalled", { name }) };
      }
      return pluginResult(await addApi.installPlugin(preview.token, preview.kind === "marketplace" ? pick : null), preview.kind === "marketplace" ? (pick ?? "") : preview.id);
    },
    onSuccess: (d) => {
      setDone(d);
      // The preview stays visible for context; its token is used up, so the buttons turn off until a new preview.
      if (!d.cancelled) setUsed(true);
      refresh();
    },
    onError: (e) => setDone({ tone: "danger", text: failure(e) }),
  });
  const addOnly = useMutation({
    mutationFn: () => (preview ? addApi.addMarketplace(preview.token) : Promise.reject()),
    onSuccess: (o) => {
      if (o.kind === "cancelled") return;
      setUsed(true);
      setDone(o.kind === "done" ? { tone: "ok", text: t("extensions.add.result.marketplaceAdded") } : { tone: "danger", text: t("extensions.add.result.marketplaceFailed") });
      refresh();
    },
    onError: (e) => setDone({ tone: "danger", text: failure(e) }),
  });
  const busy = inspect.isPending || install.isPending || addOnly.isPending || used;

  return (
    <div className="flex flex-col gap-4">
      <form
        className="order-first space-y-1.5"
        onSubmit={(e) => {
          e.preventDefault();
          if (input.trim() && !inspect.isPending && !install.isPending && !addOnly.isPending) inspect.mutate();
        }}
      >
        <label htmlFor="add-link" className="text-xs font-medium text-ink-2">
          {t("extensions.add.link.label")}
        </label>
        <div className="flex gap-2">
          <input id="add-link" value={input} onChange={(e) => setInput(e.target.value)} placeholder={t("extensions.add.link.placeholder")} className={`${fieldClass} font-mono`} autoFocus />
          <Button type="submit" disabled={!input.trim() || inspect.isPending || install.isPending || addOnly.isPending}>
            {inspect.isPending ? t("extensions.add.link.checking") : t("extensions.add.link.preview")}
          </Button>
        </div>
        <p className="text-xs text-ink-3">
          {tx("extensions.add.link.help", {
            repo: <code>owner/repo</code>,
            skillDir: <code>{t("extensions.add.link.skillDirCode")}</code>,
            plugin: <code>{t("extensions.add.link.pluginCode")}</code>,
          })}
        </p>
      </form>

      {inspect.isPending && <Box tone="info">{t("extensions.add.link.inspecting")}</Box>}
      {inspect.isError && (
        <Box tone="danger">
          <p className="font-semibold">{t("extensions.add.link.cantInstall")}</p>
          <p>{String(inspect.error)}</p>
        </Box>
      )}

      {preview?.kind === "marketplace" && (
        <section className="space-y-3">
          <Header kind={t("extensions.add.marketplace.kind")} title={preview.market} sub={t("extensions.add.marketplace.sub", { source: preview.source, sha: shortSha(preview.sha) })} />
          {preview.description && <p className="text-[13px] text-ink-2">{preview.description}</p>}
          {preview.alreadyAdded && <Box tone="info">{t("extensions.add.marketplace.alreadyAdded")}</Box>}
          <fieldset className="space-y-1">
            <legend className="mb-1 text-xs font-medium text-ink-2">
              {preview.skipped
                ? t("extensions.add.marketplace.legendSkipped", { count: preview.plugins.length, skipped: preview.skipped })
                : t("extensions.add.marketplace.legend", { count: preview.plugins.length })}
            </legend>
            <ul className="max-h-64 divide-y divide-line overflow-auto rounded-lg border border-line">
              {preview.plugins.map((p) => (
                <li key={p.name}>
                  <label className="flex cursor-pointer items-start gap-2.5 px-3 py-2 hover:bg-paper">
                    <input type="radio" name="plugin" className="mt-1" checked={pick === p.name} onChange={() => setPick(p.name)} />
                    <span className="min-w-0 flex-1">
                      <span className="flex flex-wrap items-baseline gap-2">
                        <span className="font-mono text-[12.5px] font-medium">{p.name}</span>
                        {p.version && <span className="text-xs text-ink-3">v{p.version}</span>}
                        {p.origin !== "in_repo" && <span className="text-xs text-ink-3">{t("extensions.add.marketplace.external", { origin: p.origin === "unknown" ? t("extensions.add.marketplace.originUnknown") : p.origin })}</span>}
                        {p.declares.map((d) => (
                          <span key={d} className="rounded bg-warn-soft px-1.5 text-[11px] text-ink">
                            {d}
                          </span>
                        ))}
                      </span>
                      {p.description && <span className="block text-xs text-ink-2">{p.description}</span>}
                    </span>
                  </label>
                </li>
              ))}
            </ul>
          </fieldset>
          <Box tone="warn">{t("extensions.add.marketplace.warn")}</Box>
          <div className="flex flex-wrap gap-2">
            <Button variant="primary" disabled={!pick || busy} onClick={() => install.mutate()}>
              {install.isPending ? t("extensions.add.marketplace.installing") : pick ? t("extensions.add.marketplace.installNamed", { name: pick }) : t("extensions.add.marketplace.installPlugin")}
            </Button>
            {!preview.alreadyAdded && (
              <Button disabled={busy} onClick={() => addOnly.mutate()}>
                {t("extensions.add.marketplace.addOnly")}
              </Button>
            )}
          </div>
        </section>
      )}

      {preview?.kind === "plugin" && (
        <section className="space-y-3">
          <Header kind={t("extensions.add.plugin.kind")} title={preview.id} sub={t("extensions.add.plugin.sub")} />
          {preview.plugin?.description && <p className="text-[13px] text-ink-2">{preview.plugin.description}</p>}
          {preview.installed ? (
            <Box tone="info">{t("extensions.add.plugin.installed")}</Box>
          ) : (
            <>
              <Box tone="warn">{t("extensions.add.plugin.warn")}</Box>
              <Button variant="primary" disabled={busy} onClick={() => install.mutate()}>
                {install.isPending ? t("extensions.add.marketplace.installing") : t("extensions.add.plugin.install")}
              </Button>
            </>
          )}
        </section>
      )}

      {preview?.kind === "skill" && (
        <section className="space-y-3">
          <Header kind={t("extensions.add.skill.kind")} title={preview.name} sub={t("extensions.add.skill.sub", { source: preview.source, sha: shortSha(preview.sha) })} />
          {preview.description && <p className="text-[13px] text-ink-2">{preview.description}</p>}
          <p className="text-xs text-ink-3">
            {tx("extensions.add.skill.location", { path: <code className="font-mono text-ink-2">~/.claude/skills/{preview.name}</code> })}
          </p>
          <RiskBox risk={preview.risk} executables={preview.executables} />
          <div>
            <p className="mb-1 text-xs font-medium text-ink-2">
              {t("extensions.add.skill.files", { count: preview.files.length, size: `${(preview.totalBytes / 1024).toFixed(1)}KB` })}
              {preview.executables > 0 && <span className="ml-2 rounded bg-warn-soft px-1.5 text-ink">{t("extensions.add.skill.withExecutables", { count: preview.executables })}</span>}
            </p>
            <ul className="max-h-40 divide-y divide-line overflow-auto rounded-lg border border-line text-[12px]">
              {preview.files.map((f) => (
                <li key={f.path} className="flex items-center gap-2 px-3 py-1">
                  <FileCode2 size={12} className="text-ink-3" />
                  <span className="min-w-0 flex-1 truncate font-mono">{f.path}</span>
                  {f.executable && <span className="rounded bg-warn-soft px-1.5 text-[11px]">{t("extensions.add.skill.executable")}</span>}
                  <span className="text-ink-3 tabular-nums">{f.size.toLocaleString()}B</span>
                </li>
              ))}
            </ul>
            {preview.skipped.length > 0 && <p className="mt-1 text-xs text-ink-3">{t("extensions.add.skill.skipped", { files: preview.skipped.join(", ") })}</p>}
            <p className="mt-1 text-xs text-ink-3">{t("extensions.add.skill.noExecBit")}</p>
          </div>
          <div>
            <p className="mb-1 text-xs font-medium text-ink-2">{t("extensions.add.skill.skillMd")}</p>
            <Pre text={preview.skillMd} />
          </div>
          <div className="sticky -bottom-4 -mx-5 border-t border-line bg-sheet px-5 py-3">
            <Button variant="primary" disabled={busy} onClick={() => install.mutate()}>
              {install.isPending ? t("extensions.add.marketplace.installing") : t("extensions.add.skill.install")}
            </Button>
          </div>
        </section>
      )}

      {done && (
        <div role="status" className="order-first">
          <Box tone={done.tone}>
            {done.title && <p className="font-semibold">{done.title}</p>}
            <p>{shown(done.text)}</p>
            {done.approval && <CopyLine label={t("extensions.add.result.approval")} text={done.approval} />}
            {done.remove && (
              <div className="mt-2 opacity-80">
                <CopyLine label={t("extensions.add.result.remove")} text={done.remove} />
              </div>
            )}
          </Box>
        </div>
      )}
      {used && <p className="text-xs text-ink-3">{t("extensions.add.result.again")}</p>}
    </div>
  );
}

function Header({ kind, title, sub }: { kind: string; title: string; sub: string }) {
  return (
    <div>
      <p className="text-xs text-ink-3">{kind}</p>
      <p className="font-mono text-[15px] font-semibold">{title}</p>
      <p className="text-xs text-ink-3">{sub}</p>
    </div>
  );
}

// ---------- direct create ----------

/** The sidebar's current project, when items can be saved there (a real folder, not home). */
function useTargetProject(): { id: string; name: string; path: string | null } | null {
  const { project, projectId } = useScope("folder");
  return project && projectId ? { id: projectId, name: project.name, path: project.path } : null;
}

/** Where to save: mine or the current project (project-switcher: no project list here, the sidebar picks it). */
function KindAndPlace({ kind, setKind, project, setProject }: { kind: AuthorKind; setKind?: (k: AuthorKind) => void; project: string | null; setProject?: (p: string | null) => void }) {
  const t = useT();
  const target = useTargetProject();
  return (
    <div className="flex flex-wrap items-center gap-3">
      {setKind && <Segmented label={t("extensions.add.place.kind")} value={kind} options={KINDS.map((id) => ({ id, label: kindLabel(id) }))} onChange={setKind} />}
      {setProject &&
        (target ? (
          <Segmented
            label={t("extensions.add.place.dest")}
            value={project === target.id ? "project" : "user"}
            onChange={(v) => setProject(v === "project" ? target.id : null)}
            options={[
              { id: "user", label: t("extensions.add.place.global") },
              { id: "project", label: t("extensions.add.place.project", { name: target.name }) },
            ]}
          />
        ) : (
          <span className="text-xs text-ink-3">{t("extensions.add.place.globalOnly")}</span>
        ))}
    </div>
  );
}

function CreatePane({ initialKind, onAsk }: { initialKind: AuthorKind; onAsk: () => void }) {
  const t = useT();
  const qc = useQueryClient();
  const info = useQuery({ queryKey: ["authoring-info"], queryFn: addApi.info });
  const list = useQuery({ queryKey: ["skills"], queryFn: api.skillCatalog });
  const [created, setCreated] = useState<string | null>(null);
  const [kind, setKind] = useState<AuthorKind>(initialKind);
  const [project, setProject] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [desc, setDesc] = useState("");
  // `null` = follow the template; text = the user edited the body.
  const [body, setBody] = useState<string | null>(null);
  const template = info.data?.templates[kind] ?? "";
  const content = body ?? fillTemplate(template, name, desc);
  const target = useTargetProject();
  const projectPath = project && target?.id === project ? target.path : null;
  const taken = !!name && (list.data ?? []).some((s) => s.kind === kind && s.name === name && (project ? s.source === "project" && s.scope === projectPath : s.source === "user"));
  const nErr = nameError(name) ?? (taken ? t("extensions.add.create.taken") : null);
  // Workflows: Rust's meta verdict 400 ms after the last edit; a known problem turns the button off (D-183).
  const wf = useWorkflowCheck(content, name, kind === "workflow" && !!name && !nameError(name));
  const metaBad = kind === "workflow" && metaBlocks(wf.check, false);
  const create = useMutation({
    mutationFn: () => addApi.create(kind, project, name, content),
    onSuccess: () => {
      setCreated(`${kind === "command" ? "/" : ""}${name}`);
      setName("");
      setDesc("");
      setBody(null);
      qc.invalidateQueries({ queryKey: ["skills"] });
      qc.invalidateQueries({ queryKey: ["authoring-info"] });
    },
  });
  return (
    <form
      className="space-y-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (name && !nErr && !metaBad) create.mutate();
      }}
    >
      <KindAndPlace
        kind={kind}
        setKind={(k) => {
          setKind(k);
          setBody(null);
          create.reset();
        }}
        project={project}
        setProject={setProject}
      />
      {kind === "workflow" && (
        <Box tone={info.data?.workflowsDisabled ? "warn" : "info"}>
          {info.data?.workflowsDisabled && <p className="font-semibold">{t("extensions.add.create.workflowsDisabled")}</p>}
          <p>
            {t("extensions.add.create.workflowNote")}{" "}
            <button type="button" className="underline" onClick={onAsk}>
              {t("extensions.add.create.askInstead")}
            </button>
          </p>
          <details className="mt-1">
            <summary className="cursor-pointer">{t("extensions.add.create.rules")}</summary>
            <ul className="mt-1 list-disc pl-5">
              <li>{t("extensions.add.create.rule1", { code: "export const meta = { name, description, phases }" })}</li>
              <li>{t("extensions.add.create.rule2")}</li>
              <li>{t("extensions.add.create.rule3")}</li>
            </ul>
          </details>
        </Box>
      )}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <label className="block space-y-1">
          <span className="text-xs font-medium text-ink-2">{t("extensions.add.create.name")}</span>
          <input value={name} onChange={(e) => (setName(e.target.value.trim()), setCreated(null))} placeholder={kind === "command" ? "standup" : kind === "skill" ? "pr-writer" : "triage"} className={`${fieldClass} font-mono`} />
          {nErr && <span className="block text-xs text-danger">{nErr}</span>}
        </label>
        <label className="block space-y-1">
          <span className="text-xs font-medium text-ink-2">{t("extensions.add.create.description")}</span>
          <input value={desc} onChange={(e) => setDesc(e.target.value)} placeholder={kind === "skill" ? t("extensions.add.create.descSkill") : t("extensions.add.create.descOther")} className={fieldClass} />
        </label>
      </div>
      <label className="block space-y-1">
        <span className="flex items-baseline gap-2 text-xs font-medium text-ink-2">
          {t("extensions.add.create.content")}
          {body !== null && (
            <button type="button" className="ml-auto font-normal text-ink-3 underline" onClick={() => setBody(null)}>
              {t("extensions.add.create.resetTemplate")}
            </button>
          )}
        </span>
        <textarea value={content} onChange={(e) => setBody(e.target.value)} rows={14} spellCheck={false} className={`${fieldClass} font-mono text-[12px] leading-relaxed`} />
      </label>
      {kind === "workflow" && name && !nameError(name) && <WorkflowCheckNote check={wf.check} name={name} />}
      <p className="text-xs text-ink-3">
        {tx("extensions.add.create.target", { path: <code className="font-mono text-ink-2">{targetPath(kind, projectPath, name)}</code> })}
      </p>
      {create.isError && <Box tone="danger">{String(create.error)}</Box>}
      {created && !create.isError && <Box tone="ok">{t("extensions.add.create.created", { name: created })}</Box>}
      <Button type="submit" variant="primary" disabled={!name || !!nErr || metaBad || create.isPending}>
        {create.isPending ? t("extensions.add.create.creating") : t("extensions.add.create.submit", { kind: kindNoun(kind) })}
      </Button>
    </form>
  );
}

// ---------- ask Claude ----------

function ProducedItem({ job, item }: { job: Job; item: Produced }) {
  const t = useT();
  const qc = useQueryClient();
  const [open, setOpen] = useState(false);
  const [project, setProject] = useState<string | null>(null);
  // Permission warnings are shown by RiskBox below.
  const warnings = item.warnings.filter((w) => !/^(allowed-tools|hooks|!`)/.test(w));
  const install = useMutation({
    mutationFn: () => addApi.installResult(job.token, item.kind, item.name, project),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["skills"] });
      qc.invalidateQueries({ queryKey: ["authoring-info"] });
    },
  });
  return (
    <li className="rounded-lg border border-line">
      <button className="flex w-full items-center gap-2 px-3 py-2 text-left" onClick={() => setOpen((v) => !v)} aria-expanded={open}>
        <span className="rounded bg-paper px-1.5 text-[11px] text-ink-2">{kindLabel(item.kind)}</span>
        <span className="font-mono text-[12.5px] font-medium">{item.kind === "command" ? `/${item.name}` : item.name}</span>
        <span className="text-xs text-ink-3">{t("extensions.add.produced.files", { count: item.files.length })}</span>
        {item.problem ? <span className="ml-auto text-xs text-danger">{t("extensions.add.produced.cannotInstall")}</span> : <span className="ml-auto text-xs text-ink-3">{open ? t("extensions.add.produced.collapse") : t("extensions.add.produced.review")}</span>}
      </button>
      {open && (
        <div className="space-y-2 border-t border-line px-3 py-2">
          {item.problem && <Box tone="danger">{item.problem}</Box>}
          {warnings.length > 0 && (
            <Box tone="warn">
              <ul className="list-disc pl-4">
                {warnings.map((w) => (
                  <li key={w}>{w}</li>
                ))}
              </ul>
            </Box>
          )}
          <RiskBox risk={item.risk} executables={item.files.filter((f) => /\.(sh|bash|zsh|py|js|mjs|cjs|ts|rb|pl)$|^[^.]+$|\/[^./]+$/.test(f)).length} />
          {item.files.length > 1 && <p className="text-xs text-ink-3">{t("extensions.add.produced.fileList", { files: item.files.join(", ") })}</p>}
          <Pre text={item.content} max="max-h-60" />
          {!item.problem && (
            <div className="flex flex-wrap items-center gap-2">
              <KindAndPlace kind={item.kind} project={project} setProject={setProject} />
              <Button variant="primary" disabled={install.isPending || install.isSuccess} onClick={() => install.mutate()}>
                {install.isSuccess ? t("extensions.add.produced.installed") : install.isPending ? t("extensions.add.produced.installing") : t("extensions.add.produced.install")}
              </Button>
            </div>
          )}
          {install.isError && <Box tone="danger">{String(install.error)}</Box>}
          {install.isSuccess && <Box tone="ok">{t("extensions.add.produced.installedNote")}</Box>}
        </div>
      )}
    </li>
  );
}

function statusOf(job: Job, agents?: AgentsPayload): { text: string; running: boolean; key?: string } {
  const s = agents?.sessions.find((x) => x.id === job.bgId);
  if (!s) return { text: job.produced.length ? t("extensions.add.status.ended") : t("extensions.add.status.noRecord"), running: false };
  const running = s.status === "working" || s.status === "waitingInput" || s.status === "idle";
  const known = ["working", "waitingInput", "idle", "done", "failed", "stopped"] as const;
  const k = known.find((x) => x === s.status);
  return { text: k ? t(`extensions.add.status.${k}`) : s.status, running, key: s.key };
}

function TerminalCommand() {
  const t = useT();
  const qc = useQueryClient();
  const info = useQuery({ queryKey: ["authoring-info"], queryFn: addApi.info });
  const state = info.data?.dduckNew;
  const toggle = useMutation({
    mutationFn: () => (state === "ours" ? addApi.removeCommand() : addApi.installCommand()),
    onSettled: () => {
      qc.invalidateQueries({ queryKey: ["authoring-info"] });
      qc.invalidateQueries({ queryKey: ["skills"] });
    },
  });
  const on = state === "ours";
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-lg border border-line px-3 py-2 text-xs text-ink-2">
      <span className="mr-auto">
        {tx("extensions.add.terminal.line", { command: <code className="font-mono text-ink">/dduck-new</code> })} <span className="text-ink-3">{t("extensions.add.terminal.note")}</span>
      </span>
      <button
        role="switch"
        aria-checked={on}
        aria-label={t("extensions.add.terminal.switchLabel")}
        disabled={!state || state === "other" || toggle.isPending}
        onClick={() => toggle.mutate()}
        className={`relative h-5 w-9 shrink-0 rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${on ? "bg-ok" : "bg-ink-3/40"}`}
      >
        <span className={`absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-sheet shadow transition-transform ${on ? "translate-x-4" : ""}`} />
      </button>
      {state === "other" && <span className="w-full text-ink-3">{t("extensions.add.terminal.taken")}</span>}
      {toggle.isError && <span className="w-full text-danger">{String(toggle.error)}</span>}
    </div>
  );
}

function AskPane({ initialKind }: { initialKind: AuthorKind }) {
  const t = useT();
  const qc = useQueryClient();
  const agents = useQuery({ queryKey: ["agents"], queryFn: api.agents, staleTime: 15_000 }).data;
  const jobs = useQuery({ queryKey: ["authoring-jobs"], queryFn: addApi.jobs, refetchInterval: 5_000 });
  const [kind, setKind] = useState<AuthorKind>(initialKind);
  const [request, setRequest] = useState("");
  const start = useMutation({
    mutationFn: () => addApi.startJob(kind, request),
    onSuccess: () => {
      setRequest("");
      qc.invalidateQueries({ queryKey: ["authoring-jobs"] });
      qc.invalidateQueries({ queryKey: ["agents"] });
    },
  });
  const discard = useMutation({ mutationFn: addApi.discardJob, onSettled: () => qc.invalidateQueries({ queryKey: ["authoring-jobs"] }) });
  return (
    <div className="space-y-4">
      <form
        className="space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          if (request.trim()) start.mutate();
        }}
      >
        <KindAndPlace kind={kind} setKind={setKind} project={null} />
        <label className="block space-y-1">
          <span className="text-xs font-medium text-ink-2">{t("extensions.add.ask.question", { kind: kindNoun(kind) })}</span>
          <textarea value={request} onChange={(e) => setRequest(e.target.value)} rows={4} placeholder={t("extensions.add.ask.placeholder")} className={fieldClass} />
        </label>
        <Box tone="info">
          {t("extensions.add.ask.info")}
        </Box>
        <GateLine force={start.isError ? isGateError(start.error) : null} />
        {start.isError && !isGateError(start.error) && <Box tone="danger">{messageOf(start.error)}</Box>}
        {start.isSuccess && <Box tone="ok">{t("extensions.add.ask.started")}</Box>}
        <Button type="submit" variant="primary" disabled={!request.trim() || start.isPending}>
          <Sparkles size={13} /> {start.isPending ? t("extensions.add.ask.starting") : t("extensions.add.modes.ask")}
        </Button>
      </form>
      <TerminalCommand />
      <section>
        <h3 className="mb-2 text-[13px] font-semibold">{t("extensions.add.ask.jobs")}</h3>
        {(jobs.data ?? []).length === 0 ? (
          <p className="text-xs text-ink-3">{t("extensions.add.ask.none")}</p>
        ) : (
          <ul className="space-y-3">
            {(jobs.data ?? []).map((j) => {
              const st = statusOf(j, agents);
              return (
                <li key={j.token} className="rounded-xl border border-line p-3">
                  <div className="mb-2 flex flex-wrap items-baseline gap-2">
                    <span className="text-[13px] font-medium">{j.title || t("extensions.add.create.submit", { kind: kindNoun(j.kind) })}</span>
                    <span className="text-xs text-ink-3">
                      {kindLabel(j.kind)} · {dateTime(j.createdMs)} · {st.text}
                    </span>
                    {st.key && (
                      <button className="ml-auto text-xs text-ink-2 underline hover:text-ink" onClick={() => window.dispatchEvent(new CustomEvent("dduck-open-task", { detail: st.key }))}>
                        {t("extensions.add.ask.openBoard")}
                      </button>
                    )}
                    <button className={`${st.key ? "" : "ml-auto "}text-xs text-ink-3 hover:text-ink disabled:opacity-40`} disabled={st.running || discard.isPending} onClick={() => discard.mutate(j.token)}>
                      {t("extensions.add.ask.dismiss")}
                    </button>
                  </div>
                  {j.produced.length === 0 ? (
                    <p className="text-xs text-ink-3">{st.running ? t("extensions.add.ask.making") : t("extensions.add.ask.noFiles")}</p>
                  ) : (
                    <ul className="space-y-2">
                      {j.produced.map((p) => (
                        <ProducedItem key={`${p.kind}-${p.name}`} job={j} item={p} />
                      ))}
                    </ul>
                  )}
                  {j.stray.length > 0 && <p className="mt-2 text-xs text-ink-3">{t("extensions.add.ask.stray", { files: j.stray.join(", ") })}</p>}
                </li>
              );
            })}
          </ul>
        )}
      </section>
    </div>
  );
}

// ---------- sheet ----------

export function AddSheet({ mode, onMode, onClose, kind = "skill", linkOnly = false }: { mode: AddMode; onMode: (m: AddMode) => void; onClose: () => void; kind?: AuthorKind; linkOnly?: boolean }) {
  const t = useT();
  useEffect(() => {
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("keydown", esc);
    return () => document.removeEventListener("keydown", esc);
  }, [onClose]);
  return (
    <div className="fixed inset-0 z-30 flex justify-end bg-ink/20" onClick={onClose}>
      <div role="dialog" aria-modal="true" aria-label={t("extensions.add.button")} onClick={(e) => e.stopPropagation()} className="flex h-full w-[min(760px,100vw)] flex-col border-l border-line bg-sheet shadow-e3">
        <div className="flex flex-wrap items-center gap-3 border-b border-line px-5 py-3">
          <h2 className="mr-auto text-[15px] font-semibold">{linkOnly ? t("extensions.add.modes.link") : t("extensions.add.button")}</h2>
          {!linkOnly && <Segmented label={t("extensions.add.modes.label")} value={mode} options={MODES.map((id) => ({ id, label: t(`extensions.add.modes.${id}`) }))} onChange={onMode} />}
          <button aria-label={t("common.actions.close")} onClick={onClose} className="rounded-md p-1 text-ink-3 hover:text-ink">
            <X size={16} />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-auto px-5 py-4">
          {(linkOnly || mode === "link") && <LinkPane />}
          {!linkOnly && mode === "create" && <CreatePane initialKind={kind} onAsk={() => onMode("ask")} />}
          {!linkOnly && mode === "ask" && <AskPane initialKind={kind} />}
        </div>
      </div>
    </div>
  );
}
