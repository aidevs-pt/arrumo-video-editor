import { invoke } from "@tauri-apps/api/core";
import { useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useLocale } from "@/lib/locale-provider";

type ReplayMode = "captured" | "current";

export function CompletedVideoReplay({ workspaceId, runId }: { workspaceId: number; runId: number }) {
  const { messages: { completedVideo: copy } } = useLocale();
  const [mode, setMode] = useState<ReplayMode>("captured");
  const [pending, setPending] = useState(false);
  const [status, setStatus] = useState<"replayQueued" | "replayError">();
  const keys = useRef<Partial<Record<ReplayMode, string>>>({});

  async function replay(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    setPending(true);
    setStatus(undefined);
    try {
      const requestKey = keys.current[mode] ??= crypto.randomUUID();
      await invoke("replay_completed_video", { workspaceId, runId, mode, requestKey });
      delete keys.current[mode];
      setStatus("replayQueued");
    } catch {
      setStatus("replayError");
    } finally {
      setPending(false);
    }
  }

  return (
    <form onSubmit={replay}>
      <FieldGroup>
        <Field>
          <FieldLabel htmlFor={`replay-mode-${runId}`}>{copy.replayMode}</FieldLabel>
          <div className="flex flex-wrap items-center gap-2">
            <Select disabled={pending} value={mode} onValueChange={(value) => setMode(value === "current" ? "current" : "captured")}>
              <SelectTrigger id={`replay-mode-${runId}`}><SelectValue /></SelectTrigger>
              <SelectContent><SelectGroup>
                <SelectItem value="captured">{copy.replayCaptured}</SelectItem>
                <SelectItem value="current">{copy.replayCurrent}</SelectItem>
              </SelectGroup></SelectContent>
            </Select>
            <Button type="submit" size="sm" variant="outline" disabled={pending}>{copy.queueReplay}</Button>
          </div>
          {status === "replayQueued" && <p role="status" className="text-sm text-muted-foreground">{copy[status]}</p>}
          {status === "replayError" && <Alert variant="destructive"><AlertDescription>{copy[status]}</AlertDescription></Alert>}
        </Field>
      </FieldGroup>
    </form>
  );
}
