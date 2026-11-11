// Mock-mode model lists (model-select). Design review only: Rust builds them from the CLI help, the settings model keys,
// recent transcripts and stored values (model_select.rs). `?models=` picks a case: cli-fail, managed, empty, env.
const param = typeof location === "undefined" ? null : new URLSearchParams(location.search).get("models");

const item = (value: string, sources: string[], extra: Record<string, unknown> = {}) => ({ value, sources, allow: "yes", ...extra });

function options() {
  const recent = param === "empty" ? [] : [item("claude-opus-5-5", ["recent"], { price: 20 }), item("claude-sonnet-5-5", ["recent"], { price: 10 }), item("claude-haiku-5-5", ["recent"], { price: 0.5 })];
  const cli =
    param === "cli-fail"
      ? []
      : [
          item("fable", ["cli"], { price: 50 }),
          item("opus", ["cli"], param === "env" ? { resolved: { id: "claude-opus-4-8", basis: "settings" }, price: 25 } : param === "empty" ? {} : { resolved: { id: "claude-opus-5-5", basis: "recent" }, price: 20 }),
          item("sonnet", ["cli"], param === "empty" ? {} : { resolved: { id: "claude-sonnet-5-5", basis: "recent" }, price: 10 }),
        ];
  // A stored type model that is no longer supported (design 11).
  const custom = [item("claude-opus-3-9", ["custom"], { replacement: { kind: "alias", to: "opus" } })];
  let items = [...cli, ...(param === "env" ? [item("claude-opus-4-8", ["settings"], { price: 25 })] : []), ...recent, ...custom];
  if (param === "managed") {
    items = items.map((i) => (["sonnet", "claude-sonnet-5-5", "haiku", "claude-haiku-5-5"].includes(i.value) ? i : { ...i, allow: "no", blockedBy: "managed" }));
  }
  const currentDefault = param === "empty" ? { value: null, basis: "account" } : param === "env" ? { value: "claude-opus-4-8", basis: "env", price: 25 } : { value: "claude-opus-5-5", basis: "recent", price: 20 };
  return { items, currentDefault, cliRead: param !== "cli-fail", dropped: 0, allowlist: param === "managed" ? "managed" : "none" };
}

export const modelHandlers: Record<string, (a: Record<string, unknown>) => unknown> = {
  model_options: () => options(),
  save_agent_model: () => undefined,
};
