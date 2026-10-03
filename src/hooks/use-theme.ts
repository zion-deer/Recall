import { useEffect, useState } from "react";
import type { Theme } from "@/lib/api";

const query = () => window.matchMedia("(prefers-color-scheme: dark)");

/** Applies the theme to <html> and returns the resolved light/dark value. */
export function useAppliedTheme(theme: Theme): "light" | "dark" {
  const [systemDark, setSystemDark] = useState(() => query().matches);

  useEffect(() => {
    const mq = query();
    const onChange = () => setSystemDark(mq.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  const resolved = theme === "system" ? (systemDark ? "dark" : "light") : theme;

  useEffect(() => {
    document.documentElement.classList.toggle("dark", resolved === "dark");
    document.documentElement.style.colorScheme = resolved;
  }, [resolved]);

  return resolved;
}
