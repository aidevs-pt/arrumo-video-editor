import type { LocaleMessages } from "@/lib/locales";
import { ImagePlus } from "lucide-react";
import { Checkbox } from "@/components/ui/checkbox";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";

export type ReferenceImage = { id: number; name: string };

export function ThumbnailReferences({ id, images, selected, copy, onChange, onImport }: {
  id: string;
  images: ReferenceImage[];
  selected: number[];
  copy: LocaleMessages["delivery"];
  onChange: (selected: number[]) => void;
  onImport: () => void;
}) {
  return (
    <FieldSet>
      <FieldLegend>{copy.thumbnail_reference_ids}</FieldLegend>
      {images.length === 0 ? <p className="text-sm text-muted-foreground">{copy.noReferenceImages}</p> : (
        <ScrollArea className={images.length > 6 ? "h-48 rounded-lg border p-3" : "rounded-lg border p-3"}>
          <FieldGroup className="gap-3">
          {images.map((image) => (
            <Field key={image.id} orientation="horizontal">
              <Checkbox id={`${id}-${image.id}`} checked={selected.includes(image.id)}
                onCheckedChange={(checked) => onChange(checked === true
                  ? [...selected, image.id]
                  : selected.filter((value) => value !== image.id))} />
              <FieldLabel htmlFor={`${id}-${image.id}`}>{image.name}</FieldLabel>
            </Field>
          ))}
        </FieldGroup>
        </ScrollArea>
      )}
      <Button type="button" size="sm" variant="outline" className="self-start" onClick={onImport}><ImagePlus data-icon="inline-start" />{copy.importReference}</Button>
    </FieldSet>
  );
}
