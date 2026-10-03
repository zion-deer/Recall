import { ChevronLeft, ChevronRight, RotateCw, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { toast } from "sonner";
import { PageHeader } from "@/components/app-shell";
import { MemoryDetailDialog } from "@/components/memory-detail-dialog";
import { MemoryRow } from "@/components/memory-row";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useAsync } from "@/hooks/use-async";
import { useNavigation } from "@/hooks/use-navigation";
import { notifyError, useRecall } from "@/hooks/use-recall";
import { api, type MemoryEvent } from "@/lib/api";
import { addDays, dayRange, formatDayLabel, formatDuration, isSameDay, startOfDay } from "@/lib/format";
import { ListSkeleton } from "@/pages/home";

const PAGE_LIMIT = 1000;
const ALL = "__all__";

const TYPE_ITEMS = [
  { value: ALL, label: "All types" },
  { value: "app_activity", label: "App activity" },
  { value: "browser_activity", label: "Websites" },
];

function toDateInput(ts: number): string {
  const d = new Date(ts);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function fromDateInput(v: string): number | null {
  const [y, m, d] = v.split("-").map(Number);
  if (!y || !m || !d) return null;
  return new Date(y, m - 1, d).getTime();
}

function hourLabel(ts: number): string {
  return new Date(ts).toLocaleTimeString(undefined, { hour: "numeric" });
}

interface PendingDelete {
  title: string;
  description: string;
  start: number;
  end: number;
}

export function MemoryPage() {
  const { memoryVersion, status, settings } = useRecall();
  const { navigate } = useNavigation();
  const [day, setDay] = useState(() => startOfDay(Date.now()));
  const [app, setApp] = useState<string>(ALL);
  const [kind, setKind] = useState<string>(ALL);
  const [selected, setSelected] = useState<MemoryEvent | null>(null);
  const [pending, setPending] = useState<PendingDelete | null>(null);
  const [reload, setReload] = useState(0);

  const range = dayRange(day);
  const isToday = isSameDay(day, Date.now());

  const events = useAsync(
    () =>
      api.listEvents({
        start: range.start,
        end: range.end,
        appName: app === ALL ? undefined : app,
        kind: kind === ALL ? undefined : kind,
        limit: PAGE_LIMIT,
      }),
    [range.start, app, kind, memoryVersion, reload],
  );
  const usage = useAsync(() => api.appUsage(range.start, range.end), [range.start, memoryVersion]);

  const appItems = useMemo(
    () => [
      { value: ALL, label: "All apps" },
      ...(usage.data ?? []).map((u) => ({ value: u.appName, label: u.appName })),
      ...(app !== ALL && !usage.data?.some((u) => u.appName === app) ? [{ value: app, label: app }] : []),
    ],
    [usage.data, app],
  );

  const groups = useMemo(() => {
    const out: { hour: number; items: MemoryEvent[] }[] = [];
    for (const e of events.data ?? []) {
      const hour = new Date(Math.max(e.startedAt, range.start)).setMinutes(0, 0, 0);
      const last = out[out.length - 1];
      if (last && last.hour === hour) last.items.push(e);
      else out.push({ hour, items: [e] });
    }
    return out;
  }, [events.data, range.start]);

  const total = (usage.data ?? []).reduce((s, u) => s + u.totalMs, 0);
  const liveId = isToday && status.state === "recording" && app === ALL ? events.data?.[0]?.id : undefined;

  function changeDay(next: number) {
    setDay(startOfDay(Math.min(next, Date.now())));
    setApp(ALL);
  }

  async function confirmDelete() {
    if (!pending) return;
    try {
      const n = await api.deleteRange(pending.start, pending.end);
      toast.success(n === 0 ? "Nothing to delete" : `Deleted ${n} ${n === 1 ? "memory" : "memories"}`);
    } catch (e) {
      notifyError(e, "Couldn't delete memories");
    } finally {
      setPending(null);
    }
  }

  function askDelete(kind: "15m" | "1h" | "day") {
    const now = Date.now();
    if (kind === "day") {
      setPending({
        title: `Delete everything from ${formatDayLabel(day).toLowerCase() === "today" ? "today" : formatDayLabel(day)}?`,
        description: "All memories from this day will be permanently removed from this computer.",
        start: range.start,
        end: range.end,
      });
    } else {
      const minutes = kind === "15m" ? 15 : 60;
      setPending({
        title: `Delete the last ${kind === "15m" ? "15 minutes" : "hour"}?`,
        description: `Memories from the last ${minutes} minutes will be permanently removed from this computer.`,
        start: now - minutes * 60_000,
        end: now + 1,
      });
    }
  }

  return (
    <div className="mx-auto w-full max-w-4xl px-5 py-8 md:px-10 md:py-10">
      <PageHeader
        title="Memory"
        description="Everything Recall remembers, in order. Click a memory for details."
        actions={
          <DropdownMenu>
            <DropdownMenuTrigger render={<Button variant="outline" size="sm" />}>
              <Trash2 /> Delete…
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-52">
              <DropdownMenuItem onClick={() => askDelete("15m")}>Last 15 minutes</DropdownMenuItem>
              <DropdownMenuItem onClick={() => askDelete("1h")}>Last hour</DropdownMenuItem>
              <DropdownMenuItem onClick={() => askDelete("day")}>
                {isToday ? "All of today" : "This entire day"}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => navigate("settings", "data")}>Everything…</DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        }
      />

      <div className="sticky top-0 z-10 -mx-2 mb-4 flex flex-wrap items-center gap-2 bg-background/90 px-2 py-2 backdrop-blur">
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon-sm" aria-label="Previous day" onClick={() => changeDay(addDays(day, -1))}>
            <ChevronLeft />
          </Button>
          <h2 className="min-w-28 text-center text-sm font-semibold">{formatDayLabel(day)}</h2>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Next day"
            disabled={isToday}
            onClick={() => changeDay(addDays(day, 1))}
          >
            <ChevronRight />
          </Button>
        </div>
        <Input
          type="date"
          aria-label="Jump to date"
          className="h-8 w-auto"
          value={toDateInput(day)}
          max={toDateInput(Date.now())}
          onChange={(e) => {
            const ts = fromDateInput(e.target.value);
            if (ts !== null) changeDay(ts);
          }}
        />
        {!isToday && (
          <Button variant="ghost" size="sm" onClick={() => changeDay(Date.now())}>
            Today
          </Button>
        )}
        <div className="ml-auto flex flex-wrap gap-2">
          <Select items={appItems} value={app} onValueChange={(v) => setApp((v as string | null) ?? ALL)}>
            <SelectTrigger size="sm" aria-label="Filter by app" className="min-w-32">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {appItems.map((i) => (
                <SelectItem key={i.value} value={i.value}>
                  {i.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select items={TYPE_ITEMS} value={kind} onValueChange={(v) => setKind((v as string | null) ?? ALL)}>
            <SelectTrigger size="sm" aria-label="Filter by memory type" className="min-w-32">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {TYPE_ITEMS.map((i) => (
                <SelectItem key={i.value} value={i.value}>
                  {i.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      {total > 0 && (
        <p className="mb-4 text-sm text-muted-foreground">
          {formatDuration(total)} of activity across {usage.data?.length}{" "}
          {usage.data?.length === 1 ? "app" : "apps"}
        </p>
      )}

      {events.status === "error" && !events.data ? (
        <div className="rounded-xl border p-6 text-center" role="alert">
          <p className="text-sm font-medium">Couldn't load this day</p>
          <p className="mt-1 text-sm text-muted-foreground">{events.error}</p>
          <Button variant="outline" size="sm" className="mt-3" onClick={() => setReload((r) => r + 1)}>
            <RotateCw /> Try again
          </Button>
        </div>
      ) : !events.data ? (
        <ListSkeleton rows={8} />
      ) : events.data.length === 0 ? (
        <div className="rounded-xl border border-dashed px-6 py-14 text-center">
          <p className="text-sm font-medium">
            {app !== ALL || kind !== ALL ? "No memories match these filters" : "Nothing remembered on this day"}
          </p>
          <p className="mx-auto mt-1 max-w-sm text-sm text-muted-foreground">
            {app !== ALL || kind !== ALL
              ? "Try a different app or memory type."
              : isToday && !settings.recordingEnabled
                ? "Recording is off. Turn it on from the sidebar to start building your memory."
                : isToday
                  ? "Activity will appear here as you use your computer."
                  : "Recall wasn't recording, or these memories were deleted."}
          </p>
        </div>
      ) : (
        <ol className="space-y-6" aria-label={`Memories for ${formatDayLabel(day)}`}>
          {groups.map((g) => (
            <li key={g.hour}>
              <h3 className="mb-1 flex items-center gap-3 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                {hourLabel(g.hour)}
                <span className="h-px flex-1 bg-border" aria-hidden />
              </h3>
              <ul>
                {g.items.map((e) => (
                  <li key={e.id}>
                    <MemoryRow event={e} onOpen={setSelected} live={e.id === liveId} />
                  </li>
                ))}
              </ul>
            </li>
          ))}
          {events.data.length >= PAGE_LIMIT && (
            <p className="text-center text-xs text-muted-foreground">
              Showing the latest {PAGE_LIMIT.toLocaleString()} memories from this day. Use the app filter to narrow down.
            </p>
          )}
        </ol>
      )}

      <MemoryDetailDialog event={selected} onClose={() => setSelected(null)} />

      <AlertDialog open={pending !== null} onOpenChange={(open) => !open && setPending(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{pending?.title}</AlertDialogTitle>
            <AlertDialogDescription>{pending?.description} This can't be undone.</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={confirmDelete}>
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
