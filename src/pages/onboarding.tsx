import { ArrowLeft, ArrowRight, EyeOff, HardDrive, Pause, Trash2 } from "lucide-react";
import { useState } from "react";
import { Logo } from "@/components/app-shell";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useAsync } from "@/hooks/use-async";
import { notifyError, useRecall } from "@/hooks/use-recall";
import { api } from "@/lib/api";
import { cn } from "@/lib/utils";

const STEPS = ["welcome", "privacy", "permissions", "start"] as const;
type Step = (typeof STEPS)[number];

export function Onboarding() {
  const { settings, updateSettings } = useRecall();
  const [step, setStep] = useState<Step>("welcome");
  const [titles, setTitles] = useState(settings.windowTitlesEnabled);
  const index = STEPS.indexOf(step);

  const next = () => setStep(STEPS[Math.min(index + 1, STEPS.length - 1)]);
  const back = () => setStep(STEPS[Math.max(index - 1, 0)]);

  async function finish(startRecording: boolean) {
    await updateSettings({
      onboardingCompleted: true,
      recordingEnabled: startRecording,
      windowTitlesEnabled: titles,
      pausedUntil: null,
    });
  }

  return (
    <div className="flex min-h-full items-center justify-center bg-background px-5 py-10">
      <div className="w-full max-w-lg">
        <div className="mb-8 flex items-center justify-between">
          <Logo className="size-9" />
          <ol className="flex gap-1.5" aria-label={`Step ${index + 1} of ${STEPS.length}`}>
            {STEPS.map((s, i) => (
              <li
                key={s}
                className={cn("h-1.5 w-6 rounded-full bg-muted transition-colors", i <= index && "bg-primary")}
              />
            ))}
          </ol>
        </div>

        <div key={step} className="animate-in fade-in-0 slide-in-from-bottom-1 duration-300">
          {step === "welcome" && (
            <>
              <h1 className="text-3xl font-semibold tracking-tight">Welcome to Recall</h1>
              <p className="mt-4 text-lg leading-relaxed text-muted-foreground">
                Recall remembers what you do on your computer so you can find it later.
              </p>
              <p className="mt-3 text-lg leading-relaxed text-muted-foreground">
                Your data stays on this computer.
              </p>
              <Footer>
                <span />
                <Button size="lg" onClick={next} autoFocus>
                  Continue <ArrowRight />
                </Button>
              </Footer>
            </>
          )}

          {step === "privacy" && (
            <>
              <h1 className="text-2xl font-semibold tracking-tight">You stay in control</h1>
              <ul className="mt-6 space-y-4">
                <Point icon={HardDrive} title="Stored only on this computer">
                  No account, no cloud, no tracking. Recall works offline.
                </Point>
                <Point icon={EyeOff} title="Sensitive apps are skipped">
                  Password managers, private messaging and private browsing windows are never recorded. You
                  can add your own exclusions.
                </Point>
                <Point icon={Pause} title="Pause any time">
                  One click in the sidebar or the system tray stops recording.
                </Point>
                <Point icon={Trash2} title="Delete anything">
                  Remove a single memory, an hour, a day, or everything.
                </Point>
              </ul>
              <div className="mt-6 flex items-start gap-4 rounded-xl border bg-card p-4">
                <div className="flex-1">
                  <label htmlFor="ob-titles" className="text-sm font-medium">
                    Remember window titles
                  </label>
                  <p className="mt-0.5 text-[13px] text-muted-foreground">
                    Titles like “Q3 report.docx” or “Rust documentation” make your memory far more useful. Turn
                    off to remember only which apps you used.
                  </p>
                </div>
                <Switch id="ob-titles" checked={titles} onCheckedChange={setTitles} />
              </div>
              <Footer>
                <Button variant="ghost" onClick={back}>
                  <ArrowLeft /> Back
                </Button>
                <Button size="lg" onClick={next}>
                  Continue <ArrowRight />
                </Button>
              </Footer>
            </>
          )}

          {step === "permissions" && <PermissionsStep onBack={back} onNext={next} />}

          {step === "start" && (
            <>
              <h1 className="text-2xl font-semibold tracking-tight">Ready to start remembering</h1>
              <p className="mt-4 leading-relaxed text-muted-foreground">
                Recall will quietly note which apps and windows you use. Closing the window keeps it running in
                your system tray.
              </p>
              <p className="mt-3 leading-relaxed text-muted-foreground">
                Screenshots and local AI answers are optional and stay off until you turn them on. Search and Ask
                use only what is already stored on this computer.
              </p>
              <Footer>
                <Button variant="ghost" onClick={() => finish(false)}>
                  Not now
                </Button>
                <Button size="lg" onClick={() => finish(true)} autoFocus>
                  Start recording
                </Button>
              </Footer>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

function PermissionsStep({ onBack, onNext }: { onBack: () => void; onNext: () => void }) {
  const [version, setVersion] = useState(0);
  const perms = useAsync(() => api.permissions(), [version]);

  async function enable(id: string) {
    try {
      await api.requestPermission(id);
    } catch (e) {
      notifyError(e, "Couldn't request permission");
    }
    setVersion((v) => v + 1);
  }

  const list = perms.data ?? [];

  return (
    <>
      <h1 className="text-2xl font-semibold tracking-tight">Permissions</h1>
      {perms.status === "loading" && !perms.data ? (
        <p className="mt-4 text-muted-foreground">Checking…</p>
      ) : list.length === 0 ? (
        <p className="mt-4 leading-relaxed text-muted-foreground">
          Good news — Recall doesn't need any special permissions on this computer.
        </p>
      ) : (
        <div className="mt-6 space-y-4">
          {list.map((p) => (
            <div key={p.id} className="rounded-xl border bg-card p-5">
              <h2 className="font-medium">{p.name}</h2>
              <p className="mt-1 text-sm leading-relaxed text-muted-foreground">
                <span className="font-medium text-foreground">Why: </span>
                {p.reason}
              </p>
              <div className="mt-4 flex items-center gap-2">
                {p.granted ? (
                  <span className="text-sm font-medium text-recording">Enabled</span>
                ) : (
                  <>
                    <Button size="sm" onClick={() => enable(p.id)}>
                      Enable
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => setVersion((v) => v + 1)}>
                      I've enabled it
                    </Button>
                  </>
                )}
              </div>
            </div>
          ))}
          <p className="text-xs text-muted-foreground">Every permission is optional. You can change this later in Settings.</p>
        </div>
      )}
      <Footer>
        <Button variant="ghost" onClick={onBack}>
          <ArrowLeft /> Back
        </Button>
        <Button size="lg" onClick={onNext}>
          {list.some((p) => !p.granted) ? "Skip for now" : "Continue"} <ArrowRight />
        </Button>
      </Footer>
    </>
  );
}

function Point({ icon: Icon, title, children }: { icon: typeof Pause; title: string; children: React.ReactNode }) {
  return (
    <li className="flex gap-3.5">
      <span className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg bg-brand-soft text-brand">
        <Icon className="size-4" aria-hidden />
      </span>
      <div>
        <p className="text-sm font-medium">{title}</p>
        <p className="mt-0.5 text-sm text-muted-foreground">{children}</p>
      </div>
    </li>
  );
}

function Footer({ children }: { children: React.ReactNode }) {
  return <div className="mt-10 flex items-center justify-between">{children}</div>;
}
