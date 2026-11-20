import { invoke } from "@tauri-apps/api/core";
import { ChevronLeft, ChevronRight, Pencil, Plus } from "lucide-react";
import { useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";

export type SubtitleRevision = { revision: number; subtitles: string };

export function SubtitleRevisionControls({ workspaceId, runId, formatId, language, languages, disabled, latest, current, onRead, onSave }: {
  workspaceId: number;
  runId: number;
  formatId: number;
  language: string;
  languages: string[];
  disabled: boolean;
  latest: SubtitleRevision;
  current: SubtitleRevision;
  onRead: (revision: SubtitleRevision) => void;
  onSave: (language: string, revision: SubtitleRevision) => void;
}) {
  const { messages: { subtitleRevisions: copy } } = useLocale();
  const [draft, setDraft] = useState<string>();
  const [newLanguage, setNewLanguage] = useState<string>();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<"readError" | "saveError">();
  const [saved, setSaved] = useState(false);
  const editing = draft !== undefined;
  const editButton = useRef<HTMLButtonElement>(null);
  const addButton = useRef<HTMLButtonElement>(null);
  const adding = newLanguage !== undefined;
  const targetLanguage = adding ? newLanguage.trim().toLowerCase() : language;
  const duplicate = adding && languages.includes(targetLanguage);
  const canSave = !!draft?.trim() && (adding ? !!targetLanguage && !duplicate : draft !== latest.subtitles);


  async function read(revision: number) {
    if (pending || editing || revision === current.revision) return;
    setError(undefined);
    setPending(true);
    try {
      const result = await invoke<SubtitleRevision>("read_completed_subtitle_revision", {
        workspaceId, runId, formatId, revision, language: language || null,
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
    if (draft === undefined || pending || !canSave) return;
    setPending(true);
    setError(undefined);
    try {
      const revision = await invoke<number>("save_completed_subtitle_revision", {
        workspaceId, runId, formatId, language: targetLanguage || null,
        expectedRevision: adding ? 0 : latest.revision, subtitles: draft,
      });
      onSave(targetLanguage, { revision, subtitles: draft });
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
      {latest.revision > (language ? 1 : 0) && <div className="me-auto flex items-center gap-1">
        <Button type="button" variant="ghost" size="icon-sm" disabled={disabled || pending || current.revision === (language ? 1 : 0)}
          aria-label={copy.previous} onClick={() => void read(current.revision - 1)}><ChevronLeft /></Button>
        <span className="px-1 text-sm" aria-live="polite">
          {current.revision === 0 ? copy.original : copy.version.replace("{number}", String(current.revision))}
        </span>
        <Button type="button" variant="ghost" size="icon-sm" disabled={disabled || pending || current.revision >= latest.revision}
          aria-label={copy.next} onClick={() => void read(current.revision + 1)}><ChevronRight /></Button>
      </div>}
      {current.revision === latest.revision && <Button ref={editButton} type="button" variant="outline" size="sm" disabled={disabled || pending}
        onClick={() => { setNewLanguage(undefined); setError(undefined); setSaved(false); setDraft(latest.subtitles); }}>
        <Pencil data-icon="inline-start" />{copy.edit}
      </Button>}
      <Button ref={addButton} type="button" variant="outline" size="sm" disabled={disabled || pending}
        onClick={() => { setNewLanguage(""); setError(undefined); setSaved(false); setDraft(""); }}>
        <Plus data-icon="inline-start" />{copy.addLanguage}
      </Button>
    </div>
    {error === "readError" && <Alert className="mb-3" variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
    {saved && <p className="mb-3 text-sm text-muted-foreground" role="status">{copy.saved}</p>}
    <Dialog open={editing} onOpenChange={(open) => { if (!open && !pending) setDraft(undefined); }}>
      <DialogContent className="max-h-[90dvh] overflow-y-auto sm:max-w-2xl" showCloseButton={!pending}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          (adding ? addButton : editButton).current?.focus();
        }}>
        <DialogHeader>
          <DialogTitle>{adding ? copy.addLanguage : copy.edit}</DialogTitle>
          <DialogDescription>{adding ? copy.addDescription : copy.description}</DialogDescription>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={(event) => void save(event)}>
          {adding && <Field data-invalid={duplicate}>
            <FieldLabel htmlFor="completed-srt-language">{copy.languageTag}</FieldLabel>
            <Input id="completed-srt-language" required maxLength={63} pattern="[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*"
              placeholder="pt-PT" autoComplete="off" spellCheck={false} aria-invalid={duplicate || undefined}
              disabled={pending} value={newLanguage} onChange={(event) => setNewLanguage(event.target.value)} />
            {duplicate && <p role="status" className="text-sm text-destructive">{copy.duplicateLanguage}</p>}
          </Field>}
          <Field>
            <FieldLabel htmlFor="completed-srt-draft">{copy.source}</FieldLabel>
            <Textarea id="completed-srt-draft" className="min-h-48" value={draft ?? ""} spellCheck={false}
              disabled={pending} onChange={(event) => setDraft(event.target.value)} />
          </Field>
          {error === "saveError" && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
          <DialogFooter>
            <Button type="button" variant="ghost" disabled={pending} onClick={() => setDraft(undefined)}>{copy.cancel}</Button>
            <Button type="submit" disabled={pending || !canSave}>{copy.save}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  </>;
}
