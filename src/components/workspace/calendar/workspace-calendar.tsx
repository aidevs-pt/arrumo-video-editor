"use client";

import { invoke } from "@tauri-apps/api/core";
import { ChevronLeft, ChevronRight, GripVertical, RectangleHorizontal, RectangleVertical, Square } from "lucide-react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { Fragment, useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Checkbox } from "@/components/ui/checkbox";
import { Skeleton } from "@/components/ui/skeleton";
import { calendarHref, completedVideoHref, isCalendarMonth } from "@/components/workspace/home/workspace-links";
import { useWorkspaceCatalog } from "@/components/workspace/home/workspace-router";
import { WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

type Profile = { id: number; name: string; color: string | null };
type Output = {
  run_id: number;
  format_id: number;
  scheduled_date: string;
  title: string;
  profile_id: number | null;
  profile_color: string | null;
  format_name: string;
  width: number;
  height: number;
};

function utcDate(year: number, month: number, day: number) {
  const date = new Date(0);
  date.setUTCFullYear(year, month, day);
  return date;
}

function monthStart(value: string | null) {
  if (isCalendarMonth(value)) return new Date(`${value}-01T00:00:00Z`);
  const now = new Date();
  return utcDate(now.getFullYear(), now.getMonth(), 1);
}

function asDate(day: Date) {
  return day.toISOString().slice(0, 10);
}

/** `monthHref` keeps month navigation inside the page that embeds the calendar. */
export function WorkspaceCalendar({ monthHref }: { monthHref?: (month: string) => string } = {}) {
  const { workspace, draftProfile } = useWorkspaceCatalog();
  const query = useSearchParams();
  if (!workspace) return null;
  return <CalendarContent key={workspace.id} monthHref={monthHref}
    workspaceId={workspace.id} profileId={draftProfile} monthQuery={query.get("month")} />;
}

type Run = { id: number; state: string; display_title: string | null;
  snapshot: { draft: { title: string }; profile?: { id: number; color: string | null } | null;
    formats?: { format: { id: number; name: string; width: number; height: number } }[] } };
type Unscheduled = { run_id: number; format_id: number; title: string; format_name: string; profile_id: number | null; profile_color: string | null; portrait: boolean };
type Orientation = "landscape" | "portrait";

const isPortrait = (output: { width: number; height: number }) => output.height > output.width;

function CalendarContent({ workspaceId, profileId, monthQuery, monthHref }: {
  workspaceId: number;
  profileId: number | null;
  monthQuery: string | null;
  monthHref?: (month: string) => string;
}) {
  const { locale, messages } = useLocale();
  const { workspace } = useWorkspaceCatalog();
  const copy = messages.calendar;
  const router = useRouter();
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const month = monthStart(monthQuery);
  const [retry, setRetry] = useState(0);
  const [results, setResults] = useState<{ key: string; rows?: Output[]; unscheduled?: Unscheduled[] }>();
  const [hiddenProfiles, setHiddenProfiles] = useState<number[]>([]);
  const [hiddenOrientations, setHiddenOrientations] = useState<Orientation[]>([]);
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const [scheduleError, setScheduleError] = useState(false);
  const year = month.getUTCFullYear();
  const monthIndex = month.getUTCMonth();
  const firstColumn = (month.getUTCDay() + 6) % 7;
  const gridStart = utcDate(year, monthIndex, 1 - firstColumn);
  const lastDay = utcDate(year, monthIndex + 1, 0);
  const cellCount = Math.ceil((firstColumn + lastDay.getUTCDate()) / 7) * 7;
  const cells = Array.from({ length: cellCount }, (_, index) =>
    asDate(utcDate(gridStart.getUTCFullYear(), gridStart.getUTCMonth(), gridStart.getUTCDate() + index)));
  const key = `${cells[0]}-${profileId}-${retry}`;
  const rows = results?.key === key ? results.rows : undefined;
  const unscheduled = results?.key === key ? results.unscheduled ?? [] : [];
  const failed = results?.key === key && !results.rows;
  const now = new Date();
  const today = asDate(utcDate(now.getFullYear(), now.getMonth(), now.getDate()));
  // Only the first letter: "capitalize" would also turn Spanish "de" into "De".
  const monthName = new Intl.DateTimeFormat(locale, { month: "long", year: "numeric", timeZone: "UTC" }).format(month);
  const monthLabel = monthName.charAt(0).toLocaleUpperCase(locale) + monthName.slice(1);
  const profileName = profiles.find((profile) => profile.id === profileId)?.name;

  function showMonth(offset: number) {
    const next = asDate(utcDate(year, monthIndex + offset, 1)).slice(0, 7);
    router.push(monthHref?.(next) ?? calendarHref(workspaceId, profileId ?? undefined, next), { scroll: false });
  }

  useEffect(() => {
    let active = true;
    invoke<Profile[]>("list_profiles", { workspaceId }).then(
      (entries) => { if (active) setProfiles(entries); },
      () => {},
    );
    return () => { active = false; };
  }, [workspaceId]);

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Output[]>("list_scheduled_outputs", { workspaceId, profileId, start: cells[0], end: cells[cells.length - 1] }),
      invoke<Output[]>("list_scheduled_outputs", { workspaceId, profileId, start: "0001-01-01", end: "9999-12-31" }),
      invoke<Run[]>("list_generation_runs", { workspaceId }),
    ]).then(([entries, everything, runs]) => {
      if (!active) return;
      const dated = new Set(everything.map((output) => `${output.run_id}-${output.format_id}`));
      const pending = runs.filter((run) => run.state === "completed" && (profileId === null || run.snapshot.profile?.id === profileId))
        .flatMap((run) => (run.snapshot.formats ?? []).map(({ format }) => ({
          run_id: run.id, format_id: format.id, title: run.display_title ?? run.snapshot.draft.title, format_name: format.name,
          profile_id: run.snapshot.profile?.id ?? null, profile_color: run.snapshot.profile?.color ?? null, portrait: isPortrait(format),
        })))
        .filter((output) => !dated.has(`${output.run_id}-${output.format_id}`));
      setResults({ key, rows: entries, unscheduled: pending });
    }, () => { if (active) setResults({ key }); });
    return () => { active = false; };
    // cells are derived from the key
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspaceId, profileId, key]);

  async function schedule(payload: string, date: string) {
    const [runId, formatId] = payload.split("-").map(Number);
    if (!runId || !formatId) return;
    setScheduleError(false);
    try {
      await invoke("set_output_schedule", { workspaceId, runId, formatId, date });
      setRetry((value) => value + 1);
    } catch {
      setScheduleError(true);
    }
  }

  const visible = (rows ?? []).filter((row) => !hiddenOrientations.includes(isPortrait(row) ? "portrait" : "landscape")
    && !(row.profile_id !== null && hiddenProfiles.includes(row.profile_id)));
  const byDate = new Map<string, Output[]>();
  for (const row of visible) byDate.set(row.scheduled_date, [...(byDate.get(row.scheduled_date) ?? []), row]);
  const weekday = new Intl.DateTimeFormat(locale, { weekday: "short", timeZone: "UTC" });
  const dayLabel = new Intl.DateTimeFormat(locale, { dateStyle: "full", timeZone: "UTC" });
  const toggle = <T,>(list: T[], value: T) => list.includes(value) ? list.filter((entry) => entry !== value) : [...list, value];

  const Shell = monthHref ? Fragment : WorkspaceShell;
  return (
    <Shell>
      <section aria-label={copy.title}>
        <header className="mb-6 flex flex-wrap items-end justify-between gap-4">
          <div>
            <h1 className={cn("leading-[1] font-[850] [font-variation-settings:'wdth'_135]", monthHref ? "text-[2rem]" : "text-[clamp(2.25rem,5vw,3.25rem)]")}>{monthLabel}</h1>
            <p className="mt-2 text-muted-foreground">{profileName && monthHref ? copy.profileIntro.replace("{profile}", profileName)
              : copy.intro.replace("{workspace}", workspace?.name ?? "")}</p>
          </div>
          <div className="flex gap-2">
            <Button variant="outline" size="icon" aria-label={copy.previousMonth} disabled={year === 1 && monthIndex === 0}
              onClick={() => showMonth(-1)}><ChevronLeft /></Button>
            <Button variant="outline" onClick={() => router.push(
              monthHref?.(today.slice(0, 7)) ?? calendarHref(workspaceId, profileId ?? undefined, today.slice(0, 7)), { scroll: false },
            )}>{copy.today}</Button>
            <Button variant="outline" size="icon" aria-label={copy.nextMonth} disabled={year >= 9999 && monthIndex === 11}
              onClick={() => showMonth(1)}><ChevronRight /></Button>
          </div>
        </header>
        <div role="group" aria-label={copy.filters} className="mb-6 flex flex-wrap items-center gap-2">
          {!monthHref && profiles.map((profile) => (
            <label key={profile.id} className="flex cursor-pointer items-center gap-2 rounded-lg border bg-card px-3 py-1.5 font-semibold">
              <Checkbox checked={!hiddenProfiles.includes(profile.id)} onCheckedChange={() => setHiddenProfiles((list) => toggle(list, profile.id))} />
              <span aria-hidden="true" className="size-3 rounded-[3px]" style={{ backgroundColor: profile.color ?? "var(--border)" }} />{profile.name}
            </label>
          ))}
          {!monthHref && profiles.length > 0 && <span aria-hidden="true" className="mx-1 h-6 w-px bg-border" />}
          {(["landscape", "portrait"] as const).map((orientation) => {
            const Icon = orientation === "portrait" ? RectangleVertical : RectangleHorizontal;
            return (
              <label key={orientation} className="flex cursor-pointer items-center gap-2 rounded-lg border bg-card px-3 py-1.5 font-semibold">
                <Checkbox checked={!hiddenOrientations.includes(orientation)} onCheckedChange={() => setHiddenOrientations((list) => toggle(list, orientation))} />
                <Icon className="size-4" aria-hidden="true" />{copy[orientation]}
              </label>
            );
          })}
        </div>
        {scheduleError && <Alert variant="destructive" className="mb-4"><AlertDescription>{copy.scheduleError}</AlertDescription></Alert>}
        {failed ? <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button size="sm" variant="outline" onClick={() => setRetry((current) => current + 1)}>{messages.queue.retry}</Button>
        </Alert> : !rows ? <Skeleton className="min-h-96" aria-label={copy.loading} /> : (
          <div className="grid items-start gap-6 2xl:grid-cols-[minmax(0,1fr)_18rem]">
            <div className="min-w-0">
              <div className="hidden overflow-hidden rounded-xl border bg-card lg:grid lg:grid-cols-7">
                {Array.from({ length: 7 }, (_, index) => (
                  <div key={index} className="border-b px-3 py-2 text-sm text-muted-foreground">
                    {weekday.format(new Date(Date.UTC(2024, 0, index + 1)))}
                  </div>
                ))}
                {cells.map((date, index) => {
                  const outside = date.slice(0, 7) !== asDate(month).slice(0, 7);
                  return (
                    <section key={date} aria-label={dayLabel.format(new Date(date))}
                      onDragOver={(event) => { event.preventDefault(); setDropTarget(date); }}
                      onDragLeave={() => setDropTarget((current) => current === date ? null : current)}
                      onDrop={(event) => {
                        event.preventDefault();
                        setDropTarget(null);
                        void schedule(event.dataTransfer.getData("text/plain"), date);
                      }}
                      className={cn("min-h-28 min-w-0 border-b p-2 transition-colors duration-150", (index + 1) % 7 !== 0 && "border-e",
                        outside && "bg-secondary/60", dropTarget === date && "bg-accent")}>
                      <time dateTime={date} aria-current={date === today ? "date" : undefined}
                        className={cn("mb-1.5 inline-grid size-7 place-items-center rounded-full text-sm tabular-nums aria-[current=date]:bg-primary aria-[current=date]:font-bold aria-[current=date]:text-primary-foreground",
                          outside ? "text-muted-foreground/70" : "text-muted-foreground")}>{Number(date.slice(-2))}</time>
                      <OutputList rows={byDate.get(date) ?? []} workspaceId={workspaceId} />
                    </section>
                  );
                })}
              </div>
              <div className="flex flex-col gap-4 lg:hidden">
                {[...byDate].map(([date, group]) => <section key={date} className="flex flex-col gap-1">
                  <h2 className="text-sm font-medium"><time dateTime={date}>{dayLabel.format(new Date(date))}</time></h2>
                  <OutputList rows={group} workspaceId={workspaceId} />
                </section>)}
              </div>
              {rows.length === 0 && unscheduled.length === 0 && <Empty><EmptyHeader>
                <EmptyTitle>{copy.empty}</EmptyTitle><EmptyDescription>{copy.emptyHint}</EmptyDescription>
              </EmptyHeader></Empty>}
            </div>
            <aside aria-labelledby="calendar-unscheduled" className="flex max-w-xl flex-col gap-2 2xl:sticky 2xl:top-20">
              <div className="flex items-center justify-between">
                <h2 id="calendar-unscheduled" className="text-[1.125rem] font-bold">{copy.unscheduled}</h2>
                <span className="rounded-[5px] bg-secondary px-2 text-sm font-bold tabular-nums">{unscheduled.length}</span>
              </div>
              <p className="text-sm text-muted-foreground">{unscheduled.length ? copy.unscheduledHint : copy.allScheduled}</p>
              {unscheduled.length > 0 && <ul className="divide-y rounded-xl border bg-card">
                {unscheduled.map((output) => {
                  const Icon = output.portrait ? RectangleVertical : RectangleHorizontal;
                  return (
                    <li key={`${output.run_id}-${output.format_id}`} draggable
                      onDragStart={(event) => { event.dataTransfer.setData("text/plain", `${output.run_id}-${output.format_id}`); event.dataTransfer.effectAllowed = "move"; }}
                      className="flex cursor-grab items-center gap-2 px-3 py-2.5 active:cursor-grabbing">
                      <GripVertical className="size-4 shrink-0 text-muted-foreground" aria-label={copy.dragLabel.replace("{title}", output.title)} />
                      <Icon className="size-4 shrink-0" style={{ color: output.profile_color ?? undefined }} aria-hidden="true" />
                      <Link href={completedVideoHref(workspaceId, output.run_id, output.format_id)} className="min-w-0 flex-1 truncate hover:text-primary">
                        {output.title}, {output.format_name}
                      </Link>
                    </li>
                  );
                })}
              </ul>}
            </aside>
          </div>
        )}
      </section>
    </Shell>
  );
}

function OutputList({ rows, workspaceId }: { rows: Output[]; workspaceId: number }) {
  if (rows.length === 0) return null;
  return <ul className="flex min-w-0 flex-col gap-1">
    {rows.map((row) => {
      const Orientation = row.width === row.height ? Square : isPortrait(row) ? RectangleVertical : RectangleHorizontal;
      const color = row.profile_color && /^#[0-9a-f]{6}$/i.test(row.profile_color) ? row.profile_color : undefined;
      return <li key={`${row.run_id}-${row.format_id}`} className="min-w-0" draggable
        onDragStart={(event) => event.dataTransfer.setData("text/plain", `${row.run_id}-${row.format_id}`)}>
        <Link href={completedVideoHref(workspaceId, row.run_id, row.format_id)} title={`${row.title}, ${row.format_name}`}
          className="flex min-w-0 items-center gap-1.5 rounded-md px-2 py-1 text-sm hover:brightness-95 focus-visible:outline-2 focus-visible:outline-ring"
          style={{ backgroundColor: color ? `color-mix(in oklch, ${color} 18%, var(--card))` : "var(--secondary)" }}>
          <Orientation aria-hidden="true" className="size-3.5 shrink-0" style={{ color }} />
          <span className="truncate">{row.title}</span>
        </Link>
      </li>;
    })}
  </ul>;
}
