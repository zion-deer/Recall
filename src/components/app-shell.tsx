import { Bot, Clock3, Home, Search, Settings } from "lucide-react";
import type { ReactNode } from "react";
import { RecordingControl, StatusDot } from "@/components/recording-control";
import { useNavigation, type Page } from "@/hooks/use-navigation";
import { useRecall } from "@/hooks/use-recall";
import { describeStatus } from "@/lib/status";
import { cn } from "@/lib/utils";

const NAV: { page: Page; label: string; icon: typeof Home; soon?: boolean }[] = [
  { page: "home", label: "Home", icon: Home },
  { page: "memory", label: "Memory", icon: Clock3 },
  { page: "search", label: "Search", icon: Search, soon: true },
  { page: "agent", label: "Agent", icon: Bot, soon: true },
  { page: "settings", label: "Settings", icon: Settings },
];

const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform);
export const MOD_KEY = isMac ? "⌘" : "Ctrl";

export function Logo({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 32 32" className={className} aria-hidden>
      <rect width="32" height="32" rx="9" className="fill-primary" />
      <path
        d="M10 21.5V10.5h6.2a4.3 4.3 0 0 1 0 8.6H10m6.2 0 4.8 2.4"
        fill="none"
        className="stroke-primary-foreground"
        strokeWidth="2.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function AppShell({ children }: { children: ReactNode }) {
  const { page, navigate } = useNavigation();
  const { status } = useRecall();
  const d = describeStatus(status);

  return (
    <div className="flex h-full flex-col md:flex-row">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:left-2 focus:top-2 focus:z-50 focus:rounded-md focus:bg-background focus:px-3 focus:py-2"
      >
        Skip to content
      </a>

      {/* Desktop sidebar */}
      <aside className="hidden w-60 shrink-0 flex-col border-r bg-sidebar px-3 py-4 md:flex">
        <div className="flex items-center gap-2.5 px-2 pb-6">
          <Logo className="size-7" />
          <span className="text-[15px] font-semibold tracking-tight">Recall</span>
        </div>
        <nav aria-label="Main" className="flex flex-1 flex-col gap-0.5">
          {NAV.map((item, i) => {
            const active = page === item.page;
            return (
              <button
                key={item.page}
                type="button"
                onClick={() => navigate(item.page)}
                aria-current={active ? "page" : undefined}
                title={`${item.label} (${MOD_KEY}${i + 1})`}
                className={cn(
                  "group flex items-center gap-3 rounded-lg px-2.5 py-2 text-sm text-sidebar-foreground/80 transition-colors outline-none hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-2 focus-visible:ring-sidebar-ring",
                  active && "bg-sidebar-accent font-medium text-sidebar-accent-foreground",
                )}
              >
                <item.icon className="size-4 opacity-80" />
                {item.label}
                {item.soon && (
                  <span className="ml-auto rounded-md bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
                    Soon
                  </span>
                )}
              </button>
            );
          })}
        </nav>
        <RecordingControl />
      </aside>

      {/* Compact header for narrow windows */}
      <header className="flex items-center gap-2 border-b bg-sidebar px-3 py-2 md:hidden">
        <Logo className="size-6" />
        <nav aria-label="Main" className="flex flex-1 gap-0.5 overflow-x-auto">
          {NAV.map((item) => (
            <button
              key={item.page}
              type="button"
              onClick={() => navigate(item.page)}
              aria-current={page === item.page ? "page" : undefined}
              aria-label={item.label}
              className={cn(
                "flex items-center gap-1.5 rounded-md px-2 py-1.5 text-xs text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring",
                page === item.page && "bg-sidebar-accent text-foreground",
              )}
            >
              <item.icon className="size-4" />
              <span className="hidden sm:inline">{item.label}</span>
            </button>
          ))}
        </nav>
        <button
          type="button"
          onClick={() => navigate("settings", "privacy")}
          className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
          aria-label={`${d.label}. Open privacy settings`}
        >
          <StatusDot tone={d.tone} />
          <span className="hidden sm:inline">{d.label}</span>
        </button>
      </header>

      <main id="main" tabIndex={-1} className="min-h-0 flex-1 overflow-y-auto outline-none">
        {children}
      </main>
    </div>
  );
}

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: string;
  description?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-end justify-between gap-4 pb-6">
      <div className="min-w-0">
        <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
        {description && <p className="mt-1 text-sm text-muted-foreground">{description}</p>}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </div>
  );
}
