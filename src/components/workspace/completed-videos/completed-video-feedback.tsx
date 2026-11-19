import { invoke } from "@tauri-apps/api/core";
import { useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";
import type { Stage } from "../generation/generation-plan";

export type VideoFeedback = { id: number; result_id: number | null; markdown: string };
export type FeedbackResult = { id: number; stage_index: number; attempt: number };

export function CompletedVideoFeedback({ workspaceId, runId, initialFeedback, results, stages }: {
  workspaceId: number;
  runId: number;
  initialFeedback: VideoFeedback[];
  results: FeedbackResult[];
  stages: { stage: Stage }[];
}) {
  const { messages: { completedVideo: copy, productionPlan } } = useLocale();
  const [feedback, setFeedback] = useState(initialFeedback);
  const [target, setTarget] = useState("");
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
        workspaceId, runId, resultId: target ? Number(target) : null, markdown,
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
    <section className="flex flex-col gap-3" aria-labelledby={`video-feedback-${runId}`}>
      <h4 id={`video-feedback-${runId}`} className="font-medium">{copy.feedback}</h4>
      {feedback.length > 0 && (
        <ul className="flex flex-col gap-2 text-sm">
          {feedback.map((entry) => (
            <li key={entry.id}>
              <span className="font-medium">{resultLabel(results.find((result) => result.id === entry.result_id))}</span>
              <p className="wrap-anywhere whitespace-pre-wrap text-muted-foreground">{entry.markdown}</p>
            </li>
          ))}
        </ul>
      )}
      <form onSubmit={save}>
        <FieldGroup>
          <Field>
            <FieldLabel htmlFor={`feedback-target-${runId}`}>{copy.feedbackTarget}</FieldLabel>
            <NativeSelect id={`feedback-target-${runId}`} className="max-w-full" value={target} disabled={pending}
              onChange={(event) => setTarget(event.target.value)}>
              <NativeSelectOption value="">{copy.wholeVideo}</NativeSelectOption>
              {results.map((result) => (
                <NativeSelectOption key={result.id} value={result.id}>{resultLabel(result)}</NativeSelectOption>
              ))}
            </NativeSelect>
          </Field>
          <Field>
            <FieldLabel htmlFor={`feedback-note-${runId}`}>{copy.feedbackNote}</FieldLabel>
            <Textarea id={`feedback-note-${runId}`} value={markdown} required disabled={pending}
              onChange={(event) => setMarkdown(event.target.value)} />
          </Field>
          <div className="flex flex-wrap items-center gap-2">
            <Button type="submit" size="sm" variant="outline" disabled={pending || !markdown.trim()}>{copy.addFeedback}</Button>
            {status === "feedbackSaved" && <p role="status" className="text-sm text-muted-foreground">{copy[status]}</p>}
          </div>
          {status === "feedbackError" && <Alert variant="destructive"><AlertDescription>{copy[status]}</AlertDescription></Alert>}
        </FieldGroup>
      </form>
    </section>
  );
}
