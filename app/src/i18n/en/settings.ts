import type ko from "../ko/settings";
import type { Shape } from "../types";

export default {
  title: "Settings",
  language: {
    title: "Language",
    description: "Used for the app and the menu bar menu",
    more: "System language uses Korean when macOS is set to Korean, and English otherwise. Changes apply right away.",
  },
  agentSettings: {
    title: "Agent settings",
    description: "Change them from the gear on the Agents tab",
    more: "Team rules, task types, and turning off, hiding or deleting agents live there.",
    open: "Open agent settings (Agents tab)",
  },
  tour: {
    title: "Tutorial",
    description: "Replay the story that introduces the duck team",
    more: "The five-card story and first task card you saw on first launch.",
    open: "Replay tutorial",
  },
  cli: {
    title: "Claude CLI",
    description: "The claude executable used for tasks",
    more: "Used to start tasks and read their status.",
    notFound: "Not found",
    version: "Version {version}",
    unverified: " (unverified version)",
    pick: "Choose executable",
    auto: "Find automatically",
  },
  login: {
    title: "Claude login",
    description: "The connection used when you hand off tasks",
  },
  defaultCwd: {
    title: "Default task folder",
    description: "Prefilled for new tasks",
    more: "You can pick a different folder for each task.",
    none: "Not set",
    pick: "Choose folder",
    clear: "Clear",
  },
  configDir: {
    title: "Claude config folder",
    description: "Where usage history is read from",
    more: "You rarely need to change this.",
    isDefault: "Default",
    pick: "Choose folder",
    reset: "Use default",
  },
  storeTitles: {
    title: "Save session titles",
    description: "Show the titles Claude gives sessions in lists",
    more: "Turning this off deletes saved titles. Prompts and responses are never saved, whatever this is set to.",
  },
  notifyWaiting: {
    title: "Waiting for input alert",
    description: "Alerts you once when a task waits for an answer",
  },
  notifyFinished: {
    title: "Finished or failed alert",
    description: "A macOS notification when a duck finishes or fails a task",
    more: "Notifications show only the task name and state.",
  },
  limitNotify: {
    title: "Usage limit alert",
    description: "Alerts you when you reach a usage limit",
    more: "Shows only the reset time and how many tasks will continue.",
  },
  limitAuto: {
    title: "Continue automatically after reset",
    description: "Continuing uses your usage allowance",
    notShipped: "Not ready yet, so for now you get the alert and a 'Continue now' button.",
    more: "Only tasks started in D.Duck continue. Tasks with risky permissions, deploys, remote control, or that you stopped yourself don't. Your Mac must be awake.",
  },
  remote: {
    title: "Phone alerts (Remote Control)",
    description: "Alerts your phone when a task finishes or needs an answer",
    more: "Connects new tasks to Remote Control in the Claude app. You could then instruct and approve that task from your phone, so turning it on asks once more. Claude may skip the alert while you're at your Mac, and deploy, security and sensitive tasks are never connected.",
    toggle: "Phone alerts",
  },
  capacity: {
    title: "Review capacity",
    description: "How many code changes you can properly read in a day",
    more: "When dev, design and deploy tasks you haven't marked reviewed go over this number, starting a new task asks once more. No limit never asks.",
    count: { one: "{count} change", other: "{count} changes" },
    unlimited: "No limit",
  },
  storePrompts: {
    title: "Save task instructions",
    description: "Keep what you wrote so you can hand it off again from past tasks",
    more: "Keeps the latest 200 for up to 90 days. Stored in app data, so Time Machine backs it up. Turning this off asks whether to delete saved instructions.",
    wipe: "Delete all past tasks",
  },
  taskReport: {
    title: "Task reports",
    description: "Every task you hand off leaves a report",
    more: "Writing the report adds a little output. Evidence screenshots and test reports stay in the app folder on this Mac and are deleted with the task record.",
  },
  localOnlyMore: "{cloud}. Unchecking asks once more. {worktree}.",
  mcp: {
    title: "MCP servers",
    description: "claude.ai connectors are MCP servers too · See the list and status on the MCP tab",
    open: "Open MCP tab",
  },
  history: {
    title: "Usage history",
    files: { one: "Reading {count} history file", other: "Reading {count} history files" },
    rebuild: "Read again from the start",
    rebuilding: "Started reading again. Progress shows at the bottom left.",
    noneSkipped: "No lines were skipped.",
    skipped: { one: "{count} line skipped", other: "{count} lines skipped" },
  },
} satisfies Shape<typeof ko>;
