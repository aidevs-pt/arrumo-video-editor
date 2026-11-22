"use client";

import { useLocale } from "@/lib/locale-provider";
import { WorkspaceColumns, WorkspaceForm, WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, FileText, Film, Lock, RotateCcw } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { LayerGauge } from "@/components/workspace/layer-gauge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { PortraitFields, type PortraitDerivatives, type SourceProfile } from "./portrait-fields";
import { ThumbnailReferences, type ReferenceImage } from "./thumbnail-references";

type Supplemental = {
  thumbnails: number;
  titles: number;
  descriptions: number;
  tags: boolean;
  thumbnail_guidance: string;
  thumbnail_reference_ids: number[];
  specifications: string;
  portrait_derivatives: PortraitDerivatives;
  burned_captions: boolean;
};

type Delivery = {
  required: ("final_video" | "subtitles")[];
  supplemental: Supplemental;
};

type ProfileDelivery = {
  workspace: Supplemental;
  effective: Delivery;
  overrides: { [Key in keyof Supplemental]: Supplemental[Key] | null };
};

const countFields = ["thumbnails", "titles", "descriptions"] as const;
const textFields = ["thumbnail_guidance", "specifications"] as const;

export function FormatDelivery({
  workspaceId, profileId, format, onBack, embedded = false, before, profileName,
}: {
  workspaceId: number;
  profileId?: number;
  format: { id: number; name: string; width: number; height: number };
  onBack?: () => void;
  /** Workspace settings for one format, laid out as rows inside its card on the Outputs page. */
  embedded?: boolean;
  /** Embedded profile mode: shown above the setting cards, such as the size overrides. */
  before?: ReactNode;
  /** Embedded profile mode: names the profile in the summary. */
  profileName?: string;
}) {
  const [delivery, setDelivery] = useState<Delivery>();
  const [profileDelivery, setProfileDelivery] = useState<ProfileDelivery>();
  const [overrides, setOverrides] = useState<Partial<Record<keyof Supplemental, boolean>>>({});
  const isProfile = profileId !== undefined;
  const [images, setImages] = useState<ReferenceImage[]>([]);
  const [references, setReferences] = useState<number[]>([]);
  const [profiles, setProfiles] = useState<SourceProfile[]>([]);
  const [portrait, setPortrait] = useState<PortraitDerivatives>();
  const showPortrait = format.width > format.height;
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [pending, setPending] = useState(false);
  const [result, setResult] = useState<"saved" | "saveError" | "importReferenceError" | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const { messages } = useLocale();
  const copy = messages.delivery;

  useEffect(() => { if (!embedded) heading.current?.focus(); }, [embedded]);
  useEffect(() => {
    let active = true;
    async function load() {
      try {
        const assets = await invoke<(ReferenceImage & { kind: string })[]>("list_assets", { workspaceId });
        if (!active) return;
        setImages(assets.filter((asset) => asset.kind === "image"));
        if (showPortrait) {
          const rows = await invoke<SourceProfile[]>("list_profiles", { workspaceId });
          if (!active) return;
          setProfiles(rows);
        }
        if (profileId === undefined) {
          const value = await invoke<Delivery>("get_format_delivery", { workspaceId, formatId: format.id });
          if (!active) return;
          setReferences(value.supplemental.thumbnail_reference_ids);
          setPortrait(value.supplemental.portrait_derivatives);
          setDelivery(value);
        } else {
          const value = await invoke<ProfileDelivery>("get_profile_delivery", { workspaceId, profileId, formatId: format.id });
          if (!active) return;
          setProfileDelivery(value);
          setReferences(value.effective.supplemental.thumbnail_reference_ids);
          setPortrait(value.effective.supplemental.portrait_derivatives);
          setDelivery(value.effective);
        }
      } catch {
        if (active) setLoadError(true);
      }
    }
    void load();
    return () => { active = false; };
  }, [workspaceId, profileId, format.id, attempt, showPortrait]);

  function overridden(field: keyof Supplemental) {
    return overrides[field] ?? (profileDelivery?.overrides[field] != null);
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending || !portrait) return;
    const fields = new FormData(event.currentTarget);
    const settings: Supplemental = {
      thumbnails: Number(fields.get("thumbnails")),
      titles: Number(fields.get("titles")),
      descriptions: Number(fields.get("descriptions")),
      tags: fields.has("tags"),
      thumbnail_guidance: String(fields.get("thumbnail_guidance") ?? ""),
      specifications: String(fields.get("specifications") ?? ""),
      thumbnail_reference_ids: references,
      portrait_derivatives: portrait,
      burned_captions: fields.has("burned_captions"),
    };
    setPending(true);
    setResult(null);
    try {
      if (isProfile) {
        const custom = Object.fromEntries((Object.keys(settings) as (keyof Supplemental)[])
          .filter(overridden).map((field) => [field, settings[field]]));
        await invoke("set_profile_delivery", { workspaceId, profileId, formatId: format.id, overrides: custom });
        setProfileDelivery(await invoke<ProfileDelivery>("get_profile_delivery", { workspaceId, profileId, formatId: format.id }));
      } else {
        await invoke("set_format_delivery", { workspaceId, formatId: format.id, settings });
      }
      setResult("saved");
    } catch {
      setResult("saveError");
    } finally {
      setPending(false);
    }
  }

  async function importReference(target: "thumbnail" | "portrait") {
    if (pending) return;
    setPending(true);
    setResult(null);
    try {
      const source = await open({
        title: copy.importReference,
        filters: [{ name: copy.thumbnail_reference_ids, extensions: ["png", "jpg", "jpeg", "webp", "gif", "avif"] }],
      });
      if (!source) return;
      const image = await invoke<ReferenceImage>("import_asset", {
        workspaceId, source, thumbnailReference: true,
      });
      setImages((current) => [...current, image]);
      if (target === "thumbnail") {
        setReferences((current) => [...current, image.id]);
      } else {
        setPortrait((current) => current?.source.type === "manual" ? {
          ...current,
          source: {
            ...current.source,
            thumbnail_reference_ids: [...current.source.thumbnail_reference_ids, image.id],
          },
        } : current);
      }
    } catch {
      setResult("importReferenceError");
    } finally {
      setPending(false);
    }
  }

  function overrideControl(field: keyof Supplemental) {
    if (!profileDelivery) return null;
    const value = profileDelivery.workspace[field];
    const display = typeof value === "boolean" ? (value ? copy.on : copy.off)
      : Array.isArray(value) ? value.map((id) => images.find((image) => image.id === id)?.name ?? id).join(", ") || copy.empty
      : typeof value === "object" ? String(value.count)
      : String(value) || copy.empty;
    return (
      <div className="flex flex-wrap items-start justify-between gap-2 rounded-lg border bg-card px-3 py-2">
        <div className="grid min-w-0 gap-1">
          <LayerGauge layer={overridden(field) ? "profile" : "workspace"} />
          <p className="max-h-24 overflow-auto whitespace-pre-wrap text-xs text-muted-foreground">
            {copy.workspaceValue}: {display}
          </p>
        </div>
        <Field orientation="horizontal" className="w-fit">
          <Switch id={`override-${field}`} checked={overridden(field)}
            aria-label={`${copy.override}, ${copy[field]}`}
            onCheckedChange={(checked) => {
              setResult(null);
              setOverrides((current) => ({ ...current, [field]: checked }));
            }} />
          <FieldLabel htmlFor={`override-${field}`}>{copy.override}</FieldLabel>
        </Field>
      </div>
    );
  }

  if (embedded && !isProfile) {
    const formatName = format.name.toLocaleLowerCase();
    const row = (title: string, hint: string, children: ReactNode) => (
      <div className="grid gap-3 px-4 py-5 sm:px-5 md:grid-cols-[minmax(0,15rem)_minmax(0,1fr)] md:gap-6">
        <div>
          <h4 className="font-semibold">{title}</h4>
          <p className="text-sm text-muted-foreground">{hint}</p>
        </div>
        <div className="flex min-w-0 flex-col gap-3">{children}</div>
      </div>
    );
    const count = (field: (typeof countFields)[number], label: string) => (
      <label className="flex items-center gap-2 text-sm">
        <span>{label}</span>
        <Input name={field} type="number" min={0} max={65535} required defaultValue={delivery?.supplemental[field]}
          className="w-20 tabular-nums" aria-label={copy[field]} />
      </label>
    );
    if (loadError) return (
      <Alert variant="destructive" className="m-4 w-auto">
        <AlertDescription>{copy.loadError}</AlertDescription>
        <Button type="button" variant="outline" onClick={() => { setLoadError(false); setAttempt((value) => value + 1); }}>{copy.retry}</Button>
      </Alert>
    );
    if (!delivery || !portrait) return <Skeleton className="m-4 h-40" role="status" aria-label={copy.loading} />;
    return (
      <form aria-busy={pending} onSubmit={save} onChange={() => setResult(null)}>
        <fieldset disabled={pending} className="divide-y divide-border/70">
          {row(copy.rowThumbnails, copy.rowThumbnailsHint.replace("{format}", formatName), <>
            <label className="flex items-center gap-2 text-sm">
              <Input name="thumbnails" type="number" min={0} max={65535} required defaultValue={delivery.supplemental.thumbnails}
                className="w-20 tabular-nums" aria-label={copy.thumbnails} />
              <span>{copy.perVideo}</span>
            </label>
            <Textarea name="thumbnail_guidance" aria-label={copy.thumbnail_guidance} defaultValue={delivery.supplemental.thumbnail_guidance}
              placeholder={copy.thumbnailGuidancePlaceholder} className="min-h-16" />
            <ThumbnailReferences id={`delivery-reference-${format.id}`} workspaceId={workspaceId} images={images} selected={references} copy={copy}
              onChange={setReferences} onImport={() => void importReference("thumbnail")} />
          </>)}
          {showPortrait && row(copy.rowPortrait, copy.rowPortraitHint.replace("{format}", formatName),
            <PortraitFields workspaceId={workspaceId} value={portrait} profiles={profiles} images={images} copy={copy}
              onChange={setPortrait} onImportReference={() => void importReference("portrait")} compact />)}
          {row(copy.rowText, copy.rowTextHint, <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
            {count("titles", copy.titlesShort)}
            {count("descriptions", copy.descriptionsShort)}
            <label className="flex items-center gap-2 text-sm">
              <Checkbox name="tags" defaultChecked={delivery.supplemental.tags} />{copy.tagsShort}
            </label>
          </div>)}
          {row(copy.rowCaptions, copy.rowCaptionsHint,
            <label className="flex items-center gap-2 text-sm">
              <Checkbox name="burned_captions" defaultChecked={delivery.supplemental.burned_captions} />{copy.burned_captions}
            </label>)}
          {row(copy.rowSpecs, copy.rowSpecsHint,
            <Textarea name="specifications" aria-label={copy.specifications} defaultValue={delivery.supplemental.specifications} className="min-h-20" />)}
          <div className="flex flex-wrap items-center justify-end gap-3 px-4 py-4 sm:px-5">
            {result === "saved" && <p role="status" className="text-sm text-muted-foreground">{copy.saved}</p>}
            {(result === "saveError" || result === "importReferenceError") && <p role="alert" className="text-sm text-destructive">{copy[result]}</p>}
            <Button type="submit">{copy.save}</Button>
          </div>
        </fieldset>
      </form>
    );
  }

  if (embedded && isProfile) {
    const formatName = format.name.toLocaleLowerCase();
    if (loadError) return (
      <Alert variant="destructive">
        <AlertDescription>{copy.loadError}</AlertDescription>
        <Button type="button" variant="outline" onClick={() => { setLoadError(false); setAttempt((value) => value + 1); }}>{copy.retry}</Button>
      </Alert>
    );
    if (!delivery || !portrait || !profileDelivery) return <Skeleton className="h-52 w-full" role="status" aria-label={copy.loading} />;
    const workspace = profileDelivery.workspace;
    const effective = profileDelivery.effective.supplemental;
    const layer = (field: keyof Supplemental) => profileDelivery.overrides[field] != null ? "profile" as const : "workspace" as const;
    const groups: { key: string; title: string; fields: (keyof Supplemental)[]; workspaceValue: string; body: ReactNode }[] = [
      { key: "thumbnails", title: copy.rowThumbnails, fields: ["thumbnails", "thumbnail_guidance", "thumbnail_reference_ids"], workspaceValue: String(workspace.thumbnails), body: <>
        <label className="flex flex-wrap items-center gap-2 text-sm">
          <Input name="thumbnails" type="number" min={0} max={65535} required defaultValue={delivery.supplemental.thumbnails}
            disabled={!overridden("thumbnails")} className="w-20 tabular-nums" aria-label={copy.thumbnails} />
          <span>{copy.perFormatVideo.replace("{format}", formatName)}</span>
        </label>
        <Field data-disabled={!overridden("thumbnail_guidance")}>
          <FieldLabel htmlFor={`thumbnail_guidance-${format.id}`}>{copy.whatEach}</FieldLabel>
          <Textarea id={`thumbnail_guidance-${format.id}`} name="thumbnail_guidance" defaultValue={delivery.supplemental.thumbnail_guidance}
            disabled={!overridden("thumbnail_guidance")} className="min-h-16" />
        </Field>
        {overridden("thumbnail_reference_ids")
          ? <ThumbnailReferences id={`delivery-reference-${format.id}`} workspaceId={workspaceId} images={images} selected={references} copy={copy}
              onChange={setReferences} onImport={() => void importReference("thumbnail")} />
          : <p className="text-sm text-muted-foreground">{copy.thumbnail_reference_ids}: {workspace.thumbnail_reference_ids.length || copy.empty}</p>}
      </> },
      ...(showPortrait ? [{ key: "portrait", title: copy.rowPortraitFrom.replace("{format}", formatName), fields: ["portrait_derivatives" as const], workspaceValue: String(workspace.portrait_derivatives.count),
        body: overridden("portrait_derivatives")
          ? <PortraitFields workspaceId={workspaceId} value={portrait} profiles={profiles} images={images} copy={copy}
              onChange={setPortrait} onImportReference={() => void importReference("portrait")} compact />
          : <p className="text-sm text-muted-foreground">{copy.portraitCount}: {workspace.portrait_derivatives.count}</p> }] : []),
      { key: "text", title: copy.rowText, fields: ["titles", "descriptions", "tags"], workspaceValue: `${workspace.titles}, ${workspace.descriptions}`, body:
        <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
          {(["titles", "descriptions"] as const).map((field) => (
            <label key={field} className="flex items-center gap-2 text-sm">
              <span>{field === "titles" ? copy.titlesShort : copy.descriptionsShort}</span>
              <Input name={field} type="number" min={0} max={65535} required defaultValue={delivery.supplemental[field]}
                disabled={!overridden(field)} className="w-20 tabular-nums" aria-label={copy[field]} />
            </label>
          ))}
          <label className="flex items-center gap-2 text-sm">
            <Checkbox name="tags" defaultChecked={delivery.supplemental.tags} disabled={!overridden("tags")} />{copy.tagsShort}
          </label>
        </div> },
      { key: "captions", title: copy.rowCaptions, fields: ["burned_captions"], workspaceValue: workspace.burned_captions ? copy.on : copy.off, body:
        <label className="flex items-center gap-2 text-sm">
          <Checkbox name="burned_captions" defaultChecked={delivery.supplemental.burned_captions} disabled={!overridden("burned_captions")} />{copy.burned_captions}
        </label> },
      { key: "specs", title: copy.rowSpecs, fields: ["specifications"], workspaceValue: workspace.specifications.trim() ? copy.written : copy.empty, body:
        <Textarea name="specifications" aria-label={copy.specifications} defaultValue={delivery.supplemental.specifications}
          disabled={!overridden("specifications")} className="min-h-20" /> },
    ];
    const setGroup = (fields: (keyof Supplemental)[], on: boolean) => {
      setResult(null);
      setOverrides((current) => ({ ...current, ...Object.fromEntries(fields.map((field) => [field, on])) }));
    };
    const summary: [string, string, keyof Supplemental | null][] = [
      [copy.summaryAlways, copy.summaryVideoSubtitles, null],
      [copy.rowThumbnails, String(effective.thumbnails), "thumbnails"],
      ...(showPortrait ? [[copy.rowPortrait, String(effective.portrait_derivatives.count), "portrait_derivatives"] as [string, string, keyof Supplemental]] : []),
      [copy.summaryTitles, `${effective.titles}, ${effective.descriptions}`, "titles"],
      [copy.tagsShort, effective.tags ? copy.on : copy.off, "tags"],
      [copy.rowCaptions, effective.burned_captions ? copy.on : copy.off, "burned_captions"],
      [copy.rowSpecs, effective.specifications.trim() ? copy.written : copy.empty, "specifications"],
    ];
    return (
      <div className="grid items-start gap-8 lg:grid-cols-[minmax(0,1fr)_17rem]">
        <div className="flex min-w-0 flex-col gap-4">
          <p className="flex items-start gap-3 rounded-xl bg-secondary p-4 text-sm">
            <Lock className="mt-0.5 size-4 shrink-0" aria-hidden="true" />{copy.alwaysShipsFormat.replace("{format}", formatName)}
          </p>
          {before}
          <form aria-busy={pending} onSubmit={save} onChange={() => setResult(null)}>
          <fieldset disabled={pending} className="flex flex-col gap-4">
            {groups.map((group) => {
              const on = group.fields.some(overridden);
              return (
                <section key={group.key} aria-labelledby={`group-${group.key}-${format.id}`} className="rounded-xl border bg-card p-4 sm:p-5">
                  <div className="mb-3 flex flex-wrap items-center gap-3">
                    <Switch id={`override-${group.key}-${format.id}`} checked={on} aria-label={`${copy.override}, ${group.title}`}
                      onCheckedChange={(checked) => setGroup(group.fields, checked)} />
                    <h3 id={`group-${group.key}-${format.id}`} className="min-w-0 flex-1 font-semibold">{group.title}</h3>
                    {on ? <>
                      <span className="rounded-md bg-attention px-2 py-1 text-xs font-semibold text-ink">
                        {copy.overriddenHere.replace("{value}", group.workspaceValue)}
                      </span>
                      <Button type="button" size="sm" variant="outline" onClick={() => setGroup(group.fields, false)}>
                        <RotateCcw data-icon="inline-start" />{copy.reset}
                      </Button>
                    </> : <LayerGauge layer="workspace" />}
                  </div>
                  <div className="flex flex-col gap-3">{group.body}</div>
                </section>
              );
            })}
            <div className="flex flex-wrap items-center justify-end gap-3">
              {result === "saved" && <p role="status" className="text-sm text-muted-foreground">{copy.saved}</p>}
              {(result === "saveError" || result === "importReferenceError") && <p role="alert" className="text-sm text-destructive">{copy[result]}</p>}
              <Button type="submit">{copy.save}</Button>
            </div>
          </fieldset>
          </form>
        </div>
        <aside aria-labelledby={`summary-${format.id}`} className="lg:sticky lg:top-4">
          <h3 id={`summary-${format.id}`} className="mb-3 font-semibold">
            {copy.summaryTitle.replace("{format}", formatName).replace("{profile}", profileName ?? "")}
          </h3>
          <dl className="divide-y rounded-xl border bg-card px-4 text-sm">
            <div className="flex items-center justify-between gap-3 py-2.5">
              <dt className="text-muted-foreground">{copy.summarySize}</dt>
              <dd className="tabular-nums">{format.width} × {format.height}</dd>
            </div>
            {summary.map(([label, value, field]) => (
              <div key={label} className="flex items-start justify-between gap-3 py-2.5">
                <dt className="text-muted-foreground">{label}</dt>
                <dd className="flex flex-col items-end gap-1 text-end">
                  <span className="tabular-nums">{value}</span>
                  {field ? <LayerGauge layer={layer(field)} /> : <LayerGauge layer="app" />}
                </dd>
              </div>
            ))}
          </dl>
          <p className="mt-3 text-sm text-muted-foreground">{copy.summaryHint}</p>
        </aside>
      </div>
    );
  }

  return (
    <WorkspaceShell>
      <Button type="button" variant="ghost" onClick={onBack} disabled={pending}>
        <ArrowLeft data-icon="inline-start" />
        {isProfile ? copy.backProfile : copy.back}
      </Button>
      <WorkspaceHeading>
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
        <p>{format.name}</p>
      </WorkspaceHeading>
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((value) => value + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : delivery ? (
        <WorkspaceColumns wideForm>
          <Card aria-labelledby="delivery-required">
            <CardHeader><CardTitle><h2 id="delivery-required">{copy.required}</h2></CardTitle></CardHeader>
            <CardContent>
              <ul className="flex flex-col gap-3 text-sm">
                {delivery.required.map((output) => {
                  const Icon = output === "final_video" ? Film : FileText;
                  return <li key={output} className="flex items-center gap-2"><Icon className="size-4 text-muted-foreground" aria-hidden="true" />{copy[output]}</li>;
                })}
              </ul>
            </CardContent>
          </Card>
          <WorkspaceForm  aria-labelledby="delivery-supplemental" aria-busy={pending}
            onSubmit={save} onChange={() => setResult(null)}>
            <h2 id="delivery-supplemental">{copy.supplemental}</h2>
            <fieldset disabled={pending}>
              <FieldGroup>
                <div className="grid grid-cols-[repeat(auto-fit,minmax(115px,1fr))] gap-3">
                  {countFields.map((field) => (
                    <Field key={field}>
                      <FieldLabel htmlFor={field}>{copy[field]}</FieldLabel>
                      <Input id={field} name={field} type="number" min={0} max={65535} required hidden={isProfile && !overridden(field)} disabled={isProfile && !overridden(field)}
                        defaultValue={delivery.supplemental[field]} />
                      {overrideControl(field)}
                    </Field>
                  ))}
                </div>
                <Field>
                  <Field orientation="horizontal">
                    <Checkbox id="tags" name="tags" disabled={isProfile && !overridden("tags")}
                      defaultChecked={delivery.supplemental.tags} />
                    <FieldLabel htmlFor="tags">{copy.tags}</FieldLabel>
                  </Field>
                  {overrideControl("tags")}
                </Field>
                <Field>
                  <Field orientation="horizontal">
                    <Checkbox id="burned_captions" name="burned_captions" disabled={isProfile && !overridden("burned_captions")}
                      defaultChecked={delivery.supplemental.burned_captions} />
                    <FieldLabel htmlFor="burned_captions">{copy.burned_captions}</FieldLabel>
                  </Field>
                  {overrideControl("burned_captions")}
                </Field>
                <Separator />
                {textFields.map((field) => (
                  <Field key={field}>
                    <FieldLabel htmlFor={field}>{copy[field]}</FieldLabel>
                    <Textarea id={field} name={field} hidden={isProfile && !overridden(field)}
                      disabled={isProfile && !overridden(field)} defaultValue={delivery.supplemental[field]} />
                    {overrideControl(field)}
                  </Field>
                ))}
                {isProfile && !overridden("thumbnail_reference_ids") && <FieldLabel>{copy.thumbnail_reference_ids}</FieldLabel>}
                {isProfile && overrideControl("thumbnail_reference_ids")}
                {(!isProfile || overridden("thumbnail_reference_ids")) && (
                  <ThumbnailReferences id="delivery-reference" workspaceId={workspaceId} images={images} selected={references} copy={copy} onChange={setReferences} onImport={() => void importReference("thumbnail")} />
                )}
                {showPortrait && portrait && (
                  <FieldGroup>
                    {isProfile && !overridden("portrait_derivatives") && <FieldLabel>{copy.portrait_derivatives}</FieldLabel>}
                    {isProfile && overrideControl("portrait_derivatives")}
                    {(!isProfile || overridden("portrait_derivatives")) && (
                      <PortraitFields workspaceId={workspaceId} value={portrait} profiles={profiles} images={images} copy={copy} onChange={setPortrait} onImportReference={() => void importReference("portrait")} />
                    )}
                  </FieldGroup>
                )}
                {result === "saved" && <p role="status">{copy.saved}</p>}
                {(result === "saveError" || result === "importReferenceError") && (
                  <Alert variant="destructive"><AlertDescription>{copy[result]}</AlertDescription></Alert>
                )}
                <Button type="submit">{copy.save}</Button>
              </FieldGroup>
            </fieldset>
          </WorkspaceForm>
        </WorkspaceColumns>
      ) : <Skeleton className="h-52 w-full" role="status" aria-label={copy.loading} />}
    </WorkspaceShell>
  );
}
