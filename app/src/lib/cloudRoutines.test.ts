import { describe, expect, it } from "vitest";
import type { CloudRoutine } from "./api";
import { cloudDiff, cloudScheduleText, draftOf, fromLocalInput, pastOnce, toLocalInput, validateCloudDraft, zoneLabel } from "./cloudRoutines";

const now = new Date(2026, 10, 8, 12, 0).getTime();
const once: CloudRoutine = { id: "trig_1", name: "알림", enabled: false, cron: null, runOnceAt: now - 86_400_000, nextRunAt: null, endedReason: "run_once_fired", schedule: { kind: "once", at: now - 86_400_000 } };

describe("cloud routine edit", () => {
  it("describes schedules in local time", () => {
    expect(cloudScheduleText({ cron: "0 0 * * *", schedule: { kind: "daily", hour: 9, minute: 0 } })).toBe("매일 09:00");
    expect(cloudScheduleText({ cron: "30 23 * * 0,2,4", schedule: { kind: "weekly", weekdays: 0b10101, hour: 8, minute: 30 } })).toBe("매주 월·수·금 08:30");
    expect(cloudScheduleText({ cron: "0 8 1 * *", schedule: null })).toBe("0 8 1 * *");
    expect(cloudScheduleText({ cron: null, schedule: { kind: "once", at: new Date(2026, 10, 12, 9, 5).getTime() } })).toBe("한 번 · 2026. 11. 12. 09:05");
  });

  it("round-trips the datetime input", () => {
    const t = new Date(2026, 10, 12, 9, 5).getTime();
    expect(toLocalInput(t)).toBe("2026-11-12T09:05");
    expect(fromLocalInput("2026-11-12T09:05")).toBe(t);
    expect(fromLocalInput("")).toBeNull();
  });

  it("refuses to enable a past one-time routine and empty weekdays", () => {
    expect(pastOnce(once.schedule, now)).toBe(true);
    expect(validateCloudDraft({ ...draftOf(once), enabled: true }, now)).toMatch("지난");
    expect(validateCloudDraft({ ...draftOf(once), enabled: true, schedule: { kind: "once", at: now + 3_600_000 } }, now)).toBeNull();
    expect(validateCloudDraft({ name: "x", enabled: true, schedule: { kind: "weekly", weekdays: 0, hour: 9, minute: 0 } }, now)).toMatch("요일");
    expect(validateCloudDraft({ ...draftOf(once), name: " " }, now)).toMatch("이름");
  });

  it("sends only changed fields", () => {
    expect(cloudDiff(once, draftOf(once))).toEqual({});
    expect(cloudDiff(once, { ...draftOf(once), name: " 알림 (테스트) " })).toEqual({ name: "알림 (테스트)" });
    const at = now + 3_600_000;
    expect(cloudDiff(once, { ...draftOf(once), schedule: { kind: "once", at } })).toEqual({ schedule: { kind: "once", at } });
  });

  it("names the time zone", () => {
    expect(zoneLabel("Asia/Seoul")).toBe("KST");
    expect(zoneLabel("UTC")).toBe("UTC");
  });

  it("keeps an untouched draft empty and an enabled-only change minimal", () => {
    const on = { ...once, enabled: true, schedule: { kind: "daily" as const, hour: 9, minute: 0 }, runOnceAt: null, cron: "0 0 * * *" };
    expect(cloudDiff(on, { ...draftOf(on), enabled: false })).toEqual({ enabled: false });
    expect(validateCloudDraft({ ...draftOf(on), name: "가".repeat(81) }, now)).toMatch("이름");
  });
});
