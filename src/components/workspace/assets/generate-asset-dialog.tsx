"use client";

import { invoke } from "@tauri-apps/api/core";
import { AudioLines, Clapperboard, ImageIcon, Sparkles } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";
import { usePluginText } from "@/lib/plugin-text";

type Provider = { id: string; name: string };
type Preferences = { available: Provider[]; effective: string[] };
type Kind = "image" | "audio" | "video";

const CAPABILITY: Record<Kind, string> = { image: "image-generation", audio: "text-to-speech", video: "video-generation" };
const DURATIONS = [3, 5, 8, 10, 15];

const SIZES = [
  { ratio: "16:9", pixels: "1920 × 1080" },
  { ratio: "9:16", pixels: "1080 × 1920" },
  { ratio: "1:1", pixels: "1080 × 1080" },
  { ratio: "4:5", pixels: "1080 × 1350" },
];

/** Describe an image or a clip, or write text to voice, and the workspace's providers make it for the library. */
export function GenerateAssetDialog({ workspaceId, disabled, onCreated, imagesOnly = false }: {
  workspaceId: number;
  disabled?: boolean;
  onCreated: (ids: number[]) => void;
  /** Only images, with the trigger naming the first image provider, as on the brand page. */
  imagesOnly?: boolean;
}) {
  const { messages: { assets: { generate: copy } } } = useLocale();
  const pluginText = usePluginText();
  const [open, setOpen] = useState(false);
  const [kind, setKind] = useState<Kind>("image");
  const [all, setAll] = useState<Record<string, Preferences>>();
  const providers = all ? all[CAPABILITY[kind]] ?? { available: [], effective: [] } : undefined;
  const [duration, setDuration] = useState(5);
  const [prompt, setPrompt] = useState("");
  const [provider, setProvider] = useState("");
  const [ratio, setRatio] = useState("16:9");
  const [count, setCount] = useState(1);
  const [notes, setNotes] = useState("");
  const [pending, setPending] = useState(false);
  const [problem, setProblem] = useState<string>();

  useEffect(() => {
    if (!open && !imagesOnly) return;
    invoke<Record<string, Preferences>>("list_provider_preferences", { workspaceId }).then(setAll, () => setAll({}));
  }, [imagesOnly, open, workspaceId]);

  const name = (entry: Provider) => pluginText.name(entry.id, entry.name);
  const inOrder = providers?.effective.map((id) => providers.available.find((entry) => entry.id === id)).filter((entry): entry is Provider => !!entry) ?? [];
  const none = providers !== undefined && inOrder.length === 0 && !provider;

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (pending || !prompt.trim()) return;
    setPending(true);
    setProblem(undefined);
    try {
      if (kind === "image") {
        onCreated(await invoke<number[]>("generate_library_images", {
          workspaceId, prompt: prompt.trim(), aspectRatio: ratio, count, notes: notes.trim(), provider: provider || null,
        }));
      } else if (kind === "video") {
        onCreated([await invoke<number>("generate_library_video", {
          workspaceId, prompt: prompt.trim(), aspectRatio: ratio, durationSeconds: duration, notes: notes.trim(), provider: provider || null,
        })]);
      } else {
        onCreated([await invoke<number>("generate_library_speech", {
          workspaceId, text: prompt.trim(), notes: notes.trim(), provider: provider || null,
        })]);
      }
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
      <Button variant="outline" disabled={disabled}><Sparkles data-icon="inline-start" />
        {imagesOnly && inOrder[0] ? copy.openWith.replace("{provider}", name(inOrder[0])) : copy.open}</Button>
    </DialogTrigger>
    <DialogContent className="sm:max-w-xl">
      <DialogHeader>
        <DialogTitle>{imagesOnly ? copy.styleTitle : copy.title}</DialogTitle>
        <DialogDescription>{imagesOnly ? copy.styleDescription : copy.description}</DialogDescription>
      </DialogHeader>
      <form onSubmit={submit}>
        <FieldGroup>
          {!imagesOnly && <Tabs value={kind} onValueChange={(value) => { const next = value as Kind; setKind(next); if (next === "video" && ratio === "4:5") setRatio("16:9"); setProvider(""); setProblem(undefined); }}>
            <TabsList className="w-full justify-start">
              <TabsTrigger value="image" disabled={pending} className="flex-none"><ImageIcon data-icon="inline-start" />{copy.image}</TabsTrigger>
              <TabsTrigger value="audio" disabled={pending} className="flex-none"><AudioLines data-icon="inline-start" />{copy.audio}</TabsTrigger>
              <TabsTrigger value="video" disabled={pending} className="flex-none"><Clapperboard data-icon="inline-start" />{copy.video}</TabsTrigger>
            </TabsList>
          </Tabs>}
          <Field data-disabled={pending}>
            <FieldLabel htmlFor="generate-prompt">{kind === "audio" ? copy.speechText : copy.prompt}</FieldLabel>
            <Textarea id="generate-prompt" value={prompt} onChange={(event) => setPrompt(event.target.value)} disabled={pending}
              placeholder={kind === "image" ? copy.promptPlaceholder : kind === "video" ? copy.videoPlaceholder : copy.speechPlaceholder} maxLength={kind === "image" ? 4000 : kind === "video" ? 2500 : 5000} className="min-h-24" required />
          </Field>
          <div className={kind === "audio" ? "grid gap-4" : "grid gap-4 sm:grid-cols-[1fr_1fr_6rem]"}>
            <Field data-disabled={pending}>
              <FieldLabel htmlFor="generate-provider">{copy.provider}</FieldLabel>
              <NativeSelect id="generate-provider" value={provider} disabled={pending} onChange={(event) => setProvider(event.target.value)}>
                <NativeSelectOption value="">{inOrder[0] ? copy.firstAvailable.replace("{provider}", name(inOrder[0])) : copy.defaultOrder}</NativeSelectOption>
                {providers?.available.map((entry) => <NativeSelectOption key={entry.id} value={entry.id}>{name(entry)}</NativeSelectOption>)}
              </NativeSelect>
            </Field>
            {kind !== "audio" && <><Field data-disabled={pending}>
              <FieldLabel htmlFor="generate-size">{copy.size}</FieldLabel>
              <NativeSelect id="generate-size" value={ratio} disabled={pending} onChange={(event) => setRatio(event.target.value)}>
                {SIZES.filter((size) => kind === "image" || size.ratio !== "4:5").map((size) => <NativeSelectOption key={size.ratio} value={size.ratio}>{kind === "image" ? `${size.ratio}, ${size.pixels}` : size.ratio}</NativeSelectOption>)}
              </NativeSelect>
            </Field>
            {kind === "image" ? <Field data-disabled={pending}>
              <FieldLabel htmlFor="generate-count">{copy.variations}</FieldLabel>
              <NativeSelect id="generate-count" value={count} disabled={pending} onChange={(event) => setCount(Number(event.target.value))}>
                {[1, 2, 3, 4].map((value) => <NativeSelectOption key={value} value={value}>{value}</NativeSelectOption>)}
              </NativeSelect>
            </Field> : <Field data-disabled={pending}>
              <FieldLabel htmlFor="generate-length">{copy.length}</FieldLabel>
              <NativeSelect id="generate-length" value={duration} disabled={pending} onChange={(event) => setDuration(Number(event.target.value))}>
                {DURATIONS.map((value) => <NativeSelectOption key={value} value={value}>{copy.seconds.replace("{count}", String(value))}</NativeSelectOption>)}
              </NativeSelect>
            </Field>}</>}
          </div>
          <Field data-disabled={pending}>
            <FieldLabel htmlFor="generate-notes">{copy.notes}</FieldLabel>
            <Input id="generate-notes" value={notes} onChange={(event) => setNotes(event.target.value)} disabled={pending}
              placeholder={copy.notesPlaceholder} maxLength={8000} aria-describedby="generate-notes-hint" />
            <FieldDescription id="generate-notes-hint">{copy.notesHint}</FieldDescription>
          </Field>
          {none && <Alert><AlertDescription>{kind === "image" ? copy.noProvider : kind === "video" ? copy.noVideoProvider : copy.noSpeechProvider}</AlertDescription></Alert>}
          {problem && <Alert variant="destructive" role="alert"><AlertDescription>{problem}</AlertDescription></Alert>}
          {pending && <p role="status" className="text-sm text-muted-foreground">{kind === "video" ? copy.videoPendingHint : copy.pendingHint}</p>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={pending} onClick={() => setOpen(false)}>{copy.cancel}</Button>
            <Button type="submit" disabled={pending || !prompt.trim() || none}>
              {pending ? copy.generating : kind === "audio" ? copy.submitAudio : kind === "video" ? copy.submitVideo : count === 1 ? copy.submitOne : copy.submitMany.replace("{count}", String(count))}
            </Button>
          </DialogFooter>
        </FieldGroup>
      </form>
    </DialogContent>
  </Dialog>;
}
