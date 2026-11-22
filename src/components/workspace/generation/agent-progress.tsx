"use client";

import { invoke } from "@tauri-apps/api/core";
import { BrainIcon, Clapperboard, SearchIcon, SparklesIcon } from "lucide-react";
import { useReducedMotion } from "motion/react";
import { useEffect, useState } from "react";
import {
  ChainOfThought, ChainOfThoughtContent, ChainOfThoughtHeader, ChainOfThoughtSearchResult,
  ChainOfThoughtSearchResults, ChainOfThoughtStep,
} from "@/components/ai-elements/chain-of-thought";
import { Shimmer } from "@/components/ai-elements/shimmer";
import { useLocale } from "@/lib/locale-provider";

type Summary = { id: number; status: string; startedAt: number };
type Step = { kind: "thinking" | "search" | "step"; text: string };
type Log = Summary & { steps: Step[] };

const REFRESH_MS = 1500;
const MAX_SHOWN = 8;

/** What the run's agent is doing right now: its thoughts and searches as they happen, or the
 * host's own work (such as rendering) once the agent has answered. */
export function AgentProgress({ workspaceId, runId, agent, stage }: {
  workspaceId: number;
  runId: number;
  agent: string;
  stage: string;
}) {
  const { messages: { agentProgress: copy } } = useLocale();
  const reduceMotion = useReducedMotion();
  const [log, setLog] = useState<Log | null>();
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const [latest] = await invoke<Summary[]>("agent_logs", { workspaceId, runId });
        const detail = latest?.status === "running"
          ? await invoke<Log | null>("agent_log", { workspaceId, logId: latest.id }) : null;
        if (active) { setLog(detail); setNow(Date.now()); }
      } catch {
        if (active) setLog(null);
      }
      if (active) timer = setTimeout(refresh, REFRESH_MS);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [workspaceId, runId]);

  const seconds = log ? Math.max(0, Math.round(now / 1000 - log.startedAt)) : 0;
  const steps = log?.steps.slice(-MAX_SHOWN) ?? [];
  const heading = log
    ? copy.thinking.replace("{agent}", agent).replace("{seconds}", String(seconds))
    : copy.producing.replace("{stage}", stage);
  const searches = steps.filter((step) => step.kind === "search");

  return (
    <section aria-label={copy.label} className="rounded-xl border bg-card p-4 text-card-foreground">
      <ChainOfThought defaultOpen>
        <ChainOfThoughtHeader>
          {reduceMotion ? heading : <Shimmer as="span" duration={2.4}>{heading}</Shimmer>}
        </ChainOfThoughtHeader>
        <ChainOfThoughtContent>
          {!log && (
            <ChainOfThoughtStep icon={Clapperboard} label={copy.hostWork} status="active" />
          )}
          {log && steps.length === 0 && (
            <ChainOfThoughtStep icon={SparklesIcon} label={copy.reading} status="active" />
          )}
          {steps.filter((step) => step.kind !== "search").map((step, index, shown) => (
            <ChainOfThoughtStep key={`${index}:${step.text}`} icon={step.kind === "thinking" ? BrainIcon : SparklesIcon}
              label={step.text} status={index === shown.length - 1 ? "active" : "complete"} />
          ))}
          {searches.length > 0 && (
            <ChainOfThoughtStep icon={SearchIcon} label={copy.searching} status="complete">
              <ChainOfThoughtSearchResults>
                {searches.map((step) => <ChainOfThoughtSearchResult key={step.text}>{step.text}</ChainOfThoughtSearchResult>)}
              </ChainOfThoughtSearchResults>
            </ChainOfThoughtStep>
          )}
        </ChainOfThoughtContent>
      </ChainOfThought>
    </section>
  );
}
