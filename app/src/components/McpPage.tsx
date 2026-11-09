// MCP tab (nav-ia, D-114): configured MCP servers straight from the settings files — name, service icon, transport
// and scope (전역 · 이 프로젝트 · 플러그인 제공). Opening the tab runs `claude mcp list` once per app run from the home
// folder (D-138, replaces "opening starts nothing"); "다시 확인" runs it in the current project. Rust keeps the cool-down
// and one run at a time.
import { useQuery } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { api, type McpServer } from "../lib/api";
import { ConnectorIcon } from "../lib/connectorIcons";
import { dateTime } from "../lib/format";
import { mcpDisplayName, transportLabel } from "../lib/mcpNames";
import { useScope } from "./ProjectContext";
import { ScopeBar } from "./ScopeBar";
import { Badge, Button, Empty, PageBody, PageHeader, Sheet } from "./ui";
import { COPY, useAuth } from "../lib/auth";
import { useT } from "../i18n/react";

type Row = {
  key: string;
  /** Name as `claude mcp list` prints it (status matching, icon). */ id: string;
  name: string;
  host: string | null;
  transport: string | null;
  scope: string;
  group: "user" | "project" | "plugin" | "claudeAi" | "other";
  /** Shown on the project side. */ current: boolean;
  /** Shown on the 전역 side. */ global: boolean;
};

const SCOPE_KEY = { user: "user", local: "local", project: "project" } as const satisfies Record<McpServer["scope"], string>;
const GROUP_KEY = { user: "user", project: "project", plugin: "plugin", claudeAi: "claudeAi", other: "extra" } as const satisfies Record<Row["group"], string>;
const STATE: Record<
  string,
  { key: "connected" | "needsAuth" | "failed" | "unknown"; tone: "ok" | "warn" | "danger" | "plain" }
> = {
  connected: { key: "connected", tone: "ok" },
  needsAuth: { key: "needsAuth", tone: "warn" },
  failed: { key: "failed", tone: "danger" },
  other: { key: "unknown", tone: "plain" },
};

/** `claude mcp list` names: `notion`, `plugin:expo:expo`, `claude.ai Gmail`. */
function statusKey(name: string): string {
  return name.toLowerCase();
}

export function McpPage() {
  const t = useT();
  const { projectId, project } = useScope("folder");
  const auth = useAuth().view;
  // claude.ai connectors work only with a claude.ai login; unknown says nothing (claude-login).
  const needsClaudeAi = auth !== null && auth.login !== "unknown" && !auth.claudeAi;
  const [only, setOnly] = useState<"global" | "project">("global");
  const scope = only === "project" && projectId ? "project" : "global";
  const list = useQuery({
    queryKey: ["mcp-servers", projectId],
    queryFn: () => api.mcpServers(projectId),
  });
  const plugins = useQuery({ queryKey: ["plugins"], queryFn: api.plugins });
  // Opening the tab loads the connectors once per app run (D-138), from the home folder: claude.ai connectors and
  // the user's own servers, never an unconfirmed project's .mcp.json. "다시 확인" checks in the current project (Rust
  // asks first in a folder Claude does not trust). The result is shared by every mount, so leaving the tab mid-check
  // neither drops it nor starts a second run.
  const runIn = useRef<string | null>(null);
  const sq = useQuery({
    queryKey: ["mcp-status"],
    queryFn: async () => {
      const where = runIn.current;
      return { list: await api.mcpStatus(where), at: Date.now(), where };
    },
    staleTime: Infinity,
    gcTime: Infinity,
    retry: false,
    refetchOnMount: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });
  const status = sq.data ?? null;
  const checking = sq.isFetching;
  const error = sq.error ? String(sq.error) : null;
  const check = () => {
    runIn.current = projectId;
    void sq.refetch();
  };
  const rows: Row[] = [
    ...(list.data?.servers ?? []).map((s) => ({
      key: `${s.scope}:${s.name}`,
      id: s.name,
      name: s.name,
      host: s.host,
      transport: transportLabel(s.transport),
      scope: t(`extensions.mcp.scope.${SCOPE_KEY[s.scope]}`),
      group: (s.scope === "user" ? "user" : "project") as Row["group"],
      current: s.scope !== "user",
      global: s.scope === "user",
    })),
    ...(plugins.data ?? [])
      .filter((p) => p.enabled || p.projectEnabled)
      .flatMap((p) =>
        p.mcp.map((m) => {
          const id = `plugin:${p.name}:${m.name}`;
          return {
            key: id,
            id,
            name: mcpDisplayName(id),
            host: null,
            transport: transportLabel(m.kind),
            scope: t("extensions.mcp.scope.plugin", { name: p.name }),
            group: "plugin" as const,
            current:
              p.scope === "project" || p.scope === "local" || p.projectEnabled,
            global: p.enabled && p.scope !== "project" && p.scope !== "local",
          };
        }),
      ),
  ];
  const known = new Set(rows.map((r) => statusKey(r.id)));
  const extra: Row[] = (status?.list.connectors ?? [])
    .filter((c) => !known.has(statusKey(c.name)))
    // claude.ai connectors come with the account and apply in every project, so both scopes list them.
    .map((c): Row => ({
      key: `other:${c.name}`,
      id: c.name,
      name:
        c.kind === "claudeAi"
          ? c.name.replace(/^claude\.ai /, "")
          : mcpDisplayName(c.name),
      host: c.host,
      transport: null,
      scope: c.kind === "claudeAi" ? t("extensions.mcp.scope.account") : "Claude Code",
      group: c.kind === "claudeAi" ? "claudeAi" : "other",
      current: true,
      global: true,
    }));
  const state = new Map(
    (status?.list.connectors ?? []).map((c) => [statusKey(c.name), c.state]),
  );
  const shown = [...rows, ...extra].filter((r) =>
    scope === "global" ? r.global : r.current,
  );
  const groups = (["project", "user", "plugin", "claudeAi", "other"] as const)
    .map((g) => [g, shown.filter((r) => r.group === g)] as const)
    .filter(([, rs]) => rs.length > 0);
  const needsAuth = [...state.values()].includes("needsAuth");
  // Same server under two names (a direct `notion` and the claude.ai Notion connector): say so on both rows.
  const sameHost = (r: Row) =>
    r.host
      ? shown.find((o) => o !== r && o.host === r.host && o.group !== r.group)
      : undefined;
  return (
    <div>
      <PageHeader title="MCP">
        <span className="text-xs text-ink-3">
          {status
            ? t("extensions.mcp.checkedAt", { time: dateTime(status.at) })
            : t("extensions.mcp.runsBriefly")}
        </span>
        <Button
          disabled={checking}
          onClick={check}
          title={t("extensions.mcp.checkTitle")}
        >
          {checking
            ? t("extensions.mcp.checking")
            : status
              ? t("extensions.mcp.recheck")
              : t("extensions.mcp.check")}
        </Button>
      </PageHeader>
      <PageBody>
        <ScopeBar
          need="folder"
          value={only}
          onChange={setOnly}
          note={project ? undefined : t("extensions.mcp.scopeNote")}
        />
        {error && (
          <p role="alert" className="text-xs text-danger">
            {error}
          </p>
        )}
        {(list.data?.unreadable ?? []).length > 0 && (
          <p className="text-xs text-ink-2">
            {t("extensions.mcp.unreadable", { files: list.data?.unreadable.join(", ") ?? "" })}
          </p>
        )}
        {list.isError ? (
          <Sheet>
            <Empty title={t("extensions.mcp.loadFailed")}>
              {String(list.error)}
            </Empty>
          </Sheet>
        ) : groups.length === 0 ? (
          <Sheet>
            <Empty
              title={
                list.isPending
                  ? t("extensions.mcp.loading")
                  : t("extensions.mcp.empty")
              }
            >
              {!list.isPending && t("extensions.mcp.emptyHint")}
            </Empty>
          </Sheet>
        ) : null}
        {!list.isError &&
          groups.map(([g, rs]) => (
            <Sheet key={g} className="overflow-hidden">
              <h2 className="border-b border-line px-4 py-2 text-xs font-semibold text-ink-2">
                {t(`extensions.mcp.group.${GROUP_KEY[g]}`)}{" "}
                <span className="font-normal text-ink-3">{rs.length}</span>
                {g === "claudeAi" && needsClaudeAi && <span className="ml-2 font-normal text-ink-3">· {COPY.connector}</span>}
              </h2>
              <ul className="divide-y divide-line">
                {rs.map((r) => {
                  const st = state.get(statusKey(r.id));
                  return (
                    <li
                      key={r.key}
                      className={`flex items-center gap-3 px-4 py-2.5 ${scope === "global" && r.current && r.group !== "claudeAi" && r.group !== "other" ? "border-l-2 border-l-ink pl-[14px]" : ""}`}
                    >
                      <ConnectorIcon name={r.id} host={r.host} />
                      <span className="min-w-0 flex-1">
                        <span
                          className="block truncate text-[13px] font-medium"
                          title={r.id}
                        >
                          {r.name}
                        </span>
                        <span className="block truncate text-xs text-ink-3">
                          {[
                            r.scope,
                            r.transport,
                            r.host,
                            sameHost(r) &&
                              t("extensions.mcp.sameServer", { name: sameHost(r)?.name ?? "" }),
                          ]
                            .filter(Boolean)
                            .join(" · ")}
                        </span>
                      </span>
                      {st ? (
                        <Badge pill tone={STATE[st].tone}>
                          {t(`extensions.mcp.state.${STATE[st].key}`)}
                        </Badge>
                      ) : (
                        <span className="text-xs text-ink-3">
                          {!status
                            ? t("extensions.mcp.notChecked")
                            : r.group === "project" && status.where === null
                              ? t("extensions.mcp.recheckHere")
                              : t("extensions.mcp.notInList")}
                        </span>
                      )}
                    </li>
                  );
                })}
              </ul>
            </Sheet>
          ))}
        {!status && !list.isError && (
          <div data-testid="claude-ai-hint">
            <Sheet className="overflow-hidden">
              <h2 className="border-b border-line px-4 py-2 text-xs font-semibold text-ink-2">
                {t("extensions.mcp.group.claudeAi")}
                {needsClaudeAi && <span className="ml-2 font-normal text-ink-3">· {COPY.connector}</span>}
              </h2>
              <div className="flex flex-wrap items-center gap-3 px-4 py-3">
                <p className="min-w-0 flex-1 text-[13px] text-ink-2">
                  {checking
                    ? t("extensions.mcp.connectorsLoading")
                    : t("extensions.mcp.connectorsFailed")}
                </p>
                {!checking && <Button onClick={check}>{t("extensions.mcp.loadConnectors")}</Button>}
              </div>
            </Sheet>
          </div>
        )}
        {needsAuth && (
          <p className="text-xs text-ink-3">
            {t("extensions.mcp.needsAuthHelp")}
          </p>
        )}
        <p className="text-xs text-ink-3">
          {t("extensions.mcp.manageHelp")}
        </p>
      </PageBody>
    </div>
  );
}
