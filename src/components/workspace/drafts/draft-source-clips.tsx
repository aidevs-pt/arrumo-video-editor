import { ArrowDown, ArrowUp, Film, Upload, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

export type DraftSource = { id: number; name: string };

export function DraftSourceClips({ assets, sources, disabled, importing, dragging, onImport, onChange }: {
  assets: DraftSource[];
  sources: number[];
  disabled: boolean;
  importing: boolean;
  dragging: boolean;
  onImport: () => void;
  onChange: (sourceIds: number[]) => void;
}) {
  const { messages: { drafts: copy, videoCatalog } } = useLocale();
  const names = new Map(assets.map((asset) => [asset.id, asset.name]));
  const selected = new Set(sources);

  function move(index: number, offset: number) {
    const next = [...sources];
    [next[index], next[index + offset]] = [next[index + offset], next[index]];
    onChange(next);
  }

  return (
    <div className="grid items-start gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(16rem,0.48fr)]">
      <section className="grid min-w-0 gap-4" aria-label={copy.sources}>
        <div className={cn("flex flex-col items-start gap-2 rounded-xl border border-dashed bg-muted p-4",
          dragging && "border-primary bg-accent")}>
          <div className="flex items-center gap-2 font-semibold">
            <Upload className="size-4 text-primary" aria-hidden="true" />
            {videoCatalog.drop}
          </div>
          <p className="text-sm text-muted-foreground">{copy.sourcesHint}</p>
          <Button type="button" variant="outline" disabled={disabled} onClick={onImport}>
            {importing ? copy.importing : copy.import}
          </Button>
        </div>
        {assets.length > 0 && (
          <div>
            <h2 className="font-sans mb-2 text-sm font-semibold">{copy.existingClips}</h2>
            <ul className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,8rem),1fr))] gap-2.5">
              {assets.map((asset) => (
                <li key={asset.id} className="min-w-0">
                  <label className={cn("flex cursor-pointer flex-col gap-1.5 rounded-lg border bg-card p-1.5 text-sm hover:border-primary",
                    selected.has(asset.id) && "border-primary ring-1 ring-primary",
                    disabled && "cursor-default opacity-70")}>
                    <span className="relative grid aspect-video place-items-center rounded-sm bg-muted text-muted-foreground">
                      <Film className="size-5" aria-hidden="true" />
                      <Checkbox className="absolute start-1.5 top-1.5" disabled={disabled} checked={selected.has(asset.id)}
                        onCheckedChange={(checked) => onChange(checked === true
                          ? [...sources, asset.id] : sources.filter((id) => id !== asset.id))} />
                    </span>
                    <span className="min-w-0 truncate px-1" title={asset.name}>{asset.name}</span>
                  </label>
                </li>
              ))}
            </ul>
          </div>
        )}
      </section>
      <Card size="sm" aria-labelledby="draft-sources-title">
        <CardHeader>
          <CardTitle><h2 id="draft-sources-title">{copy.order}</h2></CardTitle>
          {!sources.length && <CardDescription>{copy.noSources}</CardDescription>}
        </CardHeader>
        {sources.length > 0 && (
          <CardContent className="px-3">
            <ol className="divide-y border-y" aria-label={copy.order}>
              {sources.map((id, index) => {
                const name = names.get(id) ?? String(id);
                return (
                  <li key={id} className="grid grid-cols-[1.75rem_minmax(0,1fr)_auto] items-center gap-1 py-2">
                    <span className="text-center text-xs text-muted-foreground tabular-nums">{index + 1}</span>
                    <span className="truncate text-sm font-medium" title={name}>{name}</span>
                    <div className="flex gap-0.5">
                      <Button type="button" size="icon-sm" variant="ghost" disabled={disabled || index === 0}
                        aria-label={`${copy.up} ${name}`} title={copy.up} onClick={() => move(index, -1)}><ArrowUp /></Button>
                      <Button type="button" size="icon-sm" variant="ghost" disabled={disabled || index === sources.length - 1}
                        aria-label={`${copy.down} ${name}`} title={copy.down} onClick={() => move(index, 1)}><ArrowDown /></Button>
                      <Button type="button" size="icon-sm" variant="ghost" disabled={disabled}
                        aria-label={`${copy.removeSource} ${name}`} title={copy.removeSource}
                        onClick={() => onChange(sources.filter((sourceId) => sourceId !== id))}><X /></Button>
                    </div>
                  </li>
                );
              })}
            </ol>
          </CardContent>
        )}
      </Card>
    </div>
  );
}
