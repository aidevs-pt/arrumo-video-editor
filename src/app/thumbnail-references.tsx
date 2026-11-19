import type { LocaleMessages } from "@/lib/locales";
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
      {images.length === 0 ? <p>{copy.noReferenceImages}</p> : (
        <FieldGroup>
          {images.map((image) => (
            <Field key={image.id} orientation="horizontal">
              <input id={`${id}-${image.id}`} type="checkbox" checked={selected.includes(image.id)}
                onChange={(event) => onChange(event.target.checked
                  ? [...selected, image.id]
                  : selected.filter((value) => value !== image.id))} />
              <FieldLabel htmlFor={`${id}-${image.id}`}>{image.name}</FieldLabel>
            </Field>
          ))}
        </FieldGroup>
      )}
      <Button type="button" size="sm" variant="outline" className="self-start" onClick={onImport}>{copy.importReference}</Button>
    </FieldSet>
  );
}
