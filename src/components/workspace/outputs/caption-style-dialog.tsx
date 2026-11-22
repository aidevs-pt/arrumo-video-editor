"use client";

import { invoke } from "@tauri-apps/api/core";
import { Captions } from "lucide-react";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";

export type CaptionStyle = {
  font: "sans" | "serif" | "mono";
  bold: boolean;
  size: number;
  color: string;
  outline_color: string;
  outline: number;
  background: boolean;
  position: "bottom" | "middle" | "top";
  margin: number | null;
};

export const DEFAULT_CAPTION_STYLE: CaptionStyle = {
  font: "sans", bold: true, size: 55, color: "#FFFFFF", outline_color: "#000000", outline: 10,
  background: false, position: "bottom", margin: null,
};

/** Shows a caption style as a short line, such as "Sans, 5.5%, bottom". */
export function useCaptionSummary() {
  const { messages } = useLocale();
  const copy = messages.captionStyle;
  return (style: CaptionStyle) => [copy.fonts[style.font], `${(style.size / 10).toLocaleString()}%`, copy.positions[style.position]].join(", ");
}

/** Edits a caption style with a live preview drawn by the same renderer as the export. */
export function CaptionStyleDialog({ value, onChange, format, disabled, copySources = [], loadStyle }: {
  value: CaptionStyle;
  onChange: (style: CaptionStyle) => void;
  format: { width: number; height: number; name: string };
  disabled?: boolean;
  /** Other formats whose style can be copied in. */
  copySources?: { id: number; name: string }[];
  loadStyle?: (formatId: number) => Promise<CaptionStyle>;
}) {
  const { messages } = useLocale();
  const copy = messages.captionStyle;
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(value);
  const [sample, setSample] = useState(copy.sample);
  const [preview, setPreview] = useState<string>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!open) return;
    let active = true;
    const timer = window.setTimeout(() => {
      invoke<string>("preview_caption_style", { style: draft, width: format.width, height: format.height, text: sample.trim() || copy.sample })
        .then((url) => { if (active) { setPreview(url); setFailed(false); } }, () => { if (active) setFailed(true); });
    }, 250);
    return () => { active = false; window.clearTimeout(timer); };
  }, [open, draft, sample, format.width, format.height, copy.sample]);

  const set = <Key extends keyof CaptionStyle>(key: Key, next: CaptionStyle[Key]) => setDraft((current) => ({ ...current, [key]: next }));
  const portrait = format.height > format.width;

  return (
    <Dialog open={open} onOpenChange={(next) => { setOpen(next); if (next) setDraft(value); }}>
      <DialogTrigger asChild>
        <Button type="button" variant="outline" size="sm" disabled={disabled}><Captions data-icon="inline-start" />{copy.open}</Button>
      </DialogTrigger>
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>{copy.title.replace("{format}", format.name)}</DialogTitle>
          <DialogDescription>{copy.description}</DialogDescription>
        </DialogHeader>
        <div className="grid gap-6 md:grid-cols-[minmax(0,1fr)_16rem]">
          <div className="flex flex-col gap-3">
            <div className={portrait ? "mx-auto w-full max-w-64" : "w-full"}>
              {preview
                // eslint-disable-next-line @next/next/no-img-element
                ? <img src={preview} alt={copy.previewAlt} className="w-full rounded-lg border" style={{ aspectRatio: `${format.width} / ${format.height}` }} />
                : <div className="w-full animate-pulse rounded-lg bg-secondary motion-reduce:animate-none" style={{ aspectRatio: `${format.width} / ${format.height}` }} role="status" aria-label={copy.loading} />}
            </div>
            {failed && <Alert variant="destructive"><AlertDescription>{copy.previewError}</AlertDescription></Alert>}
            <Field>
              <FieldLabel htmlFor="caption-sample">{copy.sampleLabel}</FieldLabel>
              <Textarea id="caption-sample" value={sample} maxLength={200} rows={2} onChange={(event) => setSample(event.target.value)} />
            </Field>
          </div>
          <div className="flex flex-col gap-4">
            {copySources.length > 0 && loadStyle && <Field>
              <FieldLabel htmlFor="caption-copy">{copy.copyFrom}</FieldLabel>
              <NativeSelect id="caption-copy" value="" onChange={(event) => {
                const id = Number(event.target.value);
                if (id) void loadStyle(id).then(setDraft, () => setFailed(true));
              }}>
                <NativeSelectOption value="">{copy.copyChoose}</NativeSelectOption>
                {copySources.map((source) => <NativeSelectOption key={source.id} value={source.id}>{source.name}</NativeSelectOption>)}
              </NativeSelect>
            </Field>}
            <Field>
              <FieldLabel htmlFor="caption-font">{copy.font}</FieldLabel>
              <NativeSelect id="caption-font" value={draft.font} onChange={(event) => set("font", event.target.value as CaptionStyle["font"])}>
                {(["sans", "serif", "mono"] as const).map((font) => <NativeSelectOption key={font} value={font}>{copy.fonts[font]}</NativeSelectOption>)}
              </NativeSelect>
            </Field>
            <label className="flex items-center justify-between gap-3 text-sm font-medium">
              {copy.bold}<Switch checked={draft.bold} onCheckedChange={(checked) => set("bold", checked)} />
            </label>
            <Field>
              <FieldLabel htmlFor="caption-size">{copy.size.replace("{value}", (draft.size / 10).toLocaleString())}</FieldLabel>
              <input id="caption-size" type="range" min={20} max={120} step={1} value={draft.size}
                onChange={(event) => set("size", Number(event.target.value))} className="accent-primary" />
            </Field>
            <div className="grid grid-cols-2 gap-3">
              <Field>
                <FieldLabel htmlFor="caption-color">{copy.color}</FieldLabel>
                <Input id="caption-color" type="color" value={draft.color.toLowerCase()} className="h-10 p-1"
                  onChange={(event) => set("color", event.target.value.toUpperCase())} />
              </Field>
              <Field>
                <FieldLabel htmlFor="caption-outline-color">{draft.background ? copy.boxColor : copy.outlineColor}</FieldLabel>
                <Input id="caption-outline-color" type="color" value={draft.outline_color.toLowerCase()} className="h-10 p-1"
                  onChange={(event) => set("outline_color", event.target.value.toUpperCase())} />
              </Field>
            </div>
            <label className="flex items-center justify-between gap-3 text-sm font-medium">
              {copy.background}<Switch checked={draft.background} onCheckedChange={(checked) => set("background", checked)} />
            </label>
            {!draft.background && <Field>
              <FieldLabel htmlFor="caption-outline">{copy.outline.replace("{value}", String(draft.outline))}</FieldLabel>
              <input id="caption-outline" type="range" min={0} max={30} step={1} value={draft.outline}
                onChange={(event) => set("outline", Number(event.target.value))} className="accent-primary" />
            </Field>}
            <Field>
              <FieldLabel htmlFor="caption-position">{copy.position}</FieldLabel>
              <NativeSelect id="caption-position" value={draft.position} onChange={(event) => set("position", event.target.value as CaptionStyle["position"])}>
                {(["bottom", "middle", "top"] as const).map((position) => <NativeSelectOption key={position} value={position}>{copy.positions[position]}</NativeSelectOption>)}
              </NativeSelect>
            </Field>
            {draft.position !== "middle" && <Field>
              <FieldLabel htmlFor="caption-margin">{draft.margin === null ? copy.marginAuto : copy.margin.replace("{value}", String(draft.margin))}</FieldLabel>
              <input id="caption-margin" type="range" min={-1} max={40} step={1} value={draft.margin ?? -1}
                aria-valuetext={draft.margin === null ? copy.marginAuto : copy.margin.replace("{value}", String(draft.margin))}
                onChange={(event) => { const next = Number(event.target.value); set("margin", next < 0 ? null : next); }} className="accent-primary" />
            </Field>}
            <Button type="button" variant="ghost" size="sm" onClick={() => setDraft(DEFAULT_CAPTION_STYLE)}>{copy.reset}</Button>
          </div>
        </div>
        <DialogFooter>
          <DialogClose asChild><Button type="button" variant="outline">{copy.cancel}</Button></DialogClose>
          <Button type="button" onClick={() => { onChange(draft); setOpen(false); }}>{copy.apply}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
