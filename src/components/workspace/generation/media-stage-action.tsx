import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useLocale } from "@/lib/locale-provider";

import { DeliverySubtitles, type DeliveryFormat } from "./delivery-subtitles";

export type MediaStage = "preview" | "final_render" | "verify_delivery";

const labels = {
  preview: "renderWholePreview",
  final_render: "renderFinalVideo",
  verify_delivery: "verifyFiles",
} as const;

export function MediaStageAction({ workspaceId, runId, stage, available, canImportSubtitles, formats }: {
  workspaceId: number;
  runId: number;
  stage: MediaStage;
  available: boolean;
  canImportSubtitles: boolean;
  formats: DeliveryFormat[];
}) {
  const { messages: { productionPlan: copy } } = useLocale();
  const [importing, setImporting] = useState(false);
  const [pending, setPending] = useState(false);
  const [status, setStatus] = useState<"mediaUnavailable" | "mediaFailed">();

  async function processStage() {
    if (pending || importing || !available) return;
    setPending(true);
    setStatus(undefined);
    try {
      const processed = await invoke<boolean>("process_desktop_media_stage", { workspaceId, runId, stage });
      if (!processed) setStatus("mediaUnavailable");
    } catch {
      setStatus("mediaFailed");
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="mt-3 flex flex-col items-start gap-2">
      {stage === "verify_delivery" && (
        <DeliverySubtitles workspaceId={workspaceId} runId={runId} formats={formats}
          disabled={pending || importing || !canImportSubtitles} onBusy={setImporting} />
      )}
      <Button type="button" size="sm" variant="outline" disabled={pending || importing || !available}
        onClick={() => void processStage()}>
        {pending ? copy.processingMedia : copy[labels[stage]]}
      </Button>
      {stage === "verify_delivery" && <p className="text-sm text-muted-foreground">{copy.verifyHint}</p>}
      {status && <Alert variant="destructive"><AlertDescription>{copy[status]}</AlertDescription></Alert>}
    </div>
  );
}
