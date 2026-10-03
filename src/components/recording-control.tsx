import { ChevronDown, Pause, Play, Power } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useRecall } from "@/hooks/use-recall";
import { describeStatus, type StatusTone } from "@/lib/status";
import { cn } from "@/lib/utils";

const toneDot: Record<StatusTone, string> = {
  recording: "bg-recording",
  paused: "bg-paused",
  off: "bg-muted-foreground/50",
  warning: "bg-destructive",
};

export function StatusDot({ tone, className }: { tone: StatusTone; className?: string }) {
  return (
    <span className={cn("relative inline-flex size-2.5 shrink-0", className)} aria-hidden>
      {tone === "recording" && (
        <span className="absolute inset-0 animate-ping rounded-full bg-recording opacity-40 [animation-duration:2.4s]" />
      )}
      <span className={cn("relative inline-flex size-2.5 rounded-full", toneDot[tone])} />
    </span>
  );
}

export const PAUSE_OPTIONS = [
  { minutes: 15, label: "Pause for 15 minutes" },
  { minutes: 60, label: "Pause for 1 hour" },
  { minutes: 240, label: "Pause for 4 hours" },
  { minutes: undefined, label: "Pause until I resume" },
] as const;

/** The global recording status and pause control shown in the sidebar. */
export function RecordingControl({ compact = false }: { compact?: boolean }) {
  const { status, settings, pause, resume, updateSettings } = useRecall();
  const d = describeStatus(status);
  const paused = status.state === "paused";
  const off = !settings.recordingEnabled;

  return (
    <div
      className={cn(
        "rounded-xl border bg-card/70 p-3",
        paused && "border-paused/50 bg-paused/10",
      )}
      role="status"
      aria-live="polite"
    >
      <div className="flex items-center gap-2.5">
        <StatusDot tone={d.tone} />
        <div className="min-w-0 flex-1">
          <p className="text-sm font-medium leading-tight">{d.label}</p>
          {!compact && d.detail && (
            <p className="truncate text-xs text-muted-foreground" title={d.detail}>
              {d.detail}
            </p>
          )}
        </div>
      </div>
      <div className="mt-2.5">
        {off ? (
          <Button size="sm" className="w-full" onClick={() => updateSettings({ recordingEnabled: true })}>
            <Power /> Turn on recording
          </Button>
        ) : paused ? (
          <Button size="sm" className="w-full" onClick={resume}>
            <Play /> Resume recording
          </Button>
        ) : (
          <DropdownMenu>
            <DropdownMenuTrigger render={<Button size="sm" variant="outline" className="w-full" />}>
              <Pause /> Pause recording <ChevronDown className="ml-auto opacity-60" />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-56">
              <DropdownMenuGroup>
                <DropdownMenuLabel>Nothing is recorded while paused</DropdownMenuLabel>
                {PAUSE_OPTIONS.map((o) => (
                  <DropdownMenuItem key={o.label} onClick={() => pause(o.minutes)}>
                    {o.label}
                  </DropdownMenuItem>
                ))}
              </DropdownMenuGroup>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={() => updateSettings({ recordingEnabled: false })}>
                Turn recording off
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </div>
    </div>
  );
}
