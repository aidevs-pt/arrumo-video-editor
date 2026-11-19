import { invoke } from "@tauri-apps/api/core";
import { Captions } from "lucide-react";
import { useEffect, useState } from "react";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { Alert, AlertAction, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";

type DeliveryPreview = { video_url: string; subtitles: string };

export function CompletedOutputPreview({ workspaceId, runId, formatId, label }: {
  workspaceId: number;
  runId: number;
  formatId: number;
  label: string;
}) {
  const { messages: { completedVideo: copy, queue } } = useLocale();
  const [preview, setPreview] = useState<DeliveryPreview>();
  const [error, setError] = useState<"previewError" | "playbackError">();
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;
    invoke<DeliveryPreview>("preview_completed_output", { workspaceId, runId, formatId }).then(
      (media) => { if (active) setPreview(media); },
      () => { if (active) setError("previewError"); },
    );
    return () => { active = false; };
  }, [attempt, formatId, runId, workspaceId]);

  return (
    <div className="flex min-w-0 flex-col gap-3">
      {error ? (
        <Alert variant="destructive">
          <AlertDescription>{copy[error]}</AlertDescription>
          <AlertAction>
            <Button variant="outline" size="sm" onClick={() => {
              setPreview(undefined);
              setError(undefined);
              setAttempt((current) => current + 1);
            }}>{queue.retry}</Button>
          </AlertAction>
        </Alert>
      ) : !preview ? (
        <div role="status" aria-label={copy.loadingPreview}>
          <Skeleton className="aspect-video w-full rounded-xl" />
        </div>
      ) : (
        <video aria-label={label} className="aspect-video w-full rounded-xl bg-black object-contain"
          controls playsInline preload="metadata" src={preview.video_url} onError={() => setError("playbackError")} />
      )}
      {preview?.subtitles && (
        <Accordion type="single" collapsible>
          <AccordionItem value="subtitles">
            <AccordionTrigger>
              <span className="flex items-center gap-2"><Captions className="size-4" aria-hidden="true" />{copy.subtitleText}</span>
            </AccordionTrigger>
            <AccordionContent>
              <pre className="max-h-64 overflow-auto rounded-lg bg-muted p-3 text-xs leading-relaxed wrap-anywhere whitespace-pre-wrap">{preview.subtitles}</pre>
            </AccordionContent>
          </AccordionItem>
        </Accordion>
      )}
    </div>
  );
}
