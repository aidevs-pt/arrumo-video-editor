"use client";

import { invoke } from "@tauri-apps/api/core";
import { Layers } from "lucide-react";
import { useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useWorkspaceCatalog } from "@/components/workspace/home/workspace-router";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { ProfileFormats } from "./profile-formats";

type Profile = { id: number; name: string };

export function ProfileRoute() {
  const { workspace, navigate } = useWorkspaceCatalog();
  const query = useSearchParams();
  const { messages: { profileFormats: copy } } = useLocale();
  const rawId = query.get("profile") ?? "";
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
    return <ProfileFormats key={`${workspaceId}-${profileId}`} workspaceId={workspaceId} profile={result.profile}
      onBack={() => navigate("profiles")} onOutputs={() => navigate("outputs")} />;
  }

  return (
    <WorkspaceShell>
      <Button variant="ghost" onClick={() => navigate("profiles")}>{copy.back}</Button>
      <WorkspaceHeading icon={Layers}><h1>{copy.title}</h1></WorkspaceHeading>
      {valid && !resolved ? <p role="status">{copy.loading}</p> :
        <Alert variant="destructive"><AlertDescription>{copy.missingProfile}</AlertDescription></Alert>}
    </WorkspaceShell>
  );
}
