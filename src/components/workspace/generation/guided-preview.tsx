import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { useLocale } from "@/lib/locale-provider";

export function GuidedPreview({ workspaceId, runId, resultId, filename, formatId }: {
  workspaceId: number;
  runId: number;
  resultId: number;
  filename: string;
  formatId: number;
}) {
  const { messages: { productionPlan: copy } } = useLocale();
  const [open, setOpen] = useState(false);
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
    <details onToggle={(event) => {
      const expanded = event.currentTarget.open;
      setOpen(expanded);
      if (!expanded) {
        setUrl(undefined);
        setError(false);
      }
    }}>
      <summary className="w-fit cursor-pointer text-sm font-medium">{copy.watchPreview} · {filename}</summary>
      {open && (
        <div className="mt-2">
          {error ? <Alert variant="destructive"><AlertDescription>{copy.playPreviewError}</AlertDescription></Alert>
            : url ? <video aria-label={filename} className="max-h-80 w-full rounded-lg bg-black"
                controls preload="metadata" src={url} onError={() => setError(true)} />
            : <p role="status" className="text-sm text-muted-foreground">{copy.loadingPreview}</p>}
        </div>
      )}
    </details>
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
