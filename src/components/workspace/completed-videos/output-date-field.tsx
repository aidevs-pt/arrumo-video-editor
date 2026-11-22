import { invoke } from "@tauri-apps/api/core";
import { enUS, es, pt } from "date-fns/locale";
import { CalendarDays } from "lucide-react";
import { useId, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import { Field, FieldLabel } from "@/components/ui/field";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useLocale } from "@/lib/locale-provider";

const calendarLocales = { en: enUS, es, pt } as const;

/** A local calendar day from `YYYY-MM-DD`, and back. */
function day(value: string) {
  const [year, month, date] = value.split("-").map(Number);
  return new Date(year, month - 1, date);
}
function isoDay(value: Date) {
  return [value.getFullYear(), value.getMonth() + 1, value.getDate()].map((part, index) => String(part).padStart(index ? 2 : 4, "0")).join("-");
}

/** The date one output is scheduled to publish. Picking a day saves it; the calendar follows the app's language. */
export function OutputDateField({ workspaceId, runId, formatId, savedDate, onSaved }: {
  workspaceId: number;
  runId: number;
  formatId: number;
  savedDate: string;
  onSaved: (date: string) => void;
}) {
  const { locale, messages: { completedVideo: copy } } = useLocale();
  const id = useId();
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState(false);
  const [status, setStatus] = useState<"saved" | "failed">();

  async function save(nextDate: string) {
    if (pending) return;
    setPending(true);
    setStatus(undefined);
    try {
      await invoke("set_output_schedule", { workspaceId, runId, formatId, date: nextDate || null });
      onSaved(nextDate);
      setStatus("saved");
    } catch {
      setStatus("failed");
    } finally {
      setPending(false);
    }
  }

  const selected = savedDate ? day(savedDate) : undefined;
  const label = selected
    ? new Intl.DateTimeFormat(locale, { weekday: "short", day: "numeric", month: "short", year: "numeric" }).format(selected)
    : copy.notScheduled;

  return (
    <div className="flex min-w-0 flex-col gap-2">
      <Field>
        <FieldLabel htmlFor={id}>{copy.scheduleDate}</FieldLabel>
        <Popover open={open} onOpenChange={setOpen}>
          <PopoverTrigger asChild>
            <Button id={id} type="button" variant="outline" disabled={pending}
              className="w-full justify-start gap-2 text-start font-normal">
              <CalendarDays data-icon="inline-start" className="text-muted-foreground" />
              <span className={selected ? "truncate" : "truncate text-muted-foreground"}>{label}</span>
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-auto p-0" align="start">
            <Calendar mode="single" selected={selected} defaultMonth={selected}
              locale={calendarLocales[locale as keyof typeof calendarLocales] ?? enUS}
              onSelect={(value) => {
                setOpen(false);
                if (value && isoDay(value) !== savedDate) void save(isoDay(value));
              }} />
          </PopoverContent>
        </Popover>
      </Field>
      {savedDate && <Button type="button" size="sm" variant="ghost" className="self-start" disabled={pending} onClick={() => void save("")}>
        {copy.clearDate}
      </Button>}
      {status === "saved" && <p role="status" className="text-sm text-muted-foreground">{copy.dateSaved}</p>}
      {status === "failed" && <Alert variant="destructive"><AlertDescription>{copy.dateError}</AlertDescription></Alert>}
    </div>
  );
}
