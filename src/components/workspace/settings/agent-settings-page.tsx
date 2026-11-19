"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, Settings2 } from "lucide-react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { useWorkspaceCatalog, workspaceHref } from "@/components/workspace/home/workspace-router";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";

type Provider = "codex" | "claude_code";
type Settings = {
  app_provider: Provider | null;
  workspace_override: Provider | null;
  effective_provider: Provider | null;
  source: "app" | "workspace" | null;
};
type Choice = Provider | "unset";
type LoginStatus = "confirmed" | "unconfirmed" | "unavailable" | "timed_out" | "failed";
const providerNames = { codex: "Codex", claude_code: "Claude Code" };

export function AgentSettingsPage() {
  const { status, workspaces, load } = useWorkspaceCatalog();
  const query = useSearchParams();
  const router = useRouter();
  const { messages: { agentSettings: copy, workspaces: workspaceCopy } } = useLocale();
  const requested = query.get("workspace");
  const setupWorkspace = workspaces.find(({ id }) => String(id) === query.get("setup"));
  const workspace = workspaces.find(({ id }) => String(id) === requested);
  return (
    <WorkspaceShell>
      <WorkspaceHeading icon={Settings2}><h1>{copy.title}</h1><p>{copy.intro}</p></WorkspaceHeading>
      {status === "browser" ? <Alert><AlertDescription>{workspaceCopy.desktop}</AlertDescription></Alert>
        : status === "error" ? <Alert variant="destructive">
          <AlertDescription>{workspaceCopy.loadError}</AlertDescription>
          <Button variant="outline" onClick={load}>{workspaceCopy.retry}</Button>
        </Alert>
        : status === "loading" ? <Skeleton className="h-40 max-w-xl" role="status" aria-label={copy.loading} />
        : (
          <div className="flex max-w-xl flex-col gap-4">
            {setupWorkspace && <Alert><AlertDescription>{copy.setupIntro}</AlertDescription></Alert>}
            <Field>
              <FieldLabel htmlFor="agent-settings-scope">{copy.scope}</FieldLabel>
              <Select value={workspace ? String(workspace.id) : "app"}
                onValueChange={(value) => router.push(value === "app" ? "/settings" : `/settings?workspace=${value}`)}>
                <SelectTrigger id="agent-settings-scope"><SelectValue /></SelectTrigger>
                <SelectContent><SelectGroup>
                  <SelectItem value="app">{copy.app}</SelectItem>
                  {workspaces.map((entry) => <SelectItem key={entry.id} value={String(entry.id)}>{entry.name}</SelectItem>)}
                </SelectGroup></SelectContent>
              </Select>
            </Field>
            {requested !== null && !workspace ? (
              <Alert variant="destructive"><AlertDescription>{workspaceCopy.missing}</AlertDescription></Alert>
            ) : <ProviderForm key={workspace?.id ?? "app"} workspaceId={workspace?.id ?? null} />}
            {setupWorkspace && <Button asChild variant="outline" className="self-start">
              <Link href={workspaceHref("context", setupWorkspace.id)}>{copy.continueSetup}</Link>
            </Button>}
          </div>
        )}
    </WorkspaceShell>
  );
}

function ProviderForm({ workspaceId }: { workspaceId: number | null }) {
  const { messages: { agentSettings: copy, workspaces: workspaceCopy } } = useLocale();
  const [settings, setSettings] = useState<Settings>();
  const [choice, setChoice] = useState<Choice>("unset");
  const [error, setError] = useState<"loadError" | "saveError">();
  const [saved, setSaved] = useState(false);
  const [pending, setPending] = useState(false);
  const [attempt, setAttempt] = useState(0);

  function selected(result: Settings): Choice {
    return (workspaceId === null ? result.app_provider : result.workspace_override) ?? "unset";
  }

  useEffect(() => {
    let active = true;
    invoke<Settings>("get_agent_settings", { workspaceId }).then(
      (result) => {
        if (!active) return;
        setSettings(result);
        setChoice((workspaceId === null ? result.app_provider : result.workspace_override) ?? "unset");
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
        workspaceId, provider: choice === "unset" ? null : choice,
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
                <Select value={choice} onValueChange={(value: Choice) => { setChoice(value); setSaved(false); }}
                  disabled={pending}>
                  <SelectTrigger id="agent-provider"><SelectValue /></SelectTrigger>
                  <SelectContent><SelectGroup>
                    <SelectItem value="unset">{workspaceId === null ? copy.unset : copy.inherit}</SelectItem>
                    <SelectItem value="codex">{providerNames.codex}</SelectItem>
                    <SelectItem value="claude_code">{providerNames.claude_code}</SelectItem>
                  </SelectGroup></SelectContent>
                </Select>
              </Field>
              <div className="flex flex-wrap items-center gap-2 text-sm">
                <span>{copy.effective}</span>
                <Badge variant="secondary">{settings.effective_provider ? providerNames[settings.effective_provider] : copy.unset}</Badge>
                {settings.source && <span className="text-muted-foreground">{settings.source === "app" ? copy.fromApp : copy.fromWorkspace}</span>}
              </div>
              {choice !== "unset" && <AgentSetupHelp key={choice} provider={choice} />}
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
  );
}


function AgentSetupHelp({ provider }: { provider: Provider }) {
  const { messages: { agentSettings: copy } } = useLocale();
  const [detected, setDetected] = useState<boolean>();
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [loginStatus, setLoginStatus] = useState<LoginStatus>();
  const [checkingLogin, setCheckingLogin] = useState(false);
  const [guideError, setGuideError] = useState(false);

  useEffect(() => {
    let active = true;
    invoke<boolean>("agent_cli_detected", { provider }).then(
      (status) => { if (active) setDetected(status); },
      () => { if (active) setFailed(true); },
    );
    return () => { active = false; };
  }, [attempt, provider]);

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
          setDetected(undefined);
          setAttempt((value) => value + 1);
        }}>{copy.recheck}</Button>
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
    </div>
  );
}
