"use client";

import { invoke } from "@tauri-apps/api/core";
import { Copy, PanelRight, Pencil, Plus, Sparkles, Trash2, Volume2, X } from "lucide-react";
import { useRouter, useSearchParams } from "next/navigation";
import { Fragment, useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { ItemContextMenu } from "@/components/workspace/item-context-menu";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Button } from "@/components/ui/button";
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
import { cn } from "@/lib/utils";

import { GenerateMotionDialog } from "./generate-motion-dialog";
import { MotionPreviewDialog } from "./motion-preview-dialog";
import { MotionShareDialog } from "./motion-share-dialog";

type Kind = "animation" | "transition";
type Settings = {
  kind: Kind; title: string; usage_markdown: string; profile_id: number | null;
  preview_asset_id: number; thumbnail_asset_id: number | null; sound_asset_id: number | null;
};
type Item = { id: number; settings: Settings };
type Asset = { id: number; name: string; kind: "video" | "image" | "audio" };
type Profile = { id: number; name: string };

/** With `profileId`, the library is fixed to that profile and renders inside the profile page. */
export function WorkspaceMotionLibrary({ workspace, profileId }: { workspace: Workspace; profileId?: number }) {
  const query = useSearchParams();
  const requested = profileId ? String(profileId) : query.get("profile") ?? "";
  const scope = /^[1-9]\d*$/.test(requested) && Number.isSafeInteger(Number(requested)) ? requested : "";
  return <MotionLibraryScope key={scope} workspace={workspace} scope={scope} embedded={profileId !== undefined} />;
}

function MotionLibraryScope({ workspace, scope, embedded }: { workspace: Workspace; scope: string; embedded: boolean }) {
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
  const [chosenId, setChosenId] = useState<number | null>(null);
  const [deletingItem, setDeletingItem] = useState<number | null>(null);
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
  const Shell = embedded ? Fragment : WorkspaceShell;
  const chosen = items?.find((item) => item.id === chosenId);
  const scopeName = (profileId: number | null) => profileId === null ? copy.workspaceScope
    : catalog?.profiles.find((profile) => profile.id === profileId)?.name ?? copy.workspaceScope;

  return (
    <Shell>
      {embedded ? <p className="mb-4 max-w-[65ch] text-muted-foreground">{copy.intro}</p> : <WorkspaceHeading>
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
        <p>{copy.intro}</p>
      </WorkspaceHeading>}
      <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
        <Field className={embedded ? "hidden" : "w-auto"}>
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
        <div className="flex flex-wrap gap-2">
        <GenerateMotionDialog<Item> workspaceId={workspaceId} profileId={scope ? Number(scope) : null}
          disabled={!items || open} onCreate={(item) => { setItems((current) => [...(current ?? []), item]); setChosenId(item.id); }} />
        <Dialog open={open} onOpenChange={(next) => {
          if (saving) return;
          setSaveError(false);
          setOpen(next);
        }}>
          <DialogTrigger asChild>
            <Button variant="outline" disabled={!items || !videos.length} onClick={() => setOperation(undefined)}><Plus data-icon="inline-start" />{copy.add}</Button>
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
                      {existing && assetId !== "" && !catalog?.assets.some((asset) => asset.id === assetId) &&
                        <NativeSelectOption value={assetId}>{copy.preservedMedia}</NativeSelectOption>}
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
      </div>
      {loadError ? <Alert variant="destructive"><AlertDescription>{copy.loadError}</AlertDescription>
        <Button variant="outline" onClick={() => { setLoadError(false); setAttempt((value) => value + 1); }}>{copy.retry}</Button>
      </Alert> : !items ? <p role="status" className="text-sm text-muted-foreground">{copy.loading}</p>
        : items.length === 0 ? <Empty className="min-h-56 rounded-xl border border-dashed">
          <EmptyHeader>
            <EmptyMedia variant="icon"><Sparkles /></EmptyMedia>
            <EmptyTitle>{copy.empty}</EmptyTitle>
            <EmptyDescription>{copy.emptyGenerate}</EmptyDescription>
          </EmptyHeader>
        </Empty>
        : <div className={cn("grid items-start gap-6", chosen && "xl:grid-cols-[minmax(0,1fr)_22rem]")}>
          <ul className="grid grid-cols-[repeat(auto-fill,minmax(min(15rem,100%),1fr))] gap-4">
            {items.map((item) => { const { id, settings } = item; return <ItemContextMenu key={id} label={settings.title} items={[
              { label: copy.showDetails, icon: PanelRight, onSelect: () => setChosenId(id) },
              { label: copy.edit, icon: Pencil, group: true, disabled: open, onSelect: () => { setChosenId(id); setOperation({ mode: "edit", item }); setSaveError(false); setOpen(true); } },
              { label: copy.duplicate, icon: Copy, disabled: open, onSelect: () => { setChosenId(id); setOperation({ mode: "duplicate", item }); setSaveError(false); setOpen(true); } },
              { label: copy.deletion.deleteTitle, icon: Trash2, destructive: true, group: true, disabled: open, onSelect: () => { setChosenId(id); setDeletingItem(id); } },
            ]}><li className="min-w-0">
              <button type="button" aria-pressed={chosen?.id === id} onClick={() => setChosenId(chosenId === id ? null : id)}
                className={cn("flex h-full w-full flex-col gap-3 rounded-xl border-2 border-border bg-card p-2 text-start transition-colors duration-150 hover:border-line-strong",
                  chosen?.id === id && "border-primary hover:border-primary")}>
                <AssetThumbnail workspaceId={workspaceId} kind={settings.thumbnail_asset_id ? "image" : "video"}
                  assetId={settings.thumbnail_asset_id ?? settings.preview_asset_id} className="w-full bg-ink/90" />
                <span className="flex flex-col items-start gap-2 px-1.5 pb-1.5">
                  <span className="flex w-full items-center gap-2 font-semibold wrap-anywhere">
                    <span className="min-w-0 flex-1">{settings.title}</span>
                    {settings.sound_asset_id && <Volume2 className="size-4 shrink-0 text-muted-foreground" aria-label={copy.audio} />}
                  </span>
                  <span className="flex flex-wrap gap-1.5">
                    <span className="rounded-md border px-2 py-0.5 text-sm">{copy[settings.kind]}</span>
                    <span className="rounded-md bg-secondary px-2 py-0.5 text-sm font-semibold">{scopeName(settings.profile_id)}</span>
                  </span>
                  {settings.usage_markdown && <span className="line-clamp-2 text-sm text-muted-foreground wrap-anywhere">{settings.usage_markdown}</span>}
                </span>
              </button>
            </li></ItemContextMenu>; })}
          </ul>
          {chosen && <aside className="flex flex-col gap-4 rounded-xl border bg-card p-5 xl:sticky xl:top-20" aria-labelledby="motion-panel-title">
            <div className="flex items-start justify-between gap-2">
              <h2 id="motion-panel-title" className="min-w-0 text-[1.5rem] leading-tight wrap-anywhere">{chosen.settings.title}</h2>
              <Button variant="ghost" size="icon-sm" aria-label={copy.close} onClick={() => setChosenId(null)}><X /></Button>
            </div>
            <AssetThumbnail workspaceId={workspaceId} kind={chosen.settings.thumbnail_asset_id ? "image" : "video"}
              assetId={chosen.settings.thumbnail_asset_id ?? chosen.settings.preview_asset_id} className="w-full bg-ink/90" />
            <MotionPreviewDialog workspaceId={workspaceId} profileId={scope ? Number(scope) : null}
              itemId={chosen.id} title={chosen.settings.title} />
            {chosen.settings.usage_markdown && <div>
              <h3 className="mb-1 font-bold">{copy.usage}</h3>
              <p className="whitespace-pre-wrap text-muted-foreground wrap-anywhere">{chosen.settings.usage_markdown}</p>
            </div>}
            <p className="flex flex-wrap items-center gap-2 font-bold">{copy.sharedWith}
              <span className="rounded-md bg-secondary px-2 py-0.5 text-sm">{scopeName(chosen.settings.profile_id)}</span></p>
            <div className="flex flex-wrap gap-2 border-t pt-4">
              {(["edit", "duplicate"] as const).map((mode) => (
                <Button key={mode} type="button" variant="outline" disabled={open}
                  onClick={() => {
                    setOperation({ mode, item: chosen });
                    setSaveError(false);
                    setOpen(true);
                  }}>{mode === "edit" ? <Pencil data-icon="inline-start" /> : <Copy data-icon="inline-start" />}{copy[mode]}</Button>
              ))}
              <GenerateMotionDialog<Item> workspaceId={workspaceId} profileId={chosen.settings.profile_id}
                basedOn={{ id: chosen.id, title: chosen.settings.title }} disabled={open}
                onCreate={(item) => { setItems((current) => [...(current ?? []), item]); setChosenId(item.id); }} />
              {chosen.settings.profile_id !== null && catalog && <MotionShareDialog
                workspaceId={workspaceId} itemId={chosen.id} title={chosen.settings.title}
                ownerId={chosen.settings.profile_id} profiles={catalog.profiles} disabled={open}
                onSave={() => setAttempt((value) => value + 1)} />}
              <ConfirmTitleDelete title={chosen.settings.title} disabled={open} copy={copy.deletion}
                open={deletingItem === chosen.id} onOpenChange={(value) => setDeletingItem(value ? chosen.id : null)}
                onConfirm={(confirmedTitle) => invoke<boolean>("delete_motion_library_item", {
                  workspaceId, itemId: chosen.id, confirmedTitle,
                })}
                onDelete={() => {
                  setItems((current) => current?.filter((item) => item.id !== chosen.id));
                  setChosenId(null);
                  heading.current?.focus();
                }} />
            </div>
          </aside>}
        </div>}
    </Shell>
  );
}
