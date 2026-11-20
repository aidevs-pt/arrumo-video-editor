"use client";

import { useLocale } from "@/lib/locale-provider";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Upload, Images } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { FieldGroup } from "@/components/ui/field";
import { Checkbox } from "@/components/ui/checkbox";
import { MarkdownEditor } from "@/components/markdown/editor";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardFooter } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { AssetOrganizationDialog, type AssetOrganization } from "./asset-organization-dialog";
import { ConfirmTitleDelete } from "../confirm-title-delete";
import { AssetThumbnail } from "./asset-thumbnail";

const kinds = ["image", "audio", "video"] as const;
type MediaKind = typeof kinds[number];
type Asset = { id: number; name: string; kind: MediaKind; notes: string };

export function WorkspaceAssets({ workspace }: {
  workspace: { id: number; name: string };

}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [assets, setAssets] = useState<Asset[]>();
  const [organizations, setOrganizations] = useState<Record<number, AssetOrganization>>({});
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [filter, setFilter] = useState("all");
  const [grouped, setGrouped] = useState(false);
  const [groupFilter, setGroupFilter] = useState("");
  const [labelFilter, setLabelFilter] = useState("");
  const [busy, setBusy] = useState<"import" | "save" | null>(null);
  const [progress, setProgress] = useState<{ completed: number; total: number }>();
  const [error, setError] = useState<"importError" | "notesError" | null>(null);
  const [editing, setEditing] = useState<Asset[] | null>(null);
  const [notes, setNotes] = useState("");
  const [selected, setSelected] = useState<Set<number>>(() => new Set());
  const { locale, messages } = useLocale();
  const copy = messages.assets;

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Asset[]>("list_assets", { workspaceId: workspace.id }),
      invoke<Record<number, AssetOrganization>>("list_asset_organization", { workspaceId: workspace.id }),
    ]).then(
      ([rows, metadata]) => {
        if (active) {
          setAssets(rows);
          setOrganizations(metadata);
        }
      },
      () => { if (active) setLoadError(true); },
    );
    return () => { active = false; };
  }, [workspace.id, attempt]);

  function onAssetRemoved(assetId: number) {
    setAssets((rows) => rows?.filter((asset) => asset.id !== assetId));
    setOrganizations(({ [assetId]: _removed, ...remaining }) => remaining);
    setSelected((current) => {
      const remaining = new Set(current);
      remaining.delete(assetId);
      return remaining;
    });
    heading.current?.focus();
  }

  async function importFiles() {
    if (busy) return;
    setBusy("import");
    setError(null);
    try {
      const files = await open({ multiple: true, title: copy.pickTitle });
      if (!files) return;
      setProgress({ completed: 0, total: files.length });
      for (const [index, source] of files.entries()) {
        try {
          const asset = await invoke<Asset>("import_asset", { workspaceId: workspace.id, source });
          setAssets((rows) => [...(rows ?? []), asset]);
          setFilter("all");
        } catch {
          setError("importError");
        } finally {
          setProgress({ completed: index + 1, total: files.length });
        }
      }
    } catch {
      setError("importError");
    } finally {
      setProgress(undefined);
      setBusy(null);
    }
  }

  async function saveNotes(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editing || busy) return;
    setBusy("save");
    setError(null);
    try {
      const updated = await invoke<Asset[]>("set_batch_asset_notes", {
        workspaceId: workspace.id, assetIds: editing.map(({ id }) => id), notes,
      });
      const changes = new Map(updated.map((asset) => [asset.id, asset]));
      setAssets((rows) => rows?.map((row) => changes.get(row.id) ?? row));
      setEditing(null);
    } catch {
      setError("notesError");
    } finally {
      setBusy(null);
    }
  }

  function edit(items: Asset[]) {
    if (items.length === 0) return;
    setError(null);
    setNotes(items.length === 1 ? items[0].notes : "");
    setEditing(items);
  }

  const selectedAssets = assets?.filter(({ id }) => selected.has(id)) ?? [];
  const selectedLabel = copy.selectionCount.replace("{count}", String(selectedAssets.length));
  const metadata = Object.values(organizations);
  const groups = [...new Set(metadata.flatMap(({ group }) => group ? [group] : []))]
    .sort((left, right) => left.localeCompare(right, locale));
  const labelNames = new Map<string, string>();
  for (const { labels } of metadata) {
    for (const label of labels) {
      const key = label.toLowerCase();
      if (!labelNames.has(key)) labelNames.set(key, label);
    }
  }
  const labels = [...labelNames.values()].sort((left, right) => left.localeCompare(right, locale));
  const activeGroup = groups.includes(groupFilter) ? groupFilter : "";
  const activeLabel = labels.includes(labelFilter) ? labelFilter : "";
  const visible = assets?.filter((asset) => {
    const organization = organizations[asset.id];
    return (filter === "all" || asset.kind === filter)
      && (!activeGroup || organization?.group === activeGroup)
      && (!activeLabel || organization?.labels.some((label) => label.toLowerCase() === activeLabel.toLowerCase()));
  });
  const sections = new Map<string | null, Asset[]>();
  const showGroups = grouped && groups.length > 0;
  for (const asset of visible ?? []) {
    const key = showGroups ? organizations[asset.id]?.group ?? null : null;
    const items = sections.get(key);
    if (items) items.push(asset);
    else sections.set(key, [asset]);
  }
  const sectionsInOrder = [...groups, null].flatMap((key) => {
    const items = sections.get(key);
    return items ? [{ key, items }] : [];
  });
  return (
    <WorkspaceShell>
      <WorkspaceHeading icon={Images}>
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
        <p>{copy.intro}</p>
      </WorkspaceHeading>
      <section aria-labelledby="asset-list-title" className="flex flex-col gap-4">
        <h2 id="asset-list-title" className="sr-only">{copy.list}</h2>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <Tabs value={filter} onValueChange={setFilter}>
            <TabsList aria-label={copy.filter} className="h-auto max-w-full flex-wrap">
              <TabsTrigger value="all">{copy.all}</TabsTrigger>
              {kinds.map((kind) => <TabsTrigger key={kind} value={kind}>{copy[kind]}</TabsTrigger>)}
            </TabsList>
          </Tabs>
          <Button type="button" onClick={() => void importFiles()} disabled={!assets || busy !== null || editing !== null}>
            <Upload data-icon="inline-start" />{busy === "import" ? copy.picking : copy.pick}
          </Button>
        </div>
        {(groups.length > 0 || labels.length > 0) && <div className="flex flex-wrap gap-2">
          {groups.length > 0 && <AssetFilter label={copy.filterGroup} all={copy.allGroups}
            options={groups} value={activeGroup} onChange={setGroupFilter} />}
          {labels.length > 0 && <AssetFilter label={copy.filterLabel} all={copy.allLabels}
            options={labels} value={activeLabel} onChange={setLabelFilter} />}
          {groups.length > 0 && <div className="flex items-center gap-2 px-1">
            <Switch id="asset-group-view" checked={grouped} onCheckedChange={setGrouped} />
            <Label htmlFor="asset-group-view">{copy.groupView}</Label>
          </div>}
        </div>}
        {selectedAssets.length > 0 && <div className="flex flex-wrap items-center gap-2 rounded-md border bg-muted/30 p-2">
          <p role="status" className="me-auto px-1 text-sm tabular-nums">{selectedLabel}</p>
          <Button type="button" size="sm" variant="outline" disabled={busy !== null}
            onClick={() => edit(selectedAssets)}>{copy.editSelected}</Button>
          <Button type="button" size="sm" variant="ghost" disabled={busy !== null}
            onClick={() => setSelected(new Set())}>{copy.clearSelection}</Button>
        </div>}
        {progress && <div role="status" className="flex flex-col gap-2 text-sm">
          {copy.picking} {progress.completed}/{progress.total}
          <Progress aria-label={copy.picking} value={progress.completed / progress.total * 100} />
        </div>}
        {error === "importError" && <Alert variant="destructive"><AlertDescription>{copy.importError}</AlertDescription></Alert>}
        {loadError ? (
          <Alert variant="destructive">
            <AlertDescription>{copy.loadError}</AlertDescription>
            <Button type="button" variant="outline" onClick={() => {
              setLoadError(false);
              setAttempt((value) => value + 1);
            }}>{copy.retry}</Button>
          </Alert>
        ) : !assets ? (
          <div role="status" aria-label={copy.loading} className="grid grid-cols-2 gap-4 md:grid-cols-3">
            {[0, 1, 2].map((item) => <Skeleton key={item} className="aspect-video" />)}
          </div>
        ) : visible?.length ? (
          <div className="flex flex-col gap-5">
            {sectionsInOrder.map(({ key, items }) => (
              <section key={key === null ? "ungrouped" : `group:${key}`} className="flex flex-col gap-2"
                aria-label={showGroups ? key ?? copy.ungrouped : undefined}>
                {showGroups && <h3 className="wrap-anywhere text-sm font-semibold">{key ?? copy.ungrouped}</h3>}
                <ul className="grid grid-cols-[repeat(auto-fill,minmax(min(12rem,100%),1fr))] gap-4">
                  {items.map((asset) => (
                    <li key={asset.id} className="min-w-0">
                      <Card className="gap-3 pt-0">
                        <AssetThumbnail workspaceId={workspace.id} assetId={asset.id} kind={asset.kind} />
                        <CardContent className="flex min-w-0 flex-col gap-2">
                          <h3 title={asset.name} className="truncate font-semibold">{asset.name}</h3>
                          <div className="flex flex-wrap gap-1">
                            <Badge variant="secondary">{copy[asset.kind]}</Badge>
                            {organizations[asset.id]?.group && <Badge variant="secondary" className="max-w-full wrap-anywhere whitespace-normal">{organizations[asset.id].group}</Badge>}
                            {organizations[asset.id]?.labels.map((label) => <Badge key={label} variant="outline" className="max-w-full wrap-anywhere whitespace-normal">{label}</Badge>)}
                            {!asset.notes.trim() && <Badge variant="outline">{copy.noNotes}</Badge>}
                          </div>
                          {asset.notes.trim() && <p className="line-clamp-2 wrap-anywhere text-sm text-muted-foreground">{asset.notes}</p>}
                        </CardContent>
                        <CardFooter className="flex-wrap gap-2 py-2">
                          <Checkbox aria-label={`${copy.selectAsset} · ${asset.name}`}
                            checked={selected.has(asset.id)} disabled={busy !== null}
                            onCheckedChange={(checked) => setSelected((current) => {
                              const next = new Set(current);
                              if (checked === true) next.add(asset.id);
                              else next.delete(asset.id);
                              return next;
                            })} />
                          <Button type="button" size="sm" variant="ghost" disabled={busy !== null}
                            aria-label={`${copy.edit} · ${asset.name}`}
                            onClick={() => edit([asset])}>{copy.edit}</Button>
                          <AssetOrganizationDialog workspaceId={workspace.id} assetId={asset.id} name={asset.name}
                            organization={organizations[asset.id]} disabled={busy !== null}
                            onSave={(organization) => setOrganizations((current) => ({ ...current, [asset.id]: organization }))} />
                          <ConfirmTitleDelete title={asset.name} disabled={busy !== null} copy={copy.removal}
                            onConfirm={(confirmedName) => invoke<boolean>("remove_asset", {
                              workspaceId: workspace.id, assetId: asset.id, confirmedName,
                            })}
                            onDelete={() => onAssetRemoved(asset.id)} />
                        </CardFooter>
                      </Card>
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </div>
        ) : (
          <Empty className="min-h-56 rounded-xl border">
            <EmptyHeader><EmptyMedia variant="icon"><Images /></EmptyMedia><EmptyTitle>{copy.empty}</EmptyTitle></EmptyHeader>
            {assets.length === 0 && <Button onClick={() => void importFiles()} disabled={busy !== null}>{copy.pick}</Button>}
          </Empty>
        )}
      </section>
      <Dialog open={editing !== null} onOpenChange={(open) => {
        if (!open && busy === null) { setEditing(null); setError(null); }
      }}>
        {editing && <DialogContent showCloseButton={busy === null}>
          <DialogHeader>
            <DialogTitle>{editing.length === 1 ? copy.edit : copy.editSelected}</DialogTitle>
            <DialogDescription>{editing.length === 1 ? editing[0].name :
              copy.batchDescription.replace("{count}", String(editing.length))}</DialogDescription>
          </DialogHeader>
          <form onSubmit={saveNotes}>
            <FieldGroup>
              <MarkdownEditor id="asset-notes" label={copy.notes} value={notes}
                disabled={busy !== null} onChange={setNotes} />
              {error === "notesError" && <Alert variant="destructive"><AlertDescription>{copy.notesError}</AlertDescription></Alert>}
              <DialogFooter>
                <Button type="button" variant="outline" disabled={busy !== null} onClick={() => setEditing(null)}>{copy.cancel}</Button>
                <Button type="submit" disabled={busy !== null}>{copy.save}</Button>
              </DialogFooter>
            </FieldGroup>
          </form>
        </DialogContent>}
      </Dialog>
    </WorkspaceShell>
  );
}

function AssetFilter({ label, all, options, value, onChange }: {
  label: string;
  all: string;
  options: string[];
  value: string;
  onChange: (value: string) => void;
}) {
  return <Select value={value ? `value:${value}` : "all"} onValueChange={(next) =>
    onChange(next === "all" ? "" : next.slice("value:".length))}>
    <SelectTrigger aria-label={label} className="w-full sm:w-44"><SelectValue /></SelectTrigger>
    <SelectContent><SelectGroup>
      <SelectItem value="all">{all}</SelectItem>
      {options.map((option) => <SelectItem key={option} value={`value:${option}`}>{option}</SelectItem>)}
    </SelectGroup></SelectContent>
  </Select>;
}
