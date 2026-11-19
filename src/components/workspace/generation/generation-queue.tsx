"use client";

import { useLocale } from "@/lib/locale-provider";
import { WorkspaceHeading, WorkspaceRows, WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, ListVideo, Plus } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { GenerationRunRow, runNeedsReview, type Run } from "./generation-run-row";

import { RunProviders } from "./run-providers";

type Draft = { id: number; title: string };
export function GenerationQueue({ workspaceId, drafts, initialDraftId, onBack }: {
  workspaceId: number;
  drafts: Draft[];
  initialDraftId: number | null;

  onBack: () => void;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const request = useRef<{ key: string; payload: string } | null>(null);
  const [selectedId, setSelectedId] = useState(initialDraftId);
  const [runs, setRuns] = useState<Run[]>();
  const [attempt, setAttempt] = useState(0);
  const [loadError, setLoadError] = useState(false);
  const [pending, setPending] = useState(false);
  const [submitError, setSubmitError] = useState(false);
  const { messages } = useLocale();
  const copy = messages.queue;
  const selected = drafts.find((draft) => draft.id === selectedId);

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    if (pending) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const items = await invoke<Run[]>("list_generation_runs", { workspaceId });
        if (!active) return;
        setRuns(items);
        setLoadError(false);
      } catch {
        if (active) setLoadError(true);
      }
      if (active) timer = setTimeout(refresh, 2000);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [workspaceId, attempt, pending]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selected || !runs || pending) return;
    const fields = new FormData(event.currentTarget);
    const options = {
      autopilot: fields.has("autopilot"),
      platform: String(fields.get("platform") ?? "").trim() || null,
      provider_overrides: JSON.parse(String(fields.get("provider-overrides") ?? "{}")) as Record<string, string[]>,
    };
    const payload = JSON.stringify({ draftId: selected.id, options });
    if (request.current?.payload !== payload) {
      request.current = { key: crypto.randomUUID(), payload };
    }
    setPending(true);
    setSubmitError(false);
    try {
      const run = await invoke<Run>("enqueue_video_draft", {
        workspaceId, draftId: selected.id, requestKey: request.current.key, options,
      });
      const next = [...runs.filter((item) => item.id !== run.id), run].sort((a, b) => a.id - b.id);
      setRuns(next);
      const queued = new Set(next.map((item) => item.draft_id));
      setSelectedId(drafts.find((draft) => !queued.has(draft.id))?.id ?? null);
      request.current = null;
    } catch {
      setSubmitError(true);
    } finally {
      setPending(false);
    }
  }

  const groups = runs ? [
    { id: "all", label: copy.runs, runs: [...runs].reverse() },
    { id: "review", label: copy.reviewFilter, runs: runs.filter(runNeedsReview).reverse() },
    { id: "completed", label: copy.completed, runs: runs.filter((run) => run.state === "completed").reverse() },
  ] : [];

  return (
    <WorkspaceShell>
      <Button type="button" variant="ghost" disabled={pending} onClick={onBack}>
        <ArrowLeft data-icon="inline-start" />{copy.back}
      </Button>
      <WorkspaceHeading icon={ListVideo}><h1 ref={heading} tabIndex={-1}>{copy.title}</h1></WorkspaceHeading>
      <div className="flex flex-col gap-4">
        {loadError && (
          <Alert variant="destructive">
            <AlertDescription>{copy.loadError}</AlertDescription>
            <Button type="button" variant="outline" onClick={() => {
              setLoadError(false);
              setAttempt((value) => value + 1);
            }}>{copy.retry}</Button>
          </Alert>
        )}
        {selected && runs && (
          <Card size="sm">
            <CardHeader><CardTitle>{copy.options} · {selected.title}</CardTitle></CardHeader>
            <CardContent>
              <form key={selected.id} onSubmit={submit} aria-busy={pending}>
                <fieldset disabled={pending}>
                  <FieldGroup>
                    <Field>
                      <FieldLabel htmlFor="run-platform">{copy.platform}</FieldLabel>
                      <Input id="run-platform" name="platform" />
                    </Field>
                    <Field orientation="horizontal">
                      <Switch id="run-autopilot" name="autopilot" disabled={pending} aria-describedby="run-autopilot-description" />
                      <div className="flex flex-col gap-1">
                        <FieldLabel htmlFor="run-autopilot">{copy.autopilot}</FieldLabel>
                        <FieldDescription id="run-autopilot-description">{copy.autopilotHint}</FieldDescription>
                      </div>
                    </Field>
                    <RunProviders workspaceId={workspaceId} disabled={pending} />
                    {submitError && <Alert variant="destructive"><AlertDescription>{copy.submitError}</AlertDescription></Alert>}
                    <Button className="self-start" type="submit" disabled={pending}>
                      <Plus data-icon="inline-start" />{pending ? copy.submitting : copy.submit}
                    </Button>
                  </FieldGroup>
                </fieldset>
              </form>
            </CardContent>
          </Card>
        )}
        {runs ? (
          <Tabs defaultValue="all" className="min-w-0 gap-4">
            <TabsList variant="line" className="max-w-full justify-start overflow-x-auto" aria-label={copy.filterRuns}>
              {groups.map(({ id, label, runs: group }) => (
                <TabsTrigger value={id} key={id}>{label} <span className="tabular-nums text-muted-foreground">{group.length}</span></TabsTrigger>
              ))}
            </TabsList>
            {groups.map(({ id, runs: group }) => (
              <TabsContent value={id} key={id}>
                {group.length ? (
                  <WorkspaceRows compact>
                    {group.map((run) => <GenerationRunRow key={run.id} run={run} workspaceId={workspaceId} />)}
                  </WorkspaceRows>
                ) : (
                  <Empty className="border">
                    <EmptyHeader>
                      <EmptyMedia variant="icon"><ListVideo aria-hidden="true" /></EmptyMedia>
                      <EmptyTitle>{id === "all" ? copy.empty : copy.filterEmpty}</EmptyTitle>
                      {id === "all" && <EmptyDescription>{copy.emptyDescription}</EmptyDescription>}
                    </EmptyHeader>
                    {id === "all" && !selected && <EmptyContent>
                      <Button type="button" variant="outline" onClick={onBack}>{copy.back}</Button>
                    </EmptyContent>}
                  </Empty>
                )}
              </TabsContent>
            ))}
          </Tabs>
        ) : !loadError && <Skeleton className="min-h-36" aria-label={copy.loading} />}
      </div>
    </WorkspaceShell>
  );
}
