import { CalendarDays, Search, X } from "lucide-react";
import { useState, type FormEvent } from "react";
import { PageHeader } from "@/components/app-shell";
import { MemoryDetailDialog } from "@/components/memory-detail-dialog";
import { MemoryRow } from "@/components/memory-row";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { api, toRecallError, type MemoryEvent } from "@/lib/api";
import { addDays, formatDayLabel, startOfDay } from "@/lib/format";
import { ListSkeleton } from "@/pages/home";

const KIND_ITEMS = [
  { value: "all", label: "All memories" },
  { value: "browser_activity", label: "Websites" },
  { value: "app_activity", label: "App activity" },
];

const DATE_ITEMS = [
  { value: "all", label: "Any time" },
  { value: "today", label: "Today" },
  { value: "yesterday", label: "Yesterday" },
  { value: "7", label: "Past 7 days" },
  { value: "30", label: "Past 30 days" },
];

export function SearchPage() {
  const [text, setText] = useState("");
  const [submitted, setSubmitted] = useState("");
  const [kind, setKind] = useState("all");
  const [date, setDate] = useState("all");
  const [results, setResults] = useState<MemoryEvent[] | null>(null);
  const [selected, setSelected] = useState<MemoryEvent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function run(e?: FormEvent) {
    e?.preventDefault();
    const query = text.trim();
    if (!query) return;
    setSubmitted(query);
    setLoading(true);
    setError(null);
    const now = Date.now();
    let start: number | undefined;
    let end: number | undefined;
    if (date === "today") start = startOfDay(now);
    if (date === "yesterday") {
      start = addDays(now, -1);
      end = startOfDay(now);
    }
    if (date === "7" || date === "30") start = addDays(now, -Number(date));
    try {
      setResults(
        await api.searchEvents({
          text: query,
          start,
          end,
          kind: kind === "all" ? undefined : kind,
          limit: 200,
        }),
      );
    } catch (err) {
      setError(toRecallError(err).message);
      setResults(null);
    } finally {
      setLoading(false);
    }
  }

  function clear() {
    setText("");
    setSubmitted("");
    setResults(null);
    setError(null);
  }

  return (
    <div className="mx-auto w-full max-w-4xl px-5 py-8 md:px-10 md:py-10">
      <PageHeader
        title="Search"
        description="Find apps, window titles, websites, and URLs in your local memory."
      />
      <form onSubmit={run} className="space-y-3">
        <div className="flex items-center gap-2 rounded-xl border bg-card p-2 shadow-sm focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/30">
          <Search className="ml-2 size-4 shrink-0 text-muted-foreground" aria-hidden />
          <Input
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="Search your memory…"
            aria-label="Search your memory"
            className="h-10 border-0 bg-transparent shadow-none focus-visible:ring-0 dark:bg-transparent"
            autoFocus
            maxLength={500}
          />
          {text && (
            <Button type="button" variant="ghost" size="icon-sm" onClick={clear} aria-label="Clear search">
              <X />
            </Button>
          )}
          <Button type="submit" disabled={!text.trim() || loading}>
            Search
          </Button>
        </div>
        <div className="flex flex-wrap gap-2">
          <Select items={KIND_ITEMS} value={kind} onValueChange={(v) => setKind((v as string) ?? "all")}>
            <SelectTrigger size="sm" aria-label="Memory type" className="min-w-32">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {KIND_ITEMS.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select items={DATE_ITEMS} value={date} onValueChange={(v) => setDate((v as string) ?? "all")}>
            <SelectTrigger size="sm" aria-label="Date range" className="min-w-32">
              <CalendarDays />
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {DATE_ITEMS.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </form>

      <section className="mt-7" aria-live="polite">
        {loading ? (
          <ListSkeleton rows={6} />
        ) : error ? (
          <div className="rounded-xl border p-6 text-center" role="alert">
            <p className="text-sm font-medium">Search failed</p>
            <p className="mt-1 text-sm text-muted-foreground">{error}</p>
            <Button variant="outline" size="sm" className="mt-3" onClick={() => run()}>
              Try again
            </Button>
          </div>
        ) : results ? (
          <>
            <p className="mb-3 text-sm text-muted-foreground">
              {results.length === 0
                ? `No memories found for “${submitted}”`
                : `${results.length} ${results.length === 1 ? "result" : "results"} for “${submitted}”`}
            </p>
            {results.length === 0 ? (
              <div className="rounded-xl border border-dashed px-6 py-12 text-center">
                <p className="text-sm font-medium">Try different words</p>
                <p className="mt-1 text-sm text-muted-foreground">
                  Search app names, page titles, website domains, or parts of a URL.
                </p>
              </div>
            ) : (
              <ol className="divide-y rounded-xl border bg-card px-2">
                {results.map((event) => (
                  <li key={event.id}>
                    <MemoryRow event={event} onOpen={setSelected} showTime={false} />
                    <p className="-mt-1 mb-2 ml-11 text-xs text-muted-foreground">
                      {formatDayLabel(event.startedAt)},{" "}
                      {new Date(event.startedAt).toLocaleTimeString(undefined, {
                        hour: "numeric",
                        minute: "2-digit",
                      })}
                    </p>
                  </li>
                ))}
              </ol>
            )}
          </>
        ) : (
          <div className="rounded-xl border border-dashed px-6 py-12 text-center">
            <Search className="mx-auto size-6 text-muted-foreground" aria-hidden />
            <p className="mt-3 text-sm font-medium">Search everything Recall remembers</p>
            <p className="mx-auto mt-1 max-w-sm text-sm text-muted-foreground">
              Results stay on this computer. No search terms or memories are sent anywhere.
            </p>
          </div>
        )}
      </section>
      <MemoryDetailDialog event={selected} onClose={() => setSelected(null)} />
    </div>
  );
}
