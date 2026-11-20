import { invoke } from "@tauri-apps/api/core";
import { Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { useLocale } from "@/lib/locale-provider";

export function GuidedPreview({ workspaceId, runId, resultId, filename, formatId, label = filename, initiallyOpen = false }: {
  workspaceId: number;
  runId: number;
  resultId: number;
  filename: string;
  formatId: number;
  label?: string;
  initiallyOpen?: boolean;
}) {
  const { messages: { productionPlan: copy } } = useLocale();
  const [open, setOpen] = useState(initiallyOpen);
  const [url, setUrl] = useState<string>();
  const [error, setError] = useState(false);

  useEffect(() => {
    if (!open) return;
    let active = true;
    invoke<string>("open_guided_preview", { workspaceId, runId, resultId, formatId })
      .then((value) => { if (active) setUrl(value); })
      .catch(() => { if (active) setError(true); });
    return () => { active = false; };
  }, [formatId, open, resultId, runId, workspaceId]);

  return (
    <Collapsible className="min-w-0 rounded-lg border p-2" open={open} onOpenChange={(expanded) => {
      setOpen(expanded);
      if (!expanded) {
        setUrl(undefined);
        setError(false);
      }
    }}>
      <CollapsibleTrigger asChild>
        <Button type="button" variant="outline" className="h-auto min-h-8 max-w-full whitespace-normal">
          <Play data-icon="inline-start" /><span>{copy.watchPreview}</span><span className="text-muted-foreground">{label}</span>
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent className="mt-2">
          {error ? <Alert variant="destructive"><AlertDescription>{copy.playPreviewError}</AlertDescription></Alert>
            : url ? <video aria-label={filename} className="max-h-80 w-full rounded-sm bg-black"
                controls preload="metadata" src={url} onError={() => setError(true)} />
            : <p role="status" className="text-sm text-muted-foreground">{copy.loadingPreview}</p>}
      </CollapsibleContent>
    </Collapsible>
  );
}

export function previewFiles(value: unknown) {
  if (!Array.isArray(value)) return [];
  return value.flatMap((filename) => {
    if (typeof filename !== "string") return [];
    const match = /^preview-\d+-(\d+)\.mp4$/.exec(filename);
    const formatId = Number(match?.[1]);
    return Number.isSafeInteger(formatId) && formatId > 0 ? [{ filename, formatId }] : [];
  });
}
