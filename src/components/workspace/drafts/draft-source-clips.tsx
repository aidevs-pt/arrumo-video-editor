import { ArrowDown, ArrowUp, Film, Upload, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { useLocale } from "@/lib/locale-provider";

export type DraftSource = { id: number; name: string };

export function DraftSourceClips({ assets, sources, disabled, importing, onImport, onChange }: {
  assets: DraftSource[];
  sources: number[];
  disabled: boolean;
  importing: boolean;
  onImport: () => void;
  onChange: (sourceIds: number[]) => void;
}) {
  const { messages: { drafts: copy } } = useLocale();
  const names = new Map(assets.map((asset) => [asset.id, asset.name]));

  function move(index: number, offset: number) {
    const next = [...sources];
    [next[index], next[index + offset]] = [next[index + offset], next[index]];
    onChange(next);
  }

  return (
    <Card aria-labelledby="draft-sources-title">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <h2 id="draft-sources-title">{copy.sources}</h2>
          {sources.length > 0 && <Badge variant="secondary">{sources.length}</Badge>}
        </CardTitle>
        <CardDescription>{copy.sourcesHint}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        {sources.length ? (
          <div className="grid gap-2">
            <h3 className="text-sm font-medium">{copy.order}</h3>
            <ScrollArea className={sources.length > 5 ? "h-60 rounded-lg border" : "rounded-lg border"}>
              <ol className="divide-y" aria-label={copy.order}>
                {sources.map((id, index) => {
                  const name = names.get(id) ?? String(id);
                  return (
                    <li key={id} className="flex min-w-0 items-center gap-2 px-2 py-1.5">
                      <span className="w-6 shrink-0 text-center text-xs text-muted-foreground tabular-nums">{index + 1}</span>
                      <span className="min-w-0 flex-1 truncate text-sm font-medium" title={name}>{name}</span>
                      <div className="flex shrink-0 gap-0.5">
                        <Button type="button" size="icon-sm" variant="ghost" disabled={disabled || index === 0}
                          aria-label={`${copy.up} · ${name}`} title={copy.up} onClick={() => move(index, -1)}><ArrowUp /></Button>
                        <Button type="button" size="icon-sm" variant="ghost" disabled={disabled || index === sources.length - 1}
                          aria-label={`${copy.down} · ${name}`} title={copy.down} onClick={() => move(index, 1)}><ArrowDown /></Button>
                        <Button type="button" size="icon-sm" variant="ghost" disabled={disabled}
                          aria-label={`${copy.removeSource} · ${name}`} title={copy.removeSource}
                          onClick={() => onChange(sources.filter((sourceId) => sourceId !== id))}><X /></Button>
                      </div>
                    </li>
                  );
                })}
              </ol>
            </ScrollArea>
          </div>
        ) : (
          <Empty className="border-dashed p-4 md:p-6">
            <EmptyHeader>
              <EmptyMedia variant="icon"><Film /></EmptyMedia>
              <EmptyTitle>{copy.noSources}</EmptyTitle>
              <EmptyDescription>{copy.noSourcesHint}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        )}
        <Button type="button" variant="outline" className="justify-self-start" disabled={disabled} onClick={onImport}>
          <Upload data-icon="inline-start" />{importing ? copy.importing : copy.import}
        </Button>
        {assets.length > 0 && (
          <>
            <Separator />
            <div className="grid gap-2">
              <h3 className="text-sm font-medium">{copy.existingClips}</h3>
              <ScrollArea className={assets.length > 5 ? "h-48 rounded-lg border" : "rounded-lg border"}>
                <ul className="divide-y">
                  {assets.map((asset) => (
                    <li key={asset.id}>
                      <label className="flex min-w-0 items-center gap-3 px-3 py-2 text-sm hover:bg-muted">
                        <Checkbox disabled={disabled} checked={sources.includes(asset.id)}
                          onCheckedChange={(checked) => onChange(checked === true
                            ? [...sources, asset.id] : sources.filter((sourceId) => sourceId !== asset.id))} />
                        <span className="min-w-0 truncate" title={asset.name}>{asset.name}</span>
                      </label>
                    </li>
                  ))}
                </ul>
              </ScrollArea>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
