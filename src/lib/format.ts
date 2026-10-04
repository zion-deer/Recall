const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** Start of the local calendar day containing `ts`. */
export function startOfDay(ts: number): number {
  const d = new Date(ts);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** Start of the local day `days` after the day containing `ts` (DST-safe). */
export function addDays(ts: number, days: number): number {
  const d = new Date(startOfDay(ts));
  d.setDate(d.getDate() + days);
  return d.getTime();
}

export function dayRange(ts: number): { start: number; end: number } {
  return { start: startOfDay(ts), end: addDays(ts, 1) };
}

export function isSameDay(a: number, b: number): boolean {
  return startOfDay(a) === startOfDay(b);
}

export function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
}

export function formatDayLabel(ts: number, now = Date.now()): string {
  if (isSameDay(ts, now)) return "Today";
  if (isSameDay(ts, addDays(now, -1))) return "Yesterday";
  const sameYear = new Date(ts).getFullYear() === new Date(now).getFullYear();
  return new Date(ts).toLocaleDateString(undefined, {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: sameYear ? undefined : "numeric",
  });
}

export function formatDateTime(ts: number): string {
  return new Date(ts).toLocaleString(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    second: "2-digit",
  });
}

/** Human duration: "45s", "12m", "1h 05m". */
export function formatDuration(ms: number): string {
  const safe = Math.max(0, ms);
  if (safe < MINUTE) return `${Math.round(safe / 1000)}s`;
  if (safe < HOUR) return `${Math.round(safe / MINUTE)}m`;
  const h = Math.floor(safe / HOUR);
  const m = Math.round((safe % HOUR) / MINUTE);
  if (m === 60) return `${h + 1}h`;
  return m === 0 ? `${h}h` : `${h}h ${String(m).padStart(2, "0")}m`;
}

export function formatRelative(ts: number, now = Date.now()): string {
  const diff = now - ts;
  if (diff < MINUTE) return "just now";
  if (diff < HOUR) return `${Math.floor(diff / MINUTE)} min ago`;
  if (diff < DAY && isSameDay(ts, now)) return formatTime(ts);
  return `${formatDayLabel(ts, now)}, ${formatTime(ts)}`;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}

/** Describes when a pause ends. `pausedUntil` near Number.MAX means "until resumed". */
export function describePause(pausedUntil: number | null, now = Date.now()): string {
  if (pausedUntil == null) return "";
  if (pausedUntil > now + 400 * DAY) return "Paused until you resume";
  if (isSameDay(pausedUntil, now)) return `Paused until ${formatTime(pausedUntil)}`;
  return `Paused until ${formatDayLabel(pausedUntil, now)}, ${formatTime(pausedUntil)}`;
}

const APP_NAMES: Record<string, string> = {
  "google-chrome": "Google Chrome",
  "google-chrome-stable": "Google Chrome",
  chrome: "Google Chrome",
  chromium: "Chromium",
  "microsoft-edge": "Microsoft Edge",
  msedge: "Microsoft Edge",
  firefox: "Firefox",
  safari: "Safari",
  code: "Visual Studio Code",
  "code-oss": "Visual Studio Code",
  cursor: "Cursor",
  slack: "Slack",
  discord: "Discord",
  spotify: "Spotify",
  finder: "Finder",
  terminal: "Terminal",
  iterm2: "iTerm",
  warp: "Warp",
};

/** Turns a process name like `google-chrome` into a name people recognize. */
export function displayAppName(name: string | null | undefined): string {
  const raw = (name ?? "").trim();
  if (!raw) return "Unknown app";
  const known = APP_NAMES[raw.toLowerCase()];
  if (known) return known;
  if (raw.includes("-") || raw.includes("_") || raw === raw.toLowerCase()) {
    return raw
      .replace(/[-_]+/g, " ")
      .replace(/\b\w/g, (c) => c.toUpperCase());
  }
  return raw;
}

export function retentionLabel(days: number | null): string {
  switch (days) {
    case null:
      return "Forever";
    case 7:
      return "7 days";
    case 30:
      return "30 days";
    case 180:
      return "6 months";
    case 365:
      return "1 year";
    default:
      return `${days} days`;
  }
}
