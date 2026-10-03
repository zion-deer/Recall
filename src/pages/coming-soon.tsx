import { Bot, Check, Search } from "lucide-react";
import { PageHeader } from "@/components/app-shell";
import { Button } from "@/components/ui/button";
import { useNavigation } from "@/hooks/use-navigation";

function Planned({
  icon: Icon,
  title,
  summary,
  points,
}: {
  icon: typeof Search;
  title: string;
  summary: string;
  points: string[];
}) {
  const { navigate } = useNavigation();
  return (
    <div className="mx-auto w-full max-w-2xl px-5 py-8 md:px-10 md:py-10">
      <PageHeader title={title} />
      <div className="rounded-2xl border bg-card p-8">
        <div className="flex size-11 items-center justify-center rounded-xl bg-brand-soft text-brand">
          <Icon className="size-5" aria-hidden />
        </div>
        <p className="mt-5 inline-block rounded-md bg-muted px-2 py-0.5 text-xs font-medium text-muted-foreground">
          Not available in this version
        </p>
        <p className="mt-3 text-[15px] leading-relaxed">{summary}</p>
        <ul className="mt-5 space-y-2.5 text-sm text-muted-foreground">
          {points.map((p) => (
            <li key={p} className="flex gap-2.5">
              <Check className="mt-0.5 size-4 shrink-0 text-brand" aria-hidden />
              {p}
            </li>
          ))}
        </ul>
        <Button variant="outline" className="mt-7" onClick={() => navigate("memory")}>
          Browse your timeline
        </Button>
      </div>
    </div>
  );
}

export function SearchPage() {
  return (
    <Planned
      icon={Search}
      title="Search"
      summary="Search is the next thing we're building. You'll be able to type a few words and instantly find what you saw or worked on."
      points={[
        "Full-text search across app names, window titles, pages, and files",
        "Filter results by date, app, website, and memory type",
        "Jump from a result straight to that moment in your timeline",
        "Runs entirely on your computer and works offline",
      ]}
    />
  );
}

export function AgentPage() {
  return (
    <Planned
      icon={Bot}
      title="Agent"
      summary="Later, Recall will be able to help you get things done — like reopening yesterday's files — but only through specific, safe actions you approve."
      points={[
        "A fixed set of actions (open a file, open a website, create a folder) — never arbitrary commands",
        "Anything that changes your files asks you first, in plain language",
        "A visible log of every action the agent takes",
        "Off by default. Memory works without it.",
      ]}
    />
  );
}
