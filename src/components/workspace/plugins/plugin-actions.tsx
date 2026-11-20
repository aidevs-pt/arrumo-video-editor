"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useId, useState, type FormEvent } from "react";

import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";

type InstalledAction = {
  pluginId: string;
  pluginName: string;
  packageSha256: string;
  action: { id: string; label: string; description: string };
};

type ActionJob = {
  id: string;
  pluginId: string;
  actionId: string;
  state: "queued" | "running" | "completed" | "failed";
  result: string | null;
};

export function PluginActions({ workspaceId, revision = "" }: {
  workspaceId: number | null;
  revision?: string;
}) {
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
            <AccordionTrigger className="wrap-anywhere">
              <span>{entry.action.label}<span className="block text-xs text-muted-foreground">{entry.pluginName}</span></span>
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
              label={actions.find((entry) => entry.pluginId === job.pluginId && entry.action.id === job.actionId)?.action.label ?? job.actionId}
              onChanged={refreshHistory} />
          ))}
        </Accordion>
      </section>}
    </CardContent>
  </Card>;
}

function ActionForm({ entry, workspaceId, onChanged }: { entry: InstalledAction; workspaceId: number | null; onChanged: () => void }) {
  const { messages: { pluginActions: copy } } = useLocale();
  const field = useId();
  const result = useId();
  const [input, setInput] = useState("{}");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<"invalidInput" | "runError">();
  const [output, setOutput] = useState<string>();

  async function run(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    setError(undefined);
    setOutput(undefined);
    try {
      JSON.parse(input);
    } catch {
      setError("invalidInput");
      return;
    }
    setPending(true);
    try {
      const id = crypto.randomUUID();
      await invoke<void>("queue_plugin_action_job", { request: {
        id, workspaceId, inputJson: input, actionId: entry.action.id,
        pluginId: entry.pluginId, packageSha256: entry.packageSha256,
      } });
      onChanged();
      const job = await invoke<ActionJob>("process_plugin_action_job", { workspaceId, id });
      if (job.state === "failed") setError("runError");
      if (job.state === "completed") setOutput(job.result ?? undefined);
    } catch (reason) {
      setError(reason === "plugin_action_input_invalid" ? "invalidInput" : "runError");
    } finally {
      onChanged();
      setPending(false);
    }
  }

  return <form className="flex flex-col gap-3" onSubmit={run} aria-busy={pending}>
    <Field data-invalid={error === "invalidInput"}>
      <FieldDescription id={field + "-description"}>{entry.action.description}</FieldDescription>
      <FieldLabel htmlFor={field}>{copy.input}</FieldLabel>
      <Textarea id={field} aria-describedby={field + "-description"} aria-invalid={error === "invalidInput"}
        className="max-h-60 font-mono" spellCheck={false} maxLength={65_536} disabled={pending}
        value={input} onChange={(event) => { setInput(event.target.value); setError(undefined); setOutput(undefined); }} />
    </Field>
    <Button type="submit" size="sm" className="self-start" disabled={pending}>
      {pending ? copy.running : copy.run}
    </Button>
    {error && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
    {output !== undefined && <section className="flex min-w-0 flex-col gap-2" aria-labelledby={result}>
      <h3 id={result} className="text-sm font-medium">{copy.result}</h3>
      <pre tabIndex={0} className="max-h-72 overflow-auto rounded-md border bg-muted p-3 text-xs">{output}</pre>
    </section>}
    <span role="status" className="sr-only">{output !== undefined ? copy.finished : ""}</span>
  </form>;
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
