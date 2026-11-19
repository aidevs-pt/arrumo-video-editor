"use client";

import { useLocale } from "@/lib/locale-provider";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Upload, Images } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardFooter } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { AssetThumbnail } from "./asset-thumbnail";

const kinds = ["image", "audio", "video"] as const;
type MediaKind = typeof kinds[number];
type Asset = { id: number; name: string; kind: MediaKind; notes: string };

export function WorkspaceAssets({ workspace }: {
  workspace: { id: number; name: string };

}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [assets, setAssets] = useState<Asset[]>();
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [filter, setFilter] = useState("all");
  const [busy, setBusy] = useState<"import" | "save" | null>(null);
  const [progress, setProgress] = useState<{ completed: number; total: number }>();
  const [error, setError] = useState<"importError" | "notesError" | null>(null);
  const [editing, setEditing] = useState<Asset | null>(null);
  const { messages } = useLocale();
  const copy = messages.assets;

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    invoke<Asset[]>("list_assets", { workspaceId: workspace.id }).then(
      (rows) => { if (active) setAssets(rows); },
      () => { if (active) setLoadError(true); },
    );
    return () => { active = false; };
  }, [workspace.id, attempt]);

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
    const notes = String(new FormData(event.currentTarget).get("notes") ?? "");
    setBusy("save");
    setError(null);
    try {
      const asset = await invoke<Asset>("set_asset_notes", { workspaceId: workspace.id, assetId: editing.id, notes });
      setAssets((rows) => rows?.map((row) => row.id === asset.id ? asset : row));
      setEditing(null);
    } catch {
      setError("notesError");
    } finally {
      setBusy(null);
    }
  }

  const visible = assets?.filter((asset) => filter === "all" || asset.kind === filter);
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
          <ul className="grid grid-cols-[repeat(auto-fill,minmax(min(12rem,100%),1fr))] gap-4">
            {visible.map((asset) => (
              <li key={asset.id} className="min-w-0">
                <Card className="gap-3 pt-0">
                  <AssetThumbnail workspaceId={workspace.id} assetId={asset.id} kind={asset.kind} />
                  <CardContent className="flex min-w-0 flex-col gap-2">
                    <h3 title={asset.name} className="truncate font-semibold">{asset.name}</h3>
                    <div className="flex flex-wrap gap-1">
                      <Badge variant="secondary">{copy[asset.kind]}</Badge>
                      {!asset.notes.trim() && <Badge variant="outline">{copy.noNotes}</Badge>}
                    </div>
                    {asset.notes.trim() && <p className="line-clamp-2 wrap-anywhere text-sm text-muted-foreground">{asset.notes}</p>}
                  </CardContent>
                  <CardFooter className="py-2">
                    <Button type="button" size="sm" variant="ghost" disabled={busy !== null}
                      aria-label={`${copy.edit} · ${asset.name}`}
                      onClick={() => { setError(null); setEditing(asset); }}>{copy.edit}</Button>
                  </CardFooter>
                </Card>
              </li>
            ))}
          </ul>
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
          <DialogHeader><DialogTitle>{copy.edit}</DialogTitle><DialogDescription>{editing.name}</DialogDescription></DialogHeader>
          <form onSubmit={saveNotes}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="asset-notes">{copy.notes}</FieldLabel>
                <Textarea id="asset-notes" name="notes" defaultValue={editing.notes} disabled={busy !== null} />
              </Field>
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
