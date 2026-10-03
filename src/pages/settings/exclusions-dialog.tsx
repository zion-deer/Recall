import { Plus, X } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { notifyError } from "@/hooks/use-recall";
import { api, toRecallError, type Exclusion, type ExclusionKind } from "@/lib/api";

const KIND_INFO: Record<ExclusionKind, { label: string; placeholder: string }> = {
  app: { label: "App", placeholder: "App name, like Slack or Signal" },
  title: { label: "Window title contains", placeholder: "Words like “Incognito” or “Payroll”" },
  website: { label: "Website", placeholder: "example.com" },
  folder: { label: "Folder", placeholder: "/Users/you/Private or C:\\Private" },
};

export interface ExclusionsDialogConfig {
  title: string;
  description: string;
  kinds: ExclusionKind[];
  note?: string;
}

export function ExclusionsDialog({
  config,
  onClose,
}: {
  config: ExclusionsDialogConfig | null;
  onClose: () => void;
}) {
  const [items, setItems] = useState<Exclusion[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [kind, setKind] = useState<ExclusionKind>("app");
  const [pattern, setPattern] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!config) return;
    setKind(config.kinds[0]);
    setPattern("");
    setFormError(null);
    setItems(null);
    setLoadError(null);
    api.listExclusions().then(setItems, (e) => setLoadError(toRecallError(e).message));
  }, [config]);

  const visible = (items ?? []).filter((i) => config?.kinds.includes(i.kind));

  async function add(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setFormError(null);
    try {
      const { exclusion, removedMemories } = await api.addExclusion(kind, pattern);
      setItems((prev) => [...(prev ?? []), exclusion]);
      setPattern("");
      toast.success(`Excluded ${exclusion.pattern}`, {
        description:
          removedMemories > 0
            ? `${removedMemories} existing ${removedMemories === 1 ? "memory was" : "memories were"} deleted.`
            : "It won't be recorded from now on.",
      });
    } catch (err) {
      setFormError(toRecallError(err).message);
    } finally {
      setBusy(false);
    }
  }

  async function remove(item: Exclusion) {
    try {
      await api.removeExclusion(item.id);
      setItems((prev) => (prev ?? []).filter((i) => i.id !== item.id));
    } catch (err) {
      notifyError(err, "Couldn't remove exclusion");
    }
  }

  const kindItems = config?.kinds.map((k) => ({ value: k, label: KIND_INFO[k].label })) ?? [];

  return (
    <Dialog open={config !== null} onOpenChange={(open) => !open && onClose()}>
      {config && (
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>{config.title}</DialogTitle>
            <DialogDescription>{config.description}</DialogDescription>
          </DialogHeader>

          <form onSubmit={add} className="space-y-2">
            <div className="flex flex-wrap gap-2">
              {config.kinds.length > 1 && (
                <Select items={kindItems} value={kind} onValueChange={(v) => v && setKind(v as ExclusionKind)}>
                  <SelectTrigger aria-label="Exclusion type" className="h-9">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {kindItems.map((k) => (
                      <SelectItem key={k.value} value={k.value}>
                        {k.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
              <Input
                aria-label={KIND_INFO[kind].label}
                placeholder={KIND_INFO[kind].placeholder}
                value={pattern}
                onChange={(e) => setPattern(e.target.value)}
                aria-invalid={formError ? true : undefined}
                className="h-9 min-w-48 flex-1"
                maxLength={256}
              />
              <Button type="submit" className="h-9" disabled={busy || !pattern.trim()}>
                <Plus /> Add
              </Button>
            </div>
            {formError && (
              <p className="text-xs text-destructive" role="alert">
                {formError}
              </p>
            )}
            {config.note && <p className="text-xs text-muted-foreground">{config.note}</p>}
          </form>

          <div className="max-h-72 overflow-y-auto rounded-lg border">
            {loadError ? (
              <p className="p-4 text-sm text-destructive">{loadError}</p>
            ) : items === null ? (
              <p className="p-4 text-sm text-muted-foreground">Loading…</p>
            ) : visible.length === 0 ? (
              <p className="p-4 text-sm text-muted-foreground">Nothing excluded yet.</p>
            ) : (
              <ul className="divide-y">
                {visible.map((i) => (
                  <li key={i.id} className="flex items-center gap-3 px-3 py-2">
                    <span className="min-w-0 flex-1 truncate text-sm">{i.pattern}</span>
                    {config.kinds.length > 1 && (
                      <span className="text-xs text-muted-foreground">{KIND_INFO[i.kind].label}</span>
                    )}
                    <Button
                      variant="ghost"
                      size="icon-xs"
                      aria-label={`Stop excluding ${i.pattern}`}
                      onClick={() => remove(i)}
                    >
                      <X />
                    </Button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </DialogContent>
      )}
    </Dialog>
  );
}
