"use client";

import { invoke } from "@tauri-apps/api/core";
import { Layers, Plus } from "lucide-react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Textarea } from "@/components/ui/textarea";
import { AssetThumbnail } from "@/components/workspace/assets/asset-thumbnail";
import { ConfirmTitleDelete } from "@/components/workspace/confirm-title-delete";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { workspaceHref, type Workspace } from "@/components/workspace/home/workspace-links";
import { useLocale } from "@/lib/locale-provider";

import { MotionShareDialog } from "./motion-share-dialog";

type Kind = "animation" | "transition";
type Settings = {
  kind: Kind; title: string; usage_markdown: string; profile_id: number | null;
  preview_asset_id: number; thumbnail_asset_id: number | null; sound_asset_id: number | null;
};
type Item = { id: number; settings: Settings };
type Asset = { id: number; name: string; kind: "video" | "image" | "audio" };
type Profile = { id: number; name: string };

export function WorkspaceMotionLibrary({ workspace }: { workspace: Workspace }) {
  const query = useSearchParams();
  const requested = query.get("profile") ?? "";
  const scope = /^[1-9]\d*$/.test(requested) && Number.isSafeInteger(Number(requested)) ? requested : "";
  return <MotionLibraryScope key={scope} workspace={workspace} scope={scope} />;
}

function MotionLibraryScope({ workspace, scope }: { workspace: Workspace; scope: string }) {
  const router = useRouter();
  const { messages } = useLocale();
  const heading = useRef<HTMLHeadingElement>(null);
  const copy = messages.motionLibrary;
  const [catalog, setCatalog] = useState<{ assets: Asset[]; profiles: Profile[] }>();
  const [items, setItems] = useState<Item[]>();
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [open, setOpen] = useState(false);
  const [operation, setOperation] = useState<{ mode: "edit" | "duplicate"; item: Item }>();
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const workspaceId = workspace.id;

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Asset[]>("list_assets", { workspaceId }),
      invoke<Profile[]>("list_profiles", { workspaceId }),
    ]).then(async ([assets, profiles]) => {
      if (!active) return;
      setCatalog({ assets, profiles });
      const entries = await invoke<Item[]>("list_motion_library", { workspaceId, profileId: scope ? Number(scope) : null });
      if (active) setItems(entries);
    }).catch(() => { if (active) setLoadError(true); });
    return () => { active = false; };
  }, [workspaceId, scope, attempt]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (saving) return;
    const form = new FormData(event.currentTarget);
    const title = String(form.get("title")).trim();
    const settings: Settings = {
      title,
      kind: form.get("kind") === "transition" ? "transition" : "animation",
      usage_markdown: String(form.get("usage")),
      profile_id: operation ? operation.item.settings.profile_id : scope ? Number(scope) : null,
      preview_asset_id: Number(form.get("video")),
      thumbnail_asset_id: Number(form.get("image")) || null,
      sound_asset_id: Number(form.get("audio")) || null,
    };
    if (!title || (operation?.mode !== "duplicate" && !settings.preview_asset_id)) {
      setSaveError(true);
      return;
    }
    setSaving(true);
    setSaveError(false);
    try {
      const item = operation?.mode === "duplicate"
        ? await invoke<Item>("duplicate_motion_library_item", { workspaceId, itemId: operation.item.id, title })
        : operation
          ? await invoke<Item>("update_motion_library_item", { workspaceId, itemId: operation.item.id, settings })
          : await invoke<Item>("create_motion_library_item", { workspaceId, settings });
      setItems((current) => operation?.mode === "edit"
        ? current?.map((entry) => entry.id === item.id ? item : entry)
        : [...(current ?? []), item]);
      setOpen(false);
    } catch {
      setSaveError(true);
    } finally {
      setSaving(false);
    }
  }

  const existing = operation?.item.settings;
  const dialogTitle = operation ? copy[operation.mode] : copy.add;
  const videos = catalog?.assets.filter(({ kind }) => kind === "video") ?? [];

  return (
    <WorkspaceShell>
      <WorkspaceHeading icon={Layers}>
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
        <p>{copy.intro}</p>
      </WorkspaceHeading>
      <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
        <Field className="w-auto">
          <FieldLabel htmlFor="motion-scope">{copy.scope}</FieldLabel>
          <NativeSelect id="motion-scope" value={scope} disabled={!catalog || open}
            onChange={(event) => {
              const value = event.target.value;
              router.push(workspaceHref("motion-library", workspaceId, value ? Number(value) : undefined));
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
            <Button disabled={!items || !videos.length} onClick={() => setOperation(undefined)}><Plus data-icon="inline-start" />{copy.add}</Button>
          </DialogTrigger>
          <DialogContent className="max-h-[85dvh] overflow-y-auto" onCloseAutoFocus={(event) => {
            if (!operation) return;
            event.preventDefault();
            heading.current?.focus();
          }}>
            <DialogHeader>
              <DialogTitle>{dialogTitle}</DialogTitle>
              <DialogDescription>{operation ? copy[`${operation.mode}Description`] : copy.addDescription}</DialogDescription>
            </DialogHeader>
            <form className="flex flex-col gap-4" onSubmit={save}>
              <Field><FieldLabel htmlFor="motion-title">{copy.name}</FieldLabel><Input id="motion-title" name="title" defaultValue={operation?.mode === "duplicate" ? "" : existing?.title} required /></Field>
              {operation?.mode !== "duplicate" && <>
                <Field><FieldLabel htmlFor="motion-kind">{copy.kind}</FieldLabel>
                  <NativeSelect id="motion-kind" name="kind" defaultValue={existing?.kind}>
                    <NativeSelectOption value="animation">{copy.animation}</NativeSelectOption>
                    <NativeSelectOption value="transition">{copy.transition}</NativeSelectOption>
                  </NativeSelect>
                </Field>
                {([
                  ["video", existing?.preview_asset_id ?? (videos.length === 1 ? videos[0].id : "")],
                  ["image", existing?.thumbnail_asset_id ?? ""],
                  ["audio", existing?.sound_asset_id ?? ""],
                ] as const).map(([kind, assetId]) => (
                  <Field key={kind}>
                    <FieldLabel htmlFor={`motion-${kind}`}>{copy[kind]}</FieldLabel>
                    <NativeSelect id={`motion-${kind}`} name={kind} required={kind === "video"} defaultValue={assetId}>
                      <NativeSelectOption value="">{kind === "video" ? copy.chooseVideo : copy.none}</NativeSelectOption>
                      {catalog?.assets.filter((asset) => asset.kind === kind).map((asset) =>
                        <NativeSelectOption key={asset.id} value={asset.id}>{asset.name}</NativeSelectOption>
                      )}
                    </NativeSelect>
                  </Field>
                ))}
                <Field><FieldLabel htmlFor="motion-usage">{copy.usage}</FieldLabel><Textarea id="motion-usage" name="usage" defaultValue={existing?.usage_markdown} /></Field>
              </>}
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
              <CardFooter className="flex-wrap gap-1">
                {(["edit", "duplicate"] as const).map((mode) => (
                  <Button key={mode} type="button" size="sm" variant="ghost" disabled={open}
                    aria-label={`${copy[mode]} · ${settings.title}`}
                    onClick={() => {
                      setOperation({ mode, item: { id, settings } });
                      setSaveError(false);
                      setOpen(true);
                    }}>{copy[mode]}</Button>
                ))}
                {settings.profile_id !== null && catalog && <MotionShareDialog
                  workspaceId={workspaceId} itemId={id} title={settings.title}
                  ownerId={settings.profile_id} profiles={catalog.profiles} disabled={open}
                  onSave={() => setAttempt((value) => value + 1)} />}
                <ConfirmTitleDelete title={settings.title} disabled={open} copy={copy.deletion}
                  onConfirm={(confirmedTitle) => invoke<boolean>("delete_motion_library_item", {
                    workspaceId, itemId: id, confirmedTitle,
                  })}
                  onDelete={() => {
                    setItems((current) => current?.filter((item) => item.id !== id));
                    heading.current?.focus();
                  }} />
              </CardFooter>
            </Card>
          </li>)}
        </ul>}
    </WorkspaceShell>
  );
}
