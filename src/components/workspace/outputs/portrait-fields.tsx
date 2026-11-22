import type { LocaleMessages } from "@/lib/locales";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Field, FieldGroup, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { ThumbnailReferences, type ReferenceImage } from "./thumbnail-references";

type ManualSource = {
  type: "manual";
  guidance: string;
  thumbnails_per_video: number;
  thumbnail_guidance: string;
  thumbnail_reference_ids: number[];
};
type ProfileSource = { type: "profile"; profile_id: number };
export type PortraitDerivatives = { count: number; source: ManualSource | ProfileSource };
export type SourceProfile = { id: number; name: string };

const emptyManual: ManualSource = {
  type: "manual", guidance: "", thumbnails_per_video: 0, thumbnail_guidance: "", thumbnail_reference_ids: [],
};

export function PortraitFields({ workspaceId, value, profiles, images, copy, onChange, onImportReference, compact = false }: {
  workspaceId: number;
  value: PortraitDerivatives;
  profiles: SourceProfile[];
  images: ReferenceImage[];
  copy: LocaleMessages["delivery"];
  onChange: (value: PortraitDerivatives) => void;
  onImportReference: () => void;
  /** Inside a labelled row: no legend, and short number inputs. */
  compact?: boolean;
}) {
  const { count, source } = value;
  return (
    <FieldSet>
      <FieldLegend className={compact ? "sr-only" : undefined}>{copy.portrait_derivatives}</FieldLegend>
      <FieldGroup>
        <Field>
          <FieldLabel htmlFor="portrait-count">{copy.portraitCount}</FieldLabel>
          <Input id="portrait-count" className={compact ? "w-24 tabular-nums" : undefined} type="number" min={0} max={65535} required value={count}
            onChange={(event) => onChange({ ...value, count: Number(event.target.value) })} />
        </Field>
        {count > 0 && (
          <Field>
            <FieldLabel htmlFor="portrait-source">{copy.portraitSource}</FieldLabel>
            <Select value={source.type === "manual" ? "manual" : String(source.profile_id)}
              onValueChange={(selected) => onChange({
                ...value,
                source: selected === "manual" ? emptyManual : { type: "profile", profile_id: Number(selected) },
              })}>
              <SelectTrigger id="portrait-source" className="w-full"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="manual">{copy.portraitManual}</SelectItem>
                  {profiles.map((profile) => <SelectItem key={profile.id} value={String(profile.id)}>{profile.name}</SelectItem>)}
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
        )}
        {count > 0 && source.type === "profile" && (
          <p className="text-sm text-muted-foreground">
            {copy.portraitFollowsProfile.replace("{profile}", profiles.find((profile) => profile.id === source.profile_id)?.name ?? "")}
          </p>
        )}
        {count > 0 && source.type === "manual" && (
          <>
            <Field>
              <FieldLabel htmlFor="portrait-guidance">{copy.portraitGuidance}</FieldLabel>
              <Textarea id="portrait-guidance" value={source.guidance}
                onChange={(event) => onChange({ ...value, source: { ...source, guidance: event.target.value } })} />
            </Field>
            <Field>
              <FieldLabel htmlFor="portrait-thumbnails">{copy.portraitThumbnails}</FieldLabel>
              <Input id="portrait-thumbnails" className={compact ? "w-24 tabular-nums" : undefined} type="number" min={0} max={65535} required value={source.thumbnails_per_video}
                onChange={(event) => onChange({ ...value, source: { ...source, thumbnails_per_video: Number(event.target.value) } })} />
            </Field>
            <Field>
              <FieldLabel htmlFor="portrait-thumbnail-guidance">{copy.portraitThumbnailGuidance}</FieldLabel>
              <Textarea id="portrait-thumbnail-guidance" value={source.thumbnail_guidance}
                onChange={(event) => onChange({ ...value, source: { ...source, thumbnail_guidance: event.target.value } })} />
            </Field>
            <ThumbnailReferences id="portrait-reference" workspaceId={workspaceId} images={images} selected={source.thumbnail_reference_ids} copy={copy} onImport={onImportReference}
              onChange={(ids) => onChange({ ...value, source: { ...source, thumbnail_reference_ids: ids } })} />
          </>
        )}
      </FieldGroup>
    </FieldSet>
  );
}
