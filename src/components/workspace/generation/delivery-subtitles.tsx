import { invoke } from "@tauri-apps/api/core";
import { useId, useState } from "react";
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
  const hintId = useId();
  const [saved, setSaved] = useState<Record<number, "subtitlesImported" | "subtitlesGenerated">>({});
  const [pending, setPending] = useState<{ formatId: number; action: "import" | "generate" }>();
  const [error, setError] = useState<{ formatId: number; key: "subtitleImportError" | "subtitleGenerationError" }>();

  async function updateSubtitles(formatId: number, action: "import" | "generate") {
    if (disabled || pending) return;
    setPending({ formatId, action });
    onBusy(true);
    setError(undefined);
    try {
      if (action === "generate") {
        await invoke<number>("generate_delivery_subtitles", { workspaceId, runId, formatId });
        setSaved((current) => ({ ...current, [formatId]: "subtitlesGenerated" }));
      } else {
        const imported = await invoke<boolean>("import_delivery_subtitles", {
          workspaceId, runId, formatId, pickerTitle: copy.importSubtitles,
        });
        if (imported) setSaved((current) => ({ ...current, [formatId]: "subtitlesImported" }));
      }
    } catch {
      setError({ formatId, key: action === "generate" ? "subtitleGenerationError" : "subtitleImportError" });
    } finally {
      setPending(undefined);
      onBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <p id={hintId} className="text-sm text-muted-foreground">{copy.generateSubtitlesHint}</p>
      <ul className="flex flex-col gap-2">
        {formats.map((format) => (
          <li key={format.id} aria-busy={pending?.formatId === format.id} className="flex flex-col gap-2">
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-sm font-medium">{format.name}</span>
              <Button type="button" size="sm" variant="outline" disabled={disabled || !!pending || !!saved[format.id]}
                aria-label={`${copy.generateSubtitles} · ${format.name}`} aria-describedby={hintId}
                onClick={() => void updateSubtitles(format.id, "generate")}>
                {pending?.formatId === format.id && pending.action === "generate" ? copy.generatingSubtitles : copy.generateSubtitles}
              </Button>
              <Button type="button" size="sm" variant="outline" disabled={disabled || !!pending}
                aria-label={`${copy.importSubtitles} · ${format.name}`}
                onClick={() => void updateSubtitles(format.id, "import")}>{copy.importSubtitles}</Button>
              <span role="status" className="text-sm text-muted-foreground">
                {saved[format.id] && copy[saved[format.id]]}
              </span>
            </div>
            {error?.formatId === format.id && <Alert variant="destructive"><AlertDescription>{copy[error.key]}</AlertDescription></Alert>}
          </li>
        ))}
      </ul>
    </div>
  );
}
