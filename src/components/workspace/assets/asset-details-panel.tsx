"use client";

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { RectangleHorizontal, RectangleVertical, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { MarkdownEditor } from "@/components/markdown/editor";
import { useLocale } from "@/lib/locale-provider";
import { TagInput } from "@/components/ui/tag-input";
import { cn } from "@/lib/utils";
import { ConfirmTitleDelete } from "../confirm-title-delete";
import { AssetThumbnail } from "./asset-thumbnail";
import type { Asset, AssetOrganization } from "./workspace-assets";

type Profile = { id: number; name: string };
type Format = { id: number; name: string; width: number; height: number };
type Share = { profile_id: number; format_id: number | null };

/** Describes, organizes and shares one asset or several at once; nothing is written until Save. */
export function AssetDetailsPanel({ workspaceId, assets, organizations, onClose, onSaved, onReplaced, onRemoved }: {
  workspaceId: number;
  assets: Asset[];
  organizations: Record<number, AssetOrganization>;
  onClose: () => void;
  onSaved: (assets: Asset[], organizations: Record<number, AssetOrganization>) => void;
  onReplaced: (previous: number, replacement: Asset) => void;
  onRemoved: (id: number) => void;
}) {
  const { messages } = useLocale();
  const copy = messages.assetLibrary;
  const single = assets.length === 1 ? assets[0] : null;
  const [notes, setNotes] = useState(single?.notes ?? "");
  const [replaceNotes, setReplaceNotes] = useState(single !== null);
  const [group, setGroup] = useState(single ? organizations[single.id]?.group ?? "" : "");
  const [labels, setLabels] = useState<string[]>(single ? organizations[single.id]?.labels ?? [] : []);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [profileId, setProfileId] = useState("");
  const [formats, setFormats] = useState<Format[]>([]);
  const [scope, setScope] = useState<number | null>(null);
  const [shares, setShares] = useState<Share[]>([]);
  const [formatNames, setFormatNames] = useState<Record<number, string>>({});
  const [state, setState] = useState<"idle" | "saving" | "saved" | "error">("idle");

  useEffect(() => {
    let active = true;
    invoke<Profile[]>("list_profiles", { workspaceId }).then((rows) => { if (active) setProfiles(rows); }, () => {});
    invoke<{ id: number; name: string }[]>("list_output_formats", { workspaceId }).then((rows) => {
      if (active) setFormatNames(Object.fromEntries(rows.map(({ id, name }) => [id, name])));
    }, () => {});
    if (single) invoke<Share[]>("list_asset_profile_shares", { workspaceId, assetId: single.id })
      .then((rows) => { if (active) setShares(rows); }, () => {});
    return () => { active = false; };
  }, [single, workspaceId]);

  useEffect(() => {
    if (!profileId) return;
    let active = true;
    invoke<Format[]>("list_profile_formats", { workspaceId, profileId: Number(profileId) })
      .then((rows) => { if (active) setFormats(rows); }, () => { if (active) setFormats([]); });
    return () => { active = false; };
  }, [profileId, workspaceId]);

  async function save() {
    setState("saving");
    try {
      const ids = assets.map(({ id }) => id);
      const updated = replaceNotes
        ? await invoke<Asset[]>("set_batch_asset_notes", { workspaceId, assetIds: ids, notes })
        : [];
      const organization: Record<number, AssetOrganization> = {};
      for (const asset of assets) {
        const current = organizations[asset.id] ?? { group: null, labels: [] };
        const entered = labels;
        const settings = single
          ? { group: group.trim() || null, labels: entered }
          : {
            group: group.trim() || current.group,
            labels: [...current.labels, ...entered.filter((label) => !current.labels.some((existing) => existing.toLowerCase() === label.toLowerCase()))],
          };
        organization[asset.id] = await invoke<AssetOrganization>("set_asset_organization", { workspaceId, assetId: asset.id, settings });
      }
      if (profileId) {
        for (const id of ids) {
          await invoke("share_asset_with_profile", { workspaceId, assetId: id, profileId: Number(profileId), formatId: scope });
        }
        if (single) setShares(await invoke<Share[]>("list_asset_profile_shares", { workspaceId, assetId: single.id }));
        setProfileId("");
        setScope(null);
      }
      onSaved(updated, organization);
      setState("saved");
    } catch {
      setState("error");
    }
  }

  async function replace(asset: Asset) {
    try {
      const source = await open({ multiple: false, title: messages.assets.replaceFile });
      if (!source) return;
      onReplaced(asset.id, await invoke<Asset>("replace_asset", { workspaceId, assetId: asset.id, source }));
    } catch {
      setState("error");
    }
  }

  const profileName = (id: number) => profiles.find((profile) => profile.id === id)?.name ?? "";
  const busy = state === "saving";

  return (
    <aside className="flex flex-col gap-5 rounded-xl border bg-card p-5 xl:sticky xl:top-20" aria-labelledby="asset-panel-title">
      <div className="flex items-start justify-between gap-3">
        <h2 id="asset-panel-title" className="font-sans min-w-0 text-[1.125rem] leading-snug font-bold wrap-anywhere">
          {single ? copy.panelOne.replace("{name}", single.name) : copy.panelMany.replace("{count}", String(assets.length))}
        </h2>
        <Button variant="ghost" size="icon-sm" aria-label={copy.closePanel} onClick={onClose}><X /></Button>
      </div>
      <ul className="flex flex-wrap gap-2">
        {assets.slice(0, 6).map((asset) => (
          <li key={asset.id} className={single ? "w-full" : undefined}>
            <AssetThumbnail workspaceId={workspaceId} assetId={asset.id} kind={asset.kind} className={single ? "aspect-video w-full" : "aspect-[4/3] w-16"} />
          </li>
        ))}
        {assets.length > 6 && <li className="grid aspect-[4/3] w-16 place-items-center rounded-thumb bg-secondary text-sm font-bold">+{assets.length - 6}</li>}
      </ul>
      <Field>
        <FieldLabel htmlFor="asset-context">{copy.context}</FieldLabel>
        {!single && <label className="flex items-center gap-2 text-sm">
          <Checkbox checked={!replaceNotes} onCheckedChange={(value) => setReplaceNotes(value !== true)} />{copy.keepContext}
        </label>}
        {replaceNotes && <MarkdownEditor id="asset-context" label={copy.context} value={notes} disabled={busy}
          onChange={(value) => { setNotes(value); setState("idle"); }} />}
        {!single && replaceNotes && <FieldDescription>{copy.contextHintMany}</FieldDescription>}
      </Field>
      <Field>
        <FieldLabel htmlFor="asset-group">{copy.group}</FieldLabel>
        <Input id="asset-group" value={group} maxLength={120} placeholder={copy.groupPlaceholder} disabled={busy}
          onChange={(event) => { setGroup(event.target.value); setState("idle"); }} />
      </Field>
      <Field>
        <FieldLabel htmlFor="asset-labels">{copy.labels}</FieldLabel>
        <TagInput id="asset-labels" value={labels} disabled={busy} removeLabel={copy.removeLabel}
          suggestions={[...new Set(Object.values(organizations).flatMap((entry) => entry.labels))]}
          onChange={(next) => { setLabels(next); setState("idle"); }} />
        <FieldDescription>{copy.labelsHint}</FieldDescription>
      </Field>
      <div className="flex flex-col gap-2 border-t pt-4">
        <FieldLabel htmlFor="asset-share-profile">{copy.share}</FieldLabel>
        {single && shares.length > 0 && <p className="text-sm text-muted-foreground">
          {copy.sharedWith}: {shares.map((share) => share.format_id === null ? profileName(share.profile_id)
            : `${profileName(share.profile_id)} (${copy.onlyFormat.replace("{format}", formatNames[share.format_id] ?? "")})`).join(", ")}
        </p>}
        <NativeSelect id="asset-share-profile" value={profileId} disabled={busy}
          onChange={(event) => { setProfileId(event.target.value); setScope(null); setState("idle"); }}>
          <NativeSelectOption value="">{copy.noShare}</NativeSelectOption>
          {profiles.map((profile) => <NativeSelectOption key={profile.id} value={profile.id}>{profile.name}</NativeSelectOption>)}
        </NativeSelect>
        {profileId && <div role="radiogroup" aria-label={copy.share} className="flex flex-wrap gap-1 rounded-lg bg-secondary p-1">
          {[{ id: null, name: copy.wholeProfile, portrait: false }, ...formats.map((format) => ({
            id: format.id, name: copy.onlyFormat.replace("{format}", format.name), portrait: format.height > format.width,
          }))].map((option) => {
            const Icon = option.portrait ? RectangleVertical : RectangleHorizontal;
            return (
              <button key={option.id ?? "all"} type="button" role="radio" aria-checked={scope === option.id}
                onClick={() => setScope(option.id)}
                className={cn("flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm font-semibold text-muted-foreground hover:text-foreground",
                  scope === option.id && "bg-card text-foreground shadow-xs")}>
                {option.id !== null && <Icon className="size-3.5" aria-hidden="true" />}{option.name}
              </button>
            );
          })}
        </div>}
      </div>
      {state === "error" && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
      <Button size="lg" disabled={busy} onClick={() => void save()}>
        {busy ? copy.saving : state === "saved" ? copy.saved : single ? copy.saveOne : copy.saveMany.replace("{count}", String(assets.length))}
      </Button>
      {single && <div className="flex flex-wrap items-center gap-2 border-t pt-4">
        <Button variant="outline" size="sm" disabled={busy} onClick={() => void replace(single)}>{messages.assets.replaceFile}</Button>
        <ConfirmTitleDelete title={single.name} size="sm" disabled={busy} copy={messages.assets.removal}
          onConfirm={(confirmedName) => invoke<boolean>("remove_asset", { workspaceId, assetId: single.id, confirmedName })}
          onDelete={() => onRemoved(single.id)} />
      </div>}
    </aside>
  );
}
