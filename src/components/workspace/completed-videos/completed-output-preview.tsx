import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { useLocale } from "@/lib/locale-provider";

type DeliveryPreview = { video_url: string; subtitles: string };

export function CompletedOutputPreview({ workspaceId, runId, formatId, filename }: {
  workspaceId: number;
  runId: number;
  formatId: number;
  filename: string;
}) {
  const { messages: { completedVideo: copy } } = useLocale();
  const [open, setOpen] = useState(false);
  const [preview, setPreview] = useState<DeliveryPreview>();
  const [error, setError] = useState(false);
  const [playbackError, setPlaybackError] = useState(false);

  useEffect(() => {
    if (!open) return;
    let active = true;
    invoke<DeliveryPreview>("preview_completed_output", { workspaceId, runId, formatId }).then((media) => {
      if (active) setPreview(media);
    }).catch(() => {
      if (active) setError(true);
    });
    return () => { active = false; };
  }, [formatId, open, runId, workspaceId]);

  return (
    <details className="text-foreground" onToggle={(event) => {
      const expanded = event.currentTarget.open;
      setOpen(expanded);
      if (!expanded) {
        setPreview(undefined);
        setError(false);
        setPlaybackError(false);
      }
    }}>
      <summary className="cursor-pointer" aria-label={`${copy.previewOutput} · ${filename}`}>{copy.previewOutput}</summary>
      {open && (
        <div className="mt-2 flex flex-col gap-3">
          {error ? <Alert variant="destructive"><AlertDescription>{copy.previewError}</AlertDescription></Alert>
            : !preview ? <p role="status" className="text-muted-foreground">{copy.loadingPreview}</p>
            : (
              <>
                {playbackError && <Alert variant="destructive"><AlertDescription>{copy.playbackError}</AlertDescription></Alert>}
                <video aria-label={filename} className="max-h-80 w-full rounded-lg bg-black"
                  controls preload="metadata" src={preview.video_url} onError={() => setPlaybackError(true)} />
                <details>
                  <summary className="cursor-pointer">{copy.subtitleText}</summary>
                  <pre className="mt-2 max-h-64 overflow-auto rounded-lg border bg-muted p-3 wrap-anywhere whitespace-pre-wrap">{preview.subtitles}</pre>
                </details>
              </>
            )}
        </div>
      )}
    </details>
  );
}
