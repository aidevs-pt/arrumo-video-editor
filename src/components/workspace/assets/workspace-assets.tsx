"use client";

import { useLocale } from "@/lib/locale-provider";
import { WorkspaceHeading, WorkspaceRows, WorkspaceShell } from "@/components/workspace/layout";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Upload, Images } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";

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
        <p>{workspace.name}</p>
      </WorkspaceHeading>
      <section aria-labelledby="asset-list-title">
        <div className="mb-4 flex flex-wrap items-baseline justify-between gap-2.5">
          <h2 id="asset-list-title" className="flex min-w-0 items-baseline gap-2.5 font-[680]">{copy.list}</h2>
          <Button type="button" onClick={() => void importFiles()} disabled={!assets || busy !== null || editing !== null}>
            <Upload data-icon="inline-start" />
            {busy === "import" ? copy.picking : copy.pick}
          </Button>
        </div>
        {progress && <p role="status">{copy.picking} {progress.completed}/{progress.total}</p>}
        {loadError ? (
          <Alert variant="destructive">
            <AlertDescription>{copy.loadError}</AlertDescription>
            <Button type="button" variant="outline" onClick={() => {
              setLoadError(false);
              setAttempt((value) => value + 1);
            }}>{copy.retry}</Button>
          </Alert>
        ) : !assets ? <p role="status">{copy.loading}</p> : (
          <>
            <Field orientation="horizontal" className="mb-4">
              <FieldLabel htmlFor="asset-filter">{copy.filter}</FieldLabel>
              <NativeSelect id="asset-filter" className="w-full"
                disabled={editing !== null} value={filter} onChange={(event) => setFilter(event.target.value)}>
                <NativeSelectOption value="all">{copy.all}</NativeSelectOption>
                {kinds.map((kind) => <NativeSelectOption key={kind} value={kind}>{copy[kind]}</NativeSelectOption>)}
              </NativeSelect>
            </Field>
            {error === "importError" && <Alert variant="destructive"><AlertDescription>{copy.importError}</AlertDescription></Alert>}
            {visible?.length ? (
              <WorkspaceRows>
                {visible.map((asset) => (
                  <li key={asset.id}>
                    <div className="flex flex-wrap items-baseline justify-between gap-2.5">
                      <div>
                        <h3 className="flex min-w-0 items-baseline gap-2.5 font-[680]">{asset.name}</h3>
                        <p className="mt-1.25 whitespace-pre-wrap text-muted-foreground">{copy[asset.kind]}</p>
                      </div>
                      <Button type="button" size="sm" variant="outline" disabled={busy !== null || editing !== null}
                        aria-label={`${copy.edit} · ${asset.name}`}
                        onClick={() => { setError(null); setEditing(asset); }}>{copy.edit}</Button>
                    </div>
                    {editing?.id === asset.id ? (
                      <form onSubmit={saveNotes} className="mt-3.5">
                        <FieldGroup>
                          <Field>
                            <FieldLabel htmlFor="asset-notes">{copy.notes}</FieldLabel>
                            <Textarea id="asset-notes" name="notes" defaultValue={asset.notes} disabled={busy !== null} />
                          </Field>
                          {error === "notesError" && <Alert variant="destructive"><AlertDescription>{copy.notesError}</AlertDescription></Alert>}
                          <div className="flex flex-wrap items-baseline gap-2.5">
                            <Button type="submit" disabled={busy !== null}>{copy.save}</Button>
                            <Button type="button" variant="outline" disabled={busy !== null}
                              onClick={() => { setError(null); setEditing(null); }}>{copy.cancel}</Button>
                          </div>
                        </FieldGroup>
                      </form>
                    ) : <p className="mt-1.25 whitespace-pre-wrap text-muted-foreground">{asset.notes || copy.noNotes}</p>}
                  </li>
                ))}
              </WorkspaceRows>
            ) : <p>{copy.empty}</p>}
          </>
        )}
      </section>
    </WorkspaceShell>
  );
}
