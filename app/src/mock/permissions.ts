// Mock-mode data for the permissions tab (design review only).
import { mockProjectName, mockProjectPath } from "./projects";
import rawCatalog from "../../crates/dduck-core/data/permission_catalog.json";
import { pickLang } from "../lib/bundled";
import { backendLang } from "./lang";

// Bundled text fields are {ko, en} pairs (app-language), picked in the backend's language on every call like Rust.
const catalog = () => pickLang(rawCatalog, backendLang());

const view = (path: string, allow: string[], ask: string[], deny: string[], extra = {}) => ({
  path, exists: true, token: "t", allow, ask, deny, defaultMode: null as string | null, other: [] as [string, unknown][], keys: ["allow", "ask", "deny"], readonlyReason: null as string | null, managedOnly: false, error: null, ...extra,
});
const risk = { hooks: false, mcpServers: false, dangerMode: false, unreadable: false };
const agent = (id: string, name: string, source: string, tools: string[], disallowedTools: string[], extra = {}) => ({
  id, name, source, scope: source === "project" ? "/Users/me/Agents/dev_team" : null, path: `/Users/me/Agents/dev_team/.claude/agents/${name}.md`, description: null, model: null,
  tools, disallowedTools, toolsRaw: tools, disallowedRaw: disallowedTools, malformed: false, editable: source !== "plugin", readonlyReason: source === "plugin" ? "플러그인 agent는 읽기 전용이에요" : null, primary: true, usable: true, risk, ...extra,
});

export const permissionHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  permission_scopes: (a) => {
    const id = (a.project as string | null) ?? null;
    const project = mockProjectPath(id);
    return {
      catalog: catalog(),
      project: id,
      projectName: mockProjectName(id),
      user: view("/Users/me/.claude/settings.json", ["Read", "Grep", "Glob", "Bash(git status:*)", "Bash(git diff:*)", "Bash(npm run:*)", "Bash(git:*)", "WebSearch"], ["Bash(git push:*)", "WebFetch"], ["Bash(sudo:*)", "Read(./.env)", "Read(~/.ssh/**)"], {
        defaultMode: "default",
        other: [["additionalDirectories", ["../shared-docs"]]],
        keys: ["allow", "ask", "deny", "defaultMode", "additionalDirectories"],
      }),
      shared: project ? view(`${project}/.claude/settings.json`, ["Edit", "Write", "Bash(npm test:*)", "mcp__notion__search", "weird rule(("], [], ["Bash(git push:*)", "Bash(rm -rf:*)"], { keys: ["allow", "deny"] }) : null,
      local: project ? { ...view(`${project}/.claude/settings.local.json`, [], [], []), exists: false, token: "" } : null,
      managed: { sources: [], mdm: null, managedOnly: false },
      agents: [
        agent("f-design-critic", "design-critic", "project", ["Read", "Grep", "Glob"], ["Edit", "Write"]),
        agent("f-qa-verifier", "qa-verifier", "project", ["Read", "Bash"], []),
        agent("f-release-helper", "release-helper", "user", [], ["Bash(git push:*)"]),
        agent("f-code-reviewer", "agent-skills:code-reviewer", "plugin", ["Read", "Grep"], []),
      ],
      modes: ["default", "acceptEdits", "plan", "auto", "dontAsk"],
    };
  },
  save_permissions: () => undefined,
  // Rough stand-in for the Rust matcher: exact rules in the draft, then `Bash(git:*)` from the user scope.
  permission_effective: (a) => {
    const e = a.edit as { allow: string[]; ask: string[]; deny: string[] };
    const rows = catalog().items.map((i) => {
      for (const l of ["deny", "ask", "allow"] as const) if (e[l].includes(i.rule)) return { rule: i.rule, decision: l, byRule: i.rule, byScope: "이 범위" };
      if (a.scope !== "user" && i.rule === "Bash(git push:*)") return { rule: i.rule, decision: "ask", byRule: "Bash(git push:*)", byScope: "전역", byScopeCode: "user" };
      if (i.rule.startsWith("Bash(git ") && (a.scope !== "user" || e.allow.includes("Bash(git:*)"))) return { rule: i.rule, decision: "allow", byRule: "Bash(git:*)", byScope: "전역", byScopeCode: "user" };
      if (i.rule === "Bash(ls:*)") return { rule: i.rule, decision: "allow", byRule: null, byScope: null };
      return { rule: i.rule, decision: "none", byRule: null, byScope: null };
    });
    return { rows, mdmNote: null };
  },
  permission_rule_covers: (a) => (String(a.rule).startsWith("Bash(git") && !String(a.rule).includes(" ") ? ["Bash(git push:*)", "Bash(git push --force:*)", "Bash(git reset --hard:*)"] : []),
  save_agent_tools: () => undefined,
  explain_permission: (a) => {
    const call = String(a.call ?? "");
    const notes = ["참고용이에요. 권한 모드·훅·CLI 플래그·샌드박스·bypass 모드는 반영하지 않아요"];
    if (call.startsWith("Bash(git push"))
      return { decision: "deny", rule: "Bash(git push:*)", scope: "프로젝트(공유)", list: "deny", notes: [...notes, "Bash 접두 거부는 `git -C x push`, `env X=1 …` 같은 형태로 우회될 수 있어요"] };
    if (call.startsWith("Read")) return { decision: "allow", rule: "Read", scope: "전역", scopeCode: "user", list: "allow", notes };
    return { decision: "none", rule: null, scope: null, list: null, notes: ["맞는 규칙이 없어요 → 권한 모드에 따라요(보통 확인 창)", ...notes] };
  },
};
