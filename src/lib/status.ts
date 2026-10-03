import type { RecorderStatus } from "./api";
import { describePause } from "./format";

export type StatusTone = "recording" | "paused" | "off" | "warning";

export interface StatusDescription {
  tone: StatusTone;
  label: string;
  detail: string;
}

export function describeStatus(status: RecorderStatus, now = Date.now()): StatusDescription {
  switch (status.state) {
    case "recording":
      return {
        tone: "recording",
        label: "Recording",
        detail: status.currentApp ? `Remembering ${status.currentApp}` : "Remembering your activity",
      };
    case "starting":
    case "no_window":
      return { tone: "recording", label: "Recording", detail: "Waiting for an app window" };
    case "recall_focused":
      return { tone: "recording", label: "Recording", detail: "Recall doesn't record itself" };
    case "excluded":
      return { tone: "recording", label: "Recording", detail: "This window is excluded" };
    case "idle":
      return { tone: "recording", label: "Recording", detail: "You seem to be away" };
    case "paused":
      return { tone: "paused", label: "Paused", detail: describePause(status.pausedUntil, now) };
    case "off":
      return { tone: "off", label: "Recording off", detail: "Nothing is being recorded" };
    case "unavailable":
      return {
        tone: "warning",
        label: "Can't record",
        detail: status.message ?? "Activity tracking isn't available on this system",
      };
  }
}
