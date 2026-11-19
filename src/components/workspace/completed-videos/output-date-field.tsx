import { invoke } from "@tauri-apps/api/core";
import { useId, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { useLocale } from "@/lib/locale-provider";

export function OutputDateField({ workspaceId, runId, formatId, savedDate, onSaved }: {
  workspaceId: number;
  runId: number;
  formatId: number;
  savedDate: string;
  onSaved: (date: string) => void;
}) {
  const { messages: { completedVideo: copy } } = useLocale();
  const id = useId();
  const [date, setDate] = useState(savedDate);
  const [pending, setPending] = useState(false);
  const [status, setStatus] = useState<"saved" | "failed">();

  async function save(nextDate: string) {
    if (pending) return;
    setPending(true);
    setStatus(undefined);
    try {
      await invoke("set_output_schedule", { workspaceId, runId, formatId, date: nextDate || null });
      onSaved(nextDate);
      setDate(nextDate);
      setStatus("saved");
    } catch {
      setStatus("failed");
    } finally {
      setPending(false);
    }
  }

  return (
    <form className="flex min-w-0 flex-col gap-2" onSubmit={(event) => {
      event.preventDefault();
      if (date && date !== savedDate) void save(date);
    }}>
      <Field>
        <FieldLabel htmlFor={id}>{copy.scheduleDate}</FieldLabel>
        <Input className="min-w-0" id={id} type="date" min="0001-01-01" max="9999-12-31"
          value={date} disabled={pending} onChange={(event) => {
            setDate(event.target.value);
            setStatus(undefined);
          }} />
      </Field>
      <div className="flex flex-wrap gap-1">
        <Button type="submit" size="sm" variant="outline" disabled={pending || !date || date === savedDate}>
          {copy.saveDate}
        </Button>
        {savedDate && <Button type="button" size="sm" variant="ghost" disabled={pending} onClick={() => void save("")}>
          {copy.clearDate}
        </Button>}
      </div>
      {status === "saved" && <p role="status" className="text-sm text-muted-foreground">{copy.dateSaved}</p>}
      {status === "failed" && <Alert variant="destructive"><AlertDescription>{copy.dateError}</AlertDescription></Alert>}
    </form>
  );
}
