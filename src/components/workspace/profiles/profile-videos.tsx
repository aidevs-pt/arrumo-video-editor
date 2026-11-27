"use client";

import { invoke } from "@tauri-apps/api/core";
import { madeProfileFormats } from "@/lib/profile-formats";
import { ChevronRight, Film, RectangleHorizontal, RectangleVertical, Search } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { runHref, runNeedsReview, runSummary, runTitle, type Run } from "@/components/workspace/generation/generation-run-row";
import { completedVideoHref, profileHref, workspaceHref } from "@/components/workspace/home/workspace-links";
import { LayerGauge } from "@/components/workspace/layer-gauge";
import { VideoThumbnail } from "@/components/workspace/video-thumbnail";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import type { Profile } from "./profile-route";
import { useAgentChanges } from "@/lib/agent-changes";

type Draft = { id: number; title: string; intent: string; profile_id: number | null; source_ids: number[] };
type ProfileFormat = { id: number; name: string; width: number; height: number; overrides: { width: number | null; height: number | null } };
type Feedback = { id: number; title: string; markdown: string };
type Scheduled = { run_id: number; format_id: number; title: string; scheduled_date: string; format_name: string };
type Filter = "all" | "progress" | "drafts";
type Row = { kind: "draft"; draft: Draft } | { kind: "run"; run: Run };

/** How many portrait cuts a run made the way `profileId` makes its videos. */
function cutsFor(run: Run, profileId: number) {
  return (run.snapshot.formats ?? []).filter((entry) => entry.portrait_source?.profile_id === profileId)
    .reduce((total, entry) => total + (entry.delivery?.supplemental?.portrait_derivatives?.count ?? 0), 0);
}

type Data = {
  drafts: Draft[];
  /** Finished videos of other profiles whose portrait cuts followed this one. */
  cuts: { run: Run; count: number }[];
  runs: Run[];
  formats: ProfileFormat[];
  feedback: Feedback[];
  schedule: Scheduled[];
  agent: string | null;
};

function isoDate(date: Date) {
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 10);
}

export function ProfileVideos({ workspaceId, profile }: { workspaceId: number; profile: Profile }) {
  const { messages, locale } = useLocale();
  const copy = messages.profileHub;
  const [data, setData] = useState<Data>();
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [filter, setFilter] = useState<Filter>("all");
  const [search, setSearch] = useState("");

  const agentChanges = useAgentChanges();
  useEffect(() => {
    let active = true;
    const today = new Date();
    const later = new Date(today.getFullYear(), today.getMonth() + 3, today.getDate());
    Promise.all([
      invoke<Draft[]>("list_video_drafts", { workspaceId }),
      invoke<Run[]>("list_generation_runs", { workspaceId }),
      madeProfileFormats<ProfileFormat>(workspaceId, profile.id),
      invoke<Feedback[]>("list_workspace_feedback", { workspaceId }),
      invoke<Scheduled[]>("list_scheduled_outputs", { workspaceId, profileId: profile.id, start: isoDate(today), end: isoDate(later) }),
      Promise.all([
        invoke<{ effective_provider: string | null }>("get_agent_settings", { workspaceId }),
        invoke<{ id: string; label: string }[]>("list_agent_plugins"),
      ]).then(([settings, plugins]) => plugins.find(({ id }) => id === settings.effective_provider)?.label ?? null, () => null),
    ]).then(([drafts, runs, formats, feedback, schedule, agent]) => {
      if (!active) return;
      const submitted = new Set(runs.map((run) => run.draft_id));
      setData({
        drafts: drafts.filter((draft) => draft.profile_id === profile.id && !submitted.has(draft.id)),
        runs: runs.filter((run) => run.snapshot.profile?.id === profile.id).reverse(),
        cuts: runs.filter((run) => run.state === "completed" && run.snapshot.profile?.id !== profile.id)
          .map((run) => ({ run, count: cutsFor(run, profile.id) })).filter(({ count }) => count > 0).reverse(),
        formats, feedback, schedule, agent,
      });
    }, () => { if (active) setFailed(true); });
    return () => { active = false; };
  }, [attempt, profile.id, workspaceId, agentChanges]);

  if (failed) return (
    <Alert variant="destructive">
      <AlertDescription>{messages.queue.loadError}</AlertDescription>
      <Button type="button" variant="outline" onClick={() => { setFailed(false); setAttempt((value) => value + 1); }}>{messages.queue.retry}</Button>
    </Alert>
  );
  if (!data) return <Skeleton className="h-64" role="status" aria-label={messages.queue.loading} />;

  const inProgress = data.runs.filter((run) => !["completed", "cancelled", "failed"].includes(run.state));
  const rows: Row[] = filter === "drafts" ? data.drafts.map((draft) => ({ kind: "draft", draft }))
    : filter === "progress" ? inProgress.map((run) => ({ kind: "run", run }))
    : [...data.runs.map((run) => ({ kind: "run", run }) as Row), ...data.drafts.map((draft) => ({ kind: "draft", draft }) as Row)];
  const needle = search.trim().toLocaleLowerCase(locale);
  const visible = needle ? rows.filter((row) => (row.kind === "draft" ? row.draft.title : runTitle(row.run)).toLocaleLowerCase(locale).includes(needle)) : rows;
  const counts: Record<Filter, number> = { all: data.runs.length + data.drafts.length, progress: inProgress.length, drafts: data.drafts.length };
  const next = data.schedule[0];
  const dateLabel = new Intl.DateTimeFormat(locale, { weekday: "short", day: "numeric", month: "short", timeZone: "UTC" });

  return (
    <div className="grid items-start gap-10 lg:grid-cols-[minmax(0,1fr)_18rem]">
      <section className="min-w-0" aria-label={copy.videos}>
        <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
          <div role="radiogroup" aria-label={copy.filterLabel} className="inline-flex rounded-lg bg-secondary p-1">
            {(["all", "progress", "drafts"] as const).map((value) => (
              <button key={value} type="button" role="radio" aria-checked={filter === value} onClick={() => setFilter(value)}
                className={cn("rounded-md px-3 py-1.5 text-sm font-semibold text-muted-foreground transition-colors duration-150 hover:text-foreground",
                  filter === value && "bg-card text-foreground shadow-xs")}>
                {copy[value === "all" ? "filterAll" : value === "progress" ? "filterProgress" : "filterDrafts"]} <span className="tabular-nums">{counts[value]}</span>
              </button>
            ))}
          </div>
          <label className="relative w-full sm:w-72">
            <Search aria-hidden="true" className="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input type="search" value={search} onChange={(event) => setSearch(event.target.value)} placeholder={copy.search}
              aria-label={copy.search} className="ps-9" />
          </label>
        </div>
        {counts.all === 0 ? (
          <Empty className="border">
            <EmptyHeader>
              <EmptyMedia variant="icon"><Film /></EmptyMedia>
              <EmptyTitle>{copy.empty}</EmptyTitle>
              <EmptyDescription>{copy.emptyHint}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : visible.length === 0 ? <p className="border-t py-6 text-muted-foreground">{copy.noMatches}</p> : (
          <ul className="divide-y border-y">
            {visible.map((row) => row.kind === "draft"
              ? <DraftRow key={`draft-${row.draft.id}`} workspaceId={workspaceId} profile={profile} draft={row.draft} />
              : <RunRow key={`run-${row.run.id}`} workspaceId={workspaceId} profile={profile} run={row.run} />)}
          </ul>
        )}
      </section>
      <aside className="flex flex-col gap-8">
        {data.cuts.length > 0 && <section>
          <h2 className="mb-2 text-[1.3125rem]">{copy.portraitCuts}</h2>
          <ul className="flex flex-col gap-2">
            {data.cuts.map(({ run, count }) => (
              <li key={run.id}>
                <Link href={completedVideoHref(workspaceId, run.id)} className="group flex items-start gap-2.5 rounded-md py-1 hover:text-primary">
                  <RectangleVertical className="mt-0.5 size-4 shrink-0 text-muted-foreground group-hover:text-primary" aria-hidden="true" />
                  <span className="min-w-0">
                    <span className="block font-semibold wrap-anywhere">{runTitle(run)}</span>
                    <span className="block text-sm text-muted-foreground">
                      {run.snapshot.profile
                        ? (count === 1 ? copy.portraitCutFrom : copy.portraitCutsFrom.replace("{count}", String(count))).replace("{profile}", run.snapshot.profile.name)
                        : count === 1 ? copy.portraitCutOne : copy.portraitCutsCount.replace("{count}", String(count))}
                    </span>
                  </span>
                </Link>
              </li>
            ))}
          </ul>
        </section>}
        <section>
          <h2 className="mb-2 text-[1.3125rem]">{copy.formats}</h2>
          <ul className="divide-y border-y">
            {data.formats.map((format) => {
              const Icon = format.height > format.width ? RectangleVertical : RectangleHorizontal;
              const overridden = format.overrides.width !== null || format.overrides.height !== null;
              return (
                <li key={format.id} className="flex items-center gap-3 py-3">
                  <Icon className="size-4 shrink-0" aria-hidden="true" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate">{format.name}</span>
                    <span className="block text-sm text-muted-foreground tabular-nums">{format.width}×{format.height}</span>
                  </span>
                  <LayerGauge layer={overridden ? "profile" : "workspace"} />
                </li>
              );
            })}
          </ul>
          <Link href={`${profileHref(workspaceId, profile.id)}&tab=outputs`} className="mt-3 inline-block font-semibold text-primary underline underline-offset-4">{copy.changeOutputs}</Link>
        </section>
        <section>
          <h2 className="mb-2 text-[1.3125rem]">{data.agent ? copy.feedback.replace("{agent}", data.agent) : copy.feedbackFallback}</h2>
          {data.feedback.length ? <>
            <ul className="flex flex-col gap-2">
              {data.feedback.slice(0, 3).map((item) => <li key={item.id} className="text-muted-foreground">“{item.title}”</li>)}
            </ul>
            <Link href={workspaceHref("context", workspaceId)} className="mt-3 inline-block font-semibold text-primary underline underline-offset-4">{copy.seeContext}</Link>
          </> : <p className="text-muted-foreground">{copy.noFeedback}</p>}
        </section>
        <section>
          <h2 className="mb-2 text-[1.3125rem]">{copy.nextUp}</h2>
          {next ? <Link href={completedVideoHref(workspaceId, next.run_id, next.format_id)} className="text-muted-foreground hover:text-primary">
            {dateLabel.format(new Date(`${next.scheduled_date}T00:00:00Z`))}, <span className="font-semibold text-foreground">{next.title}</span> ({next.format_name})
          </Link> : <p className="text-muted-foreground">{copy.nothingScheduled}</p>}
        </section>
      </aside>
    </div>
  );
}

function DraftRow({ workspaceId, profile, draft }: { workspaceId: number; profile: Profile; draft: Draft }) {
  const { messages } = useLocale();
  const copy = messages.profileHub;
  const clips = draft.source_ids.length;
  return (
    <li className="flex items-center gap-5 py-5">
      <VideoThumbnail title={draft.title} color={profile.color} className="w-32 opacity-60 sm:w-48" />
      <div className="min-w-0 flex-1">
        <p className="font-heading text-xl leading-tight font-[850] wrap-anywhere [font-variation-settings:'wdth'_112]">{draft.title}</p>
        <p className="mt-2 flex flex-wrap items-center gap-2 text-muted-foreground">
          <span className="rounded-[5px] bg-secondary px-2 py-0.5 text-sm font-semibold text-foreground">{copy.draft}</span>
          {clips === 0 ? messages.drafts.intentOnly : clips === 1 ? messages.videoCatalog.singleClip : messages.videoCatalog.clipCount.replace("{count}", String(clips))}
        </p>
      </div>
      <Button variant="outline" asChild>
        <Link href={`${workspaceHref("drafts", workspaceId)}&draft=${draft.id}`}>{copy.continueDraft}</Link>
      </Button>
    </li>
  );
}

function RunRow({ workspaceId, profile, run }: { workspaceId: number; profile: Profile; run: Run }) {
  const { messages } = useLocale();
  const completed = run.state === "completed";
  const formats = run.snapshot.formats?.map(({ format }) => format.name).join(", ");
  const review = runNeedsReview(run);
  return (
    <li>
      <Link href={runHref(workspaceId, run)} className="group flex items-center gap-5 py-5">
        <VideoThumbnail title={runTitle(run)} color={profile.color} className="w-32 sm:w-48" />
        <span className="min-w-0 flex-1">
          <span className="block font-heading text-xl leading-tight font-[850] wrap-anywhere [font-variation-settings:'wdth'_112] group-hover:text-primary">{runTitle(run)}</span>
          {!completed && <span className={cn("mt-2 inline-block rounded-[5px] px-2 py-0.5 text-sm font-semibold",
            review ? "bg-attention text-ink" : run.state === "failed" ? "bg-destructive/10 text-destructive" : "bg-secondary")}>
            {review ? messages.productionPlan.awaitingReview : messages.queue[run.state]}
          </span>}
          <span className="mt-1.5 block text-muted-foreground">{completed ? formats : runSummary(run, messages)}</span>
        </span>
        <ChevronRight aria-hidden="true" className="size-5 shrink-0 text-muted-foreground group-hover:text-primary" />
      </Link>
    </li>
  );
}
