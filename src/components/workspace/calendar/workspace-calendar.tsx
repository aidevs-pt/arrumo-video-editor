"use client";

import { invoke } from "@tauri-apps/api/core";
import { CalendarDays, ChevronLeft, ChevronRight, RectangleHorizontal, RectangleVertical, Square } from "lucide-react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { calendarHref, completedVideoHref, isCalendarMonth } from "@/components/workspace/home/workspace-links";
import { useWorkspaceCatalog } from "@/components/workspace/home/workspace-router";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";

type Profile = { id: number; name: string };
type Output = {
  run_id: number;
  format_id: number;
  scheduled_date: string;
  title: string;
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

export function WorkspaceCalendar() {
  const { workspace, draftProfile } = useWorkspaceCatalog();
  const query = useSearchParams();
  if (!workspace) return null;
  return <CalendarContent key={workspace.id}
    workspaceId={workspace.id} profileId={draftProfile} monthQuery={query.get("month")} />;
}

function CalendarContent({ workspaceId, profileId, monthQuery }: {
  workspaceId: number;
  profileId: number | null;
  monthQuery: string | null;
}) {
  const { locale, messages } = useLocale();
  const copy = messages.calendar;
  const router = useRouter();
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const month = monthStart(monthQuery);
  const [retry, setRetry] = useState(0);
  const [results, setResults] = useState<{ key: string; rows?: Output[] }>();
  const year = month.getUTCFullYear();
  const monthIndex = month.getUTCMonth();
  const start = asDate(month);
  const end = asDate(utcDate(year, monthIndex + 1, 0));
  const key = `${start}-${profileId}-${retry}`;
  const rows = results?.key === key ? results.rows : undefined;
  const failed = results?.key === key && !results.rows;
  const firstColumn = (month.getUTCDay() + 6) % 7;
  const monthLabel = new Intl.DateTimeFormat(locale, { month: "long", year: "numeric", timeZone: "UTC" }).format(month);

  function showMonth(offset: number) {
    const next = asDate(utcDate(year, monthIndex + offset, 1)).slice(0, 7);
    router.push(calendarHref(workspaceId, profileId ?? undefined, next), { scroll: false });
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
    invoke<Output[]>("list_scheduled_outputs", { workspaceId, profileId, start, end }).then(
      (entries) => { if (active) setResults({ key, rows: entries }); },
      () => { if (active) setResults({ key }); },
    );
    return () => { active = false; };
  }, [workspaceId, profileId, start, end, key]);

  const byDate = new Map<string, Output[]>();
  for (const row of rows ?? []) {
    const group = byDate.get(row.scheduled_date);
    if (group) group.push(row);
    else byDate.set(row.scheduled_date, [row]);
  }
  const days = Array.from({ length: Number(end.slice(-2)) }, (_, index) =>
    asDate(utcDate(year, monthIndex, index + 1)));
  const weekday = new Intl.DateTimeFormat(locale, { weekday: "short", timeZone: "UTC" });
  const dayLabel = new Intl.DateTimeFormat(locale, { dateStyle: "full", timeZone: "UTC" });

  return (
    <WorkspaceShell>
      <WorkspaceHeading icon={CalendarDays}><h1>{copy.title}</h1></WorkspaceHeading>
      <Card>
        <CardHeader className="gap-3">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <CardTitle className="capitalize">{monthLabel}</CardTitle>
            <div className="flex gap-1">
              <Button variant="outline" size="icon-sm" aria-label={copy.previousMonth}
                disabled={year === 1 && monthIndex === 0}
                onClick={() => showMonth(-1)}><ChevronLeft /></Button>
              <Button variant="outline" size="icon-sm" aria-label={copy.nextMonth}
                disabled={year >= 9999 && monthIndex === 11}
                onClick={() => showMonth(1)}><ChevronRight /></Button>
            </div>
          </div>
          <Select value={profileId === null ? "all" : String(profileId)}
            onValueChange={(value) => router.push(calendarHref(workspaceId, value === "all" ? undefined : Number(value), start.slice(0, 7)), { scroll: false })}>
            <SelectTrigger aria-label={copy.profileFilter} className="w-full sm:w-56"><SelectValue /></SelectTrigger>
            <SelectContent><SelectGroup>
              <SelectItem value="all">{copy.allProfiles}</SelectItem>
              {profiles.map((profile) => <SelectItem key={profile.id} value={String(profile.id)}>{profile.name}</SelectItem>)}
            </SelectGroup></SelectContent>
          </Select>
        </CardHeader>
        <CardContent>
          {failed ? <Alert variant="destructive">
            <AlertDescription>{copy.loadError}</AlertDescription>
            <Button size="sm" variant="outline" onClick={() => setRetry((current) => current + 1)}>{messages.queue.retry}</Button>
          </Alert> : !rows ? <Skeleton className="min-h-48" aria-label={copy.loading} /> : (
            <>
              <div className="hidden overflow-hidden rounded-md border lg:grid lg:grid-cols-7">
                {Array.from({ length: 7 }, (_, index) => (
                  <div key={index} className="border-b bg-muted px-2 py-1.5 text-sm font-medium">
                    {weekday.format(new Date(Date.UTC(2024, 0, index + 1)))}
                  </div>
                ))}
                {Array.from({ length: firstColumn }, (_, index) =>
                  <div key={`blank-${index}`} aria-hidden="true" className="border-e border-b bg-muted/30" />)}
                {days.map((date) => <section key={date} aria-label={dayLabel.format(new Date(date))}
                  className="min-h-20 min-w-0 border-e border-b p-2">
                  <time dateTime={date} className="text-xs tabular-nums text-muted-foreground">{Number(date.slice(-2))}</time>
                  <OutputList rows={byDate.get(date) ?? []} workspaceId={workspaceId} />
                </section>)}
              </div>
              <div className="flex flex-col gap-4 lg:hidden">
                {[...byDate].map(([date, group]) => <section key={date} className="flex flex-col gap-1">
                  <h2 className="text-sm font-medium"><time dateTime={date}>{dayLabel.format(new Date(date))}</time></h2>
                  <OutputList rows={group} workspaceId={workspaceId} />
                </section>)}
              </div>
              {rows.length === 0 && <Empty><EmptyHeader>
                <EmptyTitle>{copy.empty}</EmptyTitle><EmptyDescription>{copy.emptyHint}</EmptyDescription>
              </EmptyHeader></Empty>}
            </>
          )}
        </CardContent>
      </Card>
    </WorkspaceShell>
  );
}

function OutputList({ rows, workspaceId }: { rows: Output[]; workspaceId: number }) {
  if (rows.length === 0) return null;
  return <ul className="flex min-w-0 flex-col gap-1.5">
    {rows.map((row) => {
      const Orientation = row.width === row.height ? Square
        : row.width > row.height ? RectangleHorizontal : RectangleVertical;
      const color = row.profile_color && /^#[0-9a-f]{6}$/i.test(row.profile_color) ? row.profile_color : undefined;
      return <li key={`${row.run_id}-${row.format_id}`} className="min-w-0">
        <Link href={completedVideoHref(workspaceId, row.run_id, row.format_id)}
          className="flex min-w-0 items-start gap-1.5 rounded-md border bg-card p-1.5 text-xs hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
          style={color ? { borderInlineStartColor: color, borderInlineStartWidth: 3 } : undefined}>
          <Orientation aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
          <span className="flex min-w-0 flex-col">
            <span className="wrap-anywhere font-medium">{row.title}</span>
            <span className="wrap-anywhere text-muted-foreground">{row.format_name}</span>
          </span>
        </Link>
      </li>;
    })}
  </ul>;
}
