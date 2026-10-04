import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { toast } from "sonner";
import {
  api,
  events,
  toRecallError,
  type AppInfo,
  type RecorderStatus,
  type Settings,
} from "@/lib/api";

interface RecallContextValue {
  info: AppInfo;
  settings: Settings;
  status: RecorderStatus;
  /** Incremented whenever stored memories change, so views can refetch. */
  memoryVersion: number;
  updateSettings: (patch: Partial<Settings>) => Promise<boolean>;
  pause: (minutes?: number) => Promise<void>;
  resume: () => Promise<void>;
}

const RecallContext = createContext<RecallContextValue | null>(null);

export function useRecall(): RecallContextValue {
  const ctx = useContext(RecallContext);
  if (!ctx) throw new Error("useRecall must be used inside RecallProvider");
  return ctx;
}

export function notifyError(e: unknown, fallback = "Something went wrong") {
  const err = toRecallError(e);
  toast.error(fallback, { description: err.message });
}

type LoadState =
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | { kind: "ready"; info: AppInfo; settings: Settings; status: RecorderStatus };

export function RecallProvider({
  children,
  loading,
  failed,
}: {
  children: ReactNode;
  loading: ReactNode;
  failed: (message: string, retry: () => void) => ReactNode;
}) {
  const [state, setState] = useState<LoadState>({ kind: "loading" });
  const [memoryVersion, setMemoryVersion] = useState(0);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;
    Promise.all([api.appInfo(), api.getSettings(), api.recorderStatus()])
      .then(([info, settings, status]) => {
        if (!cancelled) setState({ kind: "ready", info, settings, status });
      })
      .catch((e) => {
        if (!cancelled) setState({ kind: "error", message: toRecallError(e).message });
      });
    return () => {
      cancelled = true;
    };
  }, [attempt]);

  useEffect(() => {
    let memoryTimer = 0;
    const subs = [
      events.onRecorderStatus((status) =>
        setState((s) => (s.kind === "ready" ? { ...s, status } : s)),
      ),
      events.onSettingsChanged((settings) =>
        setState((s) => (s.kind === "ready" ? { ...s, settings } : s)),
      ),
      events.onMemoryChanged(() => {
        window.clearTimeout(memoryTimer);
        memoryTimer = window.setTimeout(() => setMemoryVersion((v) => v + 1), 750);
      }),
    ];
    return () => {
      window.clearTimeout(memoryTimer);
      subs.forEach((p) => p.then((unlisten) => unlisten()));
    };
  }, []);

  const settings = state.kind === "ready" ? state.settings : null;

  const updateSettings = useCallback(
    async (patch: Partial<Settings>) => {
      if (!settings) return false;
      try {
        const saved = await api.updateSettings({ ...settings, ...patch });
        setState((s) => (s.kind === "ready" ? { ...s, settings: saved } : s));
        return true;
      } catch (e) {
        notifyError(e, "Couldn't save that setting");
        return false;
      }
    },
    [settings],
  );

  const pause = useCallback(async (minutes?: number) => {
    try {
      const saved = await api.pause(minutes);
      setState((s) => (s.kind === "ready" ? { ...s, settings: saved } : s));
    } catch (e) {
      notifyError(e, "Couldn't pause recording");
    }
  }, []);

  const resume = useCallback(async () => {
    try {
      const saved = await api.resume();
      setState((s) => (s.kind === "ready" ? { ...s, settings: saved } : s));
    } catch (e) {
      notifyError(e, "Couldn't resume recording");
    }
  }, []);

  const value = useMemo<RecallContextValue | null>(
    () =>
      state.kind === "ready"
        ? {
            info: state.info,
            settings: state.settings,
            status: state.status,
            memoryVersion,
            updateSettings,
            pause,
            resume,
          }
        : null,
    [state, memoryVersion, updateSettings, pause, resume],
  );

  if (state.kind === "loading") return <>{loading}</>;
  if (state.kind === "error") return <>{failed(state.message, () => setAttempt((a) => a + 1))}</>;
  return <RecallContext.Provider value={value}>{children}</RecallContext.Provider>;
}
