import { invoke } from "@tauri-apps/api/core";
import { CodecHint } from "@/components/workspace/media/codec-hint";
import { useEffect, useMemo, useRef, useState } from "react";
import { Alert, AlertAction, AlertDescription } from "@/components/ui/alert";
import { Download } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import { useLocale } from "@/lib/locale-provider";

import { SubtitleRevisionControls, type SubtitleRevision } from "./subtitle-revision-controls";

type DeliveryPreview = SubtitleRevision & { video_url: string; languages: string[] };
type Cue = { timestamp: string; finish: string; start: number; end: number; text: string };

function seconds(timestamp: string) {
  const [hours, minutes, seconds, milliseconds] = timestamp.split(/[:,.]/).map(Number);
  return hours * 3600 + minutes * 60 + seconds + milliseconds / 1000;
}

function readCues(subtitles: string): Cue[] {
  return subtitles.replace(/\r\n?/g, "\n").trim().split(/\n[^\S\n]*\n/).flatMap((block) => {
    const lines = block.split("\n");
    const index = lines.findIndex((line) => /^\s*\d+:\d{2}:\d{2}[,.]\d{3}\s+-->/.test(line));
    if (index < 0) return [];
    const timing = lines[index].match(/^\s*(\d+:\d{2}:\d{2}[,.]\d{3})\s+-->\s+(\d+:\d{2}:\d{2}[,.]\d{3})/);
    if (!timing) return [];
    const [, timestamp, finish] = timing;
    return [{ timestamp, finish, start: seconds(timestamp), end: seconds(finish), text: lines.slice(index + 1).join("\n") }];
  });
}

export function CompletedOutputPreview({ workspaceId, runId, formatId, label, subtitleName, showTranscript = false }: {
  workspaceId: number;
  runId: number;
  formatId: number;
  label: string;
  /** The subtitle file's name, used when saving a track. */
  subtitleName?: string;
  showTranscript?: boolean;
}) {
  const { locale, messages: { completedVideo: copy, queue, subtitleRevisions: tracks } } = useLocale();
  const [preview, setPreview] = useState<DeliveryPreview>();
  const [error, setError] = useState<"previewError" | "playbackError">();
  const [attempt, setAttempt] = useState(0);
  const video = useRef<HTMLVideoElement>(null);
  const [time, setTime] = useState(-1);
  const [seekable, setSeekable] = useState(false);
  const [track, setTrack] = useState<{ language: string; latest: SubtitleRevision; historical?: SubtitleRevision }>();
  const [loadingTrack, setLoadingTrack] = useState(false);
  const [trackError, setTrackError] = useState(false);
  const languageSelect = useRef<HTMLSelectElement>(null);
  const [saving, setSaving] = useState<"saving" | "saved" | "failed">();
  const languageNames = useMemo(() => new Intl.DisplayNames([locale], { type: "language" }), [locale]);
  const displayed = track?.historical ?? track?.latest;
  const [query, setQuery] = useState("");
  const cues = useMemo(() => showTranscript && displayed ? readCues(displayed.subtitles) : [], [displayed, showTranscript]);

  const filter = query.trim().toLocaleLowerCase();
  const caption = cues.find((cue) => time >= cue.start && time < cue.end);
  const visibleCues = useMemo(() => cues.filter((cue) => !filter || cue.text.toLocaleLowerCase().includes(filter)), [cues, filter]);

  useEffect(() => {
    let active = true;
    invoke<DeliveryPreview>("preview_completed_output", { workspaceId, runId, formatId }).then(
      (media) => {
        if (!active) return;
        setPreview(media);
        setTrack({ language: "", latest: media });
      },
      () => { if (active) setError("previewError"); },
    );
    return () => { active = false; };
  }, [attempt, formatId, runId, workspaceId]);

  async function selectTrack(language: string) {
    if (!track || track.language === language || loadingTrack) return;
    setLoadingTrack(true);
    setTrackError(false);
    try {
      const latest = await invoke<SubtitleRevision>("read_completed_subtitle_revision", {
        workspaceId, runId, formatId, language: language || null,
      });
      setTrack({ language, latest });
      setQuery("");
    } catch {
      setTrackError(true);
    } finally {
      setLoadingTrack(false);
    }
  }

  function saveTrack(language: string, latest: SubtitleRevision) {
    setTrack({ language, latest });
    setTrackError(false);
    if (language && !preview?.languages.includes(language)) {
      setPreview((current) => current && { ...current, languages: [...current.languages, language].sort() });
      setQuery("");
      requestAnimationFrame(() => languageSelect.current?.focus());
    }
  }

  async function downloadTrack() {
    if (!track) return;
    setSaving("saving");
    const base = (subtitleName ?? "subtitles.srt").replace(/\.srt$/i, "");
    try {
      const written = await invoke<boolean>("export_completed_subtitles", {
        workspaceId, runId, formatId, language: track.language || null,
        revision: track.historical?.revision ?? null, fileName: `${base}${track.language ? `.${track.language}` : ""}.srt`,
      });
      setSaving(written ? "saved" : undefined);
    } catch {
      setSaving("failed");
    }
  }

  function languageName(language: string) {
    try {
      return languageNames.of(language) ?? language;
    } catch {
      return language;
    }
  }

  return (
    <div className={cn("grid min-w-0 items-start gap-5", showTranscript && "lg:grid-cols-[minmax(0,1.55fr)_minmax(0,1fr)]")}>
      {error ? (
        <Alert variant="destructive">
          <AlertDescription>{copy[error]}{error === "playbackError" && <CodecHint />}</AlertDescription>
          <AlertAction>
            <Button variant="outline" size="sm" onClick={() => {
              setPreview(undefined);
              setTrack(undefined);
              setSeekable(false);
              setTime(-1);
              setError(undefined);
              setAttempt((current) => current + 1);
            }}>{queue.retry}</Button>
          </AlertAction>
        </Alert>
      ) : !preview ? (
        <div role="status" aria-label={copy.loadingPreview}>
          <Skeleton className="aspect-video w-full rounded-xl" />
        </div>
      ) : (
        <div className="min-w-0">
          <video ref={video} aria-label={label} className="aspect-video w-full rounded-thumb bg-ink object-contain"
            controls playsInline preload="metadata" src={preview.video_url}
            onLoadedMetadata={() => { setSeekable(true); setTime(0); }}
            onTimeUpdate={showTranscript ? (event) => setTime(event.currentTarget.currentTime) : undefined}
            onError={() => { setSeekable(false); setTime(-1); setError("playbackError"); }} />
          {showTranscript && <div className="mt-2 min-h-8 text-center">
            {caption && <span className="inline-block rounded-thumb bg-ink px-2 py-1.5 text-sm font-bold whitespace-pre-wrap text-white">{caption.text}</span>}
          </div>}
        </div>
      )}
      {showTranscript && preview && track && <section className="min-w-0" aria-labelledby="delivery-transcript">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
          <h2 id="delivery-transcript" className="text-[1.3125rem]">{copy.subtitleText}</h2>
          <Button type="button" variant="outline" size="sm" disabled={saving === "saving"} onClick={() => void downloadTrack()}>
            <Download data-icon="inline-start" />{saving === "saving" ? copy.downloading : copy.download}
          </Button>
        </div>
        {preview.languages.length > 0 && preview.languages.length <= 3 ? (
          <ToggleGroup type="single" variant="outline" className="mb-3 w-full" aria-label={tracks.language}
            value={track.language} disabled={loadingTrack} onValueChange={(value) => { if (value !== track.language && (value || track.language)) void selectTrack(value); }}>
            <ToggleGroupItem value="" className="flex-1">{tracks.primary}</ToggleGroupItem>
            {preview.languages.map((language) => <ToggleGroupItem key={language} value={language} className="flex-1">{languageName(language)}</ToggleGroupItem>)}
          </ToggleGroup>
        ) : preview.languages.length > 3 && <NativeSelect ref={languageSelect} className="mb-3 w-full" aria-label={tracks.language}
          value={track.language} disabled={loadingTrack} onChange={(event) => void selectTrack(event.target.value)}>
          <NativeSelectOption value="">{tracks.primary}</NativeSelectOption>
          {preview.languages.map((language) => <NativeSelectOption key={language} value={language}>
            {languageName(language)}
          </NativeSelectOption>)}
        </NativeSelect>}
        {saving === "saved" && <p role="status" className="mb-3 text-sm text-muted-foreground">{copy.downloaded}</p>}
        {saving === "failed" && <Alert variant="destructive" className="mb-3"><AlertDescription>{copy.downloadError}</AlertDescription></Alert>}
        {trackError && <Alert variant="destructive" className="mb-3"><AlertDescription>{tracks.readError}</AlertDescription></Alert>}
        <SubtitleRevisionControls key={track.language} workspaceId={workspaceId} runId={runId} formatId={formatId}
          language={track.language} languages={preview.languages} disabled={loadingTrack}
          latest={track.latest} current={track.historical ?? track.latest}
          onRead={(historical) => setTrack((current) => current?.language === track.language ? { ...current, historical } : current)}
          onSave={saveTrack} />
        {cues.length > 0 && <Input type="search" className="mb-3" aria-label={copy.findSubtitles}
          placeholder={copy.findSubtitles} value={query} onChange={(event) => setQuery(event.target.value)} />}
        {cues.length ? <ol className="max-h-[60vh] divide-y overflow-auto border-y">
          {visibleCues.length === 0 &&
            <li className="p-3 text-sm text-muted-foreground">{copy.noMatchingSubtitles}</li>}
          {visibleCues.map((cue, index) => {
            const active = time >= cue.start && time < cue.end;
            const content = <>
              <span className="grid shrink-0 content-start text-xs text-muted-foreground">
                <span>{cue.timestamp}</span><span>{cue.finish}</span>
              </span>
              <span className="min-w-0 wrap-anywhere whitespace-pre-wrap">{cue.text}</span>
            </>;
            return <li key={`${cue.timestamp}-${index}`} className={cn(active && "bg-primary/8 font-medium")}>
              {seekable ? <button type="button" className="flex w-full gap-3 p-3 text-start text-sm hover:bg-accent focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ring"
                aria-current={active ? "true" : undefined}
                onClick={() => { if (video.current) video.current.currentTime = cue.start; }}>
                {content}
              </button> : <div className="flex gap-3 p-3 text-sm">{content}</div>}
            </li>;
          })}
        </ol> : <p className="max-h-[60vh] overflow-auto whitespace-pre-wrap text-sm">{displayed?.subtitles}</p>}
      </section>}
    </div>
  );
}
