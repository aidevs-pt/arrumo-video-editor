"use client";

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldGroup, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { draftCopy } from "./draft-copy";

type CatalogEntry = { id: number; name: string };
type Asset = CatalogEntry & { kind: string };
type Draft = { id: number; title: string; intent: string; profile_id: number | null; source_ids: number[] };

export function WorkspaceDrafts({ workspace, initialProfile, locale, onBack }: {
  workspace: CatalogEntry;
  initialProfile: number | null;
  locale: keyof typeof draftCopy;
  onBack: () => void;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [drafts, setDrafts] = useState<Draft[]>();
  const [assets, setAssets] = useState<Asset[]>([]);
  const [profiles, setProfiles] = useState<CatalogEntry[]>([]);
  const [profile, setProfile] = useState(initialProfile);
  const [intent, setIntent] = useState("");
  const [sources, setSources] = useState<number[]>([]);
  const [attempt, setAttempt] = useState(0);
  const [loadError, setLoadError] = useState(false);
  const [error, setError] = useState<"saveError" | "importError" | null>(null);
  const [pending, setPending] = useState<"saving" | "importing" | null>(null);
  const workspaceId = workspace.id;
  const copy = draftCopy[locale];
  const names = new Map(assets.map((asset) => [asset.id, asset.name]));

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Draft[]>("list_video_drafts", { workspaceId }),
      invoke<Asset[]>("list_assets", { workspaceId }),
      invoke<CatalogEntry[]>("list_profiles", { workspaceId }),
    ]).then(([items, media, entries]) => {
      if (!active) return;
      setDrafts(items);
      setAssets(media.filter((asset) => asset.kind === "video"));
      setProfiles(entries);
    }, () => { if (active) setLoadError(true); });
    return () => { active = false; };
  }, [workspaceId, attempt]);

  async function importClips() {
    if (pending) return;
    setPending("importing");
    setError(null);
    try {
      const files = await open({ title: copy.import, multiple: true,
        filters: [{ name: copy.sources, extensions: ["mp4", "mov", "webm", "mkv", "avi"] }] });
      for (const source of files ?? []) {
        try {
          const asset = await invoke<Asset>("import_asset", { workspaceId, source });
          if (asset.kind !== "video") {
            setError("importError");
            continue;
          }
          setAssets((current) => [...current, asset]);
          setSources((current) => [...current, asset.id]);
        } catch {
          setError("importError");
        }
      }
    } catch {
      setError("importError");
    } finally {
      setPending(null);
    }
  }

  function move(index: number, offset: number) {
    setSources((current) => {
      const next = [...current];
      [next[index], next[index + offset]] = [next[index + offset], next[index]];
      return next;
    });
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending || (!sources.length && !intent.trim())) return;
    const form = event.currentTarget;
    const title = String(new FormData(form).get("title") ?? "");
    setPending("saving");
    setError(null);
    try {
      const draft = await invoke<Draft>("create_video_draft", {
        workspaceId, settings: { title, intent, profile_id: profile, source_ids: sources },
      });
      setDrafts((current) => [...(current ?? []), draft]);
      form.reset();
      setProfile(initialProfile);
      setIntent("");
      setSources([]);
    } catch {
      setError("saveError");
    } finally {
      setPending(null);
    }
  }

  return (
    <main className="workspace-shell profile-shell">
      <Button type="button" variant="ghost" disabled={pending !== null} onClick={onBack}>
        <ArrowLeft data-icon="inline-start" />{copy.back}
      </Button>
      <div className="workspace-heading">
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1><p>{workspace.name}</p>
      </div>
      {loadError ? <Alert variant="destructive">
        <AlertDescription>{copy.loadError}</AlertDescription>
        <Button type="button" variant="outline" onClick={() => {
          setLoadError(false);
          setAttempt((value) => value + 1);
        }}>{copy.retry}</Button>
      </Alert> : !drafts ? <p role="status">{copy.loading}</p> : (
        <div className="workspace-layout">
          <section aria-label={copy.title}>
            {drafts.length === 0 ? <p>{copy.empty}</p> : (
              <ul className="workspace-rows" aria-live="polite">
                {drafts.map((draft) => (
                  <li key={draft.id}>
                    <h2>{draft.title}</h2>
                    {draft.profile_id !== null && <p>{profiles.find((entry) => entry.id === draft.profile_id)?.name}</p>}
                    {draft.intent && <p className="profile-guidance">{draft.intent}</p>}
                    {draft.source_ids.length > 0 && <ol className="draft-order">
                      {draft.source_ids.map((id) => <li key={id}>{names.get(id) ?? id}</li>)}
                    </ol>}
                  </li>
                ))}
              </ul>
            )}
          </section>
          <form className="workspace-form" onSubmit={save} aria-labelledby="new-draft-title" aria-busy={pending !== null}>
            <h2 id="new-draft-title">{copy.newDraft}</h2>
            <fieldset disabled={pending !== null}>
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="draft-title">{copy.name}</FieldLabel>
                  <Input id="draft-title" name="title" required />
                </Field>
                <Field>
                  <FieldLabel htmlFor="draft-profile">{copy.profile}</FieldLabel>
                  <select id="draft-profile" className="rounded-md border border-input bg-background px-2 py-1.5"
                    value={profile ?? ""} onChange={(event) => setProfile(event.target.value ? Number(event.target.value) : null)}>
                    <option value="">{copy.noProfile}</option>
                    {profiles.map((entry) => <option key={entry.id} value={entry.id}>{entry.name}</option>)}
                  </select>
                </Field>
                <Field>
                  <FieldLabel htmlFor="draft-intent">{copy.intent}</FieldLabel>
                  <Textarea id="draft-intent" value={intent} aria-describedby="draft-intent-hint" onChange={(event) => setIntent(event.target.value)} />
                  <FieldDescription id="draft-intent-hint">{copy.intentHint}</FieldDescription>
                </Field>
                <FieldSet>
                  <FieldLegend>{copy.sources}</FieldLegend>
                  <Button type="button" variant="outline" onClick={() => void importClips()}>{copy.import}</Button>
                  <FieldGroup>
                    {assets.map((asset) => (
                      <Field key={asset.id} orientation="horizontal">
                        <input id={`draft-asset-${asset.id}`} type="checkbox" checked={sources.includes(asset.id)}
                          onChange={(event) => {
                            const checked = event.target.checked;
                            setSources((current) => checked
                              ? [...current, asset.id] : current.filter((id) => id !== asset.id));
                          }} />
                        <FieldLabel htmlFor={`draft-asset-${asset.id}`}>{asset.name}</FieldLabel>
                      </Field>
                    ))}
                  </FieldGroup>
                </FieldSet>
                {sources.length > 0 && <ol className="draft-order" aria-label={copy.order}>
                  {sources.map((id, index) => (
                    <li key={id}>
                      {names.get(id) ?? id}
                      <div className="profile-edit-actions">
                        <Button type="button" size="sm" variant="outline" disabled={index === 0}
                          aria-label={`${copy.up} · ${names.get(id) ?? id}`} onClick={() => move(index, -1)}>{copy.up}</Button>
                        <Button type="button" size="sm" variant="outline" disabled={index === sources.length - 1}
                          aria-label={`${copy.down} · ${names.get(id) ?? id}`} onClick={() => move(index, 1)}>{copy.down}</Button>
                      </div>
                    </li>
                  ))}
                </ol>}
                {error && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
                <Button type="submit" disabled={!sources.length && !intent.trim()}>{pending === "saving" ? copy.saving : copy.save}</Button>
              </FieldGroup>
            </fieldset>
          </form>
        </div>
      )}
    </main>
  );
}
