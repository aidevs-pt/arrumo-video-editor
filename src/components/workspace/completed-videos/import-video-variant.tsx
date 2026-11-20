import { invoke } from "@tauri-apps/api/core";
import { Film } from "lucide-react";
import { useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useLocale } from "@/lib/locale-provider";

export function ImportVideoVariant({ workspaceId, runId, formatId, onImported }: {
  workspaceId: number;
  runId: number;
  formatId: number;
  onImported: () => void;
}) {
  const { messages: { completedVideo: copy } } = useLocale();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState(false);

  async function select() {
    if (pending) return;
    setError(false);
    setPending(true);
    try {
      const imported = await invoke<boolean>("import_completed_video_variant", {
        workspaceId, runId, formatId, pickerTitle: copy.importVariant,
      });
      if (imported) onImported();
    } catch {
      setError(true);
    } finally {
      setPending(false);
    }
  }

  return <div className="flex min-w-0 flex-col items-start gap-2">
    <Button type="button" size="sm" variant="outline" disabled={pending} onClick={() => void select()}>
      <Film data-icon="inline-start" />{pending ? copy.importingVariant : copy.importVariant}
    </Button>
    {error && <Alert variant="destructive"><AlertDescription>{copy.importVariantError}</AlertDescription></Alert>}
  </div>;
}
