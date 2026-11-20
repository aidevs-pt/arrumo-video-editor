"use client";

import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow, type DragDropEvent } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { AudioLines, Film, Folder, ImageIcon, Layers, Search, Tag, TriangleAlert, Upload } from "lucide-react";
import { useEffect, useEffectEvent, useRef, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import { WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { AssetDetailsPanel } from "./asset-details-panel";
import { AssetThumbnail } from "./asset-thumbnail";

const kinds = ["image", "audio", "video"] as const;
export type MediaKind = typeof kinds[number];
export type Asset = { id: number; name: string; kind: MediaKind; notes: string };
export type AssetOrganization = { group: string | null; labels: string[] };
type KindFilter = "all" | MediaKind | "missing";
const kindIcons = { all: Layers, image: ImageIcon, audio: AudioLines, video: Film };

export function WorkspaceAssets({ workspace }: { workspace: { id: number; name: string } }) {
  const importing = useRef(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const [assets, setAssets] = useState<Asset[]>();
  const [organizations, setOrganizations] = useState<Record<number, AssetOrganization>>({});
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [filter, setFilter] = useState<KindFilter>("all");
  const [groupFilter, setGroupFilter] = useState<string | null>(null);
  const [labelFilter, setLabelFilter] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [grouped, setGrouped] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [recent, setRecent] = useState<number[]>([]);
  const [progress, setProgress] = useState<{ completed: number; total: number }>();
  const [importError, setImportError] = useState(false);
  const [selected, setSelected] = useState<number[]>([]);
  const { locale, messages } = useLocale();
  const copy = messages.assetLibrary;
  const workspaceId = workspace.id;

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Asset[]>("list_assets", { workspaceId }),
      invoke<Record<number, AssetOrganization>>("list_asset_organization", { workspaceId }),
    ]).then(([rows, metadata]) => {
      if (!active) return;
      setAssets(rows);
      setOrganizations(metadata);
    }, () => { if (active) setLoadError(true); });
    return () => { active = false; };
  }, [workspaceId, attempt]);

  async function importFiles(paths?: string[]) {
    if (!assets || importing.current) return;
    importing.current = true;
    setImportError(false);
    try {
      const files = paths ?? await open({ multiple: true, title: messages.assets.pickTitle });
      if (!files?.length) return;
      const imported: number[] = [];
      setProgress({ completed: 0, total: files.length });
      for (const [index, source] of files.entries()) {
        try {
          const asset = await invoke<Asset>("import_asset", { workspaceId, source });
          setAssets((rows) => [...(rows ?? []), asset]);
          imported.push(asset.id);
          setRecent((current) => [...current, asset.id]);
        } catch {
          setImportError(true);
        } finally {
          setProgress({ completed: index + 1, total: files.length });
        }
      }
      if (imported.length) {
        setFilter("all");
        setGroupFilter(null);
        setLabelFilter(null);
        setSearch("");
      }
    } catch {
      setImportError(true);
    } finally {
      setProgress(undefined);
      importing.current = false;
    }
  }

  const handleDragDrop = useEffectEvent((event: DragDropEvent) => {
    const available = Boolean(assets) && !importing.current
      && !document.querySelector('[role="dialog"], [role="alertdialog"]');
    setDragging(available && (event.type === "enter" || event.type === "over"));
    if (available && event.type === "drop") void importFiles(event.paths);
  });

  useEffect(() => {
    if (!isTauri()) return;
    let active = true;
    const subscription = getCurrentWindow().onDragDropEvent(({ payload }) => {
      if (active) handleDragDrop(payload);
    });
    return () => {
      active = false;
      void subscription.then((unlisten) => unlisten(), () => {});
    };
  }, []);

  const all = assets ?? [];
  const groups = [...new Set(Object.values(organizations).flatMap(({ group }) => group ? [group] : []))]
    .sort((left, right) => left.localeCompare(right, locale));
  const labelNames = new Map<string, string>();
  for (const { labels } of Object.values(organizations)) {
    for (const label of labels) if (!labelNames.has(label.toLowerCase())) labelNames.set(label.toLowerCase(), label);
  }
  const labels = [...labelNames.values()].sort((left, right) => left.localeCompare(right, locale));
  const missing = all.filter((asset) => !asset.notes.trim()).length;
  const needle = search.trim().toLocaleLowerCase(locale);
  const visible = all.filter((asset) => {
    const organization = organizations[asset.id];
    const text = [asset.name, asset.notes, organization?.group ?? "", ...(organization?.labels ?? [])].join(" ").toLocaleLowerCase(locale);
    return (filter === "all" || (filter === "missing" ? !asset.notes.trim() : asset.kind === filter))
      && (!groupFilter || organization?.group === groupFilter)
      && (!labelFilter || organization?.labels.some((label) => label.toLowerCase() === labelFilter.toLowerCase()))
      && (!needle || text.includes(needle));
  });
  const sections = grouped && groups.length
    ? [...groups, null].map((group) => ({
      group, items: visible.filter((asset) => (organizations[asset.id]?.group ?? null) === group),
    })).filter(({ items }) => items.length)
    : [{ group: null, items: visible }];
  const selectedAssets = all.filter(({ id }) => selected.includes(id));
  const kindCount = (kind: "all" | MediaKind) => kind === "all" ? all.length : all.filter((asset) => asset.kind === kind).length;
  const filtered = filter !== "all" || groupFilter || labelFilter || needle;

  function toggle(id: number, checked: boolean) {
    setSelected((current) => checked ? [...current, id] : current.filter((value) => value !== id));
  }

  return (
    <WorkspaceShell className="max-w-[90rem]">
      <header className="mb-8 flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 ref={heading} tabIndex={-1} className="text-[clamp(2.25rem,5vw,3.25rem)] leading-[1] font-[850] outline-none [font-variation-settings:'wdth'_135]">{messages.assets.title}</h1>
          <p className="mt-2 text-lg text-muted-foreground">{copy.intro.replace("{workspace}", workspace.name)}</p>
        </div>
        <Button size="lg" onClick={() => void importFiles()} disabled={!assets || progress !== undefined}>
          <Upload data-icon="inline-start" />{copy.upload}
        </Button>
      </header>
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{messages.assets.loadError}</AlertDescription>
          <Button variant="outline" onClick={() => { setLoadError(false); setAttempt((value) => value + 1); }}>{messages.assets.retry}</Button>
        </Alert>
      ) : !assets ? (
        <Skeleton className="h-72" role="status" aria-label={messages.assets.loading} />
      ) : (
        <div className={cn("grid items-start gap-6 lg:grid-cols-[13rem_minmax(0,1fr)]",
          selectedAssets.length > 0 && "xl:grid-cols-[13rem_minmax(0,1fr)_22rem]")}>
          <nav aria-label={copy.kinds} className="flex flex-wrap items-start gap-x-6 gap-y-3 lg:flex-col lg:flex-nowrap lg:items-stretch lg:gap-6">
            <ul className="flex flex-wrap gap-1 lg:flex-col">
              {(["all", ...kinds] as const).map((kind) => {
                const Icon = kindIcons[kind];
                const active = filter === kind && !groupFilter && !labelFilter;
                return (
                  <li key={kind}>
                    <button type="button" aria-pressed={active} onClick={() => { setFilter(kind); setGroupFilter(null); setLabelFilter(null); }}
                      className={cn("flex w-full items-center gap-3 rounded-lg px-3 py-2 text-start font-semibold hover:bg-secondary",
                        active && "bg-foreground text-background hover:bg-foreground")}>
                      <Icon className={cn("size-4", active && "text-attention")} aria-hidden="true" />
                      <span className="flex-1">{copy[kind === "all" ? "all" : kind === "image" ? "images" : kind === "audio" ? "audio" : "videos"]}</span>
                      <span className={cn("rounded-[5px] px-1.5 text-sm tabular-nums", active ? "bg-white/15" : "bg-secondary")}>{kindCount(kind)}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
            {groups.length > 0 && <div>
              <h2 className="mb-1 px-3 text-sm font-semibold text-muted-foreground">{copy.groups}</h2>
              <ul className="flex flex-wrap gap-1 lg:flex-col">
                {groups.map((group) => (
                  <li key={group}>
                    <button type="button" aria-pressed={groupFilter === group} onClick={() => setGroupFilter(groupFilter === group ? null : group)}
                      className={cn("flex w-full items-center gap-3 rounded-lg px-3 py-2 text-start hover:bg-secondary", groupFilter === group && "bg-secondary font-semibold")}>
                      <Folder className="size-4 shrink-0" aria-hidden="true" />
                      <span className="min-w-0 flex-1 wrap-anywhere">{group}</span>
                      <span className="text-sm text-muted-foreground tabular-nums">{all.filter((asset) => organizations[asset.id]?.group === group).length}</span>
                    </button>
                  </li>
                ))}
              </ul>
            </div>}
            {labels.length > 0 && <div>
              <h2 className="mb-2 px-3 text-sm font-semibold text-muted-foreground">{copy.labels}</h2>
              <ul className="flex flex-wrap gap-1.5 px-1">
                {labels.map((label) => (
                  <li key={label}>
                    <button type="button" aria-pressed={labelFilter === label} onClick={() => setLabelFilter(labelFilter === label ? null : label)}
                      className={cn("flex items-center gap-1.5 rounded-md bg-secondary px-2 py-1 text-sm font-semibold hover:bg-accent",
                        labelFilter === label && "bg-foreground text-background hover:bg-foreground")}>
                      <Tag className="size-3.5" aria-hidden="true" />{label}
                    </button>
                  </li>
                ))}
              </ul>
            </div>}
            {missing > 0 && <button type="button" onClick={() => { setFilter("missing"); setGroupFilter(null); setLabelFilter(null); }}
              className="flex flex-col items-start gap-1 rounded-xl border border-[oklch(80%_.1_70)] bg-[oklch(97%_.03_80)] p-3 text-start text-sm text-[oklch(45%_.12_60)] hover:bg-[oklch(95%_.05_80)]">
              <span className="flex items-center gap-2 font-semibold"><TriangleAlert className="size-4" aria-hidden="true" />
                {missing === 1 ? copy.missingOne : copy.missing.replace("{count}", String(missing))}</span>
              <span className="underline underline-offset-4">{copy.showMissing}</span>
            </button>}
          </nav>
          <section aria-label={messages.assets.list} data-dragging={dragging}
            className="flex min-w-0 flex-col gap-4 rounded-xl outline-offset-4 data-[dragging=true]:outline-2 data-[dragging=true]:outline-primary">
            <div className="flex flex-wrap gap-3">
              <label className="relative min-w-56 flex-1">
                <Search aria-hidden="true" className="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
                <Input type="search" value={search} onChange={(event) => setSearch(event.target.value)} placeholder={copy.search} aria-label={copy.search} className="h-10 ps-9" />
              </label>
              {groups.length > 0 && <NativeSelect aria-label={copy.groupBy} value={grouped ? "group" : ""} className="h-10"
                onChange={(event) => setGrouped(event.target.value === "group")}>
                <NativeSelectOption value="">{copy.noGrouping}</NativeSelectOption>
                <NativeSelectOption value="group">{copy.groupBy}</NativeSelectOption>
              </NativeSelect>}
            </div>
            {selected.length > 0 && <div className="flex flex-wrap items-center gap-2 rounded-xl bg-secondary px-4 py-2.5" role="status">
              <span className="me-auto font-semibold tabular-nums">{copy.selected.replace("{count}", String(selected.length))}</span>
              <Button variant="ghost" size="sm" onClick={() => setSelected(visible.map(({ id }) => id))}>{copy.selectAll}</Button>
              <Button variant="ghost" size="sm" onClick={() => setSelected([])}>{copy.clear}</Button>
            </div>}
            {progress && <div role="status" className="flex flex-col gap-2 text-sm">
              {copy.uploading.replace("{done}", String(progress.completed)).replace("{total}", String(progress.total))}
              <Progress aria-hidden="true" value={progress.completed / progress.total * 100} />
            </div>}
            {importError && <Alert variant="destructive"><AlertDescription>{messages.assets.importError}</AlertDescription></Alert>}
            {all.length === 0 ? (
              <Empty className="min-h-64 rounded-xl border border-dashed">
                <EmptyHeader>
                  <EmptyMedia variant="icon"><Upload /></EmptyMedia>
                  <EmptyTitle>{copy.empty}</EmptyTitle>
                  <EmptyDescription>{copy.emptyHint}</EmptyDescription>
                </EmptyHeader>
                <Button onClick={() => void importFiles()}>{copy.upload}</Button>
              </Empty>
            ) : visible.length === 0 ? (
              <div className="flex flex-wrap items-center gap-3 border-y py-6 text-muted-foreground">
                {copy.noMatches}
                {filtered && <Button variant="outline" size="sm" onClick={() => { setFilter("all"); setGroupFilter(null); setLabelFilter(null); setSearch(""); }}>{copy.showAll}</Button>}
              </div>
            ) : sections.map(({ group, items }) => (
              <div key={group ?? "ungrouped"} className="flex flex-col gap-2">
                {sections.length > 1 && <h2 className="flex items-center gap-2 text-base font-bold">
                  <Folder className="size-4" aria-hidden="true" />{group ?? messages.assets.ungrouped}
                </h2>}
                <ul className="grid grid-cols-[repeat(auto-fill,minmax(min(9.5rem,100%),1fr))] gap-3">
                  {items.map((asset) => {
                    const checked = selected.includes(asset.id);
                    const isNew = recent.includes(asset.id);
                    const organization = organizations[asset.id];
                    const Icon = kindIcons[asset.kind];
                    return (
                      <li key={asset.id} className="min-w-0">
                        <div className={cn("relative flex flex-col gap-1.5 rounded-lg border-2 border-transparent p-1 transition-colors duration-150 hover:border-border",
                          isNew && "border-attention hover:border-attention", checked && "border-primary hover:border-primary")}>
                          <button type="button" className="block text-start" onClick={() => setSelected([asset.id])}
                            aria-label={`${messages.assets.edit}, ${asset.name}`}>
                            <AssetThumbnail workspaceId={workspaceId} assetId={asset.id} kind={asset.kind} className="aspect-[4/3] w-full" />
                          </button>
                          {isNew && <span className="pointer-events-none absolute start-2.5 top-2.5 rounded-[5px] bg-attention px-1.5 text-xs font-bold text-ink">{copy.new}</span>}
                          <Checkbox className="absolute end-2.5 top-2.5 bg-card" checked={checked}
                            aria-label={`${messages.assets.selectAsset}, ${asset.name}`} onCheckedChange={(value) => toggle(asset.id, value === true)} />
                          <span className="flex min-w-0 items-center gap-1.5 px-1 text-sm">
                            <Icon className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
                            <span className="truncate" title={asset.name}>{asset.name}</span>
                            {!asset.notes.trim() && <TriangleAlert className="ms-auto size-3.5 shrink-0 text-[oklch(55%_.14_70)]" aria-label={copy.noContext} />}
                          </span>
                          {(organization?.labels.length ?? 0) > 0 && <span className="truncate px-1">
                            <span className="rounded-[4px] bg-secondary px-1.5 py-0.5 text-xs font-semibold">{organization?.labels.join(", ")}</span>
                          </span>}
                        </div>
                      </li>
                    );
                  })}
                </ul>
              </div>
            ))}
            {all.length > 0 && <p className="rounded-xl border border-dashed p-4 text-center text-sm text-muted-foreground">{copy.dropHint}</p>}
          </section>
          {selectedAssets.length > 0 && <AssetDetailsPanel key={selected.join("-")} workspaceId={workspaceId} assets={selectedAssets}
            organizations={organizations} onClose={() => setSelected([])}
            onSaved={(updated, organization) => {
              const changes = new Map(updated.map((asset) => [asset.id, asset]));
              setAssets((rows) => rows?.map((row) => changes.get(row.id) ?? row));
              setOrganizations((current) => ({ ...current, ...organization }));
            }}
            onReplaced={(previous, replacement) => {
              setAssets((rows) => rows?.map((row) => row.id === previous ? replacement : row));
              setOrganizations(({ [previous]: organization, ...rest }) => organization ? { ...rest, [replacement.id]: organization } : rest);
              setSelected([replacement.id]);
              setRecent((current) => [...current, replacement.id]);
            }}
            onRemoved={(id) => {
              setAssets((rows) => rows?.filter((row) => row.id !== id));
              setSelected([]);
            }} />}
        </div>
      )}
    </WorkspaceShell>
  );
}
