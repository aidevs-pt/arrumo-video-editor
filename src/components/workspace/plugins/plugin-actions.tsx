"use client";

import { PluginIcon, usePluginText } from "@/lib/plugin-text";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useLocale } from "@/lib/locale-provider";

import { ActionForm, type ActionJob, type InstalledAction } from "./action-form";

export function PluginActions({ workspaceId, revision = "" }: {
  workspaceId: number | null;
  revision?: string;
}) {
  const pluginText = usePluginText();
  const { messages: { pluginActions: copy, workspaces } } = useLocale();
  const [actions, setActions] = useState<InstalledAction[]>([]);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [history, setHistory] = useState<ActionJob[]>([]);
  const [historyFailed, setHistoryFailed] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const refreshHistory = () => setRefresh((value) => value + 1);

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void listen("arrumo-plugin-jobs-changed", () => {
      if (active) setRefresh((value) => value + 1);
    }).then((unlisten) => {
      if (active) unsubscribe = unlisten;
      else unlisten();
    }).catch(() => {
      // The panel still refreshes after actions it starts.
    });
    return () => { active = false; unsubscribe?.(); };
  }, []);

  useEffect(() => {
    let active = true;
    invoke<ActionJob[]>("list_plugin_action_jobs", { workspaceId }).then(
      (items) => { if (active) { setHistory(items); setHistoryFailed(false); } },
      () => { if (active) { setHistory([]); setHistoryFailed(true); } },
    );
    return () => { active = false; };
  }, [workspaceId, refresh]);
  useEffect(() => {
    let active = true;
    invoke<InstalledAction[]>("list_plugin_actions", { workspaceId }).then(
      (items) => { if (active) { setActions(items); setFailed(false); } },
      () => { if (active) { setActions([]); setFailed(true); } },
    );
    return () => { active = false; };
  }, [workspaceId, revision, attempt]);

  if (failed) return <Alert variant="destructive">
    <AlertDescription>{copy.loadError}</AlertDescription>
    <Button type="button" variant="outline" onClick={() => setAttempt((value) => value + 1)}>{workspaces.retry}</Button>
  </Alert>;
  if (!actions.length && !history.length && !historyFailed) return null;

  return <Card key={workspaceId ?? "app"}>
    <CardHeader><CardTitle>{copy.title}</CardTitle></CardHeader>
    <CardContent className="flex flex-col gap-4">
      <Accordion type="single" collapsible>
        {actions.map((entry) => {
          const key = `${entry.pluginId}:${entry.action.id}:${entry.packageSha256}`;
          return <AccordionItem key={key} value={key}>
            <AccordionTrigger className="items-center wrap-anywhere">
              <span className="flex min-w-0 items-center gap-3"><PluginIcon pluginId={entry.pluginId} className="size-8" /><span>{pluginText.action(entry.pluginId, entry.action.id, "label", entry.action.label)}<span className="block text-xs text-muted-foreground">{pluginText.name(entry.pluginId, entry.pluginName)}</span></span></span>
            </AccordionTrigger>
            <AccordionContent><ActionForm entry={entry} workspaceId={workspaceId} onChanged={refreshHistory} /></AccordionContent>
          </AccordionItem>;
        })}
      </Accordion>
      {historyFailed && <Alert variant="destructive">
        <AlertDescription>{copy.jobsLoadError}</AlertDescription>
        <Button type="button" variant="outline" size="sm" onClick={refreshHistory}>{workspaces.retry}</Button>
      </Alert>}
      {history.length > 0 && <section aria-label={copy.recentJobs}>
        <h3 className="text-sm font-medium">{copy.recentJobs}</h3>
        <Accordion type="single" collapsible>
          {history.map((job) => (
            <JobHistoryItem key={job.id} job={job} workspaceId={workspaceId}
              label={pluginText.action(job.pluginId, job.actionId, "label", actions.find((entry) => entry.pluginId === job.pluginId && entry.action.id === job.actionId)?.action.label ?? job.actionId)}
              onChanged={refreshHistory} />
          ))}
        </Accordion>
      </section>}
    </CardContent>
  </Card>;
}

function JobHistoryItem({ job, label, workspaceId, onChanged }: {
  job: ActionJob;
  label: string;
  workspaceId: number | null;
  onChanged: () => void;
}) {
  const { messages: { pluginActions: copy } } = useLocale();
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);

  async function resume() {
    setPending(true);
    setFailed(false);
    try {
      await invoke("process_plugin_action_job", { workspaceId, id: job.id });
      onChanged();
    } catch {
      setFailed(true);
    } finally {
      setPending(false);
    }
  }

  return <AccordionItem value={job.id}>
    <AccordionTrigger className="wrap-anywhere">
      <span>{label}<span className="block text-xs text-muted-foreground">{copy.jobState[job.state]}</span></span>
    </AccordionTrigger>
    <AccordionContent>
      <div className="flex flex-col gap-2">
        {(job.state === "completed" || job.state === "failed") &&
          <pre tabIndex={0} className="max-h-72 overflow-auto rounded-md border bg-muted p-3 text-xs">{job.result}</pre>}
        {(job.state === "queued" || job.state === "running") &&
          <Button type="button" size="sm" variant="outline" disabled={pending} className="self-start"
            onClick={() => void resume()}>{pending ? copy.running : copy.resumeJob}</Button>}
        {failed && <Alert variant="destructive"><AlertDescription>{copy.runError}</AlertDescription></Alert>}
      </div>
    </AccordionContent>
  </AccordionItem>;
}
