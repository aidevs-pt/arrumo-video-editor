import { invoke } from "@tauri-apps/api/core";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { useState, type FormEvent } from "react";
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

export function MotionArtifactAction({ workspaceId, runId, available }: {
  workspaceId: number;
  runId: number;
  available: boolean;
}) {
  const { messages: { motionLibrary: copy } } = useLocale();
  const [open, setOpen] = useState(false);
  const [files, setFiles] = useState<Partial<Record<Role, string>>>({});
  const [pending, setPending] = useState(false);
  const [picking, setPicking] = useState(false);
  const [error, setError] = useState(false);
  const fields = { preview: copy.video, thumbnail: copy.image, sound: copy.audio };

  async function selectFile(role: Role) {
    setPicking(true);
    setError(false);
    try {
      const file = await pickFile({ multiple: false, title: fields[role],
        filters: [{ name: fields[role], extensions: filters[role] }] });
      if (file) setFiles((current) => ({ ...current, [role]: file }));
    } catch {
      setError(true);
    } finally {
      setPicking(false);
    }
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!available || pending || picking || !files.preview) return;
    const form = new FormData(event.currentTarget);
    const title = String(form.get("title") ?? "").trim();
    if (!title) return;
    setPending(true);
    setError(false);
    try {
      const ok = await invoke<boolean>("submit_desktop_motion_artifacts", {
        workspaceId, runId, artifacts: [{
          kind: form.get("kind"), title, usage_markdown: form.get("notes"), ...files,
        }],
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
      setFiles({});
    }}>
      <DialogTrigger asChild><Button variant="outline" size="sm" disabled={!available}>{copy.submitArtifacts}</Button></DialogTrigger>
      <DialogContent className="max-h-[85dvh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{copy.submitArtifacts}</DialogTitle>
          <DialogDescription>{copy.submitArtifactsDescription}</DialogDescription>
        </DialogHeader>
        <form className="flex flex-col gap-3" onSubmit={submit}>
          <Field><FieldLabel htmlFor="rendered-motion-title">{copy.name}</FieldLabel>
            <Input id="rendered-motion-title" name="title" required disabled={pending} /></Field>
          <Field><FieldLabel htmlFor="rendered-motion-kind">{copy.kind}</FieldLabel>
            <NativeSelect id="rendered-motion-kind" name="kind" disabled={pending}>
              <NativeSelectOption value="animation">{copy.animation}</NativeSelectOption>
              <NativeSelectOption value="transition">{copy.transition}</NativeSelectOption>
            </NativeSelect></Field>
          {(["preview", "thumbnail", "sound"] as const).map((role) => (
            <div key={role} className="flex flex-wrap items-center gap-2">
              <Button type="button" variant="outline" size="sm" disabled={pending || picking}
                onClick={() => void selectFile(role)}>{fields[role]}</Button>
              <span className="wrap-anywhere text-sm text-muted-foreground">{files[role]?.split(/[\\/]/).at(-1) ?? copy.none}</span>
              {role !== "preview" && files[role] && <Button variant="ghost" size="sm" type="button" disabled={pending || picking}
                onClick={() => setFiles((current) => ({ ...current, [role]: undefined }))}>{copy.clearArtifact}</Button>}
            </div>
          ))}
          <Field><FieldLabel htmlFor="rendered-motion-notes">{copy.usage}</FieldLabel>
            <Textarea id="rendered-motion-notes" name="notes" disabled={pending} /></Field>
          {error && <Alert variant="destructive"><AlertDescription>{copy.submitArtifactsError}</AlertDescription></Alert>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={pending || picking} onClick={() => setOpen(false)}>{copy.cancel}</Button>
            <Button type="submit" disabled={!files.preview || !available || pending || picking}>
              {pending ? copy.saving : copy.submitArtifacts}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
