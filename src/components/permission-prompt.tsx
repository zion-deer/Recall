import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { useAsync } from "@/hooks/use-async";
import { notifyError } from "@/hooks/use-recall";
import { api, type PermissionInfo } from "@/lib/api";

/** Shown on launch only for permissions the OS reports as not granted. */
export function PermissionPrompt() {
  const [version, setVersion] = useState(0);
  const [dismissed, setDismissed] = useState(false);
  const perms = useAsync(() => api.permissions(), [version]);
  const missing = (perms.data ?? []).filter((p) => p.granted === false);

  useEffect(() => {
    function onFocus() {
      setVersion((v) => v + 1);
    }
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  if (dismissed || missing.length === 0) return null;

  async function enable(id: string) {
    try {
      await api.requestPermission(id);
    } catch (e) {
      notifyError(e, "Couldn't request permission");
    }
    setVersion((v) => v + 1);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-4 backdrop-blur-sm">
      <div role="dialog" aria-labelledby="perm-title" className="w-full max-w-lg rounded-2xl border bg-card p-6 shadow-lg">
        <h2 id="perm-title" className="text-lg font-semibold">
          Enable permissions
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">
          Recall needs these so it can remember window titles and, if you turn screenshots on, the screen. Already allowed permissions are not shown.
        </p>
        <ul className="mt-4 space-y-3">
          {missing.map((p: PermissionInfo) => (
            <li key={p.id} className="rounded-xl border p-4">
              <p className="font-medium">{p.name}</p>
              <p className="mt-1 text-sm text-muted-foreground">{p.reason}</p>
              <Button className="mt-3" size="sm" onClick={() => enable(p.id)}>
                Enable {p.name}
              </Button>
            </li>
          ))}
        </ul>
        <div className="mt-5 flex justify-end gap-2">
          <Button variant="ghost" onClick={() => setDismissed(true)}>
            Not now
          </Button>
          <Button variant="outline" onClick={() => setVersion((v) => v + 1)}>
            I've enabled them
          </Button>
        </div>
      </div>
    </div>
  );
}
