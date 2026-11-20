import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type ReactNode } from "react";
import { Alert, AlertAction, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";
import type { SupplementalMedia } from "../completed-videos/supplemental-media-preview";
import type { DeliveryFormat } from "./delivery-subtitles";

type TextItem = { kind: "title" | "description" | "tags"; text: string; format_id: number | null };
type MediaItem = SupplementalMedia & { url: string | null };
type Payload = { text: TextItem[]; media: MediaItem[] };

export function SupplementalReview({ workspaceId, runId, resultId, formats, controls }: {
  workspaceId: number;
  runId: number;
  resultId: number;
  formats: DeliveryFormat[];
  controls: (kind: "text" | "media", index: number, label: string) => ReactNode;
}) {
  const { messages } = useLocale();
  const [payload, setPayload] = useState<Payload>();
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let active = true;
    invoke<Payload>("preview_pending_supplemental", { workspaceId, runId, resultId }).then(
      (result) => { if (active) setPayload(result); },
      () => { if (active) setFailed(true); },
    );
    return () => { active = false; };
  }, [workspaceId, runId, resultId, attempt]);

  if (failed) return <Alert variant="destructive">
    <AlertDescription>{messages.completedVideo.previewError}</AlertDescription>
    <AlertAction><Button type="button" size="sm" variant="outline" onClick={() => {
      setFailed(false);
      setAttempt((value) => value + 1);
    }}>{messages.queue.retry}</Button></AlertAction>
  </Alert>;
  if (!payload) return <Skeleton className="h-24" role="status" aria-label={messages.completedVideo.loadingPreview} />;

  const groups = [
    ...formats.map(({ id, name }) => ({ id, name })),
    { id: null, name: messages.productionPlan.sharedSupplemental },
  ];

  return groups.map(({ id, name }) => {
    const text = payload.text.map((item, index) => ({ item, index })).filter(({ item }) => item.format_id === id);
    const media = payload.media.map((item, index) => ({ item, index })).filter(({ item }) => item.format_id === id);
    if (!text.length && !media.length) return null;
    return <section key={id ?? "shared"} className="min-w-0">
      <h3 className="border-b pb-2 text-base font-bold">{name}</h3>
      <dl className="divide-y">
        {text.map(({ item, index }) => <div key={index} className="grid gap-1 py-3 sm:grid-cols-[8rem_minmax(0,1fr)]">
          <dt className="text-sm font-semibold text-muted-foreground">{messages.completedVideo[`supplemental_${item.kind}`]}</dt>
          <dd className="grid min-w-0 gap-2 text-sm"><p className="wrap-anywhere whitespace-pre-wrap">{item.text}</p>{controls("text", index, messages.completedVideo[`supplemental_${item.kind}`] + " " + (index + 1))}</dd>
        </div>)}
      </dl>
      {media.length > 0 && <ul className="grid gap-3 pt-3 sm:grid-cols-2">
        {media.map(({ item, index }) => <li key={index} className="min-w-0 rounded-lg border p-2.5">
          <p className="mb-2 truncate text-sm font-semibold" title={item.name}>{item.name}</p>
          <ReviewMedia item={item} />
          <div className="mt-2">{controls("media", index, item.name)}</div>
        </li>)}
      </ul>}
    </section>;
  });
}

function ReviewMedia({ item }: { item: MediaItem }) {
  const { messages } = useLocale();
  const [failed, setFailed] = useState(false);
  if (!item.url || failed) return <p className="text-sm text-muted-foreground">{messages.completedVideo.previewError}</p>;

  const unavailable = () => setFailed(true);
  if (item.kind === "image") return (
    // eslint-disable-next-line @next/next/no-img-element
    <img src={item.url} alt={item.name} loading="lazy" className="max-h-48 w-full rounded-[4px] object-contain" onError={unavailable} />
  );
  if (item.kind === "audio") return <audio src={item.url} aria-label={item.name} controls preload="none"
    className="w-full" onError={unavailable} />;
  return <video src={item.url} aria-label={item.name} controls playsInline preload="none"
    className="aspect-video w-full rounded-[4px] bg-ink object-contain" onError={unavailable} />;
}
