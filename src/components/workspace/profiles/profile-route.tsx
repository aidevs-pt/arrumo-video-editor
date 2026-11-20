"use client";

import { invoke } from "@tauri-apps/api/core";
import { Film, Plus } from "lucide-react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useEffect, useState, type FormEvent } from "react";
import { MarkdownEditor } from "@/components/markdown/editor";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldLabel } from "@/components/ui/field";
import { Skeleton } from "@/components/ui/skeleton";
import { RunStatus, runTitle, type Run } from "@/components/workspace/generation/generation-run-row";
import { completedVideoHref, profileHref, workspaceHref } from "@/components/workspace/home/workspace-links";
import { useWorkspaceCatalog } from "@/components/workspace/home/workspace-router";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { ProfileFormats } from "./profile-formats";

type Profile = { id: number; name: string; guidance: string; color: string | null };
type ProfileTab = "videos" | "context" | "outputs";
type ProfileRun = Run & { snapshot: Run["snapshot"] & { profile?: { id: number } | null } };

export function ProfileRoute() {
  const { workspace, navigate } = useWorkspaceCatalog();
  const query = useSearchParams();
  const { messages: { profileFormats: copy } } = useLocale();
  const rawId = query.get("profile") ?? "";
  const tabQuery = query.get("tab");
  const tab = tabQuery === "context" || tabQuery === "outputs" ? tabQuery : "videos";
  const profileId = Number(rawId);
  const workspaceId = workspace?.id;
  const valid = /^[1-9]\d*$/.test(rawId) && Number.isSafeInteger(profileId);
  const [result, setResult] = useState<{ workspaceId: number; profileId: number; profile?: Profile }>();

  useEffect(() => {
    if (!workspaceId || !valid) return;
    let active = true;
    invoke<Profile[]>("list_profiles", { workspaceId }).then(
      (profiles) => {
        if (active) setResult({ workspaceId, profileId, profile: profiles.find(({ id }) => id === profileId) });
      },
      () => { if (active) setResult({ workspaceId, profileId }); },
    );
    return () => { active = false; };
  }, [profileId, valid, workspaceId]);

  if (!workspaceId) return null;

  const resolved = result?.workspaceId === workspaceId && result.profileId === profileId;
  if (valid && resolved && result.profile) {
    const profile = result.profile;
    const header = <ProfileHeader workspaceId={workspaceId} profile={profile} tab={tab} />;
    if (tab === "outputs") {
      return <ProfileFormats key={`${workspaceId}-${profileId}`} workspaceId={workspaceId} profile={profile}
        header={header} onOutputs={() => navigate("outputs")} />;
    }
    return (
      <WorkspaceShell>
        {header}
        {tab === "context" ? <ProfileContext key={`${workspaceId}-${profileId}`} workspaceId={workspaceId} profile={profile}
          onSave={(updated) => setResult({ workspaceId, profileId, profile: updated })} />
          : <ProfileVideos key={`${workspaceId}-${profileId}`} workspaceId={workspaceId} profileId={profileId} />}
      </WorkspaceShell>
    );
  }

  return (
    <WorkspaceShell>
      <Button variant="ghost" onClick={() => navigate("profiles")}>{copy.back}</Button>
      <WorkspaceHeading><h1>{copy.title}</h1></WorkspaceHeading>
      {valid && !resolved ? <p role="status">{copy.loading}</p> :
        <Alert variant="destructive"><AlertDescription>{copy.missingProfile}</AlertDescription></Alert>}
    </WorkspaceShell>
  );
}

function ProfileHeader({ workspaceId, profile, tab }: { workspaceId: number; profile: Profile; tab: ProfileTab }) {
  const { messages } = useLocale();
  const href = profileHref(workspaceId, profile.id);
  const tabs = [
    { id: "videos", label: messages.videoCatalog.title },
    { id: "context", label: messages.profiles.context },
    { id: "outputs", label: messages.profiles.outputs },
    { id: "calendar", label: messages.calendar.title },
  ] as const;

  return (
    <header className="mb-5">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <WorkspaceHeading>
          <h1 className="flex items-baseline gap-2">
            {profile.color && <span className="size-3.5 shrink-0 rounded-[2px]" style={{ backgroundColor: profile.color }} aria-hidden="true" />}
            <span className="min-w-0 wrap-anywhere">{profile.name}</span>
          </h1>
        </WorkspaceHeading>
        <Button asChild>
          <Link href={`${workspaceHref("drafts", workspaceId, profile.id)}&compose=1`}>
            <Plus data-icon="inline-start" />{messages.videoCatalog.generate}
          </Link>
        </Button>
      </div>
      <nav className="flex flex-wrap gap-x-5 border-b" aria-label={profile.name}>
        {tabs.map(({ id, label }) => (
          <Link key={id}
            href={id === "calendar" ? workspaceHref("calendar", workspaceId, profile.id) : `${href}&tab=${id}`}
            aria-current={id === tab ? "page" : undefined}
            className={cn("inline-flex min-h-10 items-center border-b-2 border-transparent px-1 text-sm font-semibold text-muted-foreground transition-colors hover:text-foreground",
              id === tab && "border-primary text-foreground")}>{label}</Link>
        ))}
      </nav>
    </header>
  );
}

function ProfileVideos({ workspaceId, profileId }: { workspaceId: number; profileId: number }) {
  const { messages } = useLocale();
  const [runs, setRuns] = useState<ProfileRun[]>();
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;
    invoke<ProfileRun[]>("list_generation_runs", { workspaceId }).then(
      (rows) => {
        if (active) setRuns(rows.filter((run) => run.snapshot.profile?.id === profileId).reverse());
      },
      () => { if (active) setFailed(true); },
    );
    return () => { active = false; };
  }, [attempt, profileId, workspaceId]);

  if (failed) return (
    <Alert variant="destructive">
      <AlertDescription>{messages.queue.loadError}</AlertDescription>
      <Button type="button" variant="outline" onClick={() => {
        setFailed(false);
        setAttempt((value) => value + 1);
      }}>{messages.queue.retry}</Button>
    </Alert>
  );

  if (!runs) return <Skeleton className="h-36" role="status" aria-label={messages.queue.loading} />;
  if (!runs.length) return (
    <Empty>
      <EmptyHeader>
        <EmptyMedia variant="icon"><Film /></EmptyMedia>
        <EmptyTitle>{messages.queue.empty}</EmptyTitle>
        <EmptyDescription>{messages.queue.emptyDescription}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );

  return (
    <ul className="divide-y border-y" aria-label={messages.videoCatalog.title}>
      {runs.map((run) => (
        <li key={run.id}>
          <Link href={run.state === "completed" ? completedVideoHref(workspaceId, run.id)
            : `/generation-run?workspace=${workspaceId}&run=${run.id}`}
            className="flex items-center gap-3 px-1 py-3 hover:bg-muted/60">
            <span className="grid aspect-video w-22 shrink-0 place-items-center rounded-sm bg-muted text-muted-foreground" aria-hidden="true"><Film className="size-4" /></span>
            <span className="min-w-0 flex-1 wrap-anywhere font-semibold">{runTitle(run)}</span>
            <RunStatus run={run} />
          </Link>
        </li>
      ))}
    </ul>
  );
}

function ProfileContext({ workspaceId, profile, onSave }: {
  workspaceId: number;
  profile: Profile;
  onSave: (profile: Profile) => void;
}) {
  const { messages: { profiles: copy } } = useLocale();
  const [guidance, setGuidance] = useState(profile.guidance);
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (status === "saving" || guidance === profile.guidance) return;
    setStatus("saving");
    try {
      onSave(await invoke<Profile>("update_profile", {
        workspaceId, profileId: profile.id, name: profile.name, color: profile.color, guidance,
      }));
      setStatus("saved");
    } catch {
      setStatus("error");
    }
  }

  return (
    <form onSubmit={save} className="flex max-w-3xl flex-col items-start gap-4">
      <Field>
        <FieldLabel htmlFor="profile-context-guidance">{copy.guidance}</FieldLabel>
        <MarkdownEditor id="profile-context-guidance" label={copy.guidance} value={guidance}
          disabled={status === "saving"} onChange={(value) => { setGuidance(value); setStatus("idle"); }} />
      </Field>
      {status === "error" && <Alert variant="destructive"><AlertDescription>{copy.updateError}</AlertDescription></Alert>}
      {status === "saved" && <p role="status" className="text-sm text-muted-foreground">{copy.saved}</p>}
      <Button type="submit" disabled={status === "saving" || guidance === profile.guidance}>{copy.save}</Button>
    </form>
  );
}
