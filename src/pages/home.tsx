import { ArrowRight, Clock3, Info, Pause, Play, Power, ShieldCheck, Sparkles } from "lucide-react";
import { useState, type FormEvent } from "react";
import { AppAvatar } from "@/components/app-avatar";
import { MemoryDetailDialog } from "@/components/memory-detail-dialog";
import { MemoryRow } from "@/components/memory-row";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { useAsync } from "@/hooks/use-async";
import { useNavigation } from "@/hooks/use-navigation";
import { useRecall } from "@/hooks/use-recall";
import { api, toRecallError, type AskResponse, type MemoryEvent } from "@/lib/api";
import { dayRange, describePause, formatDuration } from "@/lib/format";

const SUGGESTIONS = [
  "What was I working on yesterday?",
  "Continue where I left off",
  "Find the PDF I opened last Tuesday",
];

function greeting(now = new Date()): string {
  const h = now.getHours();
  if (h < 5) return "Working late";
  if (h < 12) return "Good morning";
  if (h < 18) return "Good afternoon";
  return "Good evening";
}

export function HomePage() {
  const { memoryVersion, status, settings } = useRecall();
  const { navigate } = useNavigation();
  const [question, setQuestion] = useState("");
  const [busy, setBusy] = useState(false);
  const [answer, setAnswer] = useState<AskResponse | null>(null);
  const [askError, setAskError] = useState<string | null>(null);
  const [selected, setSelected] = useState<MemoryEvent | null>(null);

  const today = dayRange(Date.now());
  const recent = useAsync(() => api.listEvents({ limit: 6 }), [memoryVersion]);
  const usage = useAsync(() => api.appUsage(today.start, today.end), [memoryVersion, today.start]);

  async function ask(e: FormEvent) {
    e.preventDefault();
    const text = question.trim();
    if (!text || busy) return;
    setBusy(true);
    setAskError(null);
    try {
      setAnswer(await api.ask(text));
    } catch (err) {
      setAskError(toRecallError(err).message);
      setAnswer(null);
    } finally {
      setBusy(false);
    }
  }

  const totalToday = usage.data?.reduce((sum, u) => sum + u.totalMs, 0) ?? 0;
  const topApps = usage.data?.slice(0, 5) ?? [];
  const liveId = status.state === "recording" ? recent.data?.[0]?.id : undefined;

  return (
    <div className="mx-auto w-full max-w-4xl px-5 py-8 md:px-10 md:py-12">
      <section aria-labelledby="ask-heading" className="pb-10">
        <p className="text-sm text-muted-foreground">{greeting()}</p>
        <h1 id="ask-heading" className="mt-1 text-3xl font-semibold tracking-tight md:text-4xl">
          What do you want to remember?
        </h1>

        <form onSubmit={ask} className="mt-6">
          <label htmlFor="ask" className="sr-only">
            Ask Recall
          </label>
          <div className="flex items-center gap-2 rounded-2xl border bg-card p-2 shadow-sm transition-shadow focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/30">
            <Sparkles className="ml-2 size-5 shrink-0 text-brand" aria-hidden />
            <Input
              id="ask"
              value={question}
              onChange={(e) => setQuestion(e.target.value)}
              placeholder="What was I working on yesterday?"
              autoComplete="off"
              className="h-11 border-0 bg-transparent text-base shadow-none focus-visible:ring-0 dark:bg-transparent"
            />
            <Button type="submit" size="lg" className="h-10 rounded-xl px-4" disabled={busy || !question.trim()}>
              {busy ? "Asking…" : "Ask"}
            </Button>
          </div>
        </form>

        {askError || answer ? (
          <div className="mt-4 space-y-3 rounded-xl border bg-muted/50 p-4 text-sm" role="status">
            {askError && <p className="text-destructive">{askError}</p>}
            {answer?.message && <p>{answer.message}</p>}
            {answer?.answer && <p className="whitespace-pre-wrap text-[15px] leading-relaxed">{answer.answer}</p>}
            {answer?.status === "model_not_installed" && (
              <Button size="sm" onClick={() => navigate("settings", "ai")}>
                Install AI model
              </Button>
            )}
            {answer && answer.memories.length > 0 && (
              <div className="rounded-xl border bg-card px-2">
                {answer.memories.map((event) => (
                  <MemoryRow key={event.id} event={event} onOpen={setSelected} />
                ))}
              </div>
            )}
          </div>
        ) : (
          <div className="mt-3 flex flex-wrap gap-2">
            {SUGGESTIONS.map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => setQuestion(s)}
                className="rounded-full border bg-card px-3 py-1 text-xs text-muted-foreground outline-none transition-colors hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
              >
                {s}
              </button>
            ))}
          </div>
        )}
      </section>

      <RecordingBanner />

      <div className="grid gap-6 lg:grid-cols-[1.4fr_1fr]">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <CardTitle>Recent activity</CardTitle>
            <Button variant="ghost" size="sm" onClick={() => navigate("memory")}>
              Timeline <ArrowRight />
            </Button>
          </CardHeader>
          <CardContent className="px-2">
            {recent.status === "error" && !recent.data ? (
              <p className="px-2 py-6 text-sm text-destructive">Couldn't load activity: {recent.error}</p>
            ) : !recent.data ? (
              <ListSkeleton />
            ) : recent.data.length === 0 ? (
              <EmptyActivity recording={settings.recordingEnabled} />
            ) : (
              <div className="flex flex-col">
                {recent.data.map((e) => (
                  <MemoryRow key={e.id} event={e} onOpen={setSelected} live={e.id === liveId} />
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        <div className="flex flex-col gap-6">
          <Card>
            <CardHeader>
              <CardTitle>Today at a glance</CardTitle>
            </CardHeader>
            <CardContent>
              {!usage.data ? (
                <ListSkeleton rows={3} />
              ) : topApps.length === 0 ? (
                <p className="text-sm text-muted-foreground">
                  Time spent in each app will show up here as you work.
                </p>
              ) : (
                <div className="space-y-3">
                  <p className="text-sm text-muted-foreground">
                    <span className="text-2xl font-semibold tabular-nums text-foreground">
                      {formatDuration(totalToday)}
                    </span>{" "}
                    of activity remembered
                  </p>
                  <ul className="space-y-2.5">
                    {topApps.map((u) => (
                      <li key={u.appName} className="flex items-center gap-2.5">
                        <AppAvatar name={u.appName} className="size-6 rounded-md text-[11px]" />
                        <div className="min-w-0 flex-1">
                          <div className="flex justify-between gap-2 text-sm">
                            <span className="truncate">{u.appName}</span>
                            <span className="shrink-0 tabular-nums text-muted-foreground">
                              {formatDuration(u.totalMs)}
                            </span>
                          </div>
                          <div className="mt-1 h-1 overflow-hidden rounded-full bg-muted">
                            <div
                              className="h-full rounded-full bg-brand/70"
                              style={{ width: `${Math.max(4, (u.totalMs / (topApps[0].totalMs || 1)) * 100)}%` }}
                            />
                          </div>
                        </div>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </CardContent>
          </Card>

          <QuickActions />
        </div>
      </div>

      <MemoryDetailDialog event={selected} onClose={() => setSelected(null)} />
    </div>
  );
}

function RecordingBanner() {
  const { status, settings, resume, updateSettings } = useRecall();
  if (!settings.recordingEnabled) {
    return (
      <Banner tone="muted" icon={<Power className="size-4" />} text="Recording is off. Recall isn't remembering anything right now.">
        <Button size="sm" onClick={() => updateSettings({ recordingEnabled: true })}>
          Turn on
        </Button>
      </Banner>
    );
  }
  if (status.state === "paused") {
    return (
      <Banner tone="paused" icon={<Pause className="size-4" />} text={`${describePause(status.pausedUntil)}. Nothing is being recorded.`}>
        <Button size="sm" onClick={resume}>
          <Play /> Resume
        </Button>
      </Banner>
    );
  }
  if (status.state === "unavailable") {
    return (
      <Banner tone="muted" icon={<Info className="size-4" />} text={status.message ?? "Activity tracking isn't available on this system."} />
    );
  }
  return null;
}

function Banner({
  tone,
  icon,
  text,
  children,
}: {
  tone: "muted" | "paused";
  icon: React.ReactNode;
  text: string;
  children?: React.ReactNode;
}) {
  return (
    <div
      role="status"
      className={
        "mb-6 flex flex-wrap items-center gap-3 rounded-xl border px-4 py-3 text-sm " +
        (tone === "paused" ? "border-paused/50 bg-paused/10" : "bg-muted/50")
      }
    >
      <span className="text-muted-foreground">{icon}</span>
      <p className="flex-1">{text}</p>
      {children}
    </div>
  );
}

function QuickActions() {
  const { navigate } = useNavigation();
  const { pause, status } = useRecall();
  const actions = [
    {
      icon: Pause,
      label: "Pause for 15 minutes",
      onClick: () => pause(15),
      disabled: status.state === "paused" || status.state === "off",
    },
    { icon: Clock3, label: "Open your timeline", onClick: () => navigate("memory") },
    { icon: ShieldCheck, label: "Privacy & exclusions", onClick: () => navigate("settings", "privacy") },
  ];
  return (
    <Card>
      <CardHeader>
        <CardTitle>Quick actions</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-1 px-2">
        {actions.map((a) => (
          <Button
            key={a.label}
            variant="ghost"
            className="h-9 justify-start px-2"
            onClick={a.onClick}
            disabled={a.disabled}
          >
            <a.icon className="text-muted-foreground" /> {a.label}
          </Button>
        ))}
      </CardContent>
    </Card>
  );
}

function EmptyActivity({ recording }: { recording: boolean }) {
  return (
    <div className="px-2 py-8 text-center">
      <p className="text-sm font-medium">No memories yet</p>
      <p className="mx-auto mt-1 max-w-xs text-sm text-muted-foreground">
        {recording
          ? "Switch between a few apps and they'll appear here within seconds."
          : "Turn on recording and Recall will start remembering the apps and windows you use."}
      </p>
    </div>
  );
}

export function ListSkeleton({ rows = 5 }: { rows?: number }) {
  return (
    <div className="space-y-3 px-2 py-1" aria-busy="true" aria-label="Loading">
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="flex items-center gap-3">
          <Skeleton className="size-8 rounded-lg" />
          <div className="flex-1 space-y-1.5">
            <Skeleton className="h-3.5 w-1/3" />
            <Skeleton className="h-3 w-2/3" />
          </div>
        </div>
      ))}
    </div>
  );
}
