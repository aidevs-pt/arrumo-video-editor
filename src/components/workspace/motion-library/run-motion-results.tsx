import { invoke } from "@tauri-apps/api/core";
import Link from "next/link";
import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useLocale } from "@/lib/locale-provider";
import { workspaceHref } from "../home/workspace-links";

export type MotionStageResult = {
  id: number;
  stage_index: number;
  decision: string | null;
  result: Record<string, unknown>;
};

type MotionPlanStep = { stage: string; review_required: boolean };

export function RunMotionResults({ workspaceId, runId, results, steps, stageIndex }: {
  workspaceId: number;
  runId: number;
  results: MotionStageResult[];
  steps: MotionPlanStep[];
  stageIndex: number;
}) {
  const { messages: { motionLibrary: copy } } = useLocale();
  const items = results.flatMap(({ id, stage_index, decision, result }) => {
    const step = steps[stage_index];
    if (stage_index >= stageIndex || step?.stage !== "assets"
      || !(decision === "approved" || (!step.review_required && decision === null))
      || !Array.isArray(result.motion_items)) return [];
    return result.motion_items.flatMap((item: unknown, index: number) => {
      if (!item || typeof item !== "object" || !("title" in item) || typeof item.title !== "string"
        || !("kind" in item) || (item.kind !== "animation" && item.kind !== "transition")) return [];
      return [{ resultId: id, index, title: item.title, kind: item.kind } as const];
    });
  });

  if (items.length === 0) return null;

  return (
    <Card size="sm">
      <CardHeader><CardTitle>{copy.runItems}</CardTitle></CardHeader>
      <CardContent>
        <ul className="divide-y">
          {items.map((item) => <ResultRow key={`${item.resultId}:${item.index}`} workspaceId={workspaceId} runId={runId} {...item} />)}
        </ul>
      </CardContent>
    </Card>
  );
}

function ResultRow({ workspaceId, runId, resultId, index, title, kind }: {
  workspaceId: number;
  runId: number;
  resultId: number;
  index: number;
  title: string;
  kind: "animation" | "transition";
}) {
  const { messages: { motionLibrary: copy } } = useLocale();
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<{ profileId: number | null }>();
  const [error, setError] = useState(false);

  async function save() {
    if (saving) return;
    setSaving(true);
    setError(false);
    try {
      const item = await invoke<{ settings: { profile_id: number | null } }>("save_motion_result", {
        workspaceId, runId, resultId, index,
      });
      setSaved({ profileId: item.settings.profile_id });
    } catch {
      setError(true);
    } finally {
      setSaving(false);
    }
  }

  return (
    <li className="flex flex-wrap items-center justify-between gap-2 py-2 first:pt-0 last:pb-0">
      <div className="flex min-w-0 flex-wrap items-center gap-2">
        <Badge variant="secondary">{copy[kind]}</Badge>
        <span className="wrap-anywhere text-sm font-medium">{title}</span>
      </div>
      {saved ? <Button asChild size="sm" variant="outline">
        <Link href={workspaceHref("motion-library", workspaceId, saved.profileId ?? undefined)}>{copy.openSavedItem}</Link>
      </Button> : <Button type="button" size="sm" variant="outline" disabled={saving}
        aria-label={`${copy.saveRunItem} · ${title}`} onClick={() => void save()}>
        {saving ? copy.saving : copy.saveRunItem}
      </Button>}
      {error && <p role="alert" className="basis-full text-sm text-destructive">{copy.runItemError}</p>}
    </li>
  );
}
