"use client";

import type { LocaleMessages } from "@/lib/locales";
import { ImagePlus, Images, X } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { FieldLegend, FieldSet } from "@/components/ui/field";
import { AssetPicker } from "@/components/workspace/assets/asset-picker";
import { AssetThumbnail } from "@/components/workspace/assets/asset-thumbnail";

export type ReferenceImage = { id: number; name: string };

/** The workspace images sent to thumbnail generation as references, chosen in the asset picker. */
export function ThumbnailReferences({ id, workspaceId, images, selected, copy, onChange, onImport }: {
  id: string;
  workspaceId: number;
  images: ReferenceImage[];
  selected: number[];
  copy: LocaleMessages["delivery"];
  onChange: (selected: number[]) => void;
  onImport?: () => void;
}) {
  const [picking, setPicking] = useState(false);
  const chosen = selected.map((value) => images.find((image) => image.id === value)).filter((image) => image !== undefined);
  return (
    <FieldSet id={id}>
      <FieldLegend>{copy.thumbnail_reference_ids}</FieldLegend>
      {chosen.length > 0 ? <ul className="grid grid-cols-[repeat(auto-fill,minmax(8rem,1fr))] gap-3">
        {chosen.map((image) => <li key={image.id} className="relative flex min-w-0 flex-col gap-1.5">
          <AssetThumbnail workspaceId={workspaceId} assetId={image.id} kind="image" fit="contain" transparency className="aspect-[4/3] w-full" />
          <span className="truncate text-xs font-medium" title={image.name}>{image.name}</span>
          <Button type="button" variant="secondary" size="icon-xs" className="absolute end-1.5 top-1.5"
            aria-label={copy.removeReference.replace("{name}", image.name)}
            onClick={() => onChange(selected.filter((value) => value !== image.id))}><X /></Button>
        </li>)}
      </ul> : <p className="text-sm text-muted-foreground">{images.length ? copy.noReferencesChosen : copy.noReferenceImages}</p>}
      <div className="flex flex-wrap gap-2">
        {images.length > 0 && <Button type="button" variant="outline" size="sm" onClick={() => setPicking(true)}>
          <Images data-icon="inline-start" />{copy.chooseReferences}
        </Button>}
        {onImport && <Button type="button" variant="outline" size="sm" onClick={onImport}>
          <ImagePlus data-icon="inline-start" />{copy.importReference}
        </Button>}
      </div>
      <AssetPicker workspaceId={workspaceId} open={picking} onOpenChange={setPicking} title={copy.thumbnail_reference_ids}
        description={copy.referencesPickerHint} kinds={["image"]} multiple value={selected} onConfirm={onChange} />
    </FieldSet>
  );
}
