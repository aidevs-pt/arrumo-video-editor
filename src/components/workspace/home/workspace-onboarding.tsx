"use client";

import { PluginIcon, usePluginText } from "@/lib/plugin-text";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, ArrowUpRight, Check, Info } from "lucide-react";
import { useEffect, useId, useRef, useState, type FormEvent, type ReactNode } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Skeleton } from "@/components/ui/skeleton";
import { OnboardingKeys } from "./onboarding-keys";
import type { PluginSettingsManifest } from "@/components/workspace/settings/plugin-preferences";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

type Agent = { id: string; label: string };
type Plugin = { bundled: boolean; manifest: PluginSettingsManifest & { description: string } };
type SetupData = { agents: Agent[]; plugins: Plugin[]; effective: string | null };
type Step = 0 | 1 | 2;

export function WorkspaceOnboarding({ desktop, loading, onCreate }: {
  desktop: boolean;
  loading: boolean;
  onCreate: (name: string) => Promise<void>;
}) {
  const pluginText = usePluginText();
  const { locale, setLocale, messages: { onboarding: copy, workspaces, agentSettings } } = useLocale();
  const [step, setStep] = useState<Step>(0);
  const [setup, setSetup] = useState<SetupData>();
  const [selected, setSelected] = useState("");
  const [name, setName] = useState("");
  const [pending, setPending] = useState(false);
  const [setupError, setSetupError] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const content = useRef<HTMLElement>(null);
  const previousStep = useRef(step);
  const agent = setup?.agents.find(({ id }) => id === selected);
  const steps = [copy.agent, copy.services, copy.workspace];
  const services = setup?.plugins.filter(({ bundled, manifest }) =>
    bundled && manifest.settings?.some(({ kind }) => kind === "secret"),
  ) ?? [];

  useEffect(() => {
    if (previousStep.current !== step) content.current?.querySelector("h1")?.focus();
    previousStep.current = step;
  }, [step]);

  useEffect(() => {
    if (!desktop) return;
    let active = true;
    Promise.all([
      invoke<Agent[]>("list_agent_plugins"),
      invoke<Plugin[]>("list_plugin_catalog"),
      invoke<{ effective_provider: string | null }>("get_agent_settings", { workspaceId: null }),
    ]).then(
      ([agents, plugins, settings]) => {
        if (!active) return;
        setSetup({ agents, plugins, effective: settings.effective_provider });
        setSelected(settings.effective_provider ?? agents[0]?.id ?? "");
      },
      () => { if (active) setSetupError(true); },
    );
    return () => { active = false; };
  }, [attempt, desktop]);

  async function chooseProvider() {
    if (!setup || !agent || pending) return;
    setPending(true);
    setSaveError(false);
    try {
      if (agent.id !== setup.effective) {
        await invoke("set_agent_provider", { workspaceId: null, provider: agent.id });
        setSetup({ ...setup, effective: agent.id });
      }
      setStep(1);
    } catch {
      setSaveError(true);
    } finally {
      setPending(false);
    }
  }

  async function finish(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending || !name.trim()) return;
    setPending(true);
    setSaveError(false);
    try {
      await onCreate(name.trim());
    } catch {
      setSaveError(true);
    } finally {
      setPending(false);
    }
  }

  function back() {
    setSaveError(false);
    setStep(step === 2 ? 1 : 0);
  }

  return (
    <div className="grid min-h-svh bg-background md:grid-cols-[minmax(17rem,36%)_minmax(0,1fr)]">
      <aside className="relative isolate flex flex-col justify-between overflow-hidden bg-primary px-5 py-5 text-primary-foreground sm:px-9 md:sticky md:top-0 md:h-svh md:min-h-[32rem] md:px-[8%] md:py-10">
        <AzulejoTilework />
        <div className="flex items-center gap-2.5 font-heading text-2xl font-extrabold [font-stretch:135%]">
          <ArrumoFace />
          <span>Arrumo</span>
        </div>
        <div className="flex flex-col gap-6 md:gap-7">
          <p className="hidden max-w-[11ch] font-heading text-[clamp(2rem,3.5vw,3rem)] leading-none font-[850] tracking-[-.025em] [font-stretch:135%] md:block">{copy.tagline}</p>
          <ol className="mt-5 grid grid-cols-3 gap-2 text-sm md:mt-0 md:flex md:flex-col md:gap-3">
            {steps.map((label, index) => (
              <li key={label} aria-current={step === index ? "step" : undefined} className={cn("flex flex-col items-start gap-1 md:flex-row md:items-center md:gap-2.5", index > step ? "text-white/75" : "font-semibold")}>
                <span aria-hidden="true" className={cn("grid h-1.5 w-full shrink-0 place-items-center rounded-[4px] border border-white/55 md:size-5", index === step && "border-white bg-white", index < step && "border-attention bg-attention text-foreground")}>
                  {index < step && <Check className="hidden size-3.5 md:block" />}
                </span>
                <span>{label}{index === 0 && step > 0 && agent && <span className="ms-1 font-normal">{agent.label}</span>}</span>
              </li>
            ))}
          </ol>
          <div className="flex flex-wrap gap-x-3 gap-y-2 text-xs" aria-label={copy.language}>
            {Object.entries({ en: "English", es: "Español", pt: "Português" }).map(([code, label]) => (
              <button type="button" key={code} lang={code} aria-pressed={locale === code} className={cn("rounded-sm underline-offset-2 hover:underline focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-ring", locale === code ? "font-bold" : "underline")} onClick={() => setLocale(code)}>{label}</button>
            ))}
          </div>
        </div>
      </aside>
      <main ref={content} className="mx-auto flex w-full max-w-5xl flex-col px-5 py-7 sm:px-10 md:min-h-svh md:px-[8%] md:py-[6vh]">
        {step === 0 && <>
          <SetupHeading title={copy.agentTitle}>{copy.agentIntro}</SetupHeading>
          {setupError ? <Alert variant="destructive">
            <AlertDescription>{agentSettings.loadError}</AlertDescription>
            <Button variant="outline" onClick={() => { setSetupError(false); setAttempt((value) => value + 1); }}>{workspaces.retry}</Button>
          </Alert> : setup ? (
            <FieldSet>
              <FieldLegend className="sr-only">{agentSettings.providerChoice}</FieldLegend>
              <RadioGroup value={selected} onValueChange={setSelected} disabled={pending}
                aria-label={agentSettings.providerChoice} className="gap-3 min-[1120px]:grid-cols-2">
                {setup.agents.map((entry) => <AgentOption key={entry.id} agent={entry}
                  description={pluginText.description(entry.id, setup.plugins.find(({ manifest }) => manifest.id === entry.id)?.manifest.description ?? "")}
                  selected={selected === entry.id} />)}
              </RadioGroup>
              {!setup.agents.length && <Alert><Info /><AlertDescription>{copy.noAgents}</AlertDescription></Alert>}
            </FieldSet>
          ) : loading || desktop ? <Skeleton className="h-32" role="status" aria-label={agentSettings.loading} />
            : <Alert><Info /><AlertDescription>{workspaces.desktop}</AlertDescription></Alert>}
          {setup?.agents.length ? <Alert className="mt-5 border-transparent bg-accent">
            <Info aria-hidden="true" />
            <AlertDescription>{copy.connectionNote}</AlertDescription>
          </Alert> : null}
          {saveError && <SetupError>{agentSettings.saveError}</SetupError>}
          <footer className="mt-auto flex justify-between gap-4 pt-10">
            <p className="self-center text-sm text-muted-foreground">{copy.stepCount.replace("{step}", "1")}</p>
            <Button disabled={!agent || pending} onClick={() => void chooseProvider()}>{agent ? copy.useAgent.replace("{agent}", agent.label) : copy.next}</Button>
          </footer>
        </>}
        {step === 1 && <>
          <SetupHeading title={copy.servicesTitle.replace("{agent}", agent?.label ?? copy.agent)}>{copy.servicesIntro.replace("{agent}", agent?.label ?? copy.agent)}</SetupHeading>
          <OnboardingKeys services={services.map(({ manifest }) => manifest)} onBack={back} onDone={() => setStep(2)} />
        </>}
        {step === 2 && <form className="flex flex-1 flex-col" onSubmit={finish}>
          <SetupHeading title={copy.workspaceTitle}>{copy.workspaceIntro}</SetupHeading>
          <Field className="max-w-sm" data-disabled={pending}>
            <FieldLabel htmlFor="workspace-name">{workspaces.name}</FieldLabel>
            <Input id="workspace-name" autoComplete="organization" value={name} onChange={(event) => setName(event.target.value)} disabled={pending} required />
            <FieldDescription>{copy.workspaceHint}</FieldDescription>
          </Field>
          {saveError && <SetupError>{workspaces.createError}</SetupError>}
          <footer className="mt-auto flex justify-between gap-4 pt-10">
            <Back onClick={back} disabled={pending}>{copy.back}</Back>
            <Button type="submit" disabled={pending || !name.trim()}>{pending ? workspaces.creating : workspaces.create}</Button>
          </footer>
        </form>}
      </main>
    </div>
  );
}

function SetupHeading({ title, children }: { title: string; children: ReactNode }) {
  return <header className="mb-6 sm:mb-8">
    <h1 tabIndex={-1} className="max-w-[31ch] text-[clamp(1.6rem,2.9vw,2.125rem)] leading-[1.08] font-[850] tracking-[-.03em] outline-none [font-stretch:135%]">{title}</h1>
    <p className="mt-3 max-w-[64ch] text-muted-foreground">{children}</p>
  </header>;
}

function AzulejoTilework() {
  const pattern = useId();
  return (
    <svg aria-hidden="true" className="pointer-events-none absolute inset-x-0 top-0 -z-10 h-1/2 w-full">
      <defs>
        <pattern id={pattern} width={84} height={84} patternUnits="userSpaceOnUse">
          <g fill="none" stroke="white" strokeWidth={5} opacity={0.23}>
            <circle cx={0} cy={0} r={24} />
            <circle cx={84} cy={0} r={24} />
            <circle cx={0} cy={84} r={24} />
            <circle cx={84} cy={84} r={24} />
          </g>
          <g fill="white" opacity={0.23}>
            <rect x={38} y={20} width={8} height={15} rx={4} />
            <rect x={38} y={49} width={8} height={15} rx={4} />
            <rect x={20} y={38} width={15} height={8} rx={4} />
            <rect x={49} y={38} width={15} height={8} rx={4} />
          </g>
          <circle cx={42} cy={42} r={4.5} fill="var(--attention)" />
        </pattern>
      </defs>
      <rect width="100%" height="100%" fill={`url(#${pattern})`} />
    </svg>
  );
}

function ArrumoFace() {
  return (
    <span className="size-10 shrink-0 overflow-hidden rounded-[10px] bg-attention">
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img src="/arrumo-icon.svg" width={40} height={40} alt="" className="size-full mix-blend-multiply invert" />
    </span>
  );
}

function Back({ children, onClick, disabled }: { children: ReactNode; onClick: () => void; disabled?: boolean }) {
  return <Button type="button" variant="ghost" disabled={disabled} onClick={onClick}><ArrowLeft data-icon="inline-start" />{children}</Button>;
}

function SetupError({ children }: { children: ReactNode }) {
  return <Alert variant="destructive" className="mt-4" role="alert"><AlertDescription>{children}</AlertDescription></Alert>;
}

function AgentOption({ agent, description, selected }: {
  agent: Agent;
  description?: string;
  selected: boolean;
}) {
  const { messages: { agentSettings: copy } } = useLocale();
  const id = useId();
  const [detected, setDetected] = useState<boolean>();
  const [failed, setFailed] = useState(false);
  const [guideError, setGuideError] = useState(false);
  useEffect(() => {
    let active = true;
    invoke<boolean>("agent_cli_detected", { provider: agent.id }).then(
      (found) => { if (active) setDetected(found); },
      () => { if (active) setFailed(true); },
    );
    return () => { active = false; };
  }, [agent.id]);

  return <div className={cn("flex flex-col rounded-xl border bg-card", selected && "border-primary ring-1 ring-inset ring-primary")}>
    <label htmlFor={id} className="flex flex-1 cursor-pointer flex-col gap-2 p-4 sm:p-5">
      <span className="flex items-center justify-between gap-3">
        <span className="flex items-center gap-3">
          <PluginIcon pluginId={agent.id} className="size-9" />
          <span className="font-heading text-xl font-bold [font-stretch:112%]">{agent.label}</span>
        </span>
        <RadioGroupItem value={agent.id} id={id} />
      </span>
      {description && <span className="text-sm text-muted-foreground">{description}</span>}
    </label>
    <div className="flex flex-wrap items-center gap-x-2 gap-y-1 px-4 pb-4 text-sm sm:px-5 sm:pb-5">
      {detected && <Check aria-hidden="true" className="size-4 text-approval" />}
      <span role="status">{failed ? copy.checkFailed : detected === undefined ? copy.checking : detected ? copy.detected : copy.notDetected}</span>
      {detected === false && <button type="button" className="inline-flex items-center gap-1 font-semibold text-primary underline underline-offset-2" onClick={() => {
        setGuideError(false);
        void invoke("open_agent_setup_guide", { provider: agent.id }).catch(() => setGuideError(true));
      }}>{copy.guide}<ArrowUpRight aria-hidden="true" className="size-3.5" /></button>}
      {guideError && <p role="alert" className="text-destructive">{copy.guideError}</p>}
    </div>
  </div>;
}
