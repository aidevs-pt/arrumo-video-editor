"use client";

import { invoke } from "@tauri-apps/api/core";
import { AudioLines, Check, Film, ImageIcon, Layers, Search, TriangleAlert, Upload } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Skeleton } from "@/components/ui/skeleton";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { AssetThumbnail } from "./asset-thumbnail";
import type { Asset, AssetOrganization, MediaKind } from "./workspace-assets";

const kindIcons = { image: ImageIcon, audio: AudioLines, video: Film };

/** Notes are Markdown; the preview shows their words without the marks. */
function plain(markdown: string) {
  return markdown
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/^\s*(?:#{1,6}\s|>\s?|[-*+]\s)/gm, "")
    .replace(/\*\*|__|\*|`/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

/**
 * Picks library assets by what they look like and what they are for: large previews, each
 * asset's group and description, search and filters. Nothing changes until the choice is used.
 */
export function AssetPicker({
  workspaceId, open, onOpenChange, title, description, kinds = ["image", "audio", "video"], multiple = false,
  value, exclude = [], limits = {}, confirmLabel, onConfirm, onImport,
}: {
  workspaceId: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  /** The kinds that can be picked. */
  kinds?: MediaKind[];
  multiple?: boolean;
  value: number[];
  /** Assets that are not offered, such as ones already attached. */
  exclude?: number[];
  /** The most that can be picked of each kind. */
  limits?: Partial<Record<MediaKind, number>>;
  confirmLabel?: string;
  /** Uses the choice. The dialog stays open, showing an error, if it throws. */
  onConfirm: (ids: number[]) => void | Promise<void>;
  /** Uploads files and answers the new assets' IDs, which are then picked. */
  onImport?: () => Promise<number[]>;
}) {
  const { messages } = useLocale();
  const copy = messages.assetPicker;
  const library = messages.assetLibrary;
  const [assets, setAssets] = useState<Asset[]>();
  const [organizations, setOrganizations] = useState<Record<number, AssetOrganization>>({});
  const [loadFailed, setLoadFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [chosen, setChosen] = useState<number[]>(value);
  const [search, setSearch] = useState("");
  const [kind, setKind] = useState<MediaKind | "all">("all");
  const [group, setGroup] = useState("");
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);

  // Each opening starts from the current choice, whoever opened it.
  useEffect(() => {
    if (!open) return;
    setChosen(value);
    setSearch("");
    setKind("all");
    setGroup("");
    setFailed(false);
    setLoadFailed(false);
    // Only on opening; `value` changing while open must not discard the creator's picks.
  }, [open]);

  useEffect(() => {
    if (!open) return;
    let active = true;
    Promise.all([
      invoke<Asset[]>("list_assets", { workspaceId }),
      invoke<Record<number, AssetOrganization>>("list_asset_organization", { workspaceId }),
    ]).then(([rows, organization]) => {
      if (!active) return;
      setAssets(rows);
      setOrganizations(organization);
    }, () => { if (active) setLoadFailed(true); });
    return () => { active = false; };
  }, [open, workspaceId, attempt]);

  const offered = useMemo(() => (assets ?? [])
    .filter((asset) => kinds.includes(asset.kind) && !exclude.includes(asset.id)), [assets, kinds, exclude]);
  const groups = [...new Set(offered.map((asset) => organizations[asset.id]?.group).filter((name): name is string => !!name))]
    .sort((first, second) => first.localeCompare(second));
  const needle = search.trim().toLocaleLowerCase();
  const visible = offered.filter((asset) => {
    const organization = organizations[asset.id];
    if (kind !== "all" && asset.kind !== kind) return false;
    if (group && organization?.group !== group) return false;
    return !needle || [asset.name, asset.notes, organization?.group ?? "", ...(organization?.labels ?? [])]
      .some((text) => text.toLocaleLowerCase().includes(needle));
  });

  function reset(next: boolean) {
    if (!pending) onOpenChange(next);
  }

  function atLimit(asset: Asset) {
    const limit = limits[asset.kind];
    return limit !== undefined && chosen.filter((id) => offered.find((entry) => entry.id === id)?.kind === asset.kind).length >= limit;
  }

  function toggle(asset: Asset) {
    setFailed(false);
    if (!multiple) return setChosen([asset.id]);
    if (chosen.includes(asset.id)) return setChosen(chosen.filter((id) => id !== asset.id));
    if (!atLimit(asset)) setChosen([...chosen, asset.id]);
  }

  async function confirm() {
    setPending(true);
    setFailed(false);
    try {
      await onConfirm(chosen);
      onOpenChange(false);
    } catch {
      setFailed(true);
    } finally {
      setPending(false);
    }
  }

  async function upload() {
    if (!onImport) return;
    try {
      const added = await onImport();
      if (!added.length) return;
      setAttempt((current) => current + 1);
      setChosen((current) => multiple ? [...current, ...added.filter((id) => !current.includes(id))] : [added[0]]);
    } catch {
      setFailed(true);
    }
  }

  const limitNote = multiple ? Object.entries(limits).filter(([entry]) => kinds.includes(entry as MediaKind))
    .map(([entry, count]) => copy.limit.replace("{count}", String(count)).replace("{kind}", library[entry === "image" ? "images" : entry === "video" ? "videos" : "audio"].toLocaleLowerCase()))
    .join(" · ") : "";

  return (
    <Dialog open={open} onOpenChange={reset}>
      <DialogContent showCloseButton={!pending} className="flex max-h-[min(52rem,calc(100dvh-2rem))] flex-col gap-4 sm:max-w-4xl">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description && <DialogDescription>{description}</DialogDescription>}
        </DialogHeader>
        <div className="flex flex-wrap items-center gap-2">
          <div className="relative min-w-48 flex-1">
            <Search className="pointer-events-none absolute start-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
            <Input type="search" value={search} placeholder={library.search} aria-label={library.search} className="ps-8"
              onChange={(event) => setSearch(event.target.value)} />
          </div>
          {kinds.length > 1 && <ToggleGroup type="single" variant="outline" size="sm" value={kind} aria-label={library.kinds}
            onValueChange={(next) => { if (next) setKind(next as MediaKind | "all"); }}>
            <ToggleGroupItem value="all"><Layers aria-hidden="true" />{library.all}</ToggleGroupItem>
            {kinds.map((entry) => {
              const Icon = kindIcons[entry];
              return <ToggleGroupItem key={entry} value={entry}>
                <Icon aria-hidden="true" />{library[entry === "image" ? "images" : entry === "video" ? "videos" : "audio"]}
              </ToggleGroupItem>;
            })}
          </ToggleGroup>}
          {groups.length > 0 && <NativeSelect aria-label={library.groups} value={group} onChange={(event) => setGroup(event.target.value)}>
            <NativeSelectOption value="">{copy.allGroups}</NativeSelectOption>
            {groups.map((name) => <NativeSelectOption key={name} value={name}>{name}</NativeSelectOption>)}
          </NativeSelect>}
          {onImport && <Button type="button" variant="outline" size="sm" disabled={pending} onClick={() => void upload()}>
            <Upload data-icon="inline-start" />{copy.upload}
          </Button>}
        </div>
        <div className="-mx-1 min-h-40 flex-1 overflow-y-auto px-1 py-1">
          {loadFailed ? <Alert variant="destructive">
            <AlertDescription className="flex flex-wrap items-center gap-3">{copy.loadError}
              <Button type="button" variant="outline" size="sm" onClick={() => { setLoadFailed(false); setAttempt((current) => current + 1); }}>{copy.retry}</Button>
            </AlertDescription>
          </Alert> : !assets ? <div role="status" aria-label={copy.loading} className="grid grid-cols-[repeat(auto-fill,minmax(11rem,1fr))] gap-3">
            {Array.from({ length: 6 }, (_, index) => <Skeleton key={index} className="aspect-[4/3] w-full" />)}
          </div> : visible.length === 0 ? <p className="py-10 text-center text-sm text-muted-foreground">
            {offered.length ? library.noMatches : copy.empty}
          </p> : <ul className="grid grid-cols-[repeat(auto-fill,minmax(11rem,1fr))] gap-3">
            {visible.map((asset) => {
              const selected = chosen.includes(asset.id);
              const blocked = multiple && !selected && atLimit(asset);
              const organization = organizations[asset.id];
              const Icon = kindIcons[asset.kind];
              return <li key={asset.id} className="min-w-0">
                <button type="button" aria-pressed={selected} disabled={pending || blocked} onClick={() => toggle(asset)}
                  className={cn("group relative flex h-full w-full flex-col gap-2 rounded-xl border-2 bg-card p-1.5 text-start outline-none transition-colors motion-reduce:transition-none",
                    "focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50",
                    selected ? "border-primary" : "border-transparent hover:border-border")}>
                  <AssetThumbnail workspaceId={workspaceId} assetId={asset.id} kind={asset.kind}
                    fit={asset.kind === "image" ? "contain" : "cover"} transparency={asset.kind === "image"} className="aspect-[4/3] w-full" />
                  {selected && <span aria-hidden="true" className="absolute end-3 top-3 grid size-6 place-items-center rounded-full bg-primary text-primary-foreground shadow-sm">
                    <Check className="size-4" />
                  </span>}
                  <span className="flex min-w-0 flex-col gap-0.5 px-1 pb-1">
                    <span className="flex min-w-0 items-center gap-1.5 text-sm font-semibold">
                      <Icon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
                      <span className="truncate" title={asset.name}>{asset.name}</span>
                    </span>
                    {organization?.group && <span className="truncate text-xs text-muted-foreground">{organization.group}</span>}
                    {asset.notes.trim() ? <span className="line-clamp-3 text-xs text-muted-foreground">{plain(asset.notes)}</span>
                      : <span className="flex items-center gap-1 text-xs text-[oklch(50%_.13_60)]"><TriangleAlert className="size-3.5" aria-hidden="true" />{library.noContext}</span>}
                  </span>
                </button>
              </li>;
            })}
          </ul>}
        </div>
        {failed && <Alert variant="destructive" role="alert"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
        <DialogFooter className="items-center sm:justify-between">
          <p className="text-sm text-muted-foreground tabular-nums">
            {multiple ? [chosen.length === 1 ? library.selectedOne : library.selected.replace("{count}", String(chosen.length)), limitNote].filter(Boolean).join(" · ") : ""}
          </p>
          <div className="flex flex-wrap justify-end gap-2">
            <Button type="button" variant="outline" disabled={pending} onClick={() => reset(false)}>{copy.cancel}</Button>
            <Button type="button" disabled={pending || (!multiple && chosen.length === 0)} onClick={() => void confirm()}>
              {confirmLabel ?? copy.choose}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
