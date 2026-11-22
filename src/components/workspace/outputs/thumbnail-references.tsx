import type { LocaleMessages } from "@/lib/locales";
import { Check, ImagePlus } from "lucide-react";
import { FieldLegend, FieldSet } from "@/components/ui/field";
import { ScrollArea } from "@/components/ui/scroll-area";
import { AssetThumbnail } from "@/components/workspace/assets/asset-thumbnail";
import { cn } from "@/lib/utils";

export type ReferenceImage = { id: number; name: string };

/** Workspace images shown as previews; selected ones are sent to image generation as references. */
export function ThumbnailReferences({ id, workspaceId, images, selected, copy, onChange, onImport }: {
  id: string;
  workspaceId: number;
  images: ReferenceImage[];
  selected: number[];
  copy: LocaleMessages["delivery"];
  onChange: (selected: number[]) => void;
  onImport?: () => void;
}) {
  return (
    <FieldSet>
      <FieldLegend>{copy.thumbnail_reference_ids}</FieldLegend>
      {images.length === 0 && !onImport ? <p className="text-sm text-muted-foreground">{copy.noReferenceImages}</p> : (
        <ScrollArea className={cn("rounded-lg border", images.length > 6 && "h-72")}>
          <ul className="grid grid-cols-[repeat(auto-fill,minmax(5rem,1fr))] gap-3 p-3">
            {images.map((image) => {
              const checked = selected.includes(image.id);
              return (
                <li key={image.id}>
                  <label htmlFor={`${id}-${image.id}`} className={cn(
                    "group relative flex cursor-pointer flex-col gap-1.5 rounded-lg p-1.5 outline-none transition-colors motion-reduce:transition-none",
                    "has-[:focus-visible]:ring-[3px] has-[:focus-visible]:ring-ring/50",
                    checked ? "bg-primary/10 ring-2 ring-primary" : "hover:bg-muted")}>
                    <input id={`${id}-${image.id}`} type="checkbox" className="sr-only" checked={checked}
                      onChange={(event) => onChange(event.target.checked
                        ? [...selected, image.id]
                        : selected.filter((value) => value !== image.id))} />
                    <AssetThumbnail workspaceId={workspaceId} assetId={image.id} kind="image" />
                    <span className="truncate text-xs font-medium" title={image.name}>{image.name}</span>
                    {checked && (
                      <span aria-hidden="true" className="absolute end-2.5 top-2.5 grid size-5 place-items-center rounded-full bg-primary text-primary-foreground shadow-sm">
                        <Check className="size-3.5" />
                      </span>
                    )}
                  </label>
                </li>
              );
            })}
            {onImport && <li className="p-1.5">
              <button type="button" onClick={onImport}
                className="flex aspect-video w-full flex-col items-center justify-center gap-1 rounded-thumb border border-dashed text-muted-foreground transition-colors hover:border-primary hover:text-primary focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none motion-reduce:transition-none">
                <ImagePlus className="size-5" aria-hidden="true" />
                <span className="sr-only">{copy.importReference}</span>
              </button>
              <span className="mt-1.5 block truncate text-xs font-medium text-muted-foreground" aria-hidden="true">{copy.uploadReference}</span>
            </li>}
          </ul>
        </ScrollArea>
      )}
    </FieldSet>
  );
}
