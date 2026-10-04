import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { useAsync } from "@/hooks/use-async";
import { notifyError, useRecall } from "@/hooks/use-recall";
import { api, type PermissionInfo } from "@/lib/api";

function stillNeeded(list: PermissionInfo[]): PermissionInfo[] {
  return list.filter((p) => p.granted === false);
}

/** Shown on launch only for permissions the OS reports as not granted. */
function restartKey(version: string): string {
  return `recall.permission-restarted.${version}`;
}

export function PermissionPrompt() {
  const { info } = useRecall();
  const [version, setVersion] = useState(0);
  const [dismissed, setDismissed] = useState(false);
  const [checking, setChecking] = useState(false);
  const [needsRestart, setNeedsRestart] = useState(false);
  const restartedOnce = localStorage.getItem(restartKey(info.version)) === "1";
  const perms = useAsync(() => api.permissions(), [version]);
  const missing = stillNeeded(perms.data ?? []);

  useEffect(() => {
    if (perms.status === "ready" && restartedOnce && stillNeeded(perms.data ?? []).length > 0) {
      setDismissed(true);
    }
  }, [perms.status, perms.data, restartedOnce]);

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

  async function confirmEnabled() {
    setChecking(true);
    setNeedsRestart(false);
    try {
      const list = await api.permissions();
      setVersion((v) => v + 1);
      if (stillNeeded(list).length === 0) {
        localStorage.removeItem(restartKey(info.version));
        setDismissed(true);
        return;
      }
      // One restart per app version. macOS often reports access as off until then,
      // and restarting again does not change the answer.
      if (restartedOnce) {
        setDismissed(true);
        return;
      }
      setNeedsRestart(true);
    } catch (e) {
      notifyError(e, "Couldn't check permissions");
    } finally {
      setChecking(false);
    }
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
        {needsRestart && (
          <p className="mt-4 text-sm" role="status">
            macOS applies that permission only after Recall restarts. Restart now, and this screen stays closed if access is on.
          </p>
        )}
        <div className="mt-5 flex justify-end gap-2">
          <Button variant="ghost" onClick={() => setDismissed(true)}>
            Not now
          </Button>
          {needsRestart ? (
            <Button
              onClick={() => {
                localStorage.setItem(restartKey(info.version), "1");
                api.relaunch().catch((e) => notifyError(e, "Couldn't restart Recall"));
              }}
            >
              Restart Recall
            </Button>
          ) : (
            <Button variant="outline" disabled={checking} onClick={confirmEnabled}>
              {checking ? "Checking…" : "I've enabled them"}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
