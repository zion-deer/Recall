import { Component, lazy, Suspense, useCallback, useEffect, useMemo, useState, type ErrorInfo, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { AppShell, Logo } from "@/components/app-shell";
import { PermissionPrompt } from "@/components/permission-prompt";
import { Button } from "@/components/ui/button";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { NavigationContext, type Page, type SettingsSection } from "@/hooks/use-navigation";
import { RecallProvider, useRecall } from "@/hooks/use-recall";
import { useAppliedTheme } from "@/hooks/use-theme";
import { api, hasBackend, toRecallError, type UpdateOffer } from "@/lib/api";
import { HomePage } from "@/pages/home";
import { Onboarding } from "@/pages/onboarding";

const AskPage = lazy(() => import("@/pages/ask").then((m) => ({ default: m.AskPage })));
const MemoryPage = lazy(() => import("@/pages/memory").then((m) => ({ default: m.MemoryPage })));
const SearchPage = lazy(() => import("@/pages/search").then((m) => ({ default: m.SearchPage })));
const SettingsPage = lazy(() => import("@/pages/settings").then((m) => ({ default: m.SettingsPage })));

const PAGE_ORDER: Page[] = ["home", "memory", "search", "ask", "settings"];

function Recall() {
  const { settings } = useRecall();
  const theme = useAppliedTheme(settings.theme);
  const [page, setPage] = useState<Page>("home");
  const [settingsSection, setSettingsSection] = useState<SettingsSection>("general");

  const navigate = useCallback((p: Page, section?: SettingsSection) => {
    setPage(p);
    if (section) setSettingsSection(section);
    document.getElementById("main")?.scrollTo({ top: 0 });
  }, []);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (!(e.metaKey || e.ctrlKey) || e.altKey || e.shiftKey) return;
      const n = Number(e.key);
      if (n >= 1 && n <= PAGE_ORDER.length) {
        e.preventDefault();
        navigate(PAGE_ORDER[n - 1]);
      } else if (e.key === ",") {
        e.preventDefault();
        navigate("settings");
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate]);

  const [update, setUpdate] = useState<UpdateOffer | null>(null);
  const [updating, setUpdating] = useState(false);
  useEffect(() => {
    const subscription = listen<UpdateOffer>("update:available", (event) => setUpdate(event.payload));
    return () => {
      subscription.then((unlisten) => unlisten());
    };
  }, []);
  const nav = useMemo(() => ({ page, settingsSection, navigate }), [page, settingsSection, navigate]);

  return (
    <>
      {settings.onboardingCompleted ? (
        <NavigationContext.Provider value={nav}>
          {update && (
            <div className="flex flex-wrap items-center gap-3 border-b bg-brand-soft px-4 py-2 text-sm" role="status">
              <p className="flex-1">
                Recall {update.version} is available. You're using {update.currentVersion}.
              </p>
              <Button size="sm" variant="ghost" onClick={() => setUpdate(null)}>Later</Button>
              <Button
                size="sm"
                disabled={updating}
                onClick={() => {
                  setUpdating(true);
                  api.installUpdate().catch((error) => {
                    setUpdating(false);
                    window.alert(toRecallError(error).message);
                  });
                }}
              >
                {updating ? "Updating…" : "Update now"}
              </Button>
            </div>
          )}
          <PermissionPrompt />
          <AppShell>
            <Suspense fallback={<p className="p-8 text-sm text-muted-foreground">Loading…</p>}>
              {page === "home" && <HomePage />}
              {page === "memory" && <MemoryPage />}
              {page === "search" && <SearchPage />}
              {page === "ask" && <AskPage />}
              {page === "settings" && <SettingsPage />}
            </Suspense>
          </AppShell>
        </NavigationContext.Provider>
      ) : (
        <Onboarding />
      )}
      <Toaster theme={theme} position="bottom-right" />
    </>
  );
}

function Centered({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full items-center justify-center p-6">
      <div className="max-w-md text-center">
        <Logo className="mx-auto mb-5 size-10" />
        {children}
      </div>
    </div>
  );
}

function Loading() {
  return (
    <Centered>
      <p className="text-sm text-muted-foreground" aria-busy="true">
        Opening your memory…
      </p>
    </Centered>
  );
}

function StartupError({ message, retry }: { message: string; retry: () => void }) {
  return (
    <Centered>
      <h1 className="text-lg font-semibold">Recall couldn't load</h1>
      <p className="mt-2 text-sm text-muted-foreground">{message}</p>
      <Button className="mt-5" onClick={retry}>
        Try again
      </Button>
    </Centered>
  );
}

function NoBackend() {
  return (
    <Centered>
      <h1 className="text-lg font-semibold">Recall runs as a desktop app</h1>
      <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
        This is Recall's interface without its desktop backend, so there's no memory to show. Start the full app
        with <code className="rounded bg-muted px-1 py-0.5 font-mono text-xs">npm run tauri dev</code>.
      </p>
    </Centered>
  );
}

class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("UI crashed", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <Centered>
          <h1 className="text-lg font-semibold">Something went wrong</h1>
          <p className="mt-2 text-sm text-muted-foreground">
            Recall's window hit an unexpected error. Recording continues in the background.
          </p>
          <p className="mt-3 break-words font-mono text-xs text-muted-foreground">{this.state.error.message}</p>
          <Button className="mt-5" onClick={() => window.location.reload()}>
            Reload window
          </Button>
        </Centered>
      );
    }
    return this.props.children;
  }
}

export default function App() {
  if (!hasBackend()) return <NoBackend />;
  return (
    <ErrorBoundary>
      <TooltipProvider>
        <RecallProvider loading={<Loading />} failed={(message, retry) => <StartupError message={message} retry={retry} />}>
          <Recall />
        </RecallProvider>
      </TooltipProvider>
    </ErrorBoundary>
  );
}
