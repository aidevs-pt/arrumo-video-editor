"use client";

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { CircleAlert, FolderInput, Share2, Upload, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldLabel } from "@/components/ui/field";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Skeleton } from "@/components/ui/skeleton";
import { AssetThumbnail } from "@/components/workspace/assets/asset-thumbnail";
import { useLocale } from "@/lib/locale-provider";
import type { Profile } from "./profile-route";

type Kind = "image" | "audio" | "video";
type Asset = { id: number; name: string; kind: Kind; notes: string };
type Shared = Asset & { format_id: number | null };
type Format = { id: number; name: string };
type Catalog = { shared: Shared[]; library: Asset[]; formats: Format[]; profiles: Profile[] };

const mediaExtensions = ["mp4", "mov", "webm", "mkv", "avi", "png", "jpg", "jpeg", "webp", "gif", "mp3", "wav", "m4a", "aac", "ogg", "flac"];

function scopeValue(formatId: number | null) {
  return formatId === null ? "" : String(formatId);
}

export function ProfileAssets({ workspaceId, profile }: { workspaceId: number; profile: Profile }) {
  const { messages } = useLocale();
  const copy = messages.profileHub;
  const [catalog, setCatalog] = useState<Catalog>();
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [busy, setBusy] = useState<"uploading" | "saving" | null>(null);
  const [error, setError] = useState(false);
  const [libraryOpen, setLibraryOpen] = useState(false);
  const [sharing, setSharing] = useState<Shared | null>(null);

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Shared[]>("list_profile_assets", { workspaceId, profileId: profile.id }),
      invoke<Asset[]>("list_assets", { workspaceId }),
      invoke<Format[]>("list_profile_formats", { workspaceId, profileId: profile.id }),
      invoke<Profile[]>("list_profiles", { workspaceId }),
    ]).then(([shared, library, formats, profiles]) => {
      if (active) setCatalog({ shared, library, formats, profiles });
    }, () => { if (active) setFailed(true); });
    return () => { active = false; };
  }, [attempt, profile.id, workspaceId]);

  async function run(task: () => Promise<void>, state: "uploading" | "saving" = "saving") {
    setBusy(state);
    setError(false);
    try {
      await task();
    } catch {
      setError(true);
    } finally {
      setBusy(null);
      setAttempt((value) => value + 1);
    }
  }

  function upload() {
    void run(async () => {
      const files = await open({ multiple: true, title: copy.upload, filters: [{ name: messages.assets.title, extensions: mediaExtensions }] });
      for (const source of files ?? []) {
        const asset = await invoke<Asset>("import_asset", { workspaceId, source });
        await invoke("share_asset_with_profile", { workspaceId, assetId: asset.id, profileId: profile.id, formatId: null });
      }
    }, "uploading");
  }

  function changeScope(asset: Shared, formatId: number | null) {
    const ids = { workspaceId, assetId: asset.id, profileId: profile.id };
    void run(async () => {
      // A profile-wide share covers every format, so narrowing it must remove the wide share first.
      if (asset.format_id === null) await invoke("unshare_asset_from_profile", { ...ids, formatId: null });
      await invoke("share_asset_with_profile", { ...ids, formatId });
      if (asset.format_id !== null && formatId !== null) {
        await invoke("unshare_asset_from_profile", { ...ids, formatId: asset.format_id });
      }
    });
  }

  if (failed) return (
    <Alert variant="destructive">
      <AlertDescription>{copy.loadError}</AlertDescription>
      <Button variant="outline" onClick={() => { setFailed(false); setAttempt((value) => value + 1); }}>{copy.retry}</Button>
    </Alert>
  );
  if (!catalog) return <Skeleton className="h-48" role="status" aria-label={messages.queue.loading} />;

  const formatName = (id: number | null) => catalog.formats.find((format) => format.id === id)?.name;
  const scopeLabel = (id: number | null) => id === null ? copy.everyFormat : copy.onlyFormat.replace("{format}", formatName(id) ?? "");

  return (
    <section aria-label={messages.profiles.assets}>
      <div className="mb-6 flex flex-wrap items-start justify-between gap-4">
        <p className="max-w-[60ch] text-muted-foreground">{copy.assetsIntro.replace("{profile}", profile.name)}</p>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" onClick={() => setLibraryOpen(true)} disabled={busy !== null}><FolderInput data-icon="inline-start" />{copy.addFromLibrary}</Button>
          <Button onClick={upload} disabled={busy !== null}><Upload data-icon="inline-start" />{busy === "uploading" ? copy.uploading : copy.upload}</Button>
        </div>
      </div>
      {error && <Alert variant="destructive" className="mb-4"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
      {catalog.shared.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon"><Upload /></EmptyMedia>
            <EmptyTitle>{copy.noAssets}</EmptyTitle>
            <EmptyDescription>{copy.noAssetsHint}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <ul className="divide-y border-y">
          {catalog.shared.map((asset) => (
            <li key={`${asset.id}-${asset.format_id}`} className="flex flex-wrap items-center gap-4 py-3">
              <AssetThumbnail workspaceId={workspaceId} assetId={asset.id} kind={asset.kind} className="aspect-video w-28 shrink-0" />
              <div className="min-w-0 flex-1">
                <p className="truncate font-semibold">{asset.name}</p>
                <p className="flex items-center gap-1.5 text-sm text-muted-foreground">
                  {messages.assets[asset.kind] ?? asset.kind}
                  {!asset.notes.trim() && <span className="ms-2 inline-flex items-center gap-1"><CircleAlert className="size-3.5 text-[oklch(55%_.14_70)]" aria-hidden="true" />{copy.missingNotes}</span>}
                </p>
              </div>
              <Field className="w-auto">
                <FieldLabel htmlFor={`scope-${asset.id}-${asset.format_id}`} className="sr-only">{copy.scope}</FieldLabel>
                <NativeSelect id={`scope-${asset.id}-${asset.format_id}`} value={scopeValue(asset.format_id)} disabled={busy !== null}
                  onChange={(event) => changeScope(asset, event.target.value ? Number(event.target.value) : null)}>
                  <NativeSelectOption value="">{copy.everyFormat}</NativeSelectOption>
                  {catalog.formats.map((format) => <NativeSelectOption key={format.id} value={format.id}>{copy.onlyFormat.replace("{format}", format.name)}</NativeSelectOption>)}
                </NativeSelect>
              </Field>
              <Button variant="ghost" size="icon" aria-label={`${copy.shareOther}: ${asset.name}`} title={copy.shareOther}
                disabled={busy !== null || catalog.profiles.length < 2} onClick={() => setSharing(asset)}><Share2 /></Button>
              <Button variant="ghost" size="icon" aria-label={`${copy.remove}: ${asset.name}, ${scopeLabel(asset.format_id)}`} title={copy.remove}
                disabled={busy !== null} onClick={() => void run(async () => {
                  await invoke("unshare_asset_from_profile", { workspaceId, assetId: asset.id, profileId: profile.id, formatId: asset.format_id });
                })}><X /></Button>
            </li>
          ))}
        </ul>
      )}
      <LibraryDialog open={libraryOpen} onOpenChange={setLibraryOpen} workspaceId={workspaceId} profile={profile} catalog={catalog}
        onAdd={(ids, formatId) => run(async () => {
          for (const assetId of ids) await invoke("share_asset_with_profile", { workspaceId, assetId, profileId: profile.id, formatId });
          setLibraryOpen(false);
        })} busy={busy !== null} />
      <ShareDialog asset={sharing} onClose={() => setSharing(null)} workspaceId={workspaceId} profile={profile} catalog={catalog}
        onShare={(profileId, formatId) => run(async () => {
          if (!sharing) return;
          await invoke("share_asset_with_profile", { workspaceId, assetId: sharing.id, profileId, formatId });
          setSharing(null);
        })} busy={busy !== null} />
    </section>
  );
}

function ScopeSelect({ id, formats, value, onChange }: { id: string; formats: Format[]; value: string; onChange: (value: string) => void }) {
  const { messages: { profileHub: copy } } = useLocale();
  return (
    <Field>
      <FieldLabel htmlFor={id}>{copy.scope}</FieldLabel>
      <NativeSelect id={id} value={value} onChange={(event) => onChange(event.target.value)}>
        <NativeSelectOption value="">{copy.everyFormat}</NativeSelectOption>
        {formats.map((format) => <NativeSelectOption key={format.id} value={format.id}>{copy.onlyFormat.replace("{format}", format.name)}</NativeSelectOption>)}
      </NativeSelect>
    </Field>
  );
}

function LibraryDialog({ open: isOpen, onOpenChange, workspaceId, profile, catalog, onAdd, busy }: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  workspaceId: number;
  profile: Profile;
  catalog: Catalog;
  onAdd: (ids: number[], formatId: number | null) => void;
  busy: boolean;
}) {
  const { messages: { profileHub: copy } } = useLocale();
  const [selected, setSelected] = useState<number[]>([]);
  const [scope, setScope] = useState("");
  const sharedWide = new Set(catalog.shared.filter((asset) => asset.format_id === null).map(({ id }) => id));
  const available = catalog.library.filter((asset) => !sharedWide.has(asset.id));

  return (
    <Dialog open={isOpen} onOpenChange={(next) => { if (!busy) { onOpenChange(next); setSelected([]); } }}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{copy.libraryTitle}</DialogTitle>
          <DialogDescription>{copy.libraryHint}</DialogDescription>
        </DialogHeader>
        {available.length === 0 ? <p className="text-muted-foreground">{copy.libraryEmpty}</p> : (
          <ul className="divide-y border-y">
            {available.map((asset) => (
              <li key={asset.id}>
                <label className="flex cursor-pointer items-center gap-3 py-2">
                  <Checkbox checked={selected.includes(asset.id)} onCheckedChange={(checked) => setSelected((current) =>
                    checked === true ? [...current, asset.id] : current.filter((id) => id !== asset.id))} />
                  <AssetThumbnail workspaceId={workspaceId} assetId={asset.id} kind={asset.kind} className="aspect-video w-16 shrink-0" />
                  <span className="min-w-0 flex-1 truncate">{asset.name}</span>
                </label>
              </li>
            ))}
          </ul>
        )}
        <ScopeSelect id="library-scope" formats={catalog.formats} value={scope} onChange={setScope} />
        <DialogFooter>
          <Button disabled={busy || selected.length === 0} onClick={() => onAdd(selected, scope ? Number(scope) : null)}>
            {copy.addSelected.replace("{count}", String(selected.length)).replace("{profile}", profile.name)}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ShareDialog({ asset, onClose, workspaceId, profile, catalog, onShare, busy }: {
  asset: Shared | null;
  onClose: () => void;
  workspaceId: number;
  profile: Profile;
  catalog: Catalog;
  onShare: (profileId: number, formatId: number | null) => void;
  busy: boolean;
}) {
  const { messages: { profileHub: copy } } = useLocale();
  const others = catalog.profiles.filter(({ id }) => id !== profile.id);
  const [target, setTarget] = useState("");
  const [scope, setScope] = useState("");
  const [formats, setFormats] = useState<Format[]>([]);
  const targetId = Number(target || others[0]?.id);

  useEffect(() => {
    if (!asset || !targetId) return;
    let active = true;
    invoke<Format[]>("list_profile_formats", { workspaceId, profileId: targetId }).then(
      (entries) => { if (active) setFormats(entries); }, () => { if (active) setFormats([]); });
    return () => { active = false; };
  }, [asset, targetId, workspaceId]);

  return (
    <Dialog open={asset !== null} onOpenChange={(next) => { if (!next && !busy) { onClose(); setScope(""); } }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{copy.shareTitle.replace("{asset}", asset?.name ?? "")}</DialogTitle>
        </DialogHeader>
        <Field>
          <FieldLabel htmlFor="share-profile">{copy.shareProfile}</FieldLabel>
          <NativeSelect id="share-profile" value={String(targetId)} onChange={(event) => { setTarget(event.target.value); setScope(""); }}>
            {others.map((entry) => <NativeSelectOption key={entry.id} value={entry.id}>{entry.name}</NativeSelectOption>)}
          </NativeSelect>
        </Field>
        <ScopeSelect id="share-scope" formats={formats} value={scope} onChange={setScope} />
        <DialogFooter>
          <Button disabled={busy || !targetId} onClick={() => onShare(targetId, scope ? Number(scope) : null)}>{copy.share}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
