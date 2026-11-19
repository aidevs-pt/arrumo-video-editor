import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useLocale } from "@/lib/locale-provider";

export type DeliveryFormat = { id: number; name: string };

export function DeliverySubtitles({ workspaceId, runId, formats, disabled, onBusy }: {
  workspaceId: number;
  runId: number;
  formats: DeliveryFormat[];
  disabled: boolean;
  onBusy: (busy: boolean) => void;
}) {
  const { messages: { productionPlan: copy } } = useLocale();
  const [imported, setImported] = useState<number[]>([]);
  const [error, setError] = useState<number>();

  async function importSubtitles(formatId: number) {
    if (disabled) return;
    onBusy(true);
    setError(undefined);
    try {
      const saved = await invoke<boolean>("import_delivery_subtitles", {
        workspaceId, runId, formatId, pickerTitle: copy.importSubtitles,
      });
      if (saved) setImported((current) => current.includes(formatId) ? current : [...current, formatId]);
    } catch {
      setError(formatId);
    } finally {
      onBusy(false);
    }
  }

  return (
    <ul className="flex flex-col gap-2">
      {formats.map((format) => (
        <li key={format.id} className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-medium">{format.name}</span>
          <Button type="button" size="sm" variant="outline" disabled={disabled}
            aria-label={`${copy.importSubtitles} · ${format.name}`}
            onClick={() => void importSubtitles(format.id)}>{copy.importSubtitles}</Button>
          {imported.includes(format.id) && <span role="status" className="text-sm text-muted-foreground">{copy.subtitlesImported}</span>}
          {error === format.id && <Alert variant="destructive"><AlertDescription>{copy.subtitleImportError}</AlertDescription></Alert>}
        </li>
      ))}
    </ul>
  );
}
