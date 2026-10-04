import { convertFileSrc } from "@tauri-apps/api/core";
import { ExternalLink, EyeOff, FolderOpen, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { AppAvatar } from "@/components/app-avatar";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { notifyError } from "@/hooks/use-recall";
import { api, type MemoryEvent } from "@/lib/api";
import { displayAppName, formatDateTime, formatDuration } from "@/lib/format";

const KIND_LABELS: Record<string, string> = {
  app_activity: "App activity",
  browser_activity: "Browser activity",
};

export function MemoryDetailDialog({
  event,
  onClose,
}: {
  event: MemoryEvent | null;
  onClose: () => void;
}) {
  const [busy, setBusy] = useState(false);

  async function remove() {
    if (!event) return;
    setBusy(true);
    try {
      await api.deleteEvent(event.id);
      toast.success("Memory deleted");
      onClose();
    } catch (e) {
      notifyError(e, "Couldn't delete this memory");
    } finally {
      setBusy(false);
    }
  }

  async function excludeApp() {
    if (!event?.appName) return;
    setBusy(true);
    try {
      const { removedMemories } = await api.addExclusion("app", event.appName);
      toast.success(`Recall will no longer record ${event.appName}`, {
        description: `${removedMemories} existing ${removedMemories === 1 ? "memory was" : "memories were"} deleted.`,
      });
      onClose();
    } catch (e) {
      notifyError(e, "Couldn't exclude this app");
    } finally {
      setBusy(false);
    }
  }

  async function excludeWebsite() {
    if (!event?.url) return;
    const host = websiteHost(event.url);
    if (!host) return;
    setBusy(true);
    try {
      const { removedMemories } = await api.addExclusion("website", host);
      toast.success(`Recall will no longer record ${host}`, {
        description: `${removedMemories} existing ${removedMemories === 1 ? "memory was" : "memories were"} deleted.`,
      });
      onClose();
    } catch (e) {
      notifyError(e, "Couldn't exclude this website");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={event !== null} onOpenChange={(open) => !open && onClose()}>
      {event && (
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <div className="flex items-center gap-3">
              <AppAvatar name={event.appName} appId={event.appId} className="size-10 text-base" />
              <div className="min-w-0">
                <DialogTitle className="truncate">{displayAppName(event.appName)}</DialogTitle>
                <DialogDescription>{KIND_LABELS[event.kind] ?? event.kind}</DialogDescription>
              </div>
            </div>
          </DialogHeader>

          {event.kind === "screenshot" && event.filePath && (
            <img
              src={convertFileSrc(event.filePath)}
              alt={event.windowTitle ?? "Screenshot"}
              className="max-h-80 w-full rounded-lg object-contain"
            />
          )}
          <dl className="grid grid-cols-[7rem_1fr] gap-x-4 gap-y-2.5 text-sm">
            <Field label={event.kind === "browser_activity" ? "Page title" : "Window"}>
              {event.windowTitle ?? <Muted>Not recorded</Muted>}
            </Field>
            {event.url && <Field label="Website">{event.url}</Field>}
            {event.filePath && <Field label="File">{event.filePath}</Field>}
            <Field label={event.kind === "browser_activity" ? "Visited" : "Started"}>
              {formatDateTime(event.startedAt)}
            </Field>
            {event.kind !== "browser_activity" && (
              <>
                <Field label="Ended">{formatDateTime(event.endedAt)}</Field>
                <Field label="Duration">{formatDuration(event.endedAt - event.startedAt)}</Field>
              </>
            )}
            {event.appId && event.kind !== "browser_activity" && (
              <Field label="Application">
                <span className="font-mono text-xs">{event.appId}</span>
              </Field>
            )}
          </dl>

          <DialogFooter className="flex-wrap gap-2 sm:justify-between">
            {event.kind === "browser_activity" && event.url ? (
              <Button variant="ghost" onClick={excludeWebsite} disabled={busy}>
                <EyeOff /> Never record {truncate(websiteHost(event.url) ?? "this site", 22)}
              </Button>
            ) : event.appName ? (
              <Button variant="ghost" onClick={excludeApp} disabled={busy}>
                <EyeOff /> Never record {truncate(event.appName, 22)}
              </Button>
            ) : (
              <span />
            )}
            {event.url && (
              <Button
                variant="outline"
                onClick={() => api.openUrl(event.url!).catch((e) => notifyError(e, "Couldn't open the website"))}
              >
                <ExternalLink /> Open in browser
              </Button>
            )}
            {event.filePath && event.kind !== "screenshot" && (
              <Button
                variant="outline"
                onClick={() => api.openPath(event.filePath!).catch((e) => notifyError(e, "Couldn't open the file"))}
              >
                <FolderOpen /> Open file
              </Button>
            )}
            <Button variant="destructive" onClick={remove} disabled={busy}>
              <Trash2 /> Delete memory
            </Button>
          </DialogFooter>
        </DialogContent>
      )}
    </Dialog>
  );
}

function websiteHost(value: string): string | null {
  try {
    return new URL(value).hostname.replace(/^www\./, "");
  } catch {
    return null;
  }
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="min-w-0 break-words">{children}</dd>
    </>
  );
}

function Muted({ children }: { children: React.ReactNode }) {
  return <span className="text-muted-foreground italic">{children}</span>;
}

function truncate(s: string, n: number) {
  return s.length > n ? `${s.slice(0, n - 1)}…` : s;
}
