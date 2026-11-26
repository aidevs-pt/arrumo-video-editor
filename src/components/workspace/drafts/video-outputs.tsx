"use client";

import { Captions, ChevronDown, FileText, Image as ImageIcon, RectangleHorizontal, RectangleVertical, RotateCcw } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useId, useState } from "react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { ThumbnailReferences, type ReferenceImage } from "@/components/workspace/outputs/thumbnail-references";
import { AssetThumbnail } from "@/components/workspace/assets/asset-thumbnail";
import { CaptionStyleDialog, DEFAULT_CAPTION_STYLE, captionStyleChanges, type CaptionStyle } from "@/components/workspace/outputs/caption-style-dialog";

/** Reference images previewed in a format's summary; the rest are counted. */
const REFERENCE_PREVIEWS = 4;

type Format = { id: number; name: string; width: number; height: number };
type Supplemental = {
  thumbnails: number; titles: number; descriptions: number; tags: boolean;
  thumbnail_guidance?: string; specifications?: string; thumbnail_reference_ids?: number[];
  portrait_derivatives: { count: number; source?: { guidance?: string } };
  burned_captions?: boolean;
  caption_style?: CaptionStyle;
};
export type OutputPlan = { format: Format; supplemental: Supplemental };

/** What this video changes about one format; unset fields keep the profile's or workspace's value. */
export type FormatOverride = {
  thumbnails?: number; titles?: number; descriptions?: number; tags?: boolean;
  thumbnail_guidance?: string; specifications?: string; portrait_count?: number; portrait_guidance?: string;
  thumbnail_reference_ids?: number[]; burned_captions?: boolean; caption_style?: Partial<CaptionStyle>;
};
export type OutputChoice = { skipped: number[]; overrides: Record<string, FormatOverride> };

/** Run options for the choice: only what differs from the inherited settings. */
export function outputOptions(choice: OutputChoice, plan: OutputPlan[]) {
  const overrides = Object.fromEntries(Object.entries(choice.overrides)
    .map(([id, change]) => [id, Object.fromEntries(Object.entries(change).filter(([, value]) => value !== undefined))])
    .filter(([, change]) => Object.keys(change).length > 0));
  return {
    formats: choice.skipped.length ? plan.map(({ format }) => format.id).filter((id) => !choice.skipped.includes(id)) : null,
    format_overrides: overrides,
  };
}

export function VideoOutputs({ workspaceId, images, plan, choice, onChange, subtitles = true, disabled }: {
  workspaceId: number;
  /** Whether the video is delivered with subtitles. */
  subtitles?: boolean;
  images: ReferenceImage[];
  plan: OutputPlan[];
  choice: OutputChoice;
  onChange: (choice: OutputChoice) => void;
  disabled?: boolean;
}) {
  const { messages: { videoDetails: copy, delivery } } = useLocale();
  const [imported, setImported] = useState<ReferenceImage[]>([]);
  const [importFailed, setImportFailed] = useState(false);
  images = [...images, ...imported.filter((image) => !images.some((entry) => entry.id === image.id))];
  const produced = plan.filter(({ format }) => !choice.skipped.includes(format.id)).length;

  async function importReference(formatId: number, selected: number[]) {
    const source = await open({
      title: delivery.importReference,
      filters: [{ name: delivery.thumbnail_reference_ids, extensions: ["png", "jpg", "jpeg", "webp", "gif", "avif"] }],
    });
    if (typeof source !== "string") return;
    try {
      const image = await invoke<ReferenceImage>("import_asset", { workspaceId, source, thumbnailReference: true });
      setImportFailed(false);
      setImported((current) => [...current, image]);
      update(formatId, { thumbnail_reference_ids: [...selected, image.id] });
    } catch {
      setImportFailed(true);
    }
  }

  function update(id: number, change: FormatOverride) {
    onChange({ ...choice, overrides: { ...choice.overrides, [id]: { ...choice.overrides[id], ...change } } });
  }

  return (<>
    {importFailed && <p role="alert" className="mb-2 text-sm text-destructive">{delivery.importReferenceError}</p>}
    <ul className="flex flex-col divide-y">
      {plan.map(({ format, supplemental }) => {
        const skipped = choice.skipped.includes(format.id);
        const change = choice.overrides[format.id] ?? {};
        const value = {
          thumbnails: change.thumbnails ?? supplemental.thumbnails,
          titles: change.titles ?? supplemental.titles,
          descriptions: change.descriptions ?? supplemental.descriptions,
          tags: change.tags ?? supplemental.tags,
          portrait: change.portrait_count ?? supplemental.portrait_derivatives.count,
          captioned: change.burned_captions ?? supplemental.burned_captions ?? false,
        };
        const customised = Object.values(change).some((entry) => entry !== undefined);
        const Icon = format.height > format.width ? RectangleVertical : RectangleHorizontal;
        const references = (change.thumbnail_reference_ids ?? supplemental.thumbnail_reference_ids ?? [])
          .map((id) => images.find((image) => image.id === id)).filter((image): image is ReferenceImage => !!image);
        const lines: { icon: typeof Icon; text: string; previews?: ReferenceImage[] }[] = [];
        if (value.thumbnails) lines.push({ icon: ImageIcon, text: copy.makeThumbnails.replace("{count}", String(value.thumbnails)).replace("{format}", format.name.toLowerCase()), previews: references });
        if (value.portrait) lines.push({ icon: RectangleVertical, text: copy.makePortrait.replace("{count}", String(value.portrait)) });
        if (value.captioned && subtitles) lines.push({ icon: Captions, text: copy.makeCaptioned });
        if (value.titles || value.descriptions || value.tags) lines.push({ icon: FileText, text: [
          value.titles || value.descriptions ? copy.makeText.replace("{titles}", String(value.titles)).replace("{descriptions}", String(value.descriptions)) : "",
          value.tags ? copy.makeTags : "",
        ].filter(Boolean).join(", ") });
        return (
          <li key={format.id} className="flex flex-col gap-2 py-3 first:pt-0 last:pb-0">
            <div className="flex items-center justify-between gap-3">
              <span className={cn("flex min-w-0 items-center gap-2.5 text-sm font-semibold", skipped && "text-muted-foreground line-through")}>
                <Icon className="size-4 shrink-0" aria-hidden="true" />
                {(subtitles ? copy.makeVideo : copy.makeVideoOnly).replace("{format}", format.name.toLowerCase())}
              </span>
              <Switch checked={!skipped} disabled={disabled || (!skipped && produced === 1)}
                aria-label={copy.produceFormat.replace("{format}", format.name)}
                onCheckedChange={(on) => onChange({ ...choice, skipped: on ? choice.skipped.filter((id) => id !== format.id) : [...choice.skipped, format.id] })} />
            </div>
            {!skipped && <>
              {lines.length > 0 && <ul className="ms-6.5 flex flex-col gap-1.5 text-sm">
                {lines.map(({ icon: LineIcon, text, previews }) => (
                  <li key={text} className="flex flex-col gap-1.5">
                    <span className="flex items-start gap-2"><LineIcon className="mt-0.5 size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />{text}</span>
                    {previews && previews.length > 0 && <span className="ms-5.5 flex items-center gap-2 text-xs text-muted-foreground"
                      title={previews.map((image) => image.name).join(", ")}>
                      <span className="flex -space-x-2">
                        {previews.slice(0, REFERENCE_PREVIEWS).map((image) => (
                          <AssetThumbnail key={image.id} workspaceId={workspaceId} assetId={image.id} kind="image"
                            className="size-7 shrink-0 overflow-hidden rounded-[5px] ring-2 ring-card" />
                        ))}
                      </span>
                      {previews.length === 1 ? copy.referenceOne : copy.referenceCount.replace("{count}", String(previews.length))}
                    </span>}
                  </li>
                ))}
              </ul>}
              <Collapsible>
                <CollapsibleTrigger className="group ms-6.5 flex items-center gap-1 text-sm font-semibold text-primary">
                  {customised ? copy.customised : copy.customise}
                  <ChevronDown className="size-3.5 transition-transform group-data-[state=open]:rotate-180 motion-reduce:transition-none" aria-hidden="true" />
                </CollapsibleTrigger>
                <CollapsibleContent className="ms-6.5 mt-3 flex flex-col gap-3">
                  <div className="grid grid-cols-2 gap-3">
                    <Count label={copy.countThumbnails} value={value.thumbnails} inherited={change.thumbnails === undefined} disabled={disabled}
                      onChange={(count) => update(format.id, { thumbnails: count })} />
                    <Count label={copy.countPortrait} value={value.portrait} inherited={change.portrait_count === undefined} disabled={disabled}
                      onChange={(count) => update(format.id, { portrait_count: count })} />
                    <Count label={copy.countTitles} value={value.titles} inherited={change.titles === undefined} disabled={disabled}
                      onChange={(count) => update(format.id, { titles: count })} />
                    <Count label={copy.countDescriptions} value={value.descriptions} inherited={change.descriptions === undefined} disabled={disabled}
                      onChange={(count) => update(format.id, { descriptions: count })} />
                  </div>
                  <label className="flex items-center justify-between gap-3 text-sm font-semibold">{copy.countTags}
                    <Switch checked={value.tags} disabled={disabled} onCheckedChange={(tags) => update(format.id, { tags })} />
                  </label>
                  <label className="flex items-center justify-between gap-3 text-sm font-semibold">{copy.countCaptioned}
                    <Switch checked={value.captioned} disabled={disabled}
                      onCheckedChange={(burned_captions) => update(format.id, { burned_captions })} />
                  </label>
                  {value.captioned && <div className="flex items-center justify-between gap-3">
                    <span className={cn("text-sm", !change.caption_style && "text-muted-foreground")}>
                      {change.caption_style ? copy.captionStyleChanged : copy.captionStyleInherited}
                    </span>
                    <CaptionStyleDialog format={format} disabled={disabled}
                      value={{ ...(supplemental.caption_style ?? DEFAULT_CAPTION_STYLE), ...change.caption_style }}
                      onChange={(style) => update(format.id, {
                        caption_style: captionStyleChanges(supplemental.caption_style ?? DEFAULT_CAPTION_STYLE, style),
                      })} />
                  </div>}
                  <Guidance label={copy.thumbnailGuidance} inherited={supplemental.thumbnail_guidance ?? ""} value={change.thumbnail_guidance}
                    disabled={disabled} onChange={(text) => update(format.id, { thumbnail_guidance: text || undefined })} />
                  {value.portrait > 0 && <Guidance label={copy.portraitGuidance} inherited={supplemental.portrait_derivatives.source?.guidance ?? ""}
                    value={change.portrait_guidance} disabled={disabled} onChange={(text) => update(format.id, { portrait_guidance: text || undefined })} />}
                  {value.thumbnails > 0 && (
                    <ThumbnailReferences id={`video-reference-${format.id}`} workspaceId={workspaceId} images={images} copy={delivery}
                      selected={change.thumbnail_reference_ids ?? supplemental.thumbnail_reference_ids ?? []}
                      onChange={(ids) => update(format.id, { thumbnail_reference_ids: ids })}
                      onImport={disabled ? undefined : () => void importReference(format.id,
                        change.thumbnail_reference_ids ?? supplemental.thumbnail_reference_ids ?? [])} />
                  )}
                  <Guidance label={copy.specifications} inherited={supplemental.specifications ?? ""} value={change.specifications}
                    disabled={disabled} onChange={(text) => update(format.id, { specifications: text || undefined })} />
                  {customised && <button type="button" disabled={disabled}
                    className="flex items-center gap-1.5 self-start text-sm text-muted-foreground underline underline-offset-2 hover:text-foreground"
                    onClick={() => onChange({ ...choice, overrides: { ...choice.overrides, [format.id]: {} } })}>
                    <RotateCcw className="size-3.5" aria-hidden="true" />{copy.resetFormat}
                  </button>}
                </CollapsibleContent>
              </Collapsible>
            </>}
          </li>
        );
      })}
    </ul>
  </>);
}

function Count({ label, value, inherited, disabled, onChange }: {
  label: string; value: number; inherited: boolean; disabled?: boolean; onChange: (value: number) => void;
}) {
  const id = useId();
  return (
    <div className="flex flex-col gap-1">
      <Label htmlFor={id} className="text-xs">{label}</Label>
      <Input id={id} type="number" min={0} max={10} value={value} disabled={disabled}
        className={cn("h-8", inherited && "text-muted-foreground")}
        onChange={(event) => onChange(Math.max(0, Math.min(10, Number(event.target.value) || 0)))} />
    </div>
  );
}

function Guidance({ label, inherited, value, disabled, onChange }: {
  label: string; inherited: string; value: string | undefined; disabled?: boolean; onChange: (value: string) => void;
}) {
  const id = useId();
  return (
    <div className="flex flex-col gap-1">
      <Label htmlFor={id} className="text-xs">{label}</Label>
      <Textarea id={id} value={value ?? ""} placeholder={inherited} maxLength={8000} disabled={disabled}
        className="min-h-16 text-sm" onChange={(event) => onChange(event.target.value)} />
    </div>
  );
}
