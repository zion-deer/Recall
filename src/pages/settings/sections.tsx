import { ExternalLink, FolderOpen, RotateCw } from "lucide-react";
import { useState } from "react";
import { PAUSE_OPTIONS } from "@/components/recording-control";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useAsync } from "@/hooks/use-async";
import { notifyError, useRecall } from "@/hooks/use-recall";
import { api, type Settings, type Theme } from "@/lib/api";
import { describePause, formatBytes, formatDayLabel, retentionLabel } from "@/lib/format";
import { cn } from "@/lib/utils";
import { DeleteAllButton, ExportButton } from "./data-actions";
import { ExclusionsDialog, type ExclusionsDialogConfig } from "./exclusions-dialog";
import { Group, Row, Section, SoonBadge, ToggleRow } from "./parts";

function ChoiceSelect<T extends string | number | null>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
}) {
  const items = options.map((o) => ({ value: String(o.value), label: o.label }));
  return (
    <Select
      items={items}
      value={String(value)}
      onValueChange={(v) => {
        const match = options.find((o) => String(o.value) === v);
        if (match) onChange(match.value);
      }}
    >
      <SelectTrigger aria-label={label} className="min-w-36">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {items.map((i) => (
          <SelectItem key={i.value} value={i.value}>
            {i.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

function useSetting() {
  const { settings, updateSettings } = useRecall();
  return [settings, (patch: Partial<Settings>) => void updateSettings(patch)] as const;
}

export function GeneralSection() {
  const [s, set] = useSetting();
  const { info } = useRecall();
  return (
    <Section title="General">
      <Group>
        <ToggleRow
          label="Memory recording"
          description="When on, Recall remembers the apps and windows you use. Turn it off any time."
          checked={s.recordingEnabled}
          onChange={(v) => set({ recordingEnabled: v })}
        />
        <Row
          label="Running in the background"
          description="Closing the window keeps Recall running in your system tray so it can keep remembering. Use Quit from the tray icon to stop it completely."
        />
      </Group>
      {info.platformLimitation && (
        <p className="rounded-lg border bg-muted/40 px-4 py-3 text-sm text-muted-foreground">{info.platformLimitation}</p>
      )}
    </Section>
  );
}

const RETENTION_OPTIONS = [null, 365, 180, 30, 7].map((v) => ({ value: v, label: retentionLabel(v) }));
const IDLE_OPTIONS = [60, 300, 600, 900, 1800].map((v) => ({ value: v, label: `${v / 60} minute${v === 60 ? "" : "s"}` }));
const POLL_OPTIONS = [1, 2, 5, 10].map((v) => ({ value: v, label: `Every ${v} second${v === 1 ? "" : "s"}` }));

export function MemorySection() {
  const [s, set] = useSetting();
  const [advanced, setAdvanced] = useState(false);
  return (
    <Section title="Memory" description="What Recall remembers and for how long.">
      <Group title="What to remember">
        <ToggleRow
          label="Application activity"
          description="Which app is in front, and when you switch."
          checked={s.appActivityEnabled}
          onChange={(v) => set({ appActivityEnabled: v })}
        />
        <ToggleRow
          label="Window titles"
          description="Document, page and conversation names shown in window title bars. Turn off to remember only app names."
          checked={s.windowTitlesEnabled}
          onChange={(v) => set({ windowTitlesEnabled: v })}
          disabled={!s.appActivityEnabled}
        />
        <ToggleRow
          label="Browser activity"
          description="Page title, URL, browser, and visit time from normal browsing history. Private browsing is never imported."
          checked={s.browserActivityEnabled}
          onChange={(v) => set({ browserActivityEnabled: v })}
        />
      </Group>
      {s.browserActivityEnabled && <BrowserChoices />}
      <Group title="Retention">
        <Row
          label="Keep memories for"
          description="Older memories are deleted automatically."
          control={
            <ChoiceSelect
              label="Keep memories for"
              value={s.retentionDays}
              options={RETENTION_OPTIONS}
              onChange={(v) => set({ retentionDays: v })}
            />
          }
        />
      </Group>
      <div>
        <Button variant="ghost" size="sm" onClick={() => setAdvanced((a) => !a)} aria-expanded={advanced}>
          {advanced ? "Hide advanced settings" : "Show advanced settings"}
        </Button>
        {advanced && (
          <div className="mt-2">
            <Group>
              <Row
                label="Consider me away after"
                description="Recording stops counting time when there's no keyboard or mouse input."
                control={
                  <ChoiceSelect
                    label="Idle threshold"
                    value={s.idleThresholdSecs}
                    options={IDLE_OPTIONS}
                    onChange={(v) => set({ idleThresholdSecs: v })}
                  />
                }
              />
              <Row
                label="Check active window"
                description="More frequent checks catch short switches but use slightly more power."
                control={
                  <ChoiceSelect
                    label="Activity check interval"
                    value={s.pollIntervalSecs}
                    options={POLL_OPTIONS}
                    onChange={(v) => set({ pollIntervalSecs: v })}
                  />
                }
              />
            </Group>
          </div>
        )}
      </div>
    </Section>
  );
}

function BrowserChoices() {
  const [s, set] = useSetting();
  const statuses = useAsync(() => api.browserStatuses(), []);
  const byId = new Map(statuses.data?.map((status) => [status.id, status]));
  const choices = [
    {
      id: "chrome",
      label: "Google Chrome",
      checked: s.browserChromeEnabled,
      update: (value: boolean) => set({ browserChromeEnabled: value }),
    },
    {
      id: "edge",
      label: "Microsoft Edge",
      checked: s.browserEdgeEnabled,
      update: (value: boolean) => set({ browserEdgeEnabled: value }),
    },
    {
      id: "firefox",
      label: "Firefox",
      checked: s.browserFirefoxEnabled,
      update: (value: boolean) => set({ browserFirefoxEnabled: value }),
    },
    {
      id: "safari",
      label: "Safari",
      checked: s.browserSafariEnabled,
      update: (value: boolean) => set({ browserSafariEnabled: value }),
    },
  ] as const;

  return (
    <Group title="Browsers">
      {choices.map((choice) => {
        const status = byId.get(choice.id);
        const unavailable = status?.state === "unsupported";
        const state = !status
          ? "Checking…"
          : status.state === "ready"
            ? `${status.profileCount} ${status.profileCount === 1 ? "profile" : "profiles"} found`
            : status.state === "not_installed"
              ? "Not installed"
              : status.state === "unsupported"
                ? "Not available on this platform"
                : status.message ?? "Temporarily unavailable";
        return (
          <ToggleRow
            key={choice.id}
            label={choice.label}
            description={state}
            checked={choice.checked && !unavailable}
            onChange={choice.update}
            disabled={unavailable}
          />
        );
      })}
      <Row
        label="Private browsing"
        description="Recall reads only the normal history database that browsers persist. Incognito, InPrivate, Firefox Private Browsing, and Safari Private Browsing do not write visits there, so Recall cannot import them. Recall never attempts to bypass that boundary."
      />
    </Group>
  );
}

export function PrivacySection() {
  const [s, set] = useSetting();
  const { status, pause, resume } = useRecall();
  const [dialog, setDialog] = useState<ExclusionsDialogConfig | null>(null);
  const paused = status.state === "paused";

  return (
    <Section
      title="Privacy"
      description="Everything Recall records stays on this computer. Nothing is uploaded, and there's no account."
    >
      <Group title="Recording">
        <ToggleRow
          label="Memory recording"
          checked={s.recordingEnabled}
          onChange={(v) => set({ recordingEnabled: v })}
        />
        <Row
          label="Pause recording"
          description={paused ? describePause(status.pausedUntil) : "Temporarily stop recording everything."}
          control={
            paused ? (
              <Button size="sm" onClick={resume}>
                Resume
              </Button>
            ) : (
              <ChoiceSelect<string>
                label="Pause recording"
                value="none"
                options={[
                  { value: "none", label: "Pause…" },
                  ...PAUSE_OPTIONS.map((o) => ({ value: String(o.minutes ?? "forever"), label: o.label })),
                ]}
                onChange={(v) => v !== "none" && pause(v === "forever" ? undefined : Number(v))}
              />
            )
          }
        />
      </Group>

      <Group title="What can be recorded">
        <ToggleRow
          label="Application activity"
          checked={s.appActivityEnabled}
          onChange={(v) => set({ appActivityEnabled: v })}
        />
        <ToggleRow
          label="Browser activity"
          description="Page addresses and titles from normal browser history. Incognito and private windows are never imported."
          checked={s.browserActivityEnabled}
          onChange={(v) => set({ browserActivityEnabled: v })}
        />
        <ToggleRow
          label={<>Screenshots<SoonBadge /></>}
          description="Periodic screenshots stored only on this computer, with their own retention period."
          checked={false}
          onChange={() => {}}
          disabled
        />
      </Group>
      {s.browserActivityEnabled && <BrowserChoices />}

      <Group title="Never record">
        <Row
          label="Sensitive apps"
          description="Password managers and private messaging apps are excluded by default."
          control={
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                setDialog({
                  title: "Sensitive apps",
                  description:
                    "Recall never records these apps, or windows whose title contains these words. Adding one also deletes matching memories.",
                  kinds: ["app", "title"],
                })
              }
            >
              Configure
            </Button>
          }
        />
        <Row
          label="Excluded websites"
          description="Sites like banking, medical or personal accounts."
          control={
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                setDialog({
                  title: "Excluded websites",
                  description:
                    "Recall never stores the URL or page title for these sites, including their subdomains. Adding one also deletes matching browser memories.",
                  kinds: ["website"],
                })
              }
            >
              Configure
            </Button>
          }
        />
        <Row
          label="Excluded folders"
          description="Files inside these folders are never recorded."
          control={
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                setDialog({
                  title: "Excluded folders",
                  description: "Recall never records windows showing files from these folders.",
                  kinds: ["folder"],
                  note: "Folders are currently matched when their path appears in a window title (common in editors and file managers).",
                })
              }
            >
              Configure
            </Button>
          }
        />
      </Group>

      <Group title="Your data">
        <Row label="Export data" description="Save a copy of everything Recall has stored as a JSON file." control={<ExportButton />} />
        <Row
          label="Delete all memory"
          description="Permanently erase every memory on this computer."
          control={<DeleteAllButton />}
        />
      </Group>

      <ExclusionsDialog config={dialog} onClose={() => setDialog(null)} />
    </Section>
  );
}

export function PermissionsSection() {
  const { info } = useRecall();
  const [version, setVersion] = useState(0);
  const perms = useAsync(() => api.permissions(), [version]);

  async function request(id: string) {
    try {
      await api.requestPermission(id);
    } catch (e) {
      notifyError(e, "Couldn't request permission");
    }
    setVersion((v) => v + 1);
  }

  return (
    <Section title="Permissions" description="System permissions affect what Recall is able to remember.">
      {perms.status === "error" ? (
        <p className="text-sm text-destructive">{perms.error}</p>
      ) : !perms.data ? null : perms.data.length === 0 ? (
        <Group>
          <Row
            label="No extra permissions needed"
            description={
              info.platform === "windows"
                ? "Windows lets Recall see which app is in front without any special permission."
                : "This system doesn't require any additional permissions for activity tracking."
            }
          />
        </Group>
      ) : (
        <Group>
          {perms.data.map((p) => (
            <Row
              key={p.id}
              label={
                <span className="inline-flex items-center gap-2">
                  {p.name}
                  <span
                    className={cn(
                      "rounded-md px-1.5 py-0.5 text-[11px] font-medium",
                      p.granted ? "bg-recording/15 text-recording" : "bg-muted text-muted-foreground",
                    )}
                  >
                    {p.granted === null ? "Unknown" : p.granted ? "Allowed" : "Not allowed"}
                  </span>
                </span>
              }
              description={p.reason}
              control={
                !p.granted && (
                  <Button size="sm" variant="outline" onClick={() => request(p.id)}>
                    <ExternalLink /> Allow
                  </Button>
                )
              }
            />
          ))}
        </Group>
      )}
      <Button variant="ghost" size="sm" onClick={() => setVersion((v) => v + 1)}>
        <RotateCw /> Check again
      </Button>
    </Section>
  );
}

const THEMES: { value: Theme; label: string }[] = [
  { value: "system", label: "Match system" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

export function AppearanceSection() {
  const [s, set] = useSetting();
  return (
    <Section title="Appearance">
      <div role="radiogroup" aria-label="Theme" className="grid gap-3 sm:grid-cols-3">
        {THEMES.map((t) => (
          <button
            key={t.value}
            type="button"
            role="radio"
            aria-checked={s.theme === t.value}
            onClick={() => set({ theme: t.value })}
            className={cn(
              "rounded-xl border bg-card p-3 text-left text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring",
              s.theme === t.value ? "border-brand ring-1 ring-brand" : "hover:bg-muted/60",
            )}
          >
            <span
              aria-hidden
              className={cn(
                "mb-3 block h-14 rounded-lg border",
                t.value === "light" && "bg-[oklch(0.98_0_0)]",
                t.value === "dark" && "bg-[oklch(0.2_0.01_265)]",
                t.value === "system" && "bg-[linear-gradient(135deg,oklch(0.98_0_0)_50%,oklch(0.2_0.01_265)_50%)]",
              )}
            />
            {t.label}
          </button>
        ))}
      </div>
      <p className="text-sm text-muted-foreground">
        Recall follows your system's text size and reduced-motion preferences. Use {navigator.platform.includes("Mac") ? "⌘" : "Ctrl"} + / − to zoom.
      </p>
    </Section>
  );
}

export function PlannedSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <Section title={title}>
      <div className="rounded-xl border bg-card p-5 text-sm leading-relaxed">
        <p className="mb-2 inline-block rounded-md bg-muted px-2 py-0.5 text-xs font-medium text-muted-foreground">
          Not available in this version
        </p>
        {children}
      </div>
    </Section>
  );
}

export function UpdatesSection() {
  const { info } = useRecall();
  return (
    <Section title="Updates">
      <Group>
        <Row label="Installed version" control={<span className="text-sm tabular-nums">{info.version}</span>} />
        <Row
          label="Automatic updates"
          muted
          description="Signed, verified automatic updates are being built for an upcoming release. Until then, install new versions manually."
        />
      </Group>
    </Section>
  );
}

export function DataSection() {
  const { info, memoryVersion } = useRecall();
  const stats = useAsync(() => api.stats(), [memoryVersion]);

  async function openFolder(which: "data" | "logs") {
    try {
      await (which === "data" ? api.openDataFolder() : api.openLogFolder());
    } catch (e) {
      notifyError(e, "Couldn't open the folder");
    }
  }

  return (
    <Section title="Data" description="Your memory database lives only on this computer.">
      <Group title="Storage">
        <Row
          label="Memories stored"
          control={<span className="text-sm tabular-nums">{stats.data?.totalEvents.toLocaleString() ?? "—"}</span>}
        />
        <Row
          label="Oldest memory"
          control={
            <span className="text-sm">{stats.data?.oldestAt ? formatDayLabel(stats.data.oldestAt) : "—"}</span>
          }
        />
        <Row
          label="Database size"
          control={<span className="text-sm tabular-nums">{stats.data ? formatBytes(stats.data.databaseBytes) : "—"}</span>}
        />
        <Row
          label="Location"
          description={<span className="break-all font-mono text-xs">{info.dataDir}</span>}
          control={
            <Button variant="outline" size="sm" onClick={() => openFolder("data")}>
              <FolderOpen /> Show
            </Button>
          }
        />
      </Group>
      <Group title="Manage">
        <Row label="Export data" description="A JSON file with your memories, settings and exclusions." control={<ExportButton />} />
        <Row
          label="Delete all memory"
          description="Permanently erase every memory. Settings and exclusions are kept."
          control={<DeleteAllButton />}
        />
      </Group>
    </Section>
  );
}

export function AboutSection() {
  const { info } = useRecall();
  const platformName = { windows: "Windows", macos: "macOS", linux: "Linux (experimental)" }[info.platform];
  return (
    <Section title="About Recall">
      <Group>
        <Row label="Version" control={<span className="text-sm tabular-nums">{info.version}</span>} />
        <Row label="Platform" control={<span className="text-sm">{platformName}</span>} />
        <Row label="Database schema" control={<span className="text-sm tabular-nums">v{info.schemaVersion}</span>} />
        <Row
          label="Diagnostic logs"
          description="Local logs help diagnose problems. They never contain window titles, websites, or other memory contents, and are never uploaded."
          control={
            <Button
              variant="outline"
              size="sm"
              onClick={() => api.openLogFolder().catch((e) => notifyError(e, "Couldn't open the folder"))}
            >
              <FolderOpen /> Show logs
            </Button>
          }
        />
      </Group>
      <p className="text-sm text-muted-foreground">
        Recall is free. It has no ads, no account, no telemetry, and never sells or uploads your data.
      </p>
    </Section>
  );
}
