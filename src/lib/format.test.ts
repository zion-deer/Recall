import { describe, expect, it } from "vitest";
import {
  addDays,
  dayRange,
  describePause,
  formatBytes,
  formatDayLabel,
  formatDuration,
  retentionLabel,
  startOfDay,
} from "./format";

describe("formatDuration", () => {
  it("formats seconds, minutes and hours", () => {
    expect(formatDuration(0)).toBe("0s");
    expect(formatDuration(-5)).toBe("0s");
    expect(formatDuration(45_000)).toBe("45s");
    expect(formatDuration(12 * 60_000)).toBe("12m");
    expect(formatDuration(60 * 60_000)).toBe("1h");
    expect(formatDuration(65 * 60_000)).toBe("1h 05m");
    expect(formatDuration(119.9 * 60_000)).toBe("2h");
  });
});

describe("day helpers", () => {
  it("computes local day boundaries", () => {
    const ts = new Date(2026, 2, 8, 15, 30).getTime();
    const { start, end } = dayRange(ts);
    expect(new Date(start).getHours()).toBe(0);
    expect(new Date(end).getDate()).toBe(9);
    expect(startOfDay(end)).toBe(end);
    expect(addDays(ts, -1)).toBe(startOfDay(new Date(2026, 2, 7, 12).getTime()));
  });

  it("labels today and yesterday", () => {
    const now = new Date(2026, 9, 3, 12).getTime();
    expect(formatDayLabel(now, now)).toBe("Today");
    expect(formatDayLabel(addDays(now, -1), now)).toBe("Yesterday");
  });
});

describe("misc formatting", () => {
  it("formats bytes", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(50 * 1024 * 1024)).toBe("50 MB");
  });

  it("describes indefinite and timed pauses", () => {
    const now = new Date(2026, 9, 3, 12).getTime();
    expect(describePause(Number.MAX_SAFE_INTEGER, now)).toBe("Paused until you resume");
    expect(describePause(null, now)).toBe("");
    expect(describePause(now + 3_600_000, now)).toMatch(/^Paused until /);
  });

  it("labels retention choices", () => {
    expect(retentionLabel(null)).toBe("Forever");
    expect(retentionLabel(180)).toBe("6 months");
  });
});
