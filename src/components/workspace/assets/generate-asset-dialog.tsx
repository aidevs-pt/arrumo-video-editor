"use client";

import { invoke } from "@tauri-apps/api/core";
import { Sparkles } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";
import { usePluginText } from "@/lib/plugin-text";

type Provider = { id: string; name: string };
type Preferences = { available: Provider[]; effective: string[] };

const SIZES = [
  { ratio: "16:9", pixels: "1920 × 1080" },
  { ratio: "9:16", pixels: "1080 × 1920" },
  { ratio: "1:1", pixels: "1080 × 1080" },
  { ratio: "4:5", pixels: "1080 × 1350" },
];

/** Describe an image and have the workspace's image providers make one to four of them for the library. */
export function GenerateAssetDialog({ workspaceId, disabled, onCreated }: {
  workspaceId: number;
  disabled?: boolean;
  onCreated: (ids: number[]) => void;
}) {
  const { messages: { assets: { generate: copy } } } = useLocale();
  const pluginText = usePluginText();
  const [open, setOpen] = useState(false);
  const [providers, setProviders] = useState<Preferences>();
  const [prompt, setPrompt] = useState("");
  const [provider, setProvider] = useState("");
  const [ratio, setRatio] = useState("16:9");
  const [count, setCount] = useState(1);
  const [notes, setNotes] = useState("");
  const [pending, setPending] = useState(false);
  const [problem, setProblem] = useState<string>();

  useEffect(() => {
    if (!open) return;
    invoke<Record<string, Preferences>>("list_provider_preferences", { workspaceId }).then(
      (all) => setProviders(all["image-generation"] ?? { available: [], effective: [] }),
      () => setProviders({ available: [], effective: [] }),
    );
  }, [open, workspaceId]);

  const name = (entry: Provider) => pluginText.name(entry.id, entry.name);
  const inOrder = providers?.effective.map((id) => providers.available.find((entry) => entry.id === id)).filter((entry): entry is Provider => !!entry) ?? [];
  const none = providers !== undefined && inOrder.length === 0 && !provider;

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (pending || !prompt.trim()) return;
    setPending(true);
    setProblem(undefined);
    try {
      const ids = await invoke<number[]>("generate_library_images", {
        workspaceId, prompt: prompt.trim(), aspectRatio: ratio, count, notes: notes.trim(), provider: provider || null,
      });
      onCreated(ids);
      setOpen(false);
      setPrompt("");
      setNotes("");
    } catch (error) {
      setProblem(typeof error === "string" ? error : copy.error);
    } finally {
      setPending(false);
    }
  }

  return <Dialog open={open} onOpenChange={(next) => { if (!pending) setOpen(next); }}>
    <DialogTrigger asChild>
      <Button variant="outline" disabled={disabled}><Sparkles data-icon="inline-start" />{copy.open}</Button>
    </DialogTrigger>
    <DialogContent className="sm:max-w-xl">
      <DialogHeader>
        <DialogTitle>{copy.title}</DialogTitle>
        <DialogDescription>{copy.description}</DialogDescription>
      </DialogHeader>
      <form onSubmit={submit}>
        <FieldGroup>
          <Field data-disabled={pending}>
            <FieldLabel htmlFor="generate-prompt">{copy.prompt}</FieldLabel>
            <Textarea id="generate-prompt" value={prompt} onChange={(event) => setPrompt(event.target.value)} disabled={pending}
              placeholder={copy.promptPlaceholder} maxLength={4000} className="min-h-24" required />
          </Field>
          <div className="grid gap-4 sm:grid-cols-[1fr_1fr_6rem]">
            <Field data-disabled={pending}>
              <FieldLabel htmlFor="generate-provider">{copy.provider}</FieldLabel>
              <NativeSelect id="generate-provider" value={provider} disabled={pending} onChange={(event) => setProvider(event.target.value)}>
                <NativeSelectOption value="">{inOrder[0] ? copy.firstAvailable.replace("{provider}", name(inOrder[0])) : copy.defaultOrder}</NativeSelectOption>
                {providers?.available.map((entry) => <NativeSelectOption key={entry.id} value={entry.id}>{name(entry)}</NativeSelectOption>)}
              </NativeSelect>
            </Field>
            <Field data-disabled={pending}>
              <FieldLabel htmlFor="generate-size">{copy.size}</FieldLabel>
              <NativeSelect id="generate-size" value={ratio} disabled={pending} onChange={(event) => setRatio(event.target.value)}>
                {SIZES.map((size) => <NativeSelectOption key={size.ratio} value={size.ratio}>{`${size.ratio}, ${size.pixels}`}</NativeSelectOption>)}
              </NativeSelect>
            </Field>
            <Field data-disabled={pending}>
              <FieldLabel htmlFor="generate-count">{copy.variations}</FieldLabel>
              <NativeSelect id="generate-count" value={count} disabled={pending} onChange={(event) => setCount(Number(event.target.value))}>
                {[1, 2, 3, 4].map((value) => <NativeSelectOption key={value} value={value}>{value}</NativeSelectOption>)}
              </NativeSelect>
            </Field>
          </div>
          <Field data-disabled={pending}>
            <FieldLabel htmlFor="generate-notes">{copy.notes}</FieldLabel>
            <Input id="generate-notes" value={notes} onChange={(event) => setNotes(event.target.value)} disabled={pending}
              placeholder={copy.notesPlaceholder} maxLength={8000} aria-describedby="generate-notes-hint" />
            <FieldDescription id="generate-notes-hint">{copy.notesHint}</FieldDescription>
          </Field>
          {none && <Alert><AlertDescription>{copy.noProvider}</AlertDescription></Alert>}
          {problem && <Alert variant="destructive" role="alert"><AlertDescription>{problem}</AlertDescription></Alert>}
          {pending && <p role="status" className="text-sm text-muted-foreground">{copy.pendingHint}</p>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={pending} onClick={() => setOpen(false)}>{copy.cancel}</Button>
            <Button type="submit" disabled={pending || !prompt.trim() || none}>
              {pending ? copy.generating : (count === 1 ? copy.submitOne : copy.submitMany.replace("{count}", String(count)))}
            </Button>
          </DialogFooter>
        </FieldGroup>
      </form>
    </DialogContent>
  </Dialog>;
}
