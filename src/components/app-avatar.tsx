import { cn } from "@/lib/utils";

function hue(name: string): number {
  let h = 0;
  for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) % 360;
  return h;
}

/** A stable, colored monogram for an application. */
export function AppAvatar({ name, className }: { name: string | null; className?: string }) {
  const label = (name ?? "?").trim() || "?";
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
