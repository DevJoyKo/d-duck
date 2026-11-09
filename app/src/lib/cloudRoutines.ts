// Cloud routine edit (cloud-routine-edit): schedule sentences, local datetime input and the changed-fields diff.
// The Rust side validates again and builds the request; this only drives the form.
import type { CloudEditInput, CloudRoutine, CloudSchedule } from "./api";
import { t } from "../i18n";
import { currentLocale } from "../i18n/language";
import { weekdaysText } from "./routines";

const two = (n: number) => String(n).padStart(2, "0");
const hm = (h: number, m: number) => `${two(h)}:${two(m)}`;

export function cloudScheduleText(r: Pick<CloudRoutine, "schedule" | "cron">): string {
  const s = r.schedule;
  if (!s) return r.cron ?? "-";
  switch (s.kind) {
    case "once": {
      const d = new Date(s.at);
      // Date in the app locale ("2026. 11. 12." / "11/12/2026"); the time stays 24-hour.
      const date = d.toLocaleDateString(currentLocale(), { year: "numeric", month: "numeric", day: "numeric" });
      return t("routines.schedule.once", { date, time: hm(d.getHours(), d.getMinutes()) });
    }
    case "daily":
      return t("routines.schedule.daily", { time: hm(s.hour, s.minute) });
    case "weekly":
      return t("routines.schedule.weekly", { days: weekdaysText(s.weekdays), time: hm(s.hour, s.minute) });
  }
}

/** epoch ms → `<input type="datetime-local">` value in local time. */
export function toLocalInput(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}T${hm(d.getHours(), d.getMinutes())}`;
}

/** `<input type="datetime-local">` value (local time) → epoch ms, or null. */
export function fromLocalInput(v: string): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/.exec(v);
  if (!m) return null;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]), Number(m[4]), Number(m[5]));
  return Number.isNaN(d.getTime()) ? null : d.getTime();
}

/** A one-time routine whose time has passed must get a new time before it can be switched on. */
export function pastOnce(s: CloudSchedule | null, now: number): boolean {
  return s?.kind === "once" && s.at <= now + 60_000;
}

export interface CloudDraft {
  name: string;
  enabled: boolean;
  schedule: CloudSchedule | null;
}

export const draftOf = (r: CloudRoutine): CloudDraft => ({ name: r.name, enabled: r.enabled, schedule: r.schedule });

/** Problem with the draft, shown next to the form; null when it can be sent. */
export function validateCloudDraft(d: CloudDraft, now: number): string | null {
  const name = d.name.trim();
  if (!name || [...name].length > 80) return t("routines.cloudEdit.nameRange");
  const s = d.schedule;
  if (s?.kind === "weekly" && (s.weekdays & 0x7f) === 0) return t("routines.validate.weekdays");
  if (d.enabled && pastOnce(s, now)) return t("routines.cloudEdit.pastOn");
  return null;
}

const sameSchedule = (a: CloudSchedule | null, b: CloudSchedule | null) => JSON.stringify(a) === JSON.stringify(b);

/** Only the fields that changed (the request carries nothing else). */
export function cloudDiff(r: CloudRoutine, d: CloudDraft): CloudEditInput {
  const out: CloudEditInput = {};
  const name = d.name.trim().normalize("NFC");
  if (name !== r.name) out.name = name;
  if (d.enabled !== r.enabled) out.enabled = d.enabled;
  if (d.schedule && !sameSchedule(d.schedule, r.schedule)) out.schedule = d.schedule;
  return out;
}

/** Short name of this machine's time zone for labels: "KST" for Seoul, otherwise the browser's short name. */
export function zoneLabel(tz = Intl.DateTimeFormat().resolvedOptions().timeZone, at = new Date()): string {
  if (tz === "Asia/Seoul") return "KST";
  try {
    return new Intl.DateTimeFormat("en-US", { timeZone: tz, timeZoneName: "short" }).formatToParts(at).find((p) => p.type === "timeZoneName")?.value ?? tz;
  } catch {
    return tz;
  }
}
