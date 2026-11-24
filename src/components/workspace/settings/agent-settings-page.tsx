"use client";

import { PluginIcon } from "@/lib/plugin-text";
import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, Bot, DatabaseBackup, Gauge, KeyRound, Package, Settings2 } from "lucide-react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { useWorkspaceCatalog, workspaceHref } from "@/components/workspace/home/workspace-router";
import { WorkspaceShell } from "@/components/workspace/layout";
import { LayerGauge } from "@/components/workspace/layer-gauge";
import { useAppearance } from "@/lib/appearance";
import { type AgentCliLocation, agentCliLocation, chooseAgentProgram, resetAgentProgram } from "@/lib/agent-cli";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Switch } from "@/components/ui/switch";
import { PluginCatalog } from "./plugin-catalog";
import { PluginDetail } from "@/components/workspace/plugins/plugin-detail";
import { PluginActions } from "@/components/workspace/plugins/plugin-actions";
import { PluginGrants } from "@/components/workspace/plugins/plugin-grants";
import { AgentCapabilities, ScopedPluginPreferences } from "./plugin-preferences";
import { UpdateSettings } from "@/components/app-updates";
import { UsagePanel } from "@/components/workspace/usage/provider-usage";
import { BackupSettings } from "./backup-settings";

type AgentPlugin = { id: string; label: string };
type Settings = {
  app_provider: string | null;
  workspace_override: string | null;
  effective_provider: string | null;
  source: "app" | "workspace" | null;
};
const unset = "__inherit__";
type LoginStatus = "confirmed" | "unconfirmed" | "unavailable" | "timed_out" | "failed";


const sections = ["general", "agent", "services", "plugins", "quality", "backup"] as const;
type Section = (typeof sections)[number];
const sectionIcons = { general: Settings2, agent: Bot, services: KeyRound, plugins: Package, quality: Gauge, backup: DatabaseBackup };

export function AgentSettingsPage() {
  const { status, workspaces, load } = useWorkspaceCatalog();
  const query = useSearchParams();
  const router = useRouter();
  const { messages } = useLocale();
  const { agentSettings: copy, workspaces: workspaceCopy, settings: text } = messages;
  const studioWorkspace = query.get("workspace");
  const requested = query.get("scope") === "app" ? null : studioWorkspace;
  const setupWorkspace = workspaces.find(({ id }) => String(id) === query.get("setup"));
  const workspace = workspaces.find(({ id }) => String(id) === requested);
  const tab: Section = setupWorkspace ? "agent" : sections.find((value) => value === query.get("section")) ?? "general";
  const pluginDetail = tab === "plugins" ? query.get("plugin") : null;
  const scoped = !pluginDetail && (tab === "agent" || tab === "services" || tab === "plugins");
  const settingsHref = (target: Section, scope = requested) => {
    const params = new URLSearchParams();
    if (scope) params.set("workspace", scope);
    else if (studioWorkspace) {
      params.set("workspace", studioWorkspace);
      params.set("scope", "app");
    }
    if (target !== "general") params.set("section", target);
    return params.size ? `/settings?${params}` : "/settings";
  };
  const titles: Record<Section, [string, string]> = {
    general: [text.general, text.generalIntro],
    agent: [text.agent, text.agentIntro],
    services: [text.services, text.servicesIntro],
    plugins: [text.plugins, text.pluginsIntro],
    quality: [text.quality, text.qualityIntro],
    backup: [text.backup, text.backupIntro],
  };

  return (
    <WorkspaceShell className="grid max-w-6xl items-start gap-8 md:grid-cols-[12rem_minmax(0,1fr)]">
      <nav className="flex flex-wrap gap-1 md:sticky md:top-20 md:flex-col" aria-label={copy.title}>
        {sections.map((key) => {
          const Icon = sectionIcons[key];
          const active = key === tab;
          return (
            <Link key={key} href={settingsHref(key)} aria-current={active ? "page" : undefined}
              className={cn("flex items-center gap-2.5 rounded-lg px-3 py-2 font-semibold text-muted-foreground transition-colors duration-150 hover:bg-secondary hover:text-foreground",
                active && "bg-foreground text-background hover:bg-foreground hover:text-background")}>
              <Icon className={cn("size-4", active && "text-attention")} aria-hidden="true" />{titles[key][0]}
            </Link>
          );
        })}
      </nav>
      {pluginDetail ? <div className="min-w-0"><PluginDetail key={pluginDetail} pluginId={pluginDetail} backHref={settingsHref("plugins")} /></div> : <div className="min-w-0">
        <header className="mb-6">
          <h1 className="text-[clamp(2.25rem,5vw,3.25rem)] leading-[1] font-[850] [font-variation-settings:'wdth'_135]">{titles[tab][0]}</h1>
          <p className="mt-2 text-lg text-muted-foreground">{titles[tab][1]}</p>
        </header>
        {status === "browser" ? <Alert><AlertDescription>{workspaceCopy.desktop}</AlertDescription></Alert>
          : status === "error" ? <Alert variant="destructive">
            <AlertDescription>{workspaceCopy.loadError}</AlertDescription>
            <Button variant="outline" onClick={load}>{workspaceCopy.retry}</Button>
          </Alert>
          : status === "loading" ? <Skeleton className="h-40 max-w-xl" role="status" aria-label={copy.loading} />
          : (
            <div className="flex max-w-3xl flex-col gap-6">
              {tab === "general" && <GeneralSettings />}
              {tab === "general" && <NotificationSettings />}
              {tab === "general" && <AgentSummary workspaceId={studioWorkspace ? Number(studioWorkspace) : undefined} href={settingsHref("agent")} />}
              {tab === "general" && <PrivacySettings />}
              {tab === "general" && <UpdateSettings />}
              {tab === "quality" && <QualitySettings />}
              {tab === "backup" && <BackupSettings />}
              {setupWorkspace && <Alert><AlertDescription>{copy.setupIntro}</AlertDescription></Alert>}
              {scoped && <div className="flex flex-wrap items-center gap-3">
                <span id="settings-scope" className="font-semibold">{text.appliesTo}</span>
                <NativeSelect aria-labelledby="settings-scope" value={workspace ? String(workspace.id) : "app"}
                  onChange={(event) => router.push(settingsHref(tab, event.target.value === "app" ? null : event.target.value))}>
                  <NativeSelectOption value="app">{text.appScope}</NativeSelectOption>
                  {workspaces.map((entry) => <NativeSelectOption key={entry.id} value={entry.id}>{entry.name}</NativeSelectOption>)}
                </NativeSelect>
              </div>}
              {scoped && requested !== null && !workspace ? (
                <Alert variant="destructive"><AlertDescription>{workspaceCopy.missing}</AlertDescription></Alert>
              ) : tab === "agent" && <ProviderForm key={workspace?.id ?? "app"} workspaceId={workspace?.id ?? null} />}
              {setupWorkspace && <Button asChild variant="outline" className="self-start">
                <Link href={workspaceHref("context", setupWorkspace.id)}>{copy.continueSetup}</Link>
              </Button>}
              {tab === "plugins" && !requested && <PluginCatalog />}
              {tab === "services" && studioWorkspace && <UsagePanel key={`usage-${workspace?.id ?? "app"}`} workspaceId={workspace?.id ?? Number(studioWorkspace)} keysHref={settingsHref("plugins")} />}
              {tab === "services" && (requested === null || workspace) && <ScopedPluginPreferences key={workspace?.id ?? "app"} workspaceId={workspace?.id ?? null} />}
              {workspace && tab === "plugins" && <PluginGrants key={`grants:${workspace.id}`} workspace={workspace} />}
              {workspace && tab === "plugins" && <PluginActions key={`actions:${workspace.id}`} workspaceId={workspace.id} />}
            </div>
          )}
      </div>}
    </WorkspaceShell>
  );
}

function SettingRow({ label, hint, labelId, children }: { label: string; hint?: string; labelId: string; children: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-4 px-5 py-4">
      <div className="min-w-0 flex-1 basis-64">
        <p id={labelId} className="font-semibold">{label}</p>
        {hint && <p className="text-sm text-muted-foreground">{hint}</p>}
      </div>
      {children}
    </div>
  );
}

function Segmented<T extends string>({ labelledBy, value, options, onChange }: {
  labelledBy: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <div role="radiogroup" aria-labelledby={labelledBy} className="inline-flex flex-wrap rounded-lg bg-secondary p-1">
      {options.map((option) => (
        <button key={option.value} type="button" role="radio" aria-checked={value === option.value} onClick={() => onChange(option.value)}
          className={cn("rounded-md px-3 py-1.5 font-semibold text-muted-foreground transition-colors duration-150 hover:text-foreground",
            value === option.value && "bg-card text-foreground shadow-xs")}>{option.label}</button>
      ))}
    </div>
  );
}

function GeneralSettings() {
  const { locale, setLocale, messages: { settings: text } } = useLocale();
  const appearance = useAppearance();
  return (
    <section aria-labelledby="settings-appearance">
      <h2 id="settings-appearance" className="mb-3 text-[1.5rem]">{text.appearance}</h2>
      <div className="divide-y rounded-xl border bg-card">
        <SettingRow label={text.language} hint={text.languageHint} labelId="settings-language">
          <Segmented labelledBy="settings-language" value={locale} onChange={setLocale}
            options={[{ value: "en", label: "English" }, { value: "es", label: "Español" }, { value: "pt", label: "Português" }]} />
        </SettingRow>
        <SettingRow label={text.theme} labelId="settings-theme">
          <Segmented labelledBy="settings-theme" value={appearance.theme} onChange={appearance.setTheme} options={[
            { value: "system", label: text.themeSystem }, { value: "light", label: text.themeLight }, { value: "dark", label: text.themeDark },
          ]} />
        </SettingRow>
        <SettingRow label={text.motion} labelId="settings-motion"
          hint={appearance.motion === "system" ? text.motionSystem.replace("{state}", appearance.systemReducesMotion ? text.motionOn : text.motionOff) : undefined}>
          <div className="flex items-center gap-3">
            {appearance.motion !== "system" && <Button variant="ghost" size="sm" onClick={() => appearance.setMotion("system")}>{text.motionFollow}</Button>}
            <Switch aria-labelledby="settings-motion"
              checked={appearance.motion === "reduce" || (appearance.motion === "system" && appearance.systemReducesMotion)}
              onCheckedChange={(checked) => appearance.setMotion(checked ? "reduce" : "full")} />
          </div>
        </SettingRow>
      </div>
    </section>
  );
}

/** General › Notifications: whether generation events also reach the operating system. */
function NotificationSettings() {
  const { messages: { systemNotifications: copy, settings: text } } = useLocale();
  const [system, setSystem] = useState<boolean>();
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let active = true;
    invoke<boolean>("read_notification_settings").then((value) => { if (active) setSystem(value); }, () => { if (active) setFailed(true); });
    return () => { active = false; };
  }, []);
  async function change(value: boolean) {
    setSystem(value);
    setFailed(false);
    try { setSystem(await invoke<boolean>("write_notification_settings", { system: value })); }
    catch { setSystem(!value); setFailed(true); }
  }
  return (
    <section aria-labelledby="settings-notifications">
      <h2 id="settings-notifications" className="mb-3 text-[1.5rem]">{text.notifications}</h2>
      <div className="rounded-xl border bg-card">
        <SettingRow label={copy.setting} hint={copy.settingHint} labelId="settings-system-notifications">
          {system === undefined && !failed ? <Skeleton className="h-6 w-11" />
            : <Switch aria-labelledby="settings-system-notifications" checked={system ?? false} disabled={system === undefined} onCheckedChange={(value) => void change(value)} />}
        </SettingRow>
      </div>
      {failed && <p role="alert" className="mt-2 text-sm text-destructive">{text.notificationsError}</p>}
    </section>
  );
}

/** General › Agent: which agent writes the videos here, with a way to change it. */
function AgentSummary({ workspaceId, href }: { workspaceId?: number; href: string }) {
  const { messages: { settings: text } } = useLocale();
  const [agent, setAgent] = useState<{ name: string | null; source: Settings["source"]; others: number }>();

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Settings>("get_agent_settings", { workspaceId: workspaceId ?? null }),
      invoke<AgentPlugin[]>("list_agent_plugins"),
    ]).then(([settings, plugins]) => {
      if (!active) return;
      setAgent({
        name: plugins.find(({ id }) => id === settings.effective_provider)?.label ?? null,
        source: settings.source,
        others: plugins.filter(({ id }) => id !== settings.effective_provider).length,
      });
    }, () => { if (active) setAgent({ name: null, source: null, others: 0 }); });
    return () => { active = false; };
  }, [workspaceId]);

  return (
    <section aria-labelledby="settings-agent-summary">
      <h2 id="settings-agent-summary" className="mb-3 text-[1.5rem]">{text.agentHeading}</h2>
      {agent ? <div className="rounded-xl border bg-card">
        <SettingRow label={agent.name ?? text.noAgent} labelId="settings-agent-name"
          hint={agent.name ? (agent.source === "workspace" ? text.agentFromWorkspace : text.agentFromApp) : text.noAgentHint}>
          <Button asChild variant="outline"><Link href={href}>{agent.name && agent.others === 0 ? text.agentSettings : text.changeAgent}</Link></Button>
        </SettingRow>
      </div> : <Skeleton className="h-20" />}
    </section>
  );
}

type Privacy = { usageData: boolean; crashReports: boolean; usageAvailable: boolean; crashAvailable: boolean };

function PrivacySettings() {
  const { messages: { settings: text } } = useLocale();
  const [privacy, setPrivacy] = useState<Privacy>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    invoke<Privacy>("read_privacy_settings").then((value) => { if (active) setPrivacy(value); }, () => { if (active) setFailed(true); });
    return () => { active = false; };
  }, []);

  async function save(change: Partial<Privacy>) {
    if (!privacy) return;
    const next = { ...privacy, ...change };
    setPrivacy(next);
    try {
      setPrivacy(await invoke<Privacy>("write_privacy_settings", { usageData: next.usageData, crashReports: next.crashReports }));
      setFailed(false);
    } catch {
      setPrivacy(privacy);
      setFailed(true);
    }
  }

  return (
    <section aria-labelledby="settings-privacy">
      <h2 id="settings-privacy" className="mb-3 text-[1.5rem]">{text.privacy}</h2>
      {failed && <Alert variant="destructive" className="mb-3"><AlertDescription>{text.privacyError}</AlertDescription></Alert>}
      {privacy ? <div className="divide-y rounded-xl border bg-card">
        <SettingRow label={text.usageData} labelId="settings-usage"
          hint={privacy.usageAvailable ? text.usageDataHint : `${text.usageDataHint} ${text.notInThisBuild}`}>
          <Switch aria-labelledby="settings-usage" checked={privacy.usageData} onCheckedChange={(usageData) => void save({ usageData })} />
        </SettingRow>
        <SettingRow label={text.crashReports} labelId="settings-crash"
          hint={privacy.crashAvailable ? text.crashReportsHint : `${text.crashReportsHint} ${text.notInThisBuild}`}>
          <Switch aria-labelledby="settings-crash" checked={privacy.crashReports} onCheckedChange={(crashReports) => void save({ crashReports })} />
        </SettingRow>
      </div> : !failed && <Skeleton className="h-28" />}
    </section>
  );
}

type PreviewSettings = { override_max_edge: number | null; recommended_max_edge: number; effective_max_edge: number };
const previewSizes = [360, 480, 540, 720, 1080];

function QualitySettings() {
  const { messages: { settings: text } } = useLocale();
  const [settings, setSettings] = useState<PreviewSettings>();
  const [state, setState] = useState<"idle" | "saved" | "error">("idle");

  useEffect(() => {
    let active = true;
    invoke<PreviewSettings>("get_preview_settings").then((value) => { if (active) setSettings(value); }, () => { if (active) setState("error"); });
    return () => { active = false; };
  }, []);

  async function choose(value: string) {
    try {
      setSettings(await invoke<PreviewSettings>("set_preview_max_edge", { maxEdge: value === "auto" ? null : Number(value) }));
      setState("saved");
    } catch {
      setState("error");
    }
  }

  if (!settings) return state === "error" ? <Alert variant="destructive"><AlertDescription>{text.previewError}</AlertDescription></Alert>
    : <Skeleton className="h-24" />;
  return (
    <section className="divide-y rounded-xl border bg-card">
      <SettingRow label={text.previewSize} labelId="settings-preview" hint={text.previewHint.replace("{recommended}", String(settings.recommended_max_edge))}>
        <NativeSelect aria-labelledby="settings-preview" value={settings.override_max_edge === null ? "auto" : String(settings.override_max_edge)}
          onChange={(event) => void choose(event.target.value)}>
          <NativeSelectOption value="auto">{text.previewRecommended.replace("{size}", String(settings.recommended_max_edge))}</NativeSelectOption>
          {[...new Set([...previewSizes, settings.override_max_edge ?? 360])].sort((a, b) => a - b).map((size) =>
            <NativeSelectOption key={size} value={size}>{size} px</NativeSelectOption>)}
        </NativeSelect>
      </SettingRow>
      {state !== "idle" && <p role="status" className={cn("px-5 py-3 text-sm", state === "error" ? "text-destructive" : "text-muted-foreground")}>
        {state === "error" ? text.previewError : text.previewSaved.replace("{size}", String(settings.effective_max_edge))}
      </p>}
    </section>
  );
}

function ProviderForm({ workspaceId }: { workspaceId: number | null }) {
  const { messages: { agentSettings: copy, workspaces: workspaceCopy } } = useLocale();
  const [settings, setSettings] = useState<Settings>();
  const [choice, setChoice] = useState(unset);
  const [plugins, setPlugins] = useState<AgentPlugin[]>([]);
  const [error, setError] = useState<"loadError" | "saveError">();
  const [saved, setSaved] = useState(false);
  const [pending, setPending] = useState(false);
  const [attempt, setAttempt] = useState(0);

  function selected(result: Settings): string {
    return (workspaceId === null ? result.app_provider : result.workspace_override) ?? unset;
  }

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Settings>("get_agent_settings", { workspaceId }),
      invoke<AgentPlugin[]>("list_agent_plugins"),
    ]).then(
      ([result, available]) => {
        if (!active) return;
        setSettings(result);
        setPlugins(available);
        setChoice((workspaceId === null ? result.app_provider : result.workspace_override) ?? unset);
      },
      () => { if (active) setError("loadError"); },
    );
    return () => { active = false; };
  }, [attempt, workspaceId]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending || !settings || choice === selected(settings)) return;
    setPending(true);
    setSaved(false);
    setError(undefined);
    try {
      const result = await invoke<Settings>("set_agent_provider", {
        workspaceId, provider: choice === unset ? null : choice,
      });
      setSettings(result);
      setChoice(selected(result));
      setSaved(true);
    } catch {
      setError("saveError");
    } finally {
      setPending(false);
    }
  }

  return (
    <>
    <Card>
      <CardHeader>
        <CardTitle>{copy.provider}</CardTitle>
        <CardDescription>{workspaceId === null ? copy.appHint : copy.workspaceHint}</CardDescription>
      </CardHeader>
      <CardContent>
        {!settings ? error ? (
          <Alert variant="destructive">
            <AlertDescription>{copy[error]}</AlertDescription>
            <Button variant="outline" onClick={() => { setError(undefined); setAttempt((value) => value + 1); }}>{workspaceCopy.retry}</Button>
          </Alert>
        ) : <Skeleton className="h-20" role="status" aria-label={copy.loading} /> : (
          <form onSubmit={save}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="agent-provider">{copy.providerChoice}</FieldLabel>
                <Select value={choice} onValueChange={(value: string) => { setChoice(value); setSaved(false); }}
                  disabled={pending}>
                  <SelectTrigger id="agent-provider"><SelectValue /></SelectTrigger>
                  <SelectContent><SelectGroup>
                    <SelectItem value={unset}>{workspaceId === null ? copy.unset : copy.inherit}</SelectItem>
                    {plugins.map(({ id, label }) => <SelectItem key={id} value={id}>
                      <span className="flex items-center gap-2"><PluginIcon pluginId={id} className="size-5 rounded-sm" />{label}</span>
                    </SelectItem>)}
                  </SelectGroup></SelectContent>
                </Select>
              </Field>
              <div className="flex flex-wrap items-center gap-2 text-sm">
                <span>{copy.effective}</span>
                <Badge variant="secondary">{settings.effective_provider ? plugins.find(({ id }) => id === settings.effective_provider)?.label ?? settings.effective_provider : copy.unset}</Badge>
                {settings.source && <LayerGauge layer={settings.source} />}
              </div>
              {choice !== unset && <AgentSetupHelp key={choice} provider={choice} />}
              {error && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
              <div className="flex flex-wrap items-center gap-3">
                <Button type="submit" disabled={pending || choice === selected(settings)}>{copy.save}</Button>
                {saved && <p role="status" className="text-sm text-muted-foreground">{copy.saved}</p>}
              </div>
            </FieldGroup>
          </form>
        )}
      </CardContent>
    </Card>
    {settings?.effective_provider && <AgentCapabilities pluginId={settings.effective_provider} workspaceId={workspaceId} />}
    </>
  );
}


function AgentSetupHelp({ provider }: { provider: string }) {
  const { messages: { agentSettings: copy } } = useLocale();
  const [location, setLocation] = useState<AgentCliLocation>();
  const detected = location && location.found !== null;
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [programError, setProgramError] = useState<string>();
  const [loginStatus, setLoginStatus] = useState<LoginStatus>();
  const [checkingLogin, setCheckingLogin] = useState(false);
  const [guideError, setGuideError] = useState(false);

  useEffect(() => {
    let active = true;
    agentCliLocation(provider).then(
      (next) => { if (active) setLocation(next); },
      () => { if (active) setFailed(true); },
    );
    return () => { active = false; };
  }, [attempt, provider]);

  async function changeProgram(choose: boolean) {
    setProgramError(undefined);
    try {
      const next = choose ? await chooseAgentProgram(provider, copy.chooseProgramTitle) : await resetAgentProgram(provider);
      if (next) { setLocation(next); setLoginStatus(undefined); }
    } catch (error) {
      setProgramError(error === "agent_cli_not_runnable" ? copy.programNotRunnable : copy.programFailed);
    }
  }

  async function checkLogin() {
    setCheckingLogin(true);
    setLoginStatus(undefined);
    try {
      setLoginStatus(await invoke<LoginStatus>("agent_login_status", { provider }));
    } catch {
      setLoginStatus("failed");
    } finally {
      setCheckingLogin(false);
    }
  }

  return (
    <div className="flex flex-col gap-2.5 rounded-lg border bg-muted/30 p-3">
      <div role="status" className="text-sm">
        <p className="font-medium">{failed ? copy.checkFailed
          : detected === undefined ? copy.checking
          : detected ? copy.detected : copy.notDetected}</p>
        {location?.found && <p className="break-all text-muted-foreground">{copy.foundAt.replace("{path}", location.found)}</p>}
        <p className="text-muted-foreground">{copy.connectionHint}</p>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <Button type="button" size="sm" variant="outline" onClick={() => {
          setGuideError(false);
          void invoke("open_agent_setup_guide", { provider }).catch(() => setGuideError(true));
        }}>
          {copy.guide}<ArrowUpRight data-icon="inline-end" />
        </Button>
        <Button type="button" size="sm" variant="ghost" onClick={() => {
          setFailed(false);
          setLocation(undefined);
          setAttempt((value) => value + 1);
        }}>{copy.recheck}</Button>
        <Button type="button" size="sm" variant="outline" onClick={() => void changeProgram(true)}>{copy.chooseProgram}</Button>
        {location?.chosen && <Button type="button" size="sm" variant="ghost" onClick={() => void changeProgram(false)}>{copy.resetProgram}</Button>}
        {detected && <Button type="button" size="sm" variant="outline"
          disabled={checkingLogin} onClick={() => void checkLogin()}>
          {checkingLogin ? copy.checkingLogin : copy.checkLogin}
        </Button>}
      </div>
      <p role="status" className="text-sm text-muted-foreground">
        {loginStatus && ({
          confirmed: copy.loginConfirmed, unconfirmed: copy.loginUnconfirmed,
          unavailable: copy.notDetected, timed_out: copy.loginTimedOut, failed: copy.loginCheckFailed,
        })[loginStatus]}
      </p>
      {guideError && <p role="alert" className="text-sm text-destructive">{copy.guideError}</p>}
      {programError && <p role="alert" className="text-sm text-destructive">{programError}</p>}
    </div>
  );
}
