"use client";

import { ColorPicker } from "@/components/workspace/color-picker";
import { invoke } from "@tauri-apps/api/core";
import { Palette, Pencil, Upload } from "lucide-react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { WorkspaceCalendar } from "@/components/workspace/calendar/workspace-calendar";
import { firstLine } from "@/components/workspace/home/studio-dashboard";
import { useStudioTrail } from "@/components/workspace/home/studio-trail";
import { profileHref, workspaceHref } from "@/components/workspace/home/workspace-links";
import { useWorkspaceCatalog } from "@/components/workspace/home/workspace-router";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { WorkspaceMotionLibrary } from "@/components/workspace/motion-library/workspace-motion-library";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { ProfileAssets } from "./profile-assets";
import { ProfileBrand } from "./profile-brand";
import { ProfileContext } from "./profile-context";
import { ProfileFormats } from "./profile-formats";
import { ProfileVideos } from "./profile-videos";

export type Profile = { id: number; name: string; guidance: string; color: string | null; description: string };

const tabIds = ["videos", "context", "brand", "assets", "outputs", "animations", "calendar"] as const;
type ProfileTab = (typeof tabIds)[number];

export function ProfileRoute() {
  const { workspace, navigate } = useWorkspaceCatalog();
  const query = useSearchParams();
  const { messages: { profileFormats: copy } } = useLocale();
  const rawId = query.get("profile") ?? "";
  const tab = tabIds.find((id) => id === query.get("tab")) ?? "videos";
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

  if (!workspace || !workspaceId) return null;

  const resolved = result?.workspaceId === workspaceId && result.profileId === profileId;
  if (valid && resolved && result.profile) {
    const profile = result.profile;
    const update = (updated: Profile) => {
      setResult({ workspaceId, profileId, profile: updated });
      window.dispatchEvent(new Event("arrumo:profiles-changed"));
    };
    const header = <ProfileHeader workspaceId={workspaceId} profile={profile} tab={tab} onUpdate={update} />;
    if (tab === "outputs") {
      return <ProfileFormats key={`${workspaceId}-${profileId}`} workspaceId={workspaceId} profile={profile}
        header={header} onOutputs={() => navigate("outputs")} />;
    }
    const key = `${workspaceId}-${profileId}`;
    return (
      <WorkspaceShell className="max-w-7xl">
        {header}
        {tab === "videos" && <ProfileVideos key={key} workspaceId={workspaceId} profile={profile} />}
        {tab === "context" && <ProfileContext key={key} workspace={workspace} profile={profile} onUpdate={update} />}
        {tab === "brand" && <ProfileBrand key={key} workspaceId={workspaceId} profile={profile} onUpdate={update} />}
        {tab === "assets" && <ProfileAssets key={key} workspaceId={workspaceId} profile={profile} />}
        {tab === "animations" && <WorkspaceMotionLibrary key={key} workspace={workspace} profileId={profileId} />}
        {tab === "calendar" && <WorkspaceCalendar key={key}
          monthHref={(month) => `${profileHref(workspaceId, profileId)}&tab=calendar&month=${month}`} />}
      </WorkspaceShell>
    );
  }

  return (
    <WorkspaceShell>
      <WorkspaceHeading><h1>{copy.title}</h1></WorkspaceHeading>
      {valid && !resolved ? <p role="status">{copy.loading}</p> : <>
        <Alert variant="destructive"><AlertDescription>{copy.missingProfile}</AlertDescription></Alert>
        <Button variant="outline" className="mt-4" onClick={() => navigate("profiles")}>{copy.back}</Button>
      </>}
    </WorkspaceShell>
  );
}

function ProfileHeader({ workspaceId, profile, tab, onUpdate }: {
  workspaceId: number;
  profile: Profile;
  tab: ProfileTab;
  onUpdate: (profile: Profile) => void;
}) {
  const { messages } = useLocale();
  const copy = messages.profileHub;
  const href = profileHref(workspaceId, profile.id);
  const labels: Record<ProfileTab, string> = {
    videos: copy.videos, context: messages.profiles.context, brand: messages.profiles.brand,
    assets: messages.profiles.assets, outputs: messages.profiles.outputs, animations: copy.animations, calendar: copy.calendar,
  };
  useStudioTrail([{ label: profile.name, href }, { label: labels[tab] }]);

  return (
    <header className="mb-8">
      <div className="flex flex-wrap items-end justify-between gap-x-6 gap-y-4">
        <div className="min-w-0 max-w-3xl">
          <span aria-hidden="true" className="mb-3 block h-2 w-20 rounded-[2px]" style={{ backgroundColor: profile.color ?? "var(--border)" }} />
          <h1 className="text-[clamp(2.25rem,5vw,3.25rem)] leading-[1] font-[850] tracking-[-.015em] wrap-anywhere [font-variation-settings:'wdth'_135]">{profile.name}</h1>
          <ProfileDescription workspaceId={workspaceId} profile={profile} onUpdate={onUpdate} />
        </div>
        <div className="flex flex-wrap gap-3">
          <ProfileColor workspaceId={workspaceId} profile={profile} onUpdate={onUpdate} />
          <Button asChild>
            <Link href={`${workspaceHref("drafts", workspaceId, profile.id)}&compose=1`}>
              <Upload data-icon="inline-start" />{copy.generate.replace("{profile}", profile.name)}
            </Link>
          </Button>
        </div>
      </div>
      <div className="relative mt-8 before:absolute before:inset-x-0 before:bottom-0 before:h-px before:bg-border">
      <nav className="relative z-[1] flex gap-x-1 scroll-fade-x overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden" aria-label={copy.tabs}>
        {tabIds.map((id) => (
          <Link key={id} href={id === "videos" ? href : `${href}&tab=${id}`}
            aria-current={id === tab ? "page" : undefined}
            className={cn("inline-flex min-h-11 shrink-0 items-center border-b-[3px] border-transparent px-3 font-semibold text-muted-foreground transition-colors duration-150 hover:text-foreground",
              id === tab && "border-primary text-foreground")}>{labels[id]}</Link>
        ))}
      </nav>
    </div>
    </header>
  );
}

function ProfileDescription({ workspaceId, profile, onUpdate }: {
  workspaceId: number;
  profile: Profile;
  onUpdate: (profile: Profile) => void;
}) {
  const { messages: { profileHub: copy } } = useLocale();
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState(profile.description);
  const [state, setState] = useState<"idle" | "saving" | "error">("idle");
  const shown = profile.description || firstLine(profile.guidance);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setState("saving");
    try {
      onUpdate(await invoke<Profile>("set_profile_description", { workspaceId, profileId: profile.id, description: value }));
      setEditing(false);
      setState("idle");
    } catch {
      setState("error");
    }
  }

  if (editing) return (
    <form onSubmit={save} className="mt-3 flex max-w-2xl flex-wrap items-center gap-2">
      <Input autoFocus aria-label={copy.description} value={value} maxLength={280} disabled={state === "saving"}
        onChange={(event) => setValue(event.target.value)} className="min-w-64 flex-1"
        onKeyDown={(event) => { if (event.key === "Escape") setEditing(false); }} />
      <Button type="submit" disabled={state === "saving"}>{copy.saveDescription}</Button>
      <Button type="button" variant="ghost" onClick={() => setEditing(false)}>{copy.cancel}</Button>
      {state === "error" && <p role="alert" className="w-full text-sm text-destructive">{copy.saveError}</p>}
    </form>
  );

  return (
    <button type="button" onClick={() => { setValue(profile.description || shown); setEditing(true); }}
      className="group mt-3 flex max-w-2xl items-baseline gap-2 text-start text-lg text-muted-foreground hover:text-foreground"
      aria-label={shown ? `${copy.editDescription}: ${shown}` : copy.addDescription}>
      <span className={cn(!shown && "underline decoration-dotted underline-offset-4")}>{shown || copy.addDescription}</span>
      <Pencil aria-hidden="true" className="size-3.5 shrink-0 opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100" />
    </button>
  );
}

function ProfileColor({ workspaceId, profile, onUpdate }: {
  workspaceId: number;
  profile: Profile;
  onUpdate: (profile: Profile) => void;
}) {
  const { messages: { profileHub: copy } } = useLocale();
  const [failed, setFailed] = useState(false);

  async function choose(color: string | null) {
    setFailed(false);
    try {
      onUpdate(await invoke<Profile>("update_profile", {
        workspaceId, profileId: profile.id, name: profile.name, guidance: profile.guidance, color,
      }));
    } catch {
      setFailed(true);
    }
  }

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline">
          <Palette data-icon="inline-start" />{copy.color}
          <span aria-hidden="true" className="size-3 rounded-[3px] border" style={{ backgroundColor: profile.color ?? "transparent" }} />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-72">
        <p className="font-semibold">{copy.colorTitle}</p>
        <p className="mb-3 text-sm text-muted-foreground">{copy.colorHint}</p>
        <ColorPicker value={profile.color ?? null} onChange={(color) => void choose(color)}
          label={copy.colorTitle} customLabel={copy.customColor} noneLabel={copy.noColor} />
        {failed && <p role="alert" className="mt-2 text-sm text-destructive">{copy.saveError}</p>}
      </PopoverContent>
    </Popover>
  );
}
