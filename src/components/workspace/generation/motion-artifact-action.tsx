import { invoke } from "@tauri-apps/api/core";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";

const filters = {
  preview: ["mp4", "mov", "webm", "mkv", "avi"],
  thumbnail: ["png", "jpg", "jpeg", "webp", "gif", "avif"],
  sound: ["mp3", "wav", "flac", "m4a", "aac", "ogg"],
};
type Role = keyof typeof filters;
type DraftArtifact = { id: number; files: Partial<Record<Role, string>> };

export function MotionArtifactAction({ workspaceId, runId, available }: {
  workspaceId: number;
  runId: number;
  available: boolean;
}) {
  const { messages: { motionLibrary: copy } } = useLocale();
  const [open, setOpen] = useState(false);
  const [drafts, setDrafts] = useState<DraftArtifact[]>([{ id: 0, files: {} }]);
  const nextId = useRef(1);
  const [pending, setPending] = useState(false);
  const [picking, setPicking] = useState(false);
  const [error, setError] = useState(false);
  const fields = { preview: copy.video, thumbnail: copy.image, sound: copy.audio };

  function updateFile(id: number, role: Role, value: string | undefined) {
    setDrafts((current) => current.map((draft) => draft.id === id
      ? { ...draft, files: { ...draft.files, [role]: value } }
      : draft));
  }

  async function selectFile(id: number, role: Role) {
    setPicking(true);
    setError(false);
    try {
      const file = await pickFile({ multiple: false, title: fields[role],
        filters: [{ name: fields[role], extensions: filters[role] }] });
      if (file) updateFile(id, role, file);
    } catch {
      setError(true);
    } finally {
      setPicking(false);
    }
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!available || pending || picking || drafts.some(({ files }) => !files.preview)) return;
    const form = new FormData(event.currentTarget);
    const artifacts = drafts.map(({ id, files }) => ({
      kind: form.get(`kind-${id}`),
      title: String(form.get(`title-${id}`) ?? "").trim(),
      usage_markdown: form.get(`notes-${id}`),
      ...files,
    }));
    if (artifacts.some(({ title }) => !title)) {
      setError(true);
      return;
    }
    setPending(true);
    setError(false);
    try {
      const ok = await invoke<boolean>("submit_desktop_motion_artifacts", {
        workspaceId, runId, artifacts,
      });
      if (ok) setOpen(false);
      else setError(true);
    } catch {
      setError(true);
    } finally {
      setPending(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => {
      if (pending || picking) return;
      setOpen(next);
      setError(false);
      if (next) setDrafts([{ id: 0, files: {} }]);
    }}>
      <DialogTrigger asChild><Button variant="outline" size="sm" disabled={!available}>{copy.submitArtifacts}</Button></DialogTrigger>
      <DialogContent className="max-h-[85dvh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{copy.submitArtifacts}</DialogTitle>
          <DialogDescription>{copy.submitArtifactsDescription}</DialogDescription>
        </DialogHeader>
        <form className="flex flex-col gap-3" onSubmit={submit}>
          {drafts.map(({ id, files }, index) => (
            <fieldset key={id} disabled={pending || picking} className="flex min-w-0 flex-col gap-3 rounded-lg border p-3">
              <legend className="px-1 text-sm font-medium">
                {copy.motionBatchItem.replace("{number}", String(index + 1))}
              </legend>
              {drafts.length > 1 && <Button type="button" variant="ghost" size="sm" className="self-end"
                onClick={() => setDrafts((current) => current.filter((draft) => draft.id !== id))}>{copy.removeBatchItem}</Button>}
                <Field><FieldLabel htmlFor={`rendered-motion-title-${id}`}>{copy.name}</FieldLabel>
                  <Input id={`rendered-motion-title-${id}`} name={`title-${id}`} required disabled={pending} /></Field>
                <Field><FieldLabel htmlFor={`rendered-motion-kind-${id}`}>{copy.kind}</FieldLabel>
                  <NativeSelect id={`rendered-motion-kind-${id}`} name={`kind-${id}`} disabled={pending}>
                    <NativeSelectOption value="animation">{copy.animation}</NativeSelectOption>
                    <NativeSelectOption value="transition">{copy.transition}</NativeSelectOption>
                  </NativeSelect></Field>
                {(["preview", "thumbnail", "sound"] as const).map((role) => (
                  <div key={role} className="flex flex-wrap items-center gap-2">
                    <Button type="button" variant="outline" size="sm" disabled={pending || picking}
                      onClick={() => void selectFile(id, role)}>{fields[role]}</Button>
                    <span className="wrap-anywhere text-sm text-muted-foreground">{files[role]?.split(/[\\/]/).at(-1) ?? copy.none}</span>
                    {role !== "preview" && files[role] && <Button variant="ghost" size="sm" type="button" disabled={pending || picking}
                      onClick={() => updateFile(id, role, undefined)}>{copy.clearArtifact}</Button>}
                  </div>
                ))}
                <Field><FieldLabel htmlFor={`rendered-motion-notes-${id}`}>{copy.usage}</FieldLabel>
                  <Textarea id={`rendered-motion-notes-${id}`} name={`notes-${id}`} disabled={pending} /></Field>
            </fieldset>
          ))}
          <Button type="button" variant="outline" size="sm" className="self-start" disabled={pending || picking}
            onClick={() => {
              const id = nextId.current++;
              setDrafts((current) => [...current, { id, files: {} }]);
            }}>{copy.addBatchItem}</Button>
          {error && <Alert variant="destructive"><AlertDescription>{copy.submitArtifactsError}</AlertDescription></Alert>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={pending || picking} onClick={() => setOpen(false)}>{copy.cancel}</Button>
            <Button type="submit" disabled={drafts.some(({ files }) => !files.preview) || !available || pending || picking}>
              {pending ? copy.saving : copy.submitArtifacts}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
