"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { useLocale } from "@/lib/locale-provider";

type Stage = "script" | "assets" | "preview" | "supplemental_outputs" | "final_render" | "verify_delivery";
type Plan = { steps: { stage: Stage; review_required: boolean }[] };

export function GenerationPlan({ workspaceId, runId }: { workspaceId: number; runId: number }) {
  const { messages: { productionPlan: copy } } = useLocale();
  const [open, setOpen] = useState(false);
  const [plan, setPlan] = useState<Plan | null>();
  const [loadError, setLoadError] = useState(false);

  useEffect(() => {
    if (!open) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const current = await invoke<Plan | null>("generation_plan", { workspaceId, runId });
        if (!active) return;
        setPlan(current);
        setLoadError(false);
        if (current) return;
      } catch {
        if (active) setLoadError(true);
      }
      if (active) timer = setTimeout(refresh, 2000);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [open, workspaceId, runId]);

  return (
    <details className="mt-2.5" onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary className="cursor-pointer">{copy.title}</summary>
      {loadError ? <p role="alert" className="mt-2">{copy.loadError}</p>
        : plan === undefined ? <p role="status" className="mt-2">{copy.loading}</p>
        : plan === null ? <p role="status" className="mt-2">{copy.waiting}</p>
        : (
          <ol className="mt-2 list-decimal ps-6">
            {plan.steps.map((step) => (
              <li key={step.stage} className="py-1">
                <span>{copy[step.stage]}</span>
                {step.review_required && <Badge className="ms-2" variant="outline">{copy.reviewRequired}</Badge>}
              </li>
            ))}
          </ol>
        )}
    </details>
  );
}
