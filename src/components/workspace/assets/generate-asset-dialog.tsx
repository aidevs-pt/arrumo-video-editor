"use client";

import { invoke } from "@tauri-apps/api/core";
import { AudioLines, Check, Clapperboard, ImageIcon, Music, Sparkles, Volume2 } from "lucide-react";
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
import { cn } from "@/lib/utils";

import { AssetThumbnail } from "./asset-thumbnail";

type Provider = { id: string; name: string };
type Preferences = { available: Provider[]; effective: string[] };
type Kind = "image" | "audio" | "video";
/** What the dialog makes: library kinds, with audio split into voice, music and sound effects. */
type Tab = Kind | "music" | "sound";
type LibraryAsset = { id: number; name: string; kind: Kind };
type VideoModel = { provider: string; id: string; selected: boolean; variants: { images: [number, number]; videos: [number, number] }[] };

const MAX_IMAGE_REFERENCES = 9;
const MAX_VIDEO_REFERENCES = 3;

function takes(model: VideoModel, images: number, videos: number) {
  return model.variants.some((variant) => images >= variant.images[0] && images <= variant.images[1] && videos >= variant.videos[0] && videos <= variant.videos[1]);
}

const CAPABILITY: Record<Tab, string> = {
  image: "image-generation", audio: "text-to-speech", music: "music-generation", sound: "sound-effect-generation", video: "video-generation",
};
const DURATIONS = [3, 5, 8, 10, 15];
const MUSIC_DURATIONS = [15, 30, 60, 120, 180];
const SOUND_DURATIONS = [1, 2, 3, 5, 10];

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
  const [kind, setKind] = useState<Tab>("image");
  const made = kind === "music" || kind === "sound";
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
  const [library, setLibrary] = useState<LibraryAsset[]>([]);
  const [references, setReferences] = useState<number[]>([]);
  const [models, setModels] = useState<VideoModel[]>([]);
  const [model, setModel] = useState("");

  useEffect(() => {
    if (!open && !imagesOnly) return;
    invoke<Record<string, Preferences>>("list_provider_preferences", { workspaceId }).then(setAll, () => setAll({}));
  }, [imagesOnly, open, workspaceId]);

  useEffect(() => {
    if (!open) return;
    invoke<LibraryAsset[]>("list_assets", { workspaceId }).then(setLibrary, () => setLibrary([]));
    if (!imagesOnly) invoke<VideoModel[]>("list_video_models", { workspaceId }).then(setModels, () => setModels([]));
  }, [imagesOnly, open, workspaceId]);

  const name = (entry: Provider) => pluginText.name(entry.id, entry.name);
  const inOrder = providers?.effective.map((id) => providers.available.find((entry) => entry.id === id)).filter((entry): entry is Provider => !!entry) ?? [];
  const none = providers !== undefined && inOrder.length === 0 && !provider;
  const choices = library.filter((asset) => asset.kind === "image" || (kind === "video" && asset.kind === "video"));
  const chosen = references.filter((id) => choices.some((asset) => asset.id === id));
  const imageCount = chosen.filter((id) => choices.find((asset) => asset.id === id)?.kind === "image").length;
  const videoCount = chosen.length - imageCount;
  const videoProvider = provider || inOrder[0]?.id || "";
  const providerModels = models.filter((entry) => entry.provider === videoProvider && entry.id);
  const activeModel = providerModels.find((entry) => entry.id === model) ?? providerModels.find((entry) => entry.selected);
  const fitting = providerModels.filter((entry) => takes(entry, imageCount, videoCount));
  const unfit = kind === "video" && activeModel !== undefined && !takes(activeModel, imageCount, videoCount);
  const unsupported = kind === "video" && chosen.length > 0 && providerModels.length === 0 && models.some((entry) => entry.provider === videoProvider);
  const modelName = (id: string) => id.replace(/-/g, " ").replace(/\b([a-z])/g, (letter) => letter.toUpperCase()).replace(/ Std$/, " Standard");

  function toggleReference(asset: LibraryAsset) {
    setProblem(undefined);
    setReferences((current) => {
      if (current.includes(asset.id)) return current.filter((id) => id !== asset.id);
      const same = current.filter((id) => library.find((entry) => entry.id === id)?.kind === asset.kind).length;
      if (same >= (asset.kind === "video" ? MAX_VIDEO_REFERENCES : MAX_IMAGE_REFERENCES)) return current;
      return [...current, asset.id];
    });
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (pending || !prompt.trim()) return;
    setPending(true);
    setProblem(undefined);
    try {
      if (kind === "image") {
        onCreated(await invoke<number[]>("generate_library_images", {
          workspaceId, prompt: prompt.trim(), aspectRatio: ratio, count, notes: notes.trim(), provider: provider || null, references: chosen,
        }));
      } else if (kind === "video") {
        onCreated([await invoke<number>("generate_library_video", {
          workspaceId, prompt: prompt.trim(), aspectRatio: ratio, durationSeconds: duration, notes: notes.trim(),
          provider: provider || (model ? videoProvider : null), model: model || null, references: chosen,
        })]);
      } else if (made) {
        onCreated([await invoke<number>("generate_library_audio", {
          workspaceId, kind: kind === "music" ? "music" : "sound-effect", prompt: prompt.trim(), seconds: duration,
          notes: notes.trim(), provider: provider || null, label: kind === "music" ? copy.music : copy.sound,
        })]);
      } else {
        onCreated([await invoke<number>("generate_library_speech", {
          workspaceId, text: prompt.trim(), notes: notes.trim(), provider: provider || null,
        })]);
      }
      setOpen(false);
      setPrompt("");
      setNotes("");
      setReferences([]);
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
    <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-xl">
      <DialogHeader>
        <DialogTitle>{imagesOnly ? copy.styleTitle : copy.title}</DialogTitle>
        <DialogDescription>{imagesOnly ? copy.styleDescription : copy.description}</DialogDescription>
      </DialogHeader>
      <form onSubmit={submit}>
        <FieldGroup>
          {!imagesOnly && <Tabs value={kind} onValueChange={(value) => { const next = value as Tab; setKind(next); if (next === "video" && ratio === "4:5") setRatio("16:9"); setDuration(next === "music" ? 30 : next === "sound" ? 3 : 5); setProvider(""); setModel(""); setProblem(undefined); }}>
            <TabsList className="w-full justify-start">
              <TabsTrigger value="image" disabled={pending} className="flex-none"><ImageIcon data-icon="inline-start" />{copy.image}</TabsTrigger>
              <TabsTrigger value="audio" disabled={pending} className="flex-none"><AudioLines data-icon="inline-start" />{copy.audio}</TabsTrigger>
              <TabsTrigger value="music" disabled={pending} className="flex-none"><Music data-icon="inline-start" />{copy.music}</TabsTrigger>
              <TabsTrigger value="sound" disabled={pending} className="flex-none"><Volume2 data-icon="inline-start" />{copy.sound}</TabsTrigger>
              <TabsTrigger value="video" disabled={pending} className="flex-none"><Clapperboard data-icon="inline-start" />{copy.video}</TabsTrigger>
            </TabsList>
          </Tabs>}
          <Field data-disabled={pending}>
            <FieldLabel htmlFor="generate-prompt">{kind === "audio" ? copy.speechText : copy.prompt}</FieldLabel>
            <Textarea id="generate-prompt" value={prompt} onChange={(event) => setPrompt(event.target.value)} disabled={pending}
              placeholder={kind === "image" ? copy.promptPlaceholder : kind === "video" ? copy.videoPlaceholder : kind === "music" ? copy.musicPlaceholder : kind === "sound" ? copy.soundPlaceholder : copy.speechPlaceholder}
              maxLength={kind === "image" ? 4000 : kind === "video" ? 2500 : kind === "music" ? 2000 : kind === "sound" ? 1000 : 5000} className="min-h-24" required />
          </Field>
          <div className={kind === "audio" ? "grid gap-4" : made ? "grid gap-4 sm:grid-cols-[1fr_8rem]" : "grid gap-4 sm:grid-cols-[1fr_1fr_6rem]"}>
            <Field data-disabled={pending}>
              <FieldLabel htmlFor="generate-provider">{copy.provider}</FieldLabel>
              <NativeSelect id="generate-provider" value={provider} disabled={pending} onChange={(event) => { setProvider(event.target.value); setModel(""); }}>
                <NativeSelectOption value="">{inOrder[0] ? copy.firstAvailable.replace("{provider}", name(inOrder[0])) : copy.defaultOrder}</NativeSelectOption>
                {providers?.available.map((entry) => <NativeSelectOption key={entry.id} value={entry.id}>{name(entry)}</NativeSelectOption>)}
              </NativeSelect>
            </Field>
            {made && <Field data-disabled={pending}>
              <FieldLabel htmlFor="generate-length">{copy.length}</FieldLabel>
              <NativeSelect id="generate-length" value={duration} disabled={pending} onChange={(event) => setDuration(Number(event.target.value))}>
                {(kind === "music" ? MUSIC_DURATIONS : SOUND_DURATIONS).map((value) => <NativeSelectOption key={value} value={value}>
                  {value >= 60 ? copy.minutes.replace("{count}", String(value / 60)) : copy.seconds.replace("{count}", String(value))}</NativeSelectOption>)}
              </NativeSelect>
            </Field>}
            {kind !== "audio" && !made && <><Field data-disabled={pending}>
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
          {kind === "video" && providerModels.length > 0 && <Field data-disabled={pending}>
            <FieldLabel htmlFor="generate-model">{copy.model}</FieldLabel>
            <NativeSelect id="generate-model" value={model} disabled={pending} onChange={(event) => setModel(event.target.value)}>
              <NativeSelectOption value="">{copy.modelDefault.replace("{model}", modelName(providerModels.find((entry) => entry.selected)?.id ?? providerModels[0].id))}</NativeSelectOption>
              {providerModels.map((entry) => <NativeSelectOption key={entry.id} value={entry.id}>{modelName(entry.id)}</NativeSelectOption>)}
            </NativeSelect>
          </Field>}
          {kind !== "audio" && !made && !imagesOnly && <Field data-disabled={pending}>
            <FieldLabel id="generate-references">{copy.references}</FieldLabel>
            <FieldDescription>{kind === "video" ? copy.referencesVideoHint : copy.referencesImageHint}</FieldDescription>
            {choices.length ? <div role="group" aria-labelledby="generate-references" className="scroll-fade grid max-h-44 grid-cols-4 gap-2 overflow-y-auto p-0.5 sm:grid-cols-5">
              {choices.map((asset) => {
                const selected = chosen.includes(asset.id);
                return <button key={asset.id} type="button" aria-pressed={selected} disabled={pending} title={asset.name} onClick={() => toggleReference(asset)}
                  className="group/reference relative rounded-thumb text-left outline-none">
                  <AssetThumbnail workspaceId={workspaceId} assetId={asset.id} kind={asset.kind}
                    className={cn("group-focus-visible/reference:ring-3 group-focus-visible/reference:ring-ring/50", selected && "ring-2 ring-primary")} />
                  <span className="mt-1 block truncate text-xs text-muted-foreground">{asset.name}</span>
                  {selected && <span className="absolute top-1 right-1 grid size-5 place-items-center rounded-full bg-primary text-primary-foreground"><Check className="size-3.5" aria-hidden="true" /></span>}
                </button>;
              })}
            </div> : <p className="text-sm text-muted-foreground">{copy.referencesNone}</p>}
          </Field>}
          {unfit && activeModel && <Alert role="status"><AlertDescription>{(fitting.length ? copy.modelCantTake : copy.noModelTakes)
            .replace("{model}", modelName(activeModel.id)).replace("{models}", fitting.map((entry) => modelName(entry.id)).join(", "))}</AlertDescription></Alert>}
          {unsupported && <Alert role="status"><AlertDescription>{copy.noReferencesHere}</AlertDescription></Alert>}
          <Field data-disabled={pending}>
            <FieldLabel htmlFor="generate-notes">{copy.notes}</FieldLabel>
            <Input id="generate-notes" value={notes} onChange={(event) => setNotes(event.target.value)} disabled={pending}
              placeholder={copy.notesPlaceholder} maxLength={8000} aria-describedby="generate-notes-hint" />
            <FieldDescription id="generate-notes-hint">{copy.notesHint}</FieldDescription>
          </Field>
          {none && <Alert><AlertDescription>{kind === "image" ? copy.noProvider : kind === "video" ? copy.noVideoProvider : made ? copy.noAudioProvider : copy.noSpeechProvider}</AlertDescription></Alert>}
          {problem && <Alert variant="destructive" role="alert"><AlertDescription>{problem}</AlertDescription></Alert>}
          {pending && <p role="status" className="text-sm text-muted-foreground">{kind === "video" ? copy.videoPendingHint : made ? copy.audioPendingHint : copy.pendingHint}</p>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={pending} onClick={() => setOpen(false)}>{copy.cancel}</Button>
            <Button type="submit" disabled={pending || !prompt.trim() || none || unfit || unsupported}>
              {pending ? copy.generating : kind === "audio" || made ? copy.submitAudio : kind === "video" ? copy.submitVideo : count === 1 ? copy.submitOne : copy.submitMany.replace("{count}", String(count))}
            </Button>
          </DialogFooter>
        </FieldGroup>
      </form>
    </DialogContent>
  </Dialog>;
}
