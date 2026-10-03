import { Sparkles } from "lucide-react";
import { useState, type FormEvent } from "react";
import { PageHeader } from "@/components/app-shell";
import { MemoryDetailDialog } from "@/components/memory-detail-dialog";
import { MemoryRow } from "@/components/memory-row";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useNavigation } from "@/hooks/use-navigation";
import { api, toRecallError, type AskResponse, type MemoryEvent } from "@/lib/api";

const EXAMPLES = [
  "What was I doing yesterday?",
  "What website was I looking at about Rust?",
  "Summarize what I was doing between 2 PM and 4 PM",
];

export function AskPage() {
  const { navigate } = useNavigation();
  const [question, setQuestion] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [response, setResponse] = useState<AskResponse | null>(null);
  const [selected, setSelected] = useState<MemoryEvent | null>(null);

  async function submit(event?: FormEvent) {
    event?.preventDefault();
    const text = question.trim();
    if (!text) return;
    setBusy(true);
    setError(null);
    try {
      setResponse(await api.ask(text));
    } catch (err) {
      setError(toRecallError(err).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto w-full max-w-3xl px-5 py-8 md:px-10 md:py-10">
      <PageHeader
        title="Ask Recall"
        description="Answers use Llama 3.2 1B on this computer and only the memories relevant to your question."
      />
      <form onSubmit={submit} className="space-y-3">
        <label htmlFor="ask-recall" className="sr-only">
          Ask about your recorded activity
        </label>
        <Textarea
          id="ask-recall"
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          placeholder="What was I working on yesterday?"
          className="min-h-28 text-base"
          maxLength={1000}
          autoFocus
        />
        <div className="flex flex-wrap items-center gap-2">
          <Button type="submit" disabled={busy || !question.trim()}>
            <Sparkles /> {busy ? "Looking through your memory…" : "Ask"}
          </Button>
          <Button type="button" variant="ghost" onClick={() => navigate("settings", "ai")}>
            AI settings
          </Button>
        </div>
      </form>
      {!response && (
        <div className="mt-4 flex flex-wrap gap-2">
          {EXAMPLES.map((example) => (
            <button
              key={example}
              type="button"
              className="rounded-full border bg-card px-3 py-1 text-left text-xs text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
              onClick={() => setQuestion(example)}
            >
              {example}
            </button>
          ))}
        </div>
      )}

      <section className="mt-8 space-y-5" aria-live="polite">
        {error && (
          <div className="rounded-xl border p-4" role="alert">
            <p className="text-sm font-medium">Recall couldn't answer</p>
            <p className="mt-1 text-sm text-muted-foreground">{error}</p>
          </div>
        )}
        {response?.message && (
          <div className="rounded-xl border bg-muted/40 p-4 text-sm">
            <p>{response.message}</p>
            {response.status === "model_not_installed" && (
              <Button className="mt-3" size="sm" onClick={() => navigate("settings", "ai")}>
                Install AI model
              </Button>
            )}
            {response.status === "ai_disabled" && (
              <Button className="mt-3" size="sm" onClick={() => navigate("settings", "ai")}>
                Enable local AI
              </Button>
            )}
          </div>
        )}
        {response?.answer && (
          <div>
            <h2 className="text-sm font-medium text-muted-foreground">Answer</h2>
            <p className="mt-2 whitespace-pre-wrap text-[15px] leading-relaxed">{response.answer}</p>
          </div>
        )}
        {response && response.memories.length > 0 && (
          <div>
            <h2 className="mb-2 text-sm font-medium text-muted-foreground">Relevant memories</h2>
            <div className="rounded-xl border bg-card px-2">
              {response.memories.map((memory) => (
                <MemoryRow key={memory.id} event={memory} onOpen={setSelected} />
              ))}
            </div>
          </div>
        )}
      </section>
      <MemoryDetailDialog event={selected} onClose={() => setSelected(null)} />
    </div>
  );
}
