import { invoke } from "@tauri-apps/api/core";
import { MessageSquare } from "lucide-react";
import { useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";
import type { Stage } from "../generation/generation-plan";

export type VideoFeedback = { id: number; result_id: number | null; markdown: string; created_at: number };
export type FeedbackResult = { id: number; stage_index: number; attempt: number };

export function CompletedVideoFeedback({ workspaceId, runId, initialFeedback, results, stages }: {
  workspaceId: number;
  runId: number;
  initialFeedback: VideoFeedback[];
  results: FeedbackResult[];
  stages: { stage: Stage }[];
}) {
  const { locale, messages: { completedVideo: copy, productionPlan } } = useLocale();
  const [feedback, setFeedback] = useState(initialFeedback);
  const [target, setTarget] = useState("video");
  const [markdown, setMarkdown] = useState("");
  const [pending, setPending] = useState(false);
  const [status, setStatus] = useState<"feedbackSaved" | "feedbackError">();

  function resultLabel(result: FeedbackResult | undefined) {
    const step = result && stages[result.stage_index];
    return step ? `${productionPlan[step.stage]} · ${copy.attempt} ${result.attempt}` : copy.wholeVideo;
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending || !markdown.trim()) return;
    setPending(true);
    setStatus(undefined);
    try {
      const entry = await invoke<VideoFeedback>("add_completed_video_feedback", {
        workspaceId, runId, resultId: target === "video" ? null : Number(target), markdown,
      });
      setFeedback((current) => [...current, entry]);
      setMarkdown("");
      setStatus("feedbackSaved");
    } catch {
      setStatus("feedbackError");
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="flex flex-col gap-5">
      {feedback.length ? (
        <ol className="divide-y rounded-xl border bg-card px-4 text-sm">
          {feedback.map((entry) => {
            const date = new Date(entry.created_at * 1000);
            return (
              <li key={entry.id} className="flex flex-col gap-2 py-3">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <Badge variant="secondary">{resultLabel(results.find((result) => result.id === entry.result_id))}</Badge>
                  <time className="text-xs text-muted-foreground" dateTime={date.toISOString()}>
                    {date.toLocaleString(locale, { dateStyle: "medium", timeStyle: "short" })}
                  </time>
                </div>
                <p className="wrap-anywhere whitespace-pre-wrap">{entry.markdown}</p>
              </li>
            );
          })}
        </ol>
      ) : (
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon"><MessageSquare aria-hidden="true" /></EmptyMedia>
            <EmptyTitle>{copy.noFeedback}</EmptyTitle>
          </EmptyHeader>
        </Empty>
      )}
      <Card>
        <CardHeader><CardTitle>{copy.addFeedback}</CardTitle></CardHeader>
        <CardContent>
          <form onSubmit={save}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor={`feedback-target-${runId}`}>{copy.feedbackTarget}</FieldLabel>
                <Select value={target} disabled={pending} onValueChange={setTarget}>
                  <SelectTrigger id={`feedback-target-${runId}`} className="max-w-full"><SelectValue /></SelectTrigger>
                  <SelectContent><SelectGroup>
                    <SelectItem value="video">{copy.wholeVideo}</SelectItem>
                    {results.map((result) => (
                      <SelectItem key={result.id} value={String(result.id)}>{resultLabel(result)}</SelectItem>
                    ))}
                  </SelectGroup></SelectContent>
                </Select>
              </Field>
              <Field>
                <FieldLabel htmlFor={`feedback-note-${runId}`}>{copy.feedbackNote}</FieldLabel>
                <Textarea id={`feedback-note-${runId}`} value={markdown} required disabled={pending}
                  onChange={(event) => setMarkdown(event.target.value)} />
              </Field>
              <div className="flex flex-wrap items-center gap-2">
                <Button type="submit" disabled={pending || !markdown.trim()}>{copy.addFeedback}</Button>
                {status === "feedbackSaved" && <p role="status" className="text-sm text-muted-foreground">{copy[status]}</p>}
              </div>
              {status === "feedbackError" && <Alert variant="destructive"><AlertDescription>{copy[status]}</AlertDescription></Alert>}
            </FieldGroup>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
