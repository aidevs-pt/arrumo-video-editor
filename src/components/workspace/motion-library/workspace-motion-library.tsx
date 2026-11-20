"use client";

import { invoke } from "@tauri-apps/api/core";
import { Layers, Plus } from "lucide-react";
import Link from "next/link";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Textarea } from "@/components/ui/textarea";
import { AssetThumbnail } from "@/components/workspace/assets/asset-thumbnail";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { workspaceHref, type Workspace } from "@/components/workspace/home/workspace-links";
import { useLocale } from "@/lib/locale-provider";

type Kind = "animation" | "transition";
type Settings = {
  kind: Kind; title: string; usage_markdown: string; profile_id: number | null;
  preview_asset_id: number; thumbnail_asset_id: number | null; sound_asset_id: number | null;
};
type Item = { id: number; settings: Settings };
type Asset = { id: number; name: string; kind: "video" | "image" | "audio" };
type Profile = { id: number; name: string };

export function WorkspaceMotionLibrary({ workspace }: { workspace: Workspace }) {
  const { messages } = useLocale();
  const copy = messages.motionLibrary;
  const [catalog, setCatalog] = useState<{ assets: Asset[]; profiles: Profile[] }>();
  const [items, setItems] = useState<Item[]>();
  const [scope, setScope] = useState("");
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [open, setOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const workspaceId = workspace.id;

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Asset[]>("list_assets", { workspaceId }),
      invoke<Profile[]>("list_profiles", { workspaceId }),
      invoke<Item[]>("list_motion_library", { workspaceId, profileId: scope ? Number(scope) : null }),
    ]).then(([assets, profiles, entries]) => {
      if (!active) return;
      setCatalog({ assets, profiles });
      setItems(entries);
    }, () => { if (active) setLoadError(true); });
    return () => { active = false; };
  }, [workspaceId, scope, attempt]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (saving) return;
    const form = new FormData(event.currentTarget);
    const settings: Settings = {
      title: String(form.get("title")).trim(),
      kind: form.get("kind") === "transition" ? "transition" : "animation",
      usage_markdown: String(form.get("usage")),
      profile_id: scope ? Number(scope) : null,
      preview_asset_id: Number(form.get("video")),
      thumbnail_asset_id: Number(form.get("image")) || null,
      sound_asset_id: Number(form.get("audio")) || null,
    };
    if (!settings.title || !settings.preview_asset_id) {
      setSaveError(true);
      return;
    }
    setSaving(true);
    setSaveError(false);
    try {
      const item = await invoke<Item>("create_motion_library_item", { workspaceId, settings });
      setItems((current) => [...(current ?? []), item]);
      setOpen(false);
    } catch {
      setSaveError(true);
    } finally {
      setSaving(false);
    }
  }

  const videos = catalog?.assets.filter(({ kind }) => kind === "video") ?? [];

  return (
    <WorkspaceShell>
      <WorkspaceHeading icon={Layers}>
        <h1>{copy.title}</h1>
        <p>{copy.intro}</p>
      </WorkspaceHeading>
      <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
        <Field className="w-auto">
          <FieldLabel htmlFor="motion-scope">{copy.scope}</FieldLabel>
          <NativeSelect id="motion-scope" value={scope} disabled={!catalog || open}
            onChange={(event) => {
              setItems(undefined);
              setLoadError(false);
              setScope(event.target.value);
            }}>
            <NativeSelectOption value="">{copy.workspaceScope}</NativeSelectOption>
            {catalog?.profiles.map((profile) => <NativeSelectOption key={profile.id} value={profile.id}>{profile.name}</NativeSelectOption>)}
          </NativeSelect>
        </Field>
        <Dialog open={open} onOpenChange={(next) => {
          if (saving) return;
          setSaveError(false);
          setOpen(next);
        }}>
          <DialogTrigger asChild>
            <Button disabled={!items || !videos.length}><Plus data-icon="inline-start" />{copy.add}</Button>
          </DialogTrigger>
          <DialogContent className="max-h-[85dvh] overflow-y-auto">
            <DialogHeader><DialogTitle>{copy.add}</DialogTitle><DialogDescription>{copy.addDescription}</DialogDescription></DialogHeader>
            <form className="flex flex-col gap-4" onSubmit={save}>
              <Field><FieldLabel htmlFor="motion-title">{copy.name}</FieldLabel><Input id="motion-title" name="title" required /></Field>
              <Field><FieldLabel htmlFor="motion-kind">{copy.kind}</FieldLabel>
                <NativeSelect id="motion-kind" name="kind">
                  <NativeSelectOption value="animation">{copy.animation}</NativeSelectOption>
                  <NativeSelectOption value="transition">{copy.transition}</NativeSelectOption>
                </NativeSelect>
              </Field>
              {(["video", "image", "audio"] as const).map((kind) => (
                <Field key={kind}>
                  <FieldLabel htmlFor={`motion-${kind}`}>{copy[kind]}</FieldLabel>
                  <NativeSelect id={`motion-${kind}`} name={kind} required={kind === "video"} defaultValue={kind === "video" && videos.length === 1 ? videos[0].id : ""}>
                    <NativeSelectOption value="">{kind === "video" ? copy.chooseVideo : copy.none}</NativeSelectOption>
                    {catalog?.assets.filter((asset) => asset.kind === kind).map((asset) =>
                      <NativeSelectOption key={asset.id} value={asset.id}>{asset.name}</NativeSelectOption>
                    )}
                  </NativeSelect>
                </Field>
              ))}
              <Field><FieldLabel htmlFor="motion-usage">{copy.usage}</FieldLabel><Textarea id="motion-usage" name="usage" /></Field>
              {saveError && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
              <DialogFooter>
                <Button type="button" variant="outline" disabled={saving} onClick={() => setOpen(false)}>{copy.cancel}</Button>
                <Button type="submit" disabled={saving}>{saving ? copy.saving : copy.save}</Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </div>
      {loadError ? <Alert variant="destructive"><AlertDescription>{copy.loadError}</AlertDescription>
        <Button variant="outline" onClick={() => { setLoadError(false); setAttempt((value) => value + 1); }}>{copy.retry}</Button>
      </Alert> : !items ? <p role="status" className="text-sm text-muted-foreground">{copy.loading}</p>
        : items.length === 0 ? <div className="rounded-xl border p-6">
          <p className="text-sm text-muted-foreground">{videos.length ? copy.empty : copy.importFirst}</p>
          {!videos.length && <Button asChild variant="outline" className="mt-3"><Link href={workspaceHref("assets", workspaceId)}>{messages.assets.title}</Link></Button>}
        </div>
        : <ul className="grid grid-cols-[repeat(auto-fill,minmax(min(15rem,100%),1fr))] gap-4">
          {items.map(({ id, settings }) => <li key={id} className="min-w-0">
            <Card className="overflow-hidden pt-0">
              <AssetThumbnail workspaceId={workspaceId} kind={settings.thumbnail_asset_id ? "image" : "video"}
                assetId={settings.thumbnail_asset_id ?? settings.preview_asset_id} />
              <CardContent className="space-y-2">
                <Badge variant="secondary">{copy[settings.kind]}</Badge>
                <h2 className="wrap-anywhere font-semibold">{settings.title}</h2>
                {settings.usage_markdown && <p className="whitespace-pre-wrap wrap-anywhere text-sm text-muted-foreground">{settings.usage_markdown}</p>}
              </CardContent>
            </Card>
          </li>)}
        </ul>}
    </WorkspaceShell>
  );
}
