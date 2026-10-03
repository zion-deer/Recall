import { PageHeader } from "@/components/app-shell";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useNavigation, type SettingsSection } from "@/hooks/use-navigation";
import { cn } from "@/lib/utils";
import {
  AboutSection,
  AppearanceSection,
  DataSection,
  GeneralSection,
  MemorySection,
  PermissionsSection,
  PlannedSection,
  PrivacySection,
  UpdatesSection,
} from "./sections";

const SECTIONS: { id: SettingsSection; label: string }[] = [
  { id: "general", label: "General" },
  { id: "memory", label: "Memory" },
  { id: "privacy", label: "Privacy" },
  { id: "ai", label: "AI" },
  { id: "agent", label: "Agent" },
  { id: "permissions", label: "Permissions" },
  { id: "appearance", label: "Appearance" },
  { id: "updates", label: "Updates" },
  { id: "data", label: "Data" },
  { id: "about", label: "About" },
];

function SectionBody({ id }: { id: SettingsSection }) {
  switch (id) {
    case "general":
      return <GeneralSection />;
    case "memory":
      return <MemorySection />;
    case "privacy":
      return <PrivacySection />;
    case "ai":
      return (
        <PlannedSection title="AI">
          <p>
            AI answers about your memory are planned for a later release. They'll run on your computer by
            default using a local model, and only the few memories relevant to your question will be shown to
            the model. Cloud AI, if offered, will be optional and clearly labeled.
          </p>
        </PlannedSection>
      );
    case "agent":
      return (
        <PlannedSection title="Agent">
          <p>
            The agent will be able to take a small set of safe actions, like reopening files and websites, and
            will always ask before changing anything. It will be off by default.
          </p>
        </PlannedSection>
      );
    case "permissions":
      return <PermissionsSection />;
    case "appearance":
      return <AppearanceSection />;
    case "updates":
      return <UpdatesSection />;
    case "data":
      return <DataSection />;
    case "about":
      return <AboutSection />;
  }
}

export function SettingsPage() {
  const { settingsSection, navigate } = useNavigation();
  const items = SECTIONS.map((s) => ({ value: s.id, label: s.label }));

  return (
    <div className="mx-auto w-full max-w-5xl px-5 py-8 md:px-10 md:py-10">
      <PageHeader title="Settings" />
      <div className="flex flex-col gap-8 md:flex-row">
        <nav aria-label="Settings sections" className="hidden w-44 shrink-0 md:block">
          <ul className="sticky top-4 space-y-0.5">
            {SECTIONS.map((s) => (
              <li key={s.id}>
                <button
                  type="button"
                  onClick={() => navigate("settings", s.id)}
                  aria-current={settingsSection === s.id ? "page" : undefined}
                  className={cn(
                    "w-full rounded-lg px-3 py-1.5 text-left text-sm text-muted-foreground outline-none transition-colors hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring",
                    settingsSection === s.id && "bg-muted font-medium text-foreground",
                  )}
                >
                  {s.label}
                </button>
              </li>
            ))}
          </ul>
        </nav>
        <div className="md:hidden">
          <Select
            items={items}
            value={settingsSection}
            onValueChange={(v) => v && navigate("settings", v as SettingsSection)}
          >
            <SelectTrigger aria-label="Settings section" className="w-full">
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
        </div>
        <div className="min-w-0 max-w-2xl flex-1">
          <SectionBody id={settingsSection} />
        </div>
      </div>
    </div>
  );
}
