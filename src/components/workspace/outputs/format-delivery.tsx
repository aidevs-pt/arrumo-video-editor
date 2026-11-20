"use client";

import { useLocale } from "@/lib/locale-provider";
import { WorkspaceColumns, WorkspaceForm, WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, FileText, Film } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
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
  workspaceId, profileId, format, onBack,
}: {
  workspaceId: number;
  profileId?: number;
  format: { id: number; name: string; width: number; height: number };

  onBack: () => void;
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

  useEffect(() => { heading.current?.focus(); }, []);
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
    };
    setPending(true);
    setResult(null);
    try {
      if (isProfile) {
        const custom = Object.fromEntries((Object.keys(settings) as (keyof Supplemental)[])
          .filter(overridden).map((field) => [field, settings[field]]));
        await invoke("set_profile_delivery", { workspaceId, profileId, formatId: format.id, overrides: custom });
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
                <Separator />
                {textFields.map((field) => (
                  <Field key={field}>
                    <FieldLabel htmlFor={field}>{copy[field]}</FieldLabel>
                    <Textarea id={field} name={field} hidden={isProfile && !overridden(field)}
                      disabled={isProfile && !overridden(field)} defaultValue={delivery.supplemental[field]} />
                    {overrideControl(field)}
                  </Field>
                ))}
                {isProfile && overrideControl("thumbnail_reference_ids")}
                {(!isProfile || overridden("thumbnail_reference_ids")) && (
                  <ThumbnailReferences id="delivery-reference" images={images} selected={references} copy={copy} onChange={setReferences} onImport={() => void importReference("thumbnail")} />
                )}
                {showPortrait && portrait && (
                  <FieldGroup>
                    {isProfile && overrideControl("portrait_derivatives")}
                    {(!isProfile || overridden("portrait_derivatives")) && (
                      <PortraitFields value={portrait} profiles={profiles} images={images} copy={copy} onChange={setPortrait} onImportReference={() => void importReference("portrait")} />
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
