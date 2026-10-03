import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Theme = "system" | "light" | "dark";

export interface Settings {
  onboardingCompleted: boolean;
  recordingEnabled: boolean;
  pausedUntil: number | null;
  appActivityEnabled: boolean;
  windowTitlesEnabled: boolean;
  browserActivityEnabled: boolean;
  browserChromeEnabled: boolean;
  browserEdgeEnabled: boolean;
  browserFirefoxEnabled: boolean;
  browserSafariEnabled: boolean;
  screenshotsEnabled: boolean;
  screenshotIntervalSecs: number;
  screenshotRetentionDays: number | null;
  aiEnabled: boolean;
  retentionDays: number | null;
  idleThresholdSecs: number;
  pollIntervalSecs: number;
  theme: Theme;
}

export type RecorderState =
  | "starting"
  | "recording"
  | "paused"
  | "off"
  | "idle"
  | "excluded"
  | "recall_focused"
  | "no_window"
  | "unavailable";

export interface RecorderStatus {
  state: RecorderState;
  currentApp: string | null;
  pausedUntil: number | null;
  message: string | null;
}

export interface MemoryEvent {
  id: number;
  kind: string;
  source: string;
  startedAt: number;
  endedAt: number;
  appName: string | null;
  appId: string | null;
  windowTitle: string | null;
  url: string | null;
  filePath: string | null;
}

export interface EventQuery {
  start?: number;
  end?: number;
  appName?: string;
  kind?: string;
  limit?: number;
}

export interface SearchQuery {
  text: string;
  start?: number;
  end?: number;
  kind?: string;
  limit?: number;
}

export interface BrowserStatus {
  id: "chrome" | "edge" | "firefox" | "safari";
  name: string;
  supported: boolean;
  installed: boolean;
  profileCount: number;
  state: "ready" | "not_installed" | "unavailable" | "unsupported";
  message: string | null;
}

export interface AppUsage {
  appName: string;
  totalMs: number;
  sessions: number;
}

export interface MemoryStats {
  totalEvents: number;
  oldestAt: number | null;
  newestAt: number | null;
  databaseBytes: number;
}

export type ExclusionKind = "app" | "website" | "folder" | "title";

export interface Exclusion {
  id: number;
  kind: ExclusionKind;
  pattern: string;
  createdAt: number;
}

export interface PermissionInfo {
  id: string;
  name: string;
  reason: string;
  granted: boolean | null;
}

export interface AiStatus {
  phase: "not_installed" | "ready" | "downloading" | "generating" | "error";
  modelName: string;
  downloadedBytes: number;
  totalBytes: number;
  path: string | null;
  message: string | null;
}

export interface AskResponse {
  status: "answered" | "not_enough_memory" | "model_not_installed" | "ai_disabled" | "error";
  answer: string | null;
  memories: MemoryEvent[];
  message: string | null;
}

export interface UpdateOffer {
  currentVersion: string;
  version: string;
  notes: string | null;
}

export interface AppInfo {
  version: string;
  platform: "windows" | "macos" | "linux";
  platformLimitation: string | null;
  dataDir: string;
  logDir: string;
  schemaVersion: number;
}

/** Errors returned by the Rust backend. */
export class RecallError extends Error {
  readonly code: string;
  constructor(code: string, message: string) {
    super(message);
    this.code = code;
  }
}

export function toRecallError(e: unknown): RecallError {
  if (e instanceof RecallError) return e;
  if (e && typeof e === "object" && "message" in e) {
    const obj = e as { code?: unknown; message: unknown };
    return new RecallError(String(obj.code ?? "unknown"), String(obj.message));
  }
  return new RecallError("unknown", typeof e === "string" ? e : "Something went wrong.");
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toRecallError(e);
  }
}

export const hasBackend = () => isTauri();

export const api = {
  appInfo: () => call<AppInfo>("get_app_info"),
  getSettings: () => call<Settings>("get_settings"),
  updateSettings: (settings: Settings) => call<Settings>("update_settings", { settings }),
  pause: (minutes?: number) => call<Settings>("pause_recording", { minutes: minutes ?? null }),
  resume: () => call<Settings>("resume_recording"),
  recorderStatus: () => call<RecorderStatus>("get_recorder_status"),
  browserStatuses: () => call<BrowserStatus[]>("get_browser_statuses"),

  listEvents: (query: EventQuery) => call<MemoryEvent[]>("list_events", { query }),
  searchEvents: (query: SearchQuery) => call<MemoryEvent[]>("search_events", { query }),
  getEvent: (id: number) => call<MemoryEvent>("get_event", { id }),
  appUsage: (start: number, end: number) => call<AppUsage[]>("get_app_usage", { start, end }),
  stats: () => call<MemoryStats>("get_memory_stats"),
  deleteEvent: (id: number) => call<boolean>("delete_event", { id }),
  deleteRange: (start: number, end: number) =>
    call<number>("delete_events_in_range", { start, end }),
  deleteAll: () => call<number>("delete_all_memories"),
  deleteScreenshots: () => call<number>("delete_screenshots"),

  listExclusions: () => call<Exclusion[]>("list_exclusions"),
  addExclusion: (kind: ExclusionKind, pattern: string) =>
    call<{ exclusion: Exclusion; removedMemories: number }>("add_exclusion", { kind, pattern }),
  removeExclusion: (id: number) => call<boolean>("remove_exclusion", { id }),

  exportData: (path: string) =>
    call<{ path: string; eventCount: number }>("export_data", { path }),
  permissions: () => call<PermissionInfo[]>("get_permissions"),
  requestPermission: (id: string) => call<void>("request_permission", { id }),
  openDataFolder: () => call<void>("open_data_folder"),
  openLogFolder: () => call<void>("open_log_folder"),
  openUrl: (url: string) => call<void>("open_url", { url }),
  openPath: (path: string) => call<void>("open_path", { path }),
  aiStatus: () => call<AiStatus>("get_ai_status"),
  downloadAiModel: () => call<void>("download_ai_model"),
  cancelAiDownload: () => call<void>("cancel_ai_download"),
  removeAiModel: () => call<void>("remove_ai_model"),
  ask: (question: string) => call<AskResponse>("ask_recall", { question }),
  checkForUpdate: () => call<UpdateOffer | null>("check_for_update"),
  installUpdate: () => call<void>("install_update"),
};

export const events = {
  onRecorderStatus: (cb: (s: RecorderStatus) => void): Promise<UnlistenFn> =>
    listen<RecorderStatus>("recorder:status", (e) => cb(e.payload)),
  onMemoryChanged: (cb: () => void): Promise<UnlistenFn> => listen("memory:changed", () => cb()),
  onSettingsChanged: (cb: (s: Settings) => void): Promise<UnlistenFn> =>
    listen<Settings>("settings:changed", (e) => cb(e.payload)),
};
