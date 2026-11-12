// Typed wrappers around the Rust IPC commands. Field names follow the Rust serde camelCase output.
import { invoke } from "@tauri-apps/api/core";
import koErrors from "../i18n/ko/errors";
import enErrors from "../i18n/en/errors";
import { fill, translate } from "../i18n";
import { currentLang, localeOf, type Lang } from "../i18n/language";
import type { Message, Plural, Tree } from "../i18n/types";

export interface Query {
  fromMs: number;
  toMs: number;
  tzOffsetMinutes: number;
  projects: string[];
  models: string[];
}

export interface Tokens {
  input: number;
  output: number;
  thinking: number;
  cacheWrite: number;
  cacheRead: number;
}

export interface Summary {
  tokens: Tokens;
  estCostUsd: number;
  unpricedModels: string[];
  sessions: number;
  responses: number;
  toolCalls: number;
  skillCalls: number;
  commandCalls: number;
}

export interface Point {
  bucket: string;
  tokens: Tokens;
  estCostUsd: number;
}

export type Dimension = "model" | "project" | "tool" | "skill" | "command" | "mcpServer" | "subagent";

export interface Item {
  key: string;
  count: number;
  share: number;
  viaTool: number;
  viaSlash: number;
  tokens: Tokens | null;
  estCostUsd: number | null;
  priced: boolean;
}

export interface SessionRow {
  sessionId: string;
  title: string | null;
  label?: string | null;
  projectName: string | null;
  gitBranch: string | null;
  firstTs: number;
  lastTs: number;
  responses: number;
  tokens: Tokens;
  estCostUsd: number;
  reportedCostUsd: number | null;
  /** Filled for the sessions page only: up to 3 tools, skills and subagent types with counts. */
  topTools?: NameCount[];
  skills?: NameCount[];
  subagents?: NameCount[];
}

export interface NameCount {
  name: string;
  count: number;
}

export interface SessionDetail {
  session: SessionRow;
  models: Item[];
  tools: Item[];
  skills: Item[];
}

export interface CliInfo {
  path: string;
  version: string;
  supported: boolean;
}

export interface Status {
  cli: CliInfo | null;
  configDir: string;
  configDirExplicit: boolean;
  notifyWaiting: boolean;
  notifyFinished: boolean;
  remoteAlerts: boolean;
  storeTaskPrompts: boolean;
  /** "작업 보고서" (decision-report, D-108); on by default. */
  taskReport: boolean;
  /** "사용량 한도 알림" (usage-limit-resume); on by default. */
  limitNotify: boolean;
  /** "리셋되면 자동으로 이어서 하기"; on by default. */
  limitAutoResume: boolean;
  /** Auto resume is built but off until it is measured (spike 1.1): everything is "직접 이어야 해요". */
  limitAutoShipped: boolean;
  storeTitles: boolean;
  permissionModes: string[];
  defaultCwd: string | null;
  /** "unlimited" = 제한 없음 (no over-capacity warning anywhere). */
  reviewCapacity: number | "unlimited";
}

export type SessionStatus = "working" | "waitingInput" | "idle" | "done" | "failed" | "stopped" | "unknown";

export interface AgentSession {
  key: string;
  pid: number | null;
  id: string | null;
  sessionId: string | null;
  name: string;
  cwd: string;
  kind: "interactive" | "background" | "other";
  startedAt: number;
  status: SessionStatus;
}

export type CliError =
  | { kind: "spawn" }
  | { kind: "timeout" }
  | { kind: "untrustedWorkspace"; stderr_tail: string }
  | { kind: "failed"; code: number; stderr_tail: string }
  | { kind: "unsupportedOutput" }
  | { kind: "invalidId" }
  /** `code` + `params` (app-language): word it with `messageOf`. `text` is Rust's sentence in the app language for a
   *  code the catalog doesn't know; `reason` is the same sentence under the old name (older screens show it). */
  | { kind: "invalidInput"; code?: string; params?: Record<string, string>; text?: string; reason: string }
  | { kind: "dangerNotConfirmed" }
  | { kind: "killFailed" }
  | { kind: "cancelled" }
  | { kind: "login_required" }
  | { kind: "claude_ai_required" };

export interface TaskTag {
  agent: string | null;
  taskType: string | null;
  reviewed?: boolean;
  /** 작업 보고서 for the list chip; absent when the task was not given a report. */
  report?: ReportPresence;
  /** Draft source of a D.Duck task, restored on "다시 맡기기". */
  origin?: TaskOrigin;
  /** Loadout skills left out at start because their file changed (agent-loadout). */
  loadoutDropped?: number;
  /** Board lane of an ended task (task-lifecycle); absent = still running, or 확인 필요. */
  lane?: Lane;
}

/** task-lifecycle: 리뷰 대기 (the review badge's rows), 정리, and 정리 끝 (off the board, "완료"). */
export type Lane = "reviewWait" | "wrapup" | "done";

/** A D.Duck record on the board; `listed` = the CLI still lists it (its live card is drawn instead). */
export interface BoardCard {
  bgId: string;
  name: string | null;
  folder: string | null;
  taskType: string | null;
  createdAt: number;
  reviewed: boolean;
  listed: boolean;
  /** How it last ended while listed; `null` = never seen ended ("목록에서 사라진 작업"). */
  ended: "done" | "failed" | "stopped" | "unknown" | null;
}

export interface BoardColumn {
  total: number;
  items: BoardCard[];
}

export interface BoardWrapup {
  /** total = the review badge. */
  reviewWait: BoardColumn;
  /** Reviewed records the CLI no longer lists. */
  wrapup: BoardColumn;
}

export type WrapupWorktree = { kind: "present"; folder: string; branch: string | null } | { kind: "gone" } | { kind: "unknown" };
export type WrapupRefusal = "running" | "changes" | "notChecked" | "noWorktree" | "resumePending";

/** 정리 점검표: local git facts only, no file names. */
export interface WrapupCheck {
  worktree: WrapupWorktree | null;
  changes: number | null;
  pr: { number: number; url: string } | null;
  guard: "local_only" | "draft_pr" | null;
  refusal: WrapupRefusal | null;
  /** "resumePending": a usage-limit resume is planned, so 정리 끝 is off too. */
  wrapRefusal: WrapupRefusal | null;
}

export interface AgentsPayload {
  sessions: AgentSession[];
  error: CliError | null;
  /** Persona and task type per session key. */
  tags?: Record<string, TaskTag>;
  /** Titles the user gave sessions, by session key. */
  labels?: Record<string, string>;
  /** 리뷰 탭 badge (review-inbox); absent when the listing failed. */
  reviewPending?: number;
}

export type AgentSource = "project" | "user" | "plugin" | "builtin";

export interface AgentRisk {
  hooks: boolean;
  mcpServers: boolean;
  dangerMode: boolean;
  unreadable: boolean;
}

export interface AgentDef {
  id: string;
  name: string;
  source: AgentSource;
  scope: string | null;
  path: string | null;
  description: string | null;
  model: string | null;
  tools: string[];
  /** Frontmatter `disallowedTools` (tool-permissions). */
  disallowedTools: string[];
  /** Frontmatter `skills`, preloaded at start (agent-loadout). */
  skills?: string[];
  malformed: boolean;
  editable: boolean;
  readonlyReason: string | null;
  primary: boolean;
  /** Usable as a persona in the folder the catalog was asked for. */
  usable: boolean;
  risk: AgentRisk;
}

export type ActState = "waiting" | "failed" | "working" | "done";

export interface Activity {
  agent: string;
  state: ActState;
  count: number;
  total: number;
  detail: string | null;
  sessionKey: string | null;
  subagent: boolean;
}

export interface TaskSpec {
  cwd: string;
  prompt: string;
  name: string | null;
  permissionMode: string | null;
  agent?: string | null;
  /** Tech-lead options; the backend builds roles, policy and tool lists itself (AG-09). */
  taskType?: string | null;
  effort?: string | null;
  sensitive?: boolean;
  worktree?: boolean;
  remoteControl?: boolean;
  /** Kept with the record for "다시 맡기기" (D-069); not sent to Claude. */
  brief?: { goal: string; outOfScope: string; constraints: string; done: string } | null;
  /** Draft from usage insights; the backend enforces plan mode for `insight-claude` and home-folder tasks. */
  origin?: TaskOrigin | null;
  /** "이번 작업 보고서" unchecked: this task gets no report (the webview can only turn it off). */
  skipReport?: boolean;
  /** Review/QA with the report setting off: a report for this task only (report-evidence). Ignored for other types. */
  wantReport?: boolean;
  /** "리셋되면 자동으로 이어서 하기" unchecked: never resumed by itself after a usage-limit reset. */
  limitOptOut?: boolean;
  /** This task's model (model-select); `null` = follow the agent, type or CLI default. */
  model?: string | null;
}

/** Fixed reason codes from Rust (`limit_resume::Reason`). */
export type LimitReason =
  | "not_dduck" | "no_reset" | "opt_out" | "cap" | "deploy" | "remote" | "danger" | "auto_off" | "not_shipped"
  | "user_stopped" | "already_resumed" | "cwd_mismatch" | "untrusted" | "link" | "risk_changed" | "copy_started"
  | "cli_failed" | "attached" | "interrupted" | "late" | "expired" | "cancelled" | "session_mismatch"
  | "guard_hook_missing" | "guard_unreadable" | "local_only_refused";

export type LimitState = "auto" | "manual" | "needs_choice" | "starting" | "resumed" | "closed";

/** One usage-limit stop. Ids, times and codes only. */
export interface LimitEntry {
  id: number;
  sessionId: string;
  bgId: string | null;
  resetsAt: number | null;
  dueAt: number | null;
  state: LimitState;
  reason: LimitReason | null;
  attempts: number;
  /** Task name, else the goal's first line; `null` when unknown (never replaced by an id). */
  name: string | null;
  startedAt: number | null;
  /** Local start time ("오후 3:12"), to tell same-named tasks apart. */
  startedLabel: string | null;
  /** Local reset time from Rust ("오후 1:50", or with date and weekday beyond 24 hours). */
  label: string | null;
  dueLabel: string | null;
}

export interface LimitView {
  entries: LimitEntry[];
  banner: { resetsAt: number; label: string; auto: number; manual: number; text: string } | null;
  late: number;
  unknown: number;
  autoShipped: boolean;
  autoOn: boolean;
}

/** `news`: "내 프로젝트에 적용" from a news item (news-apply); plan mode only, enforced by the backend. */
export type TaskOrigin = "insight-local" | "insight-claude" | "news";

export type RepoStatus = { kind: "notRepo" } | { kind: "homeRoot" } | { kind: "noCommits" } | { kind: "ok" };

export interface IngestFile {
  path: string;
  skippedLines: number;
  lines: number;
}

export interface AttachResult {
  opened: boolean;
  command: string;
}

export interface IngestStatus {
  phase: "backfill" | "idle" | "error";
  filesDone: number;
  filesTotal: number;
  error: string | null;
}

export interface ArtifactRow {
  artifactId: string;
  url: string;
  title: string | null;
  kind: string;
  seq: number;
  firstSeenTs: number;
  lastTs: number;
  projectName: string | null;
}

/** A card preview read from the artifact's local source (D-102). Failures reject with a `PreviewReason` code. */
export type ArtifactPreview = { kind: "html"; text: string; truncated: boolean; scriptOnly: boolean } | { kind: "markdown"; text: string; truncated: boolean };

export type Schedule =
  | { kind: "daily"; hour: number; minute: number }
  | { kind: "weekly"; weekdays: number; hour: number; minute: number }
  | { kind: "interval"; hours: number };

export type RoutineMode = "plan" | "manual" | "acceptEdits" | "dontAsk";

export interface RoutineInput {
  name: string;
  cwd: string;
  prompt: string;
  permissionMode: RoutineMode;
  schedule: Schedule;
}

export interface RoutineSummary {
  id: number;
  name: string;
  cwd: string;
  promptLen: number;
  permissionMode: RoutineMode;
  schedule: Schedule;
  scheduleText: string;
  enabled: boolean;
  nextRunAt: number | null;
  lastStatus: string | null;
  active: boolean;
}

export interface RoutineRun {
  id: number;
  scheduledFor: number;
  startedAt: number | null;
  endedAt: number | null;
  status: string;
  bgId: string | null;
  error: string | null;
}

export interface CloudRoutine {
  id: string;
  name: string;
  enabled: boolean;
  cron: string | null;
  runOnceAt: number | null;
  nextRunAt: number | null;
  endedReason: string | null;
  /** Local-time view D.Duck can edit; null = another cron, read-only (cloud-routine-edit). */
  schedule: CloudSchedule | null;
  /** Uses a "로컬만" project's repository (delivery-guard): D.Duck does not run or enable it. */
  localOnly?: boolean;
  /** Last fire, from the list response itself (D-166). */
  lastRun?: CloudLastRun | null;
}

export interface CloudLastRun {
  status: "succeeded" | "failed" | "running" | "unknown";
  /** API code without its prefix, e.g. ENVIRONMENT_NOT_FOUND. */
  failureReason: string | null;
  firedAt: number | null;
  finishedAt: number | null;
  sessionId: string | null;
}

/** One run session (D-166). `status` is the session state, not success/failure — that comes from the log. */
export interface CloudRun {
  id: string;
  title: string;
  status: string | null;
  workerStatus: string | null;
  createdAt: number | null;
  lastEventAt: number | null;
  url: string | null;
}

export interface CloudRuns {
  runs: CloudRun[];
  hasMore: boolean;
  costUsd: number | null;
}

/** Remote run content: data, never instructions. Shown as plain text only. */
export interface CloudLogEvent {
  at: number | null;
  kind: string;
  text: string;
}

export interface CloudRunLog {
  sessionId: string;
  events: CloudLogEvent[];
  outcome: "succeeded" | "failed" | null;
  hasOlder: boolean;
  costUsd: number | null;
}

/** Weekdays: bit 0 = Monday … bit 6 = Sunday (as local routines). */
export type CloudSchedule = { kind: "once"; at: number } | { kind: "daily"; hour: number; minute: number } | { kind: "weekly"; weekdays: number; hour: number; minute: number };

export interface CloudEditInput {
  enabled?: boolean;
  name?: string;
  schedule?: CloudSchedule;
}

export type CloudEditResult = { outcome: "declined" } | { outcome: "done"; routine: CloudRoutine; costUsd: number | null };

// News tab (news-tab, D-204). Items are Claude's checked explainers in the app language; render as text only.
export type NewsCategory = "model" | "command" | "setting" | "agent" | "integration" | "ide" | "security" | "other";
/** How a feature is taken up (news-apply): works after updating, needs a setting, or is picked when wanted. */
export type NewsAdopt = "update" | "config" | "opt_in";
export interface NewsItem {
  id: string;
  title: string;
  version: string | null;
  /** YYYY-MM-DD from the changelog. */
  date: string;
  category: NewsCategory;
  summary: string;
  what: string;
  whenToUse: string;
  bestPractice: string;
  sourceUrl: string;
  firstSeenMs: number;
  seen: boolean;
  // news-apply (v18): each optional and checked on its own; null for items stored before v18.
  /** The one fact shown big on the card (`/diff`, a setting name, a number), at most 24 characters. */
  keyFact?: string | null;
  adopt?: NewsAdopt | null;
  /** 1–3 one-line steps. */
  applySteps?: string[] | null;
  verify?: string | null;
}
export type NewsErrorKind = "budget" | "turns" | "timeout" | "cancelled" | "spawn" | "failed" | "login_required" | "empty" | "malformed" | "busy" | "guard";
export interface NewsStatus {
  atMs: number;
  ok: boolean;
  added: number;
  dropped: number;
  costUsd: number | null;
  error: NewsErrorKind | null;
  daily: boolean;
  dailyOff?: boolean;
}
export interface NewsView {
  items: NewsItem[];
  status: NewsStatus | null;
  daily: { enabled: boolean; nextAtMs: number | null; capped: boolean };
  otherLang: number;
  running: boolean;
}
export type NewsRun =
  | { kind: "done"; added: number; dropped: number; costUsd: number | null }
  | { kind: "declined" }
  | { kind: "failed"; error: { kind: NewsErrorKind; costUsd?: number | null }; message: string };

// Usage insights (D-078).
export type InsightLink = "plugins" | "skills" | "permissions" | "settings";

export interface Finding {
  id: string;
  level: "high" | "medium" | "info";
  title: string;
  evidence: string;
  effect: string;
  action: string;
  detail: string;
  link: InsightLink | null;
  /** Custom tool / MCP / plugin names that appear in `evidence` (offered for removal before refining). */
  localNames: string[];
}

export interface LocalReport {
  enough: boolean;
  responses: number;
  findings: Finding[];
  notChecked: string[];
  priceDate: string | null;
  cliVersion: string | null;
}

export interface Advice {
  text: string;
  atMs: number;
  costUsd: number | null;
  stale: boolean;
}

export type InsightRunError = { kind: "budget" | "turns"; costUsd: number | null } | { kind: "timeout" | "cancelled" | "spawn" | "failed" | "empty" };

export type InsightRun = { kind: "done"; advice: Advice } | { kind: "declined" } | { kind: "failed"; error: InsightRunError; message: string };

// Brief refining (brief-grilling, D-105). Nothing here is stored; the backend owns consent, rounds and budget.
export type RefineField = "goal" | "outOfScope" | "constraints" | "done" | "other";

export interface RefineQuestion {
  id: string;
  title: string;
  body: string;
  field: RefineField;
  multi: boolean;
  options: { label: string; detail: string }[];
  recommended: number[];
}

export interface Refined {
  questions: RefineQuestion[];
  rewrite: { goal: string; outOfScope: string; constraints: string; done: string };
  finished: boolean;
  note: string;
}

export interface RefineAnswer {
  question: string;
  field: RefineField;
  chosen: string[];
  custom: string;
  skipped: boolean;
}

export interface RefineRequest {
  mode: "grill" | "rewrite";
  taskType: string | null;
  brief: { goal: string; outOfScope: string; constraints: string; done: string };
  history: RefineAnswer[][];
  localNames: string[];
  redactNames: boolean;
  token: string | null;
}

export type RefineErrorKind = "budget" | "turns" | "timeout" | "cancelled" | "spawn" | "failed" | "badResponse" | "invalidInput" | "roundLimit" | "busy" | "guard" | "workspace";

export type RefineRun =
  | { kind: "done"; token: string; result: Refined; costUsd: number | null; spentUsd: number; runsLeft: number }
  | { kind: "declined" }
  | { kind: "failed"; error: { kind: RefineErrorKind }; message: string; costUsd: number | null; spentUsd: number | null };

export interface CloudList {
  routines: CloudRoutine[];
  hasMore: boolean;
  costUsd: number | null;
}

export interface TaskRecord {
  bgId: string;
  name: string | null;
  cwd: string | null;
  agent: string | null;
  taskType: string | null;
  createdAt: number;
  promptLen: number;
  reviewed: boolean;
  /** A 작업 보고서 was asked for when it started. */
  hasReport?: boolean;
  /** 정리 끝 (task-lifecycle): shown as "완료". */
  wrapped?: boolean;
}

// ---------- 작업 보고서 (decision-report, D-108) ----------

export type ReportPresence = "ready" | "missing" | "invalid";
export type ReportStatus = "in_progress" | "done" | "failed";
export interface ReportWork {
  step: string;
  result: string;
  files: string[];
}
export interface ReportBlocked {
  what: string;
  cause: string;
  resolution: string;
  open: boolean;
  needsUser: boolean;
  afterStep: number | null;
}
export interface ReportDecision {
  title: string;
  options: string[];
  chosen: string;
  reason: string;
  reversible: "easy" | "hard" | null;
  assumed: boolean;
  deviatesFromBrief: boolean;
  afterStep: number | null;
}
// ---------- 근거·첨부 (report-evidence, D-152·D-156) ----------
export type EvidenceKind = "screenshot" | "test-report" | "command" | "file";
export type EvidenceStatus = "valid" | "missing" | "refused" | "tooLarge" | "badType" | "tooMany";
/** One evidence item as the app checked it: no path except a `file` item's repository path (in the label). */
export interface EvidenceView {
  kind: EvidenceKind;
  label: string;
  caption: string;
  status: EvidenceStatus;
  /** Failed + errored tests of a valid test report. */
  failed?: number;
  exitCode?: number;
}
export interface TestSummary {
  tool: string;
  passed: number;
  failed: number;
  skipped: number;
  failures: string[];
}
export type EvidencePreview =
  | { kind: "image"; dataUrl: string; width: number; height: number }
  | { kind: "tests"; summary: TestSummary }
  | { kind: "command"; cmd: string; exitCode: number; excerpt: string; cut: boolean }
  | { kind: "file"; path: string; lines: [number, number] | null }
  | { kind: "invalid"; status: EvidenceStatus };
export interface AttachmentView {
  title: string;
  kind: "html" | "markdown";
  status: EvidenceStatus;
  /** Valid but nothing to show after cleaning ("빈 문서"). */
  empty?: boolean;
}
export type AttachmentOpened =
  | { kind: "html"; srcdoc: string }
  /** Image lines as (name, data); matched by name and occurrence (Rust attachment.rs). */
  | { kind: "markdown"; text: string; images: { name: string; data: string | null }[] }
  | { kind: "empty" }
  | { kind: "invalid"; code: EvidenceStatus };

export interface ReportCriterion {
  criterion: string;
  met: "yes" | "partial" | "no";
  /** The one line a person reads (a version 1 report's evidence sentence). */
  summary: string;
  evidence: EvidenceView[];
  /** Version 1 criterion ("근거 형식 이전 보고서"). */
  legacy: boolean;
}
export interface ReportReference {
  kind: "file" | "url" | "doc" | "commit" | "command";
  ref: string;
  note: string;
}
export interface Report {
  version: 1 | 2;
  status: ReportStatus;
  updatedAt: string | null;
  work: ReportWork[];
  blocked: ReportBlocked[];
  decisions: ReportDecision[];
  criteria: ReportCriterion[];
  references: ReportReference[];
  attachments: AttachmentView[];
  legacy: boolean;
}
/** "위임 내용(합의)": what the app stored at start, never written by the agent. */
export interface Delegated {
  name: string | null;
  brief: { goal: string; outOfScope: string; constraints: string; done: string } | null;
  prompt: string | null;
  taskType: string | null;
  permissionMode: string | null;
  folder: string | null;
  stored: boolean;
  routine: boolean;
  /** 전달 범위 at start (delivery-guard); absent/null for older tasks. */
  deliveryGuard?: "draft_pr" | "local_only" | null;
  /** Post-check bits: 1 = a push to the remote was seen, 2 = the default branch moved. */
  deliveryAlert?: number | null;
}
export type ReportErrorCode = "missing" | "tooLarge" | "invalid" | "refused" | "unknown";
export type TaskReportView =
  | { kind: "off"; delegated: Delegated | null }
  | { kind: "missing"; delegated: Delegated | null }
  | { kind: "invalid"; delegated: Delegated | null; code: ReportErrorCode }
  | { kind: "ready"; delegated: Delegated | null; report: Report };

// ---------- 리뷰 탭 (review-inbox, D-140) ----------

export type InboxFilter = "pending" | "all";
/** "checking": no good poll yet (the card cannot be marked). */
export type InboxRunning = "working" | "waiting" | "checking";
/** The verdict fields of a report only: no work steps, evidence, reasons or references. */
export interface InboxSummary {
  status: ReportStatus;
  workCount: number;
  blocked: { what: string; open: boolean; needsUser: boolean }[];
  decisions: { title: string; reversible: "easy" | "hard" | null; assumed: boolean; deviatesFromBrief: boolean }[];
  /** `evidence`: one card line of the summary (120 characters); counts of valid evidence per kind, no names. */
  criteria: InboxCriterion[];
  legacy: boolean;
  /** Any valid test report (QA "테스트 리포트 없음"). */
  testReport: boolean;
  attachmentCount: number;
  attachmentValid?: number;
  attachmentBroken?: number;
  attachmentEmpty?: number;
  firstAttachment: { title: string; line: string | null } | null;
}
export interface VerifiedKinds {
  screenshot: number;
  testReport: number;
  command: number;
  file: number;
}
export interface InboxCriterion {
  criterion: string;
  met: ReportCriterion["met"];
  evidence: string;
  evidenceCount?: number;
  verified?: VerifiedKinds;
  testsFailed?: number;
}
/** `none`: a finished D.Duck task started without a report (D-196), listed under 전체 only. */
export type InboxReport = { kind: "ready"; summary: InboxSummary } | { kind: "missing" } | { kind: "invalid"; code: ReportErrorCode } | { kind: "none" };
export interface InboxItem {
  bgId: string;
  taskType: string | null;
  folder: string | null;
  createdAt: number;
  reviewed: boolean;
  running: InboxRunning | null;
  /** Brief goal, task name or the instruction's first line, at most 120 characters. */
  goal: string | null;
  stored: boolean;
  /** The agreed 완료 기준; "" = a brief without done lines, null = no brief. */
  done: string | null;
  report: InboxReport;
  /** delivery-guard post-check bits (1 = push seen, 2 = default branch moved); null = not checked. */
  deliveryAlert?: number | null;
  /** 정리 끝 (task-lifecycle): the only state shown as "완료". */
  wrapped?: boolean;
  /** A routine run with a report (review-routine-runs): the card says "루틴". */
  routine?: boolean;
  /** How the task last ended while listed; shown on a card without a report. */
  ended?: "done" | "failed" | "stopped" | "unknown" | null;
}
export interface Inbox {
  reportSetting: boolean;
  total: number;
  pendingOther: number;
  hasAny: boolean;
  /** Finished D.Duck tasks without a report in this scope (D-196), listed under 전체 only. */
  noReport?: number;
  /** A good `claude agents` poll has landed (no delete before it). */
  known?: boolean;
  /** Still running tasks with a report in this project. */
  running: number;
  groups: InboxGroupTotal[];
  items: InboxItem[];
}
export interface InboxGroupTotal {
  taskType: string | null;
  total: number;
}

export interface TaskRecordDetail {
  prompt: string | null;
  brief: { goal: string; outOfScope: string; constraints: string; done: string } | null;
  effort: string | null;
  worktree: boolean;
  origin?: TaskOrigin | null;
  /** model-select: layer, the model passed, the value asked for when an unsupported one was replaced, and the
   * transcript's model when it differs. All `null` for older tasks. */
  modelLayer?: string | null;
  modelUsed?: string | null;
  modelRequested?: string | null;
  modelOther?: string | null;
}

export interface SessionPage {
  rows: SessionRow[];
  total: number;
}

/** `[start byte, length]` of a transcript record. */
export type Span = [number, number];

export type TimelineItem =
  | { kind: "prompt"; ts: number; text: string; raw: Span[] }
  | { kind: "reply"; ts: number; text: string; raw: Span[] }
  | { kind: "tool"; ts: number; id: string; name: string; summary: string; ok: boolean | null; error: string | null; raw: Span[] }
  | { kind: "skill"; ts: number; id: string; name: string; ok: boolean | null; error: string | null; raw: Span[] }
  | { kind: "agent"; ts: number; id: string; subagentType: string; description: string; ok: boolean | null; error: string | null; agentId: string | null; raw: Span[] }
  | { kind: "command"; ts: number; name: string; raw: Span[] }
  | { kind: "compact"; ts: number; raw: Span[] };

export interface RawRecord {
  offset: number;
  kind: string | null;
  ts: string | null;
  /** Bookkeeping without prose: folded by default in the 원문 tab. */
  minor: boolean;
  text: string;
  json: boolean;
  truncated: boolean;
}

export interface RawPage {
  records: RawRecord[];
  /** Byte position where the page starts; 0 = start of the file. */
  start: number;
  next: number | null;
  fileLen: number;
  stopped: boolean;
}

/** Which records to read: an item's spans, a page from a byte position, a page ending at one, or the last page. */
export type RawTarget =
  | { agentId?: string | null; spans: Span[] }
  | { agentId?: string | null; from: number }
  | { agentId?: string | null; before: number }
  | { agentId?: string | null; tail: true };

export interface Timeline {
  items: TimelineItem[];
  truncatedFront: boolean;
  skippedLines: number;
}

export interface Skill {
  name: string;
  source: "project" | "user" | "plugin" | "builtin";
  scope: string | null;
  kind: "skill" | "command" | "workflow";
  description: string | null;
  uses: number;
  lastUsed: number | null;
  defined: boolean;
  /** Definition file id for the editor (skill-authoring); null for usage-only rows. */
  id?: string | null;
  /** Why the editor is read-only, decided by Rust. */
  readonly?: string | null;
  /** `<plugin>:<name>` when this is a "복사해서 내 것으로" copy (nav-ia). */
  copiedFrom?: string | null;
  /** Workflows only (D-183): whether Claude Code loads the file, why not, and the meta.name it registers when that
   * differs from the file name. */
  ccReadable?: boolean;
  reason?: import("./workflowCheck").MetaReason;
  registeredName?: string;
}

/** MCP servers read from the settings files only (nav-ia, D-114). */
export interface McpServer {
  name: string;
  transport: "stdio" | "http" | "sse" | "other";
  scope: "user" | "local" | "project";
  host: string | null;
}

export interface McpList {
  servers: McpServer[];
  unreadable: string[];
}

export interface Connector {
  name: string;
  kind: "claudeAi" | "plugin" | "remote" | "local";
  host: string | null;
  state: "connected" | "needsAuth" | "failed" | "other";
}

export interface ConnectorList {
  connectors: Connector[];
  skipped: number;
}

export interface PluginItem {
  name: string;
  description: string | null;
  uses: number;
  lastUsed: number | null;
}

export interface Plugin {
  id: string;
  name: string;
  marketplace: string;
  version: string | null;
  scope: "user" | "project" | "local" | "managed" | "other";
  enabled: boolean;
  projectEnabled: boolean;
  installedAt: string | null;
  lastUpdated: string | null;
  mcp: { name: string; kind: string | null }[];
  components: { skills: PluginItem[]; commands: PluginItem[]; agents: PluginItem[]; hooks: boolean; lsp: boolean; unreadable: boolean };
  update: PluginUpdate;
  locked: string | null;
}

export type PluginUpdate = ({ state: "available"; latest: string } | { state: "latest" } | { state: "unknown" }) & { marketplaceUpdated: string | null };
export type PluginOutcome = { kind: "done" | "needsApproval" | "locked" | "unchanged" | "cancelled" | "timeout" | "failed" };
/** Why a plugin of a batch did not run (plugin-bulk-update, plugin_bulk.rs `Skip`). */
export type PluginSkip = "badId" | "notInstalled" | "locked" | "latest" | "tooMany" | "changed";
export type PluginBulkStatus = { kind: "running" } | { kind: "finished"; outcome: PluginOutcome } | { kind: "skipped"; reason: PluginSkip } | { kind: "stopped" };
export interface PluginBulkItem {
  id: string;
  status: PluginBulkStatus;
}
/** `confirmed: false`: nothing ran (declined, or nothing to run; then `items` holds the skipped ones). */
export interface PluginBulkResult {
  confirmed: boolean;
  items: PluginBulkItem[];
}

/** First-run tour state (onboarding-tour). Booleans and card ids only. */
export interface OnboardingState {
  version: number;
  seen: string[];
  dismissed: boolean;
  /** v1 kept timelineOpen·notifyOn (still stored, no longer listed); v2 lists firstTask·refineUsed·reportOpened. */
  checklist: { firstTask: boolean; timelineOpen: boolean; notifyOn: boolean; refineUsed: boolean; reportOpened: boolean };
}

/** App language (app-language): the stored choice and what it resolves to now (`system` via the OS locale). */
export interface LanguageInfo {
  choice: "system" | "ko" | "en";
  resolved: "ko" | "en";
}

export const api = {
  status: () => invoke<Status>("get_status"),
  setPref: (key: string, value: string) => invoke<void>("set_pref", { key, value }),
  getLanguage: () => invoke<LanguageInfo>("get_language"),
  setLanguage: (choice: LanguageInfo["choice"]) => invoke<LanguageInfo>("set_language", { choice }),
  onboardingState: () => invoke<OnboardingState | null>("get_onboarding_state"),
  setOnboardingState: (state: OnboardingState) => invoke<void>("set_onboarding_state", { json: JSON.stringify(state) }),
  setStoreTitles: (enabled: boolean) => invoke<void>("set_store_titles", { enabled }),
  pickCliPath: () => invoke<CliInfo | null>("pick_cli_path"),
  resetCliPath: () => invoke<void>("reset_cli_path"),
  pickConfigDir: () => invoke<string | null>("pick_config_dir"),
  resetConfigDir: () => invoke<void>("reset_config_dir"),
  rebuildUsage: () => invoke<void>("rebuild_usage"),
  pickDirectory: () => invoke<string | null>("pick_directory"),
  ingestFiles: () => invoke<IngestFile[]>("ingest_files"),
  summary: (q: Query) => invoke<Summary>("usage_summary", { q }),
  timeseries: (q: Query) => invoke<Point[]>("usage_timeseries", { q }),
  distribution: (q: Query, dim: Dimension, top: number) => invoke<Item[]>("usage_distribution", { q, dim, top }),
  sessions: (q: Query) => invoke<SessionRow[]>("usage_sessions", { q }),
  sessionsPage: (q: Query, offset: number) => invoke<SessionPage>("usage_sessions_page", { q, offset }),
  sessionTimeline: (sessionId: string) => invoke<Timeline | null>("session_timeline", { sessionId }),
  sessionRaw: (sessionId: string, target: RawTarget, reveal: boolean) =>
    invoke<RawPage>("session_raw", {
      sessionId,
      agentId: target.agentId ?? null,
      spans: "spans" in target ? target.spans : null,
      from: "from" in target ? target.from : null,
      before: "before" in target ? target.before : null,
      tail: "tail" in target,
      reveal,
    }),
  setSessionLabel: (sessionId: string, label: string | null) => invoke<void>("set_session_label", { sessionId, label }),
  skillCatalog: () => invoke<Skill[]>("skill_catalog"),
  /** "삭제" (D-147): Rust confirms natively and moves the item to the Trash. `true` moved, `false` cancelled. */
  trashExtensionItem: (id: string, unsaved = false) => invoke<boolean>("trash_extension_item", { id, unsaved }),
  /** Starts every server (`claude mcp list`): only from the "상태 확인" button. */
  mcpStatus: (project: string | null = null) => invoke<ConnectorList>("mcp_status", { project }),
  mcpServers: (project: string | null) => invoke<McpList>("mcp_servers", { project }),
  copyItem: (itemId: string, dest: "user" | "project", name: string, project: string | null) => invoke<{ id: string | null }>("copy_item", { itemId, dest, name, project }),
  plugins: () => invoke<Plugin[]>("plugin_list"),
  setPluginEnabled: (id: string, enabled: boolean) => invoke<PluginOutcome>("set_plugin_enabled", { id, enabled }),
  updatePlugin: (id: string) => invoke<PluginOutcome>("update_plugin", { id }),
  updatePlugins: (ids: string[]) => invoke<PluginBulkResult>("update_plugins", { ids }),
  stopPluginUpdates: () => invoke<void>("stop_plugin_updates"),
  sessionDetail: (sessionId: string) => invoke<SessionDetail>("usage_session_detail", { sessionId }),
  filterOptions: () => invoke<[string[], string[]]>("usage_filter_options"),
  agents: () => invoke<AgentsPayload>("list_agents"),
  startTask: (spec: TaskSpec, startId?: string) => invoke<string>("start_task", { spec, startId: startId ?? null }),
  /** Cancels a start in flight (D-161); true when that start was still running. */
  startCancel: (startId: string) => invoke<boolean>("start_cancel", { startId }),
  repoCheck: (path: string) => invoke<RepoStatus>("repo_check", { path }),
  markReviewed: (id: string) => invoke<boolean>("mark_reviewed", { id }),
  reviewInbox: (project: string | null, filter: InboxFilter, offset: number, limit = 10) => invoke<Inbox>("review_inbox", { project, filter, offset, limit }),
  inboxMarkReviewed: (id: string) => invoke<void>("inbox_mark_reviewed", { id }),
  /** review-delete: native confirmation in the shell; `false` = cancelled. */
  inboxDelete: (id: string) => invoke<boolean>("inbox_delete", { id }),
  unmarkReviewed: (id: string) => invoke<boolean>("unmark_reviewed", { id }),
  /** `wrapupCards`: how many 정리 cards ("더 보기"; 40 by default). */
  boardWrapup: (wrapupCards?: number) => invoke<BoardWrapup>("board_wrapup", wrapupCards ? { wrapupCards } : {}),
  wrapTask: (id: string) => invoke<void>("wrap_task", { id }),
  unwrapTask: (id: string) => invoke<boolean>("unwrap_task", { id }),
  wrapupCheck: (id: string) => invoke<WrapupCheck>("wrapup_check", { id }),
  /** `false` = cancelled in the native confirm. */
  removeWorktree: (id: string) => invoke<boolean>("remove_worktree", { id }),
  setRemoteAlerts: (enabled: boolean) => invoke<boolean>("set_remote_alerts", { enabled }),
  taskRecords: () => invoke<TaskRecord[]>("task_records"),
  taskRecord: (id: string) => invoke<TaskRecordDetail | null>("task_record", { id }),
  taskReport: (id: string) => invoke<TaskReportView>("task_report", { id }),
  reportEvidence: (id: string, criterion: number, item: number) => invoke<EvidencePreview>("report_evidence", { id, criterion, item }),
  reportAttachment: (id: string, index: number) => invoke<AttachmentOpened>("report_attachment", { id, index }),
  /** Native save panel for the original file (report-copy-save). `false` = cancelled; errors are `busy`, `refused`,
   *  `failed` or `missing` (the original is gone; see SAVE_ERROR in ReportEvidence). */
  saveReportAttachment: (id: string, index: number) => invoke<boolean>("save_report_attachment", { id, index }),
  deleteTaskRecord: (id: string) => invoke<boolean>("delete_task_record", { id }),
  deleteAllTaskRecords: () => invoke<boolean>("delete_all_task_records"),
  setStoreTaskPrompts: (enabled: boolean) => invoke<void>("set_store_task_prompts", { enabled }),
  taskLogs: (id: string) => invoke<string>("task_logs", { id }),
  stopTask: (id: string) => invoke<void>("stop_task", { id }),
  /** "끝내고 넘기기" (D-167 behaviour, D-190 name): stop normally, then 리뷰 대기 (unreviewed report) or done. */
  finishTask: (id: string) => invoke<"reviewWait" | "done">("finish_task", { id }),
  limitStatus: () => invoke<LimitView>("limit_status"),
  limitCancel: (id: number) => invoke<boolean>("limit_cancel", { id }),
  setLimitOptOut: (bgId: string, off: boolean) => invoke<boolean>("set_limit_opt_out", { bgId, off }),
  limitResumeNow: (id: number) => invoke<LimitReason | null>("limit_resume_now", { id }),
  openTerminal: (key: string) => invoke<AttachResult>("open_terminal", { key }),
  forceStop: (key: string) => invoke<void>("force_stop", { key }),
  pickDefaultCwd: () => invoke<string | null>("pick_default_cwd"),
  resetDefaultCwd: () => invoke<void>("reset_default_cwd"),
  sessionPrompt: (key: string) => invoke<string | null>("session_prompt", { key }),
  agentCatalog: (cwd: string | null = null) => invoke<AgentDef[]>("agent_catalog", { cwd }),
  agentActivity: () => invoke<Activity[]>("agent_activity"),
  agentDefinition: (id: string) => invoke<{ content: string; token: string }>("agent_definition", { id }),
  saveAgentDefinition: (id: string, content: string, token: string) => invoke<void>("save_agent_definition", { id, content, token }),
  artifacts: (query: string) => invoke<ArtifactRow[]>("list_artifacts", { query }),
  openArtifact: (artifactId: string) => invoke<void>("open_artifact", { artifactId }),
  artifactPreview: (artifactId: string) => invoke<ArtifactPreview>("artifact_preview", { artifactId }),
  openCloudRoutines: () => invoke<void>("open_cloud_routines"),
  cloudRoutines: () => invoke<CloudList>("cloud_routines"),
  cloudRoutineRuns: (id: string) => invoke<CloudRuns>("cloud_routine_runs", { id }),
  cloudRunLog: (sessionId: string) => invoke<CloudRunLog>("cloud_run_log", { sessionId }),
  updateCloudRoutine: (r: CloudRoutine, edit: CloudEditInput, listedAt: number) =>
    invoke<CloudEditResult>("update_cloud_routine", { id: r.id, before: { name: r.name, enabled: r.enabled, cron: r.cron, runOnceAt: r.runOnceAt }, edit, listedAt }),
  openCloudRoutine: (id: string) => invoke<void>("open_cloud_routine", { id }),
  insightsLocal: (q: Query) => invoke<LocalReport>("usage_insights_local", { q }),
  insightsPreview: (q: Query) => invoke<string>("usage_insights_preview", { q }),
  insightsClaude: (q: Query) => invoke<InsightRun>("usage_insights_claude", { q }),
  insightsCancel: () => invoke<void>("usage_insights_cancel"),
  insightsLast: () => invoke<Advice | null>("usage_insights_last"),
  insightsDelete: () => invoke<void>("usage_insights_delete"),
  openInsightLink: (url: string) => invoke<boolean>("open_insight_link", { url }),
  newsList: () => invoke<NewsView>("news_list"),
  newsFetch: () => invoke<NewsRun>("news_fetch"),
  newsCancel: () => invoke<void>("news_cancel"),
  newsMarkSeen: (id: string) => invoke<boolean>("news_mark_seen", { id }),
  newsSetDaily: (enabled: boolean) => invoke<boolean>("news_set_daily", { enabled }),
  newsOpenSource: (id: string) => invoke<void>("news_open_source", { id }),
  refineBrief: (req: RefineRequest) => invoke<RefineRun>("refine_brief", { req }),
  refineCancel: () => invoke<void>("refine_cancel"),
  routines: () => invoke<RoutineSummary[]>("list_routines"),
  routinePrompt: (id: number) => invoke<string | null>("routine_prompt", { id }),
  routineRuns: (id: number) => invoke<RoutineRun[]>("routine_runs", { id }),
  folderTrusted: (path: string) => invoke<boolean | null>("folder_trusted", { path }),
  saveRoutine: (id: number | null, input: RoutineInput) => invoke<number>("save_routine", { id, input }),
  setRoutineEnabled: (id: number, enabled: boolean) => invoke<void>("set_routine_enabled", { id, enabled }),
  deleteRoutine: (id: number) => invoke<void>("delete_routine", { id }),
  runRoutineNow: (id: number) => invoke<void>("run_routine_now", { id }),
  trustRoutineFolder: (id: number) => invoke<boolean>("trust_routine_folder", { id }),
};

// Bundled data helper: kept in its own module (no Tauri import) so mocks can use it.
export { pickLang, type Picked } from "./bundled";

// ---------- backend codes → text (app-language design 3) ----------
// The `errors` catalog holds the wording for codes Rust sends. These helpers are for screens that show a backend
// error or the usage-limit banner; they read the current language at call time.

const ERRORS: Record<Lang, Tree> = { ko: koErrors, en: enErrors };

function errorsText(lang: Lang, path: string, params?: Record<string, string | number>): string | undefined {
  let node: Message | Tree | undefined = ERRORS[lang];
  for (const part of path.split(".")) {
    if (node === undefined || typeof node === "string" || !Object.prototype.hasOwnProperty.call(node, part)) return undefined;
    node = (node as Tree)[part];
  }
  if (typeof node === "string") return fill(node, params, lang);
  if (node && typeof (node as Plural).other === "string") {
    const p = node as Plural;
    const rule = new Intl.PluralRules(localeOf(lang)).select(Number(params?.count ?? 0));
    return fill(p[rule] ?? p.other, params, lang);
  }
  return undefined;
}

/** A backend error in the app language: a string as is, a `CliError` by kind, an `invalidInput` by its reason code
 *  (with params), else Rust's fallback sentence. Same Korean text as `cliErrorMessage` for every kind. */
export function messageOf(e: unknown, lang: Lang = currentLang()): string {
  // A plain-string error that is a known fixed sentence follows a later language change (it is kept as sent and
  // worded here at render time); any other string is shown as sent.
  if (typeof e === "string") return fixedSentenceIn(e, lang) ?? e;
  if (e instanceof Error) return fixedSentenceIn(e.message, lang) ?? e.message;
  const err = e as CliError | undefined;
  const cli = (k: string, params?: Record<string, string | number>) => errorsText(lang, `cli.${k}`, params) ?? "";
  switch (err?.kind) {
    case "invalidInput": {
      const known = err.code ? errorsText(lang, `invalidInput.${err.code}`, err.params) : undefined;
      const sentence = known ?? err.text ?? err.reason;
      return sentence ? `${sentence}.` : cli("invalidInput");
    }
    case "failed":
      return cli("failed", { code: String(err.code) });
    case "spawn":
    case "timeout":
    case "untrustedWorkspace":
    case "unsupportedOutput":
    case "invalidId":
    case "dangerNotConfirmed":
    case "cancelled":
    case "killFailed":
      return cli(err.kind);
    case "login_required":
      return loginText(lang, "gate");
    case "claude_ai_required":
      return loginText(lang, "cloud");
    default:
      return cli("unknown");
  }
}

/** The `invalidInput` sentence `s` (either language) in `lang`, if it is one. */
function fixedSentenceIn(s: string, lang: Lang): string | undefined {
  for (const from of ["ko", "en"] as const) {
    const tree = ERRORS[from].invalidInput as Tree;
    for (const [code, v] of Object.entries(tree)) if (v === s) return errorsText(lang, `invalidInput.${code}`);
  }
  return undefined;
}

/** Whether a plain-string command error is the fixed reason `code` (`cancelled`, `listChanged`, `changedOutside`,
 *  `fileChanged`, …) in either language, for screens that branch on it instead of matching words. */
export function reasonIs(e: unknown, code: string): boolean {
  const m = typeof e === "string" ? e : (e as { text?: string; reason?: string } | null)?.text ?? (e as { reason?: string } | null)?.reason;
  if (typeof m !== "string") return false;
  if ((e as { code?: string } | null)?.code === code) return true;
  return (["ko", "en"] as const).some((l) => errorsText(l, `invalidInput.${code}`) === m);
}

/** The login gate wording lives with the auth screens (auth namespace, same text as `LOGIN_COPY`). */
function loginText(lang: Lang, key: "gate" | "cloud"): string {
  return translate(lang, `auth.${key}`);
}

/** The login connection line from Rust's `connectionCode` (`claudeAi` | `token` | `other`); `fallback` otherwise. */
export function connectionText(code: string | null | undefined, fallback: string | null = null, lang: Lang = currentLang()): string | null {
  // `other` is a plural category name in catalogs, so that code is filed as `another`.
  const key = code === "other" ? "another" : code;
  return (key && errorsText(lang, `connection.${key}`)) || fallback;
}

/** A message line kept in screen state: fixed text, a backend error or catalog text worded when drawn, so a language
 *  change rewords it (app-language review: errors are kept as objects, never `String(e)`). */
export type Shown = string | { error: unknown } | { say: () => string };

export const failure = (error: unknown): Shown => ({ error });

/** Catalog text worded when drawn (`later(() => t("…"))`), for a line set now and shown after a language change. */
export const later = (say: () => string): Shown => ({ say });

export function shown(s: Shown): string {
  return typeof s === "string" ? s : "say" in s ? s.say() : messageOf(s.error);
}
