import type ko from "../ko/news";
import type { Shape } from "../types";

export default {
  title: "News",
  summary: { one: "{count} new Claude Code feature", other: "{count} new Claude Code features" },
  fetch: {
    button: "Fetch",
    running: "Fetching…",
    cancel: "Cancel",
    cost: "About $0.10–0.30 a run · You confirm first",
  },
  status: {
    never: "Not fetched yet",
    ok: { one: "Last fetched {when} · {count} new item", other: "Last fetched {when} · {count} new items" },
    cost: " · {cost}",
    dropped: { one: " · {count} left out for a bad format", other: " · {count} left out for a bad format" },
    failed: "Couldn't fetch at {when} · {reason}",
    daily: " (daily)",
  },
  daily: {
    label: "Fetch daily",
    note: "Once a day, up to about $0.40 a run",
    next: "Next fetch {when}",
    turnedOff: "Daily fetch wasn't turned on",
  },
  list: {
    label: "News list",
    earlier: "Earlier",
    recent: "New features",
    new: "New",
    meta: "v{version} · {date}",
  },
  category: {
    model: "Model",
    command: "Command",
    setting: "Setting",
    agent: "Agent",
    integration: "Integration",
    ide: "Editor",
    security: "Security",
    misc: "Other",
  },
  detail: {
    label: "{title} details",
    what: "What it is",
    when: "When to use it",
    best: "Best practice",
    open: "Open source",
    openFailed: "Couldn't open the source · {error}",
    note: "Claude summarized the official docs · Check the source",
  },
  empty: {
    title: "No news yet",
    body: "Fetch reads new Claude Code features from the official docs",
    otherLang: { one: "You have {count} item in another language · Fetch to get it in this one", other: "You have {count} items in another language · Fetch to get them in this one" },
  },
  reason: {
    budget: "Stopped at the budget limit ($0.40)",
    turns: "Stopped at the read and response limit",
    timeout: "Stopped because it didn't finish within 240 seconds",
    spawn: "Couldn't run the Claude CLI",
    failed: "The Claude CLI failed",
    loginRequired: "You need to log in",
    empty: "The reply was empty",
    malformed: "The answer wasn't in the expected format, so nothing was saved",
    busy: "Already fetching",
    guard: "Couldn't check the safety guard, so nothing ran",
  },
  error: {
    cancelled: "Cancelled",
    spent: " · Spent {cost}",
  },
} satisfies Shape<typeof ko>;
