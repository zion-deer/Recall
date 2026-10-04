import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { api, hasBackend } from "@/lib/api";
import { cn } from "@/lib/utils";

const cache = new Map<string, string | null>();

function hue(name: string): number {
  let h = 0;
  for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) % 360;
  return h;
}

function keyFor(name: string, appId: string | null | undefined): string {
  return `${name.toLowerCase()}|${appId ?? ""}`;
}

/** The application's own icon when the system has one, otherwise a monogram. */
export function AppAvatar({
  name,
  appId,
  className,
}: {
  name: string | null;
  appId?: string | null;
  className?: string;
}) {
  const label = (name ?? "?").trim() || "?";
  const key = keyFor(label, appId);
  const [src, setSrc] = useState<string | null>(() => cache.get(key) ?? null);

  useEffect(() => {
    if (!hasBackend()) return;
    const cached = cache.get(key);
    if (cached !== undefined) {
      setSrc(cached);
      return;
    }
    let cancelled = false;
    api
      .appIcon(label, appId ?? null)
      .then((url) => {
        const next = url && (url.startsWith("data:") || url.startsWith("asset:")) ? url : url ? convertFileSrc(url) : null;
        cache.set(key, next);
        if (!cancelled) setSrc(next);
      })
      .catch(() => {
        cache.set(key, null);
      });
    return () => {
      cancelled = true;
    };
  }, [key, label, appId]);

  if (src) {
    return (
      <img
        src={src}
        alt=""
        className={cn("inline-block size-8 shrink-0 rounded-lg object-contain", className)}
      />
    );
  }

  const h = hue(label.toLowerCase());
  return (
    <span
      aria-hidden
      className={cn(
        "inline-flex size-8 shrink-0 items-center justify-center rounded-lg text-[13px] font-semibold",
        className,
      )}
      style={{
        background: `oklch(0.93 0.04 ${h} / 0.9)`,
        color: `oklch(0.42 0.1 ${h})`,
      }}
    >
      {label.charAt(0).toUpperCase()}
    </span>
  );
}
