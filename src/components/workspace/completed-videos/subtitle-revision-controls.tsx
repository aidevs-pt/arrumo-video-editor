import { invoke } from "@tauri-apps/api/core";
import { ChevronLeft, ChevronRight, Pencil } from "lucide-react";
import { useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";

export type SubtitleRevision = { revision: number; subtitles: string };

export function SubtitleRevisionControls({ workspaceId, runId, formatId, latest, current, onRead, onSave }: {
  workspaceId: number;
  runId: number;
  formatId: number;
  latest: SubtitleRevision;
  current: SubtitleRevision;
  onRead: (revision: SubtitleRevision) => void;
  onSave: (revision: SubtitleRevision) => void;
}) {
  const { messages: { subtitleRevisions: copy } } = useLocale();
  const [draft, setDraft] = useState<string>();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<"readError" | "saveError">();
  const [saved, setSaved] = useState(false);
  const editing = draft !== undefined;
  const editButton = useRef<HTMLButtonElement>(null);

  async function read(revision: number) {
    if (pending || editing || revision === current.revision) return;
    setError(undefined);
    setPending(true);
    try {
      const result = await invoke<SubtitleRevision>("read_completed_subtitle_revision", {
        workspaceId, runId, formatId, revision,
      });
      onRead(result);
    } catch {
      setError("readError");
    } finally {
      setPending(false);
    }
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (draft === undefined || pending || !draft.trim() || draft === latest.subtitles) return;
    setPending(true);
    setError(undefined);
    try {
      const revision = await invoke<number>("save_completed_subtitle_revision", {
        workspaceId, runId, formatId, expectedRevision: latest.revision, subtitles: draft,
      });
      onSave({ revision, subtitles: draft });
      setDraft(undefined);
      setSaved(true);
    } catch {
      setError("saveError");
    } finally {
      setPending(false);
    }
  }

  return <>
    <div className="mb-3 flex flex-wrap items-center gap-2">
      {latest.revision > 0 && <div className="me-auto flex items-center gap-1">
        <Button type="button" variant="ghost" size="icon-sm" disabled={pending || current.revision === 0}
          aria-label={copy.previous} onClick={() => void read(current.revision - 1)}><ChevronLeft /></Button>
        <span className="px-1 text-sm" aria-live="polite">
          {current.revision === 0 ? copy.original : copy.version.replace("{number}", String(current.revision))}
        </span>
        <Button type="button" variant="ghost" size="icon-sm" disabled={pending || current.revision >= latest.revision}
          aria-label={copy.next} onClick={() => void read(current.revision + 1)}><ChevronRight /></Button>
      </div>}
      {current.revision === latest.revision && <Button ref={editButton} type="button" variant="outline" size="sm" disabled={pending}
        onClick={() => { setError(undefined); setSaved(false); setDraft(latest.subtitles); }}>
        <Pencil data-icon="inline-start" />{copy.edit}
      </Button>}
    </div>
    {error === "readError" && <Alert className="mb-3" variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
    {saved && <p className="mb-3 text-sm text-muted-foreground" role="status">{copy.saved}</p>}
    <Dialog open={editing} onOpenChange={(open) => { if (!open && !pending) setDraft(undefined); }}>
      <DialogContent className="max-h-[90dvh] overflow-y-auto sm:max-w-2xl" showCloseButton={!pending}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          editButton.current?.focus();
        }}>
        <DialogHeader>
          <DialogTitle>{copy.edit}</DialogTitle>
          <DialogDescription>{copy.description}</DialogDescription>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={(event) => void save(event)}>
          <Field>
            <FieldLabel htmlFor="completed-srt-draft">{copy.source}</FieldLabel>
            <Textarea id="completed-srt-draft" className="min-h-48" value={draft ?? ""} spellCheck={false}
              disabled={pending} onChange={(event) => setDraft(event.target.value)} />
          </Field>
          {error === "saveError" && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
          <DialogFooter>
            <Button type="button" variant="ghost" disabled={pending} onClick={() => setDraft(undefined)}>{copy.cancel}</Button>
            <Button type="submit" disabled={pending || !draft?.trim() || draft === latest.subtitles}>{copy.save}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  </>;
}
