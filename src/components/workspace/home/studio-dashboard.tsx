"use client";

import { invoke } from "@tauri-apps/api/core";
import { CalendarDays, Film, Plus } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { runNeedsReview, RunStatus, runTitle, type Run } from "@/components/workspace/generation/generation-run-row";
import { WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { completedVideoHref, profileHref, workspaceHref, type Workspace } from "./workspace-links";

type Profile = { id: number; name: string; color: string | null };
type ScheduledOutput = { run_id: number; format_id: number; title: string; scheduled_date: string; profile_color: string | null };
type Dashboard = { runs: Run[]; profiles: Profile[]; schedule: ScheduledOutput[] };

function localDate(date: Date) {
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 10);
}

export function StudioDashboard({ workspace }: { workspace: Workspace }) {
  const { messages, locale } = useLocale();
  const copy = messages.dashboard;
  const [data, setData] = useState<Dashboard>();
  const [failed, setFailed] = useState(false);
  const workspaceId = workspace.id;

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      const today = new Date();
      const end = new Date(today.getFullYear(), today.getMonth(), today.getDate() + 6);
      try {
        const [runs, profiles, schedule] = await Promise.all([
          invoke<Run[]>("list_generation_runs", { workspaceId }),
          invoke<Profile[]>("list_profiles", { workspaceId }),
          invoke<ScheduledOutput[]>("list_scheduled_outputs", {
            workspaceId, profileId: null, start: localDate(today), end: localDate(end),
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
  const date = new Intl.DateTimeFormat(locale, { weekday: "short", day: "numeric", month: "short", timeZone: "UTC" });

  return (
    <WorkspaceShell className="flex flex-col gap-8">
      <header className="flex max-w-3xl flex-col items-start gap-3">
        <h1 className="text-[clamp(1.875rem,4vw,3rem)] leading-[1.04] font-[850] tracking-[-.015em] text-balance [font-variation-settings:'wdth'_135]">{heading}</h1>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" asChild>
            <Link href={`${workspaceHref("drafts", workspaceId)}&compose=scratch`}>{messages.videoCatalog.scratch}</Link>
          </Button>
          <Button asChild>
            <Link href={`${workspaceHref("drafts", workspaceId)}&compose=1`}><Plus data-icon="inline-start" />{messages.videoCatalog.generate}</Link>
          </Button>
        </div>
      </header>
      {failed && <Alert><AlertDescription>{messages.queue.loadError}</AlertDescription></Alert>}
      {!data && !failed && <Skeleton className="h-24 w-full" aria-label={messages.queue.loading} />}
      {data && <>
        {reviews.length > 0 && <section>
          <SectionHeader title={messages.studio.reviewInbox} href={workspaceHref("generations", workspaceId)} label={copy.viewQueue} />
          <ul className="divide-y border-y">{reviews.map((run) => <DashboardRun key={run.id} workspaceId={workspaceId} run={run} />)}</ul>
        </section>}
        <div className="grid gap-8 lg:grid-cols-2">
          <section className="min-w-0">
            <SectionHeader title={copy.inProgress} href={workspaceHref("generations", workspaceId)} label={copy.viewQueue} />
            {activeRuns.length ? <ul className="divide-y border-y">
              {activeRuns.slice(0, 4).map((run) => <DashboardRun key={run.id} workspaceId={workspaceId} run={run} />)}
            </ul> : <p className="border-t py-4 text-sm text-muted-foreground">{copy.noActiveRuns}</p>}
          </section>
          <section className="min-w-0">
            <SectionHeader title={copy.publishing} href={workspaceHref("calendar", workspaceId)} label={messages.calendar.title} />
            {data.schedule.length ? <ul className="divide-y border-y">{data.schedule.slice(0, 7).map((output) => (
              <li key={`${output.run_id}-${output.format_id}`}>
                <Link href={completedVideoHref(workspaceId, output.run_id, output.format_id)}
                  className="flex items-center gap-3 py-3 text-sm hover:text-primary">
                  <CalendarDays className="size-4 shrink-0" aria-hidden="true"
                    style={{ color: output.profile_color ?? undefined }} />
                  <time className="shrink-0 text-muted-foreground" dateTime={output.scheduled_date}>
                    {date.format(new Date(`${output.scheduled_date}T00:00:00Z`))}
                  </time>
                  <span className="font-semibold">{output.title}</span>
                </Link>
              </li>
            ))}</ul> : <p className="border-t py-4 text-sm text-muted-foreground">{messages.calendar.empty}</p>}
          </section>
        </div>
        <section>
          <SectionHeader title={messages.profiles.title} href={workspaceHref("profiles", workspaceId)} label={copy.viewProfiles} />
          {data.profiles.length ? <ul className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">{data.profiles.map((profile) => (
            <li key={profile.id}>
              <Link href={profileHref(workspaceId, profile.id)} className="flex items-center gap-2.5 rounded-lg border bg-card p-4 font-semibold hover:text-primary">
                <span aria-hidden="true" className="size-2.5 shrink-0 rounded-[2px]" style={{ backgroundColor: profile.color ?? "var(--line-strong)" }} />
                {profile.name}
              </Link>
            </li>
          ))}</ul> : <p className="border-t py-4 text-sm text-muted-foreground">{messages.profiles.empty}</p>}
        </section>
      </>}
    </WorkspaceShell>
  );
}

function SectionHeader({ title, href, label }: { title: string; href: string; label: string }) {
  return <div className="mb-2 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-0">
    <h2 className="text-[1.3125rem]">{title}</h2>
    <Button variant="link" size="sm" asChild><Link href={href}>{label}</Link></Button>
  </div>;
}

function DashboardRun({ workspaceId, run }: { workspaceId: number; run: Run }) {
  return <li><Link href={`/generation-run?workspace=${workspaceId}&run=${run.id}`}
    className="flex flex-wrap items-center gap-x-3 gap-y-1 py-3 hover:text-primary">
    <span className="grid size-10 shrink-0 place-items-center rounded-sm bg-secondary"><Film className="size-4" aria-hidden="true" /></span>
    <span className="min-w-0 flex-1 font-semibold">{runTitle(run)}</span>
    <RunStatus run={run} />
  </Link></li>;
}
