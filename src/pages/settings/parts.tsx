import { useId, type ReactNode } from "react";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";

export function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold tracking-tight">{title}</h2>
        {description && <p className="mt-1 text-sm text-muted-foreground">{description}</p>}
      </div>
      {children}
    </section>
  );
}

export function Group({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <div className="space-y-2">
      {title && <h3 className="px-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">{title}</h3>}
      <div className="divide-y rounded-xl border bg-card">{children}</div>
    </div>
  );
}

export function Row({
  label,
  description,
  control,
  htmlFor,
  muted,
}: {
  label: ReactNode;
  description?: ReactNode;
  control?: ReactNode;
  htmlFor?: string;
  muted?: boolean;
}) {
  return (
    <div className={cn("flex flex-wrap items-center gap-x-6 gap-y-2 px-4 py-3.5", muted && "opacity-70")}>
      <div className="min-w-0 flex-1 basis-60">
        <label htmlFor={htmlFor} className="text-sm font-medium">
          {label}
        </label>
        {description && <p className="mt-0.5 text-[13px] leading-snug text-muted-foreground">{description}</p>}
      </div>
      {control && <div className="shrink-0">{control}</div>}
    </div>
  );
}

export function ToggleRow({
  label,
  description,
  checked,
  onChange,
  disabled,
}: {
  label: ReactNode;
  description?: ReactNode;
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  const id = useId();
  return (
    <Row
      htmlFor={id}
      label={label}
      description={description}
      muted={disabled}
      control={<Switch id={id} checked={checked} onCheckedChange={(v) => onChange(v)} disabled={disabled} />}
    />
  );
}

export function SoonBadge() {
  return (
    <span className="ml-2 rounded-md bg-muted px-1.5 py-0.5 align-middle text-[10px] font-medium text-muted-foreground">
      Coming later
    </span>
  );
}
