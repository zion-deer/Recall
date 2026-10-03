import { save } from "@tauri-apps/plugin-dialog";
import { Download, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
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
import { Input } from "@/components/ui/input";
import { notifyError } from "@/hooks/use-recall";
import { api } from "@/lib/api";

export function ExportButton() {
  const [busy, setBusy] = useState(false);

  async function run() {
    const date = new Date().toISOString().slice(0, 10);
    let path: string | null;
    try {
      path = await save({
        title: "Export your Recall data",
        defaultPath: `recall-export-${date}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
    } catch (e) {
      notifyError(e, "Couldn't open the save dialog");
      return;
    }
    if (!path) return;
    if (!path.toLowerCase().endsWith(".json")) path = `${path}.json`;
    setBusy(true);
    try {
      const res = await api.exportData(path);
      toast.success(`Exported ${res.eventCount.toLocaleString()} memories`, { description: res.path });
    } catch (e) {
      notifyError(e, "Export failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Button variant="outline" size="sm" onClick={run} disabled={busy}>
      <Download /> {busy ? "Exporting…" : "Export data"}
    </Button>
  );
}

const CONFIRM_WORD = "delete";

export function DeleteAllButton() {
  const [open, setOpen] = useState(false);
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);

  async function run() {
    setBusy(true);
    try {
      const n = await api.deleteAll();
      toast.success("All memories deleted", {
        description: `${n.toLocaleString()} ${n === 1 ? "memory was" : "memories were"} permanently removed.`,
      });
      setOpen(false);
    } catch (e) {
      notifyError(e, "Couldn't delete your memories");
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <Button
        variant="destructive"
        size="sm"
        onClick={() => {
          setTyped("");
          setOpen(true);
        }}
      >
        <Trash2 /> Delete all memory
      </Button>
      <AlertDialog open={open} onOpenChange={setOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete all of your memories?</AlertDialogTitle>
            <AlertDialogDescription>
              Every memory Recall has stored on this computer will be permanently erased. Your settings and
              exclusions are kept. This can't be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="space-y-1.5">
            <label htmlFor="confirm-delete" className="text-sm">
              Type <span className="font-mono font-semibold">{CONFIRM_WORD}</span> to confirm
            </label>
            <Input
              id="confirm-delete"
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              autoComplete="off"
              autoFocus
            />
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={busy || typed.trim().toLowerCase() !== CONFIRM_WORD}
              onClick={run}
            >
              {busy ? "Deleting…" : "Delete everything"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
