"use client";

import { PluginIcon } from "@/lib/plugin-text";
import { invoke } from "@tauri-apps/api/core";
import { ArrowRight, Plus, RectangleHorizontal, RectangleVertical, Upload } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { runHref, runNeedsReview, runSummary, runTitle, type Run } from "@/components/workspace/generation/generation-run-row";
import { WorkspaceShell } from "@/components/workspace/layout";
import { StepTrack } from "@/components/workspace/step-track";
import { VideoThumbnail } from "@/components/workspace/video-thumbnail";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { completedVideoHref, profileHref, workspaceHref, type Workspace } from "./workspace-links";
import { useAgentChanges } from "@/lib/agent-changes";

type Profile = { id: number; name: string; guidance: string; color: string | null };
type ScheduledOutput = {
  run_id: number; format_id: number; title: string; scheduled_date: string;
  profile_color: string | null; width: number; height: number;
};
type Dashboard = { runs: Run[]; profiles: Profile[]; schedule: ScheduledOutput[] };

function localDate(date: Date) {
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 10);
}

function weekDays() {
  const today = new Date();
  const monday = new Date(today.getFullYear(), today.getMonth(), today.getDate() - ((today.getDay() + 6) % 7));
  return Array.from({ length: 7 }, (_, offset) => new Date(monday.getFullYear(), monday.getMonth(), monday.getDate() + offset));
}

/** Plain text from a Markdown string's first paragraph, for one-line descriptions. */
export function firstLine(markdown: string) {
  const line = markdown.split("\n").map((value) => value.trim()).find((value) => value && !value.startsWith("#"));
  return line?.replace(/[*_`>#[\]]/g, "").replace(/\(([^)]*)\)/g, "") ?? "";
}

export function StudioDashboard({ workspace }: { workspace: Workspace }) {
  const { messages } = useLocale();
  const copy = messages.dashboard;
  const [data, setData] = useState<Dashboard>();
  const [failed, setFailed] = useState(false);
  const workspaceId = workspace.id;

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      const days = weekDays();
      try {
        const [runs, profiles, schedule] = await Promise.all([
          invoke<Run[]>("list_generation_runs", { workspaceId }),
          invoke<Profile[]>("list_profiles", { workspaceId }),
          invoke<ScheduledOutput[]>("list_scheduled_outputs", {
            workspaceId, profileId: null, start: localDate(days[0]), end: localDate(days[6]),
          }),
        ]);
        if (active) {
          setData({ runs: runs.toReversed(), profiles, schedule });
          setFailed(false);
        }
      } catch {
        if (active) setFailed(true);
      }
      if (active) timer = setTimeout(refresh, 5000);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [workspaceId]);

  const reviews = data?.runs.filter(runNeedsReview) ?? [];
  const activeRuns = data?.runs.filter((run) => ["queued", "running", "paused"].includes(run.state) && !runNeedsReview(run)) ?? [];
  const heading = reviews.length === 1 ? copy.oneReview
    : reviews.length > 1 ? copy.manyReviews.replace("{count}", String(reviews.length)) : workspace.name;
  const videosByProfile = new Map<number, number>();
  for (const run of data?.runs ?? []) {
    const id = run.snapshot.profile?.id;
    if (id !== undefined) videosByProfile.set(id, (videosByProfile.get(id) ?? 0) + 1);
  }

  return (
    <WorkspaceShell className="flex flex-col gap-10">
      <header className="flex flex-col items-start gap-6 border-b-2 border-foreground pb-8">
        <h1 className="max-w-4xl text-[clamp(2.25rem,5vw,3.25rem)] leading-[.98] font-[850] tracking-[-.015em] text-balance [font-variation-settings:'wdth'_135]">{heading}</h1>
        <div className="flex flex-wrap gap-3">
          <Button variant="outline" asChild>
            <Link href={`${workspaceHref("drafts", workspaceId)}&compose=scratch`}>{messages.videoCatalog.scratch}</Link>
          </Button>
          <Button asChild>
            <Link href={`${workspaceHref("drafts", workspaceId)}&compose=1`}><Upload data-icon="inline-start" />{messages.videoCatalog.generate}</Link>
          </Button>
        </div>
      </header>
      <ContextNudge workspaceId={workspaceId} />
      {failed && <Alert><AlertDescription>{messages.queue.loadError}</AlertDescription></Alert>}
      {!data && !failed && <Skeleton className="h-40 w-full" aria-label={messages.queue.loading} />}
      {data && <>
        {reviews.length > 0 && <ul className="-mt-4 divide-y border-b" aria-label={messages.studio.reviewInbox}>
          {reviews.map((run) => (
            <li key={run.id}>
              <Link href={runHref(workspaceId, run)} className="group flex items-center gap-5 py-5">
                <VideoThumbnail title={runTitle(run)} color={run.snapshot.profile?.color} className="w-36 sm:w-60" />
                <span className="min-w-0 flex-1">
                  {run.snapshot.profile && <ProfileTag profile={run.snapshot.profile} />}
                  <span className="mt-1 block font-heading text-[clamp(1.125rem,2vw,1.5rem)] leading-tight font-[850] wrap-anywhere [font-variation-settings:'wdth'_112] group-hover:text-primary">{runTitle(run)}</span>
                  <span className="mt-1 block text-muted-foreground">{runSummary(run, messages)}</span>
                </span>
              </Link>
            </li>
          ))}
        </ul>}
        <div className="grid gap-10 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)]">
          <section className="min-w-0" aria-labelledby="dashboard-progress">
            <SectionHeader id="dashboard-progress" title={copy.inProgress} href={workspaceHref("generations", workspaceId)} label={copy.allVideos} />
            {activeRuns.length ? <ul className="divide-y">
              {activeRuns.slice(0, 4).map((run) => (
                <li key={run.id} className="py-4 first:pt-1">
                  <Link href={runHref(workspaceId, run)} className="group flex flex-col gap-3">
                    <span className="flex items-center gap-3">
                      <span aria-hidden="true" className="size-3 shrink-0 rounded-[3px]" style={{ backgroundColor: run.snapshot.profile?.color ?? "var(--border)" }} />
                      <span className="min-w-0 flex-1 truncate font-heading text-lg font-[800] [font-variation-settings:'wdth'_108] group-hover:text-primary">{runTitle(run)}</span>
                      {run.snapshot.options?.autopilot && <span className="rounded-[5px] bg-secondary px-2 py-0.5 text-sm font-semibold text-primary">{copy.onAutopilot}</span>}
                    </span>
                    {run.progress && <StepTrack count={run.progress.stage_count} current={run.progress.stage_index} label={copy.progressLabel} />}
                    <span className="text-sm text-muted-foreground">{runSummary(run, messages)}</span>
                  </Link>
                </li>
              ))}
            </ul> : <p className="py-2 text-muted-foreground">{copy.noActiveRuns}</p>}
          </section>
          <section className="min-w-0" aria-labelledby="dashboard-week">
            <SectionHeader id="dashboard-week" title={copy.publishing} href={workspaceHref("calendar", workspaceId)} label={messages.calendar.title} />
            <PublishingWeek workspaceId={workspaceId} schedule={data.schedule} />
          </section>
        </div>
        <section aria-labelledby="dashboard-profiles">
          <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
            <h2 id="dashboard-profiles" className="text-[1.75rem]">{messages.profiles.title}</h2>
            <Button variant="outline" asChild>
              <Link href={`${workspaceHref("profiles", workspaceId)}&new=1`}><Plus data-icon="inline-start" />{messages.profiles.newProfile}</Link>
            </Button>
          </div>
          {data.profiles.length ? <ul className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">{data.profiles.map((profile) => {
            const count = videosByProfile.get(profile.id) ?? 0;
            return (
              <li key={profile.id}>
                <Link href={profileHref(workspaceId, profile.id)} className="group flex h-full flex-col gap-2 rounded-xl border bg-card p-5 hover:border-primary">
                  <span aria-hidden="true" className="mb-2 h-1.5 rounded-[2px]" style={{ backgroundColor: profile.color ?? "var(--border)" }} />
                  <span className="font-heading text-[1.5rem] leading-tight font-[850] [font-variation-settings:'wdth'_112] group-hover:text-primary">{profile.name}</span>
                  {firstLine(profile.guidance) && <span className="line-clamp-2 text-muted-foreground">{firstLine(profile.guidance)}</span>}
                  <span className="mt-auto pt-2 text-sm text-muted-foreground">
                    {count === 0 ? copy.noVideos : count === 1 ? copy.oneVideo : copy.videoCount.replace("{count}", String(count))}
                  </span>
                </Link>
              </li>
            );
          })}</ul> : <p className="text-muted-foreground">{messages.profiles.empty}</p>}
        </section>
      </>}
    </WorkspaceShell>
  );
}

function ProfileTag({ profile }: { profile: { name: string; color: string | null } }) {
  return (
    <span className="flex items-center gap-2 text-sm font-semibold">
      <span aria-hidden="true" className="size-3 rounded-[3px]" style={{ backgroundColor: profile.color ?? "var(--border)" }} />
      {profile.name}
    </span>
  );
}

function PublishingWeek({ workspaceId, schedule }: { workspaceId: number; schedule: ScheduledOutput[] }) {
  const { locale } = useLocale();
  const weekday = new Intl.DateTimeFormat(locale, { weekday: "short" });
  const today = localDate(new Date());
  return (
    <ol className="grid grid-cols-7 gap-1.5">
      {weekDays().map((day) => {
        const date = localDate(day);
        const outputs = schedule.filter((output) => output.scheduled_date === date);
        return (
          <li key={date} className={cn("flex min-h-36 min-w-0 flex-col gap-2 rounded-lg p-2",
            outputs.length ? "border bg-card" : "", date === today && "outline-2 outline-primary")}>
            <span className="text-sm"><span className="font-bold">{weekday.format(day)}</span> <span className="text-muted-foreground tabular-nums">{day.getDate()}</span></span>
            {outputs.map((output) => {
              const Icon = output.height > output.width ? RectangleVertical : RectangleHorizontal;
              return (
                <Link key={`${output.run_id}-${output.format_id}`} href={completedVideoHref(workspaceId, output.run_id, output.format_id)}
                  className="flex min-w-0 flex-col gap-1 text-sm hover:text-primary">
                  <span aria-hidden="true" className="h-1 rounded-[2px]" style={{ backgroundColor: output.profile_color ?? "var(--border)" }} />
                  <Icon className="size-3.5 text-muted-foreground" aria-hidden="true" />
                  <span lang={locale} className="line-clamp-4 font-semibold [overflow-wrap:break-word] hyphens-auto">{output.title}</span>
                </Link>
              );
            })}
          </li>
        );
      })}
    </ol>
  );
}

function SectionHeader({ id, title, href, label }: { id: string; title: string; href: string; label: string }) {
  return <div className="mb-3 flex items-baseline justify-between gap-3">
    <h2 id={id} className="min-w-0 text-[clamp(1.375rem,2.2vw,1.75rem)] leading-tight text-balance">{title}</h2>
    <Link href={href} className="shrink-0 font-semibold text-primary underline underline-offset-4">{label}</Link>
  </div>;
}

/** Points a workspace with no context yet to the place where the agent can learn about the creator. */
function ContextNudge({ workspaceId }: { workspaceId: number }) {
  const { messages } = useLocale();
  const [empty, setEmpty] = useState(false);
  const [agent, setAgent] = useState<string>();
  const [agentId, setAgentId] = useState<string>();
  const agentChanges = useAgentChanges();
  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<string | null>("get_workspace_context", { workspaceId }),
      invoke<{ effective_provider: string | null }>("get_agent_settings", { workspaceId }),
      invoke<{ id: string; label: string }[]>("list_agent_plugins"),
    ]).then(([context, settings, plugins]) => {
      if (!active) return;
      setEmpty(!context?.trim() || context === messages.context.template);
      const plugin = plugins.find(({ id }) => id === settings.effective_provider);
      setAgent(plugin?.label);
      setAgentId(plugin?.id);
    }, () => {});
    return () => { active = false; };
  }, [workspaceId, messages.context.template, agentChanges]);
  if (!empty) return null;
  const name = agent ?? messages.contextResearch.agentFallback;
  return (
    <Link href={workspaceHref("context", workspaceId)}
      className="group -mt-4 flex items-center gap-4 rounded-xl border bg-card p-4 transition-colors hover:border-primary sm:p-5">
      <PluginIcon pluginId={agentId} className="size-10" />
      <span className="min-w-0 flex-1">
        <span className="block font-bold group-hover:text-primary">{messages.dashboard.contextTitle.replace("{agent}", name)}</span>
        <span className="block text-sm text-muted-foreground">{messages.dashboard.contextHint.replace("{agent}", name)}</span>
      </span>
      <ArrowRight aria-hidden="true" className="size-5 shrink-0 text-primary" />
    </Link>
  );
}
