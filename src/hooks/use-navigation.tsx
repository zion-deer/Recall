import { createContext, useContext } from "react";

export type Page = "home" | "memory" | "search" | "agent" | "settings";

export type SettingsSection =
  | "general"
  | "memory"
  | "privacy"
  | "ai"
  | "agent"
  | "permissions"
  | "appearance"
  | "updates"
  | "data"
  | "about";

export interface NavigationValue {
  page: Page;
  settingsSection: SettingsSection;
  navigate: (page: Page, section?: SettingsSection) => void;
}

export const NavigationContext = createContext<NavigationValue | null>(null);

export function useNavigation(): NavigationValue {
  const ctx = useContext(NavigationContext);
  if (!ctx) throw new Error("useNavigation must be used inside NavigationContext");
  return ctx;
}
