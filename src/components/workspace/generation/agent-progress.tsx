"use client";

import { invoke } from "@tauri-apps/api/core";
import { BrainIcon, Clapperboard, FolderSearch, SearchIcon, SparklesIcon } from "lucide-react";
import { useReducedMotion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import {
  ChainOfThought, ChainOfThoughtContent, ChainOfThoughtHeader, ChainOfThoughtSearchResult,
  ChainOfThoughtSearchResults, ChainOfThoughtStep,
} from "@/components/ai-elements/chain-of-thought";
import { Shimmer } from "@/components/ai-elements/shimmer";
import { useLocale } from "@/lib/locale-provider";

type Summary = { id: number; status: string; startedAt: number; stage: string; attempt: number | null };
type Step = { kind: "thinking" | "search" | "step"; text: string };
type Log = Summary & { steps: Step[] };

const REFRESH_MS = 1500;

/** Everything the run's agent has done in the current stage attempt, as a log: every step of
 * every agent run (including rounds where it asked for workspace material), then the host's own
 * work (such as rendering) once the agent has answered. */
export function AgentProgress({ workspaceId, runId, agent, stage }: {
  workspaceId: number;
  runId: number;
  agent: string;
  stage: string;
}) {
  const { messages: { agentProgress: copy } } = useLocale();
  const reduceMotion = useReducedMotion();
  const [logs, setLogs] = useState<Log[]>([]);
  const [now, setNow] = useState(() => Date.now());
  // Finished runs never change, so they are read once.
  const finished = useRef(new Map<number, Log>());
  const end = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const summaries = await invoke<Summary[]>("agent_logs", { workspaceId, runId });
        const [latest] = summaries;
        const current = latest
          ? summaries.filter((entry) => entry.stage === latest.stage && entry.attempt === latest.attempt).reverse()
          : [];
        const details = await Promise.all(current.map(async (entry) => {
          const known = finished.current.get(entry.id);
          if (known) return known;
          const detail = await invoke<Log | null>("agent_log", { workspaceId, logId: entry.id });
          if (detail && detail.status !== "running") finished.current.set(entry.id, detail);
          return detail;
        }));
        if (active) { setLogs(details.filter((entry): entry is Log => entry !== null)); setNow(Date.now()); }
      } catch {
        if (active) setLogs([]);
      }
      if (active) timer = setTimeout(refresh, REFRESH_MS);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [workspaceId, runId]);

  const running = logs.at(-1)?.status === "running" ? logs.at(-1) : undefined;
  const stepCount = logs.reduce((total, log) => total + log.steps.length, 0);
  useEffect(() => { end.current?.scrollIntoView({ block: "nearest", behavior: reduceMotion ? "auto" : "smooth" }); }, [stepCount, logs.length, reduceMotion]);

  const seconds = running ? Math.max(0, Math.round(now / 1000 - running.startedAt)) : 0;
  const heading = running
    ? copy.thinking.replace("{agent}", agent).replace("{seconds}", String(seconds))
    : copy.producing.replace("{stage}", stage);

  return (
    <section aria-label={copy.label} className="rounded-xl border bg-card p-4 text-card-foreground">
      <ChainOfThought defaultOpen>
        <ChainOfThoughtHeader>
          {reduceMotion ? heading : <Shimmer as="span" duration={2.4}>{heading}</Shimmer>}
        </ChainOfThoughtHeader>
        <ChainOfThoughtContent>
          <div className="max-h-96 overflow-y-auto pe-1" role="log" aria-live="polite">
            {logs.map((log, round) => {
              const live = log === running;
              const searches = log.steps.filter((step) => step.kind === "search");
              const others = log.steps.filter((step) => step.kind !== "search");
              return (
                <div key={log.id} className="flex flex-col gap-3 pb-3">
                  {round > 0 && <ChainOfThoughtStep icon={FolderSearch} label={copy.lookingAtMaterial} status="complete" />}
                  {others.length === 0 && live && <ChainOfThoughtStep icon={SparklesIcon} label={copy.reading} status="active" />}
                  {others.map((step, index) => (
                    <ChainOfThoughtStep key={`${index}:${step.text}`} icon={step.kind === "thinking" ? BrainIcon : SparklesIcon}
                      label={step.text} status={live && index === others.length - 1 ? "active" : "complete"} />
                  ))}
                  {searches.length > 0 && (
                    <ChainOfThoughtStep icon={SearchIcon} label={copy.searching} status="complete">
                      <ChainOfThoughtSearchResults>
                        {searches.map((step, index) => <ChainOfThoughtSearchResult key={`${index}:${step.text}`}>{step.text}</ChainOfThoughtSearchResult>)}
                      </ChainOfThoughtSearchResults>
                    </ChainOfThoughtStep>
                  )}
                </div>
              );
            })}
            {!running && <ChainOfThoughtStep icon={Clapperboard} label={copy.hostWork} status="active" />}
            <div ref={end} />
          </div>
        </ChainOfThoughtContent>
      </ChainOfThought>
    </section>
  );
}
