import { AppAvatar } from "@/components/app-avatar";
import type { MemoryEvent } from "@/lib/api";
import { formatDuration, formatTime } from "@/lib/format";
import { cn } from "@/lib/utils";

export function MemoryRow({
  event,
  onOpen,
  showTime = true,
  live = false,
}: {
  event: MemoryEvent;
  onOpen: (e: MemoryEvent) => void;
  showTime?: boolean;
  live?: boolean;
}) {
  const title = event.windowTitle ?? event.url ?? event.filePath;
  return (
    <button
      type="button"
      onClick={() => onOpen(event)}
      className="group flex w-full items-center gap-3 rounded-lg px-2 py-2 text-left outline-none transition-colors hover:bg-muted/70 focus-visible:ring-2 focus-visible:ring-ring"
    >
      {showTime && (
        <time
          dateTime={new Date(event.startedAt).toISOString()}
          className="w-16 shrink-0 text-right text-xs tabular-nums text-muted-foreground"
        >
          {formatTime(event.startedAt)}
        </time>
      )}
      <AppAvatar name={event.appName} />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-medium">{event.appName ?? "Unknown app"}</span>
        <span className={cn("block truncate text-xs text-muted-foreground", !title && "italic")}>
          {title ?? "No window title"}
        </span>
      </span>
      <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
        {live ? (
          <span className="inline-flex items-center gap-1 text-recording">
            <span className="size-1.5 rounded-full bg-recording" aria-hidden />
            Now
          </span>
        ) : (
          formatDuration(event.endedAt - event.startedAt)
        )}
      </span>
    </button>
  );
}
