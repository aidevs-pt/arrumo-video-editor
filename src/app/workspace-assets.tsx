"use client";

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, Upload } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";
import { assetCopy } from "./asset-copy";

const kinds = ["image", "audio", "video"] as const;
type MediaKind = typeof kinds[number];
type Asset = { id: number; name: string; kind: MediaKind; notes: string };

export function WorkspaceAssets({ workspace, locale, onBack }: {
  workspace: { id: number; name: string };
  locale: keyof typeof assetCopy;
  onBack: () => void;
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
  const copy = assetCopy[locale];

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
    <main className="workspace-shell profile-shell">
      <Button type="button" variant="ghost" onClick={onBack} disabled={busy !== null}>
        <ArrowLeft data-icon="inline-start" />
        {copy.back}
      </Button>
      <div className="workspace-heading">
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
        <p>{workspace.name}</p>
      </div>
      <section aria-labelledby="asset-list-title">
        <div className="profile-row-heading asset-toolbar">
          <h2 id="asset-list-title" className="profile-name">{copy.list}</h2>
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
            <Field orientation="horizontal" className="asset-toolbar">
              <FieldLabel htmlFor="asset-filter">{copy.filter}</FieldLabel>
              <select id="asset-filter" className="rounded-md border border-input bg-background px-2 py-1.5"
                disabled={editing !== null} value={filter} onChange={(event) => setFilter(event.target.value)}>
                <option value="all">{copy.all}</option>
                {kinds.map((kind) => <option key={kind} value={kind}>{copy[kind]}</option>)}
              </select>
            </Field>
            {error === "importError" && <Alert variant="destructive"><AlertDescription>{copy.importError}</AlertDescription></Alert>}
            {visible?.length ? (
              <ul className="workspace-rows">
                {visible.map((asset) => (
                  <li key={asset.id}>
                    <div className="profile-row-heading">
                      <div>
                        <h3 className="profile-name">{asset.name}</h3>
                        <p className="profile-guidance">{copy[asset.kind]}</p>
                      </div>
                      <Button type="button" size="sm" variant="outline" disabled={busy !== null || editing !== null}
                        aria-label={`${copy.edit} · ${asset.name}`}
                        onClick={() => { setError(null); setEditing(asset); }}>{copy.edit}</Button>
                    </div>
                    {editing?.id === asset.id ? (
                      <form onSubmit={saveNotes} className="asset-note-form">
                        <FieldGroup>
                          <Field>
                            <FieldLabel htmlFor="asset-notes">{copy.notes}</FieldLabel>
                            <Textarea id="asset-notes" name="notes" defaultValue={asset.notes} disabled={busy !== null} />
                          </Field>
                          {error === "notesError" && <Alert variant="destructive"><AlertDescription>{copy.notesError}</AlertDescription></Alert>}
                          <div className="profile-edit-actions">
                            <Button type="submit" disabled={busy !== null}>{copy.save}</Button>
                            <Button type="button" variant="outline" disabled={busy !== null}
                              onClick={() => { setError(null); setEditing(null); }}>{copy.cancel}</Button>
                          </div>
                        </FieldGroup>
                      </form>
                    ) : <p className="profile-guidance">{asset.notes || copy.noNotes}</p>}
                  </li>
                ))}
              </ul>
            ) : <p>{copy.empty}</p>}
          </>
        )}
      </section>
    </main>
  );
}
