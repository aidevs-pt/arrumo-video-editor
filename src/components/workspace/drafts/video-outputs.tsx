"use client";

import { ChevronDown, FileText, Image as ImageIcon, RectangleHorizontal, RectangleVertical, RotateCcw } from "lucide-react";
import { useId } from "react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

type Format = { id: number; name: string; width: number; height: number };
type Supplemental = {
  thumbnails: number; titles: number; descriptions: number; tags: boolean;
  thumbnail_guidance?: string; specifications?: string;
  portrait_derivatives: { count: number; source?: { guidance?: string } };
};
export type OutputPlan = { format: Format; supplemental: Supplemental };

/** What this video changes about one format; unset fields keep the profile's or workspace's value. */
export type FormatOverride = {
  thumbnails?: number; titles?: number; descriptions?: number; tags?: boolean;
  thumbnail_guidance?: string; specifications?: string; portrait_count?: number; portrait_guidance?: string;
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

export function VideoOutputs({ plan, choice, onChange, disabled }: {
  plan: OutputPlan[];
  choice: OutputChoice;
  onChange: (choice: OutputChoice) => void;
  disabled?: boolean;
}) {
  const { messages: { videoDetails: copy } } = useLocale();
  const produced = plan.filter(({ format }) => !choice.skipped.includes(format.id)).length;

  function update(id: number, change: FormatOverride) {
    onChange({ ...choice, overrides: { ...choice.overrides, [id]: { ...choice.overrides[id], ...change } } });
  }

  return (
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
        };
        const customised = Object.values(change).some((entry) => entry !== undefined);
        const Icon = format.height > format.width ? RectangleVertical : RectangleHorizontal;
        const lines: { icon: typeof Icon; text: string }[] = [];
        if (value.thumbnails) lines.push({ icon: ImageIcon, text: copy.makeThumbnails.replace("{count}", String(value.thumbnails)).replace("{format}", format.name.toLowerCase()) });
        if (value.portrait) lines.push({ icon: RectangleVertical, text: copy.makePortrait.replace("{count}", String(value.portrait)) });
        if (value.titles || value.descriptions || value.tags) lines.push({ icon: FileText, text: [
          value.titles || value.descriptions ? copy.makeText.replace("{titles}", String(value.titles)).replace("{descriptions}", String(value.descriptions)) : "",
          value.tags ? copy.makeTags : "",
        ].filter(Boolean).join(", ") });
        return (
          <li key={format.id} className="flex flex-col gap-2 py-3 first:pt-0 last:pb-0">
            <div className="flex items-start justify-between gap-3">
              <span className={cn("flex items-start gap-2.5 font-semibold", skipped && "text-muted-foreground line-through")}>
                <Icon className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
                {copy.makeVideo.replace("{format}", format.name.toLowerCase())}
              </span>
              <Switch checked={!skipped} disabled={disabled || (!skipped && produced === 1)}
                aria-label={copy.produceFormat.replace("{format}", format.name)}
                onCheckedChange={(on) => onChange({ ...choice, skipped: on ? choice.skipped.filter((id) => id !== format.id) : [...choice.skipped, format.id] })} />
            </div>
            {!skipped && <>
              {lines.length > 0 && <ul className="ms-6.5 flex flex-col gap-1.5 text-sm">
                {lines.map(({ icon: LineIcon, text }) => (
                  <li key={text} className="flex items-start gap-2"><LineIcon className="mt-0.5 size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />{text}</li>
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
                  <Guidance label={copy.thumbnailGuidance} inherited={supplemental.thumbnail_guidance ?? ""} value={change.thumbnail_guidance}
                    disabled={disabled} onChange={(text) => update(format.id, { thumbnail_guidance: text || undefined })} />
                  {value.portrait > 0 && <Guidance label={copy.portraitGuidance} inherited={supplemental.portrait_derivatives.source?.guidance ?? ""}
                    value={change.portrait_guidance} disabled={disabled} onChange={(text) => update(format.id, { portrait_guidance: text || undefined })} />}
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
  );
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
