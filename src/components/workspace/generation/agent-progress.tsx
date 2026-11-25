"use client";

import { invoke } from "@tauri-apps/api/core";
import { BrainIcon, Clapperboard, FolderSearch, SearchIcon, SparklesIcon, TriangleAlert } from "lucide-react";
import { useReducedMotion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import {
  ChainOfThought, ChainOfThoughtContent, ChainOfThoughtHeader, ChainOfThoughtSearchResult,
  ChainOfThoughtSearchResults, ChainOfThoughtStep,
} from "@/components/ai-elements/chain-of-thought";
import { Shimmer } from "@/components/ai-elements/shimmer";
import { useLocale } from "@/lib/locale-provider";

type Summary = { id: number; status: string; startedAt: number; stage: string; round: number | null; problem: string | null };
type Step = { kind: "thinking" | "search" | "step"; text: string };
type Log = Summary & { steps: Step[]; prompt: string };

/** Follow-ups that hand over workspace material the agent asked for start with this line. */
const MATERIAL_FOLLOW_UP = "MATERIAL YOU ASKED FOR";

const REFRESH_MS = 1500;

/** Everything the run's agent has done so far, oldest first, grouped by step and round (a new
 * round starts when the creator sends feedback): every step of every agent run, including the
 * follow-ups that hand over workspace material it asked for, then the host's own work (such as
 * rendering) once the agent has answered. */
export function AgentProgress({ workspaceId, runId, agent, stage, stageLabel }: {
  workspaceId: number;
  runId: number;
  agent: string;
  stage: string;
  stageLabel: (stage: string) => string;
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
        const details = await Promise.all([...summaries].reverse().map(async (entry) => {
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
  const groups = logs.reduce<{ key: string; stage: string; round: number; logs: Log[] }[]>((all, log) => {
    const round = log.round ?? 1;
    const key = `${log.stage}:${round}`;
    const last = all.at(-1);
    if (last?.key === key) last.logs.push(log);
    else all.push({ key, stage: log.stage, round, logs: [log] });
    return all;
  }, []);
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
          <div className="max-h-96 scroll-fade overflow-y-auto pe-1" role="log" aria-live="polite">
            {groups.map((group, groupIndex) => (
              <div key={group.key} className="flex flex-col gap-3 pb-4 last:pb-0">
                <p className="text-xs font-bold tracking-wide text-muted-foreground uppercase">
                  {group.round > 1
                    ? copy.stageRound.replace("{stage}", stageLabel(group.stage)).replace("{round}", String(group.round))
                    : stageLabel(group.stage)}
                </p>
                {group.logs.map((log) => {
                  const live = log === running;
                  const searches = log.steps.filter((step) => step.kind === "search");
                  const others = log.steps.filter((step) => step.kind !== "search");
                  return (
                    <div key={log.id} className="flex flex-col gap-3 pb-3 last:pb-0">
                      {log.prompt.startsWith(MATERIAL_FOLLOW_UP) && <ChainOfThoughtStep icon={FolderSearch} label={copy.lookingAtMaterial} status="complete" />}
                      {others.length === 0 && live && <ChainOfThoughtStep icon={SparklesIcon} label={copy.reading} status="active" />}
                      {others.map((step, index) => (
                        <ChainOfThoughtStep key={`${index}:${step.text}`} icon={step.kind === "thinking" ? BrainIcon : SparklesIcon}
                          label={step.text} status={live && index === others.length - 1 ? "active" : "complete"} />
                      ))}
                      {log.status === "failed" && log.problem && <ChainOfThoughtStep icon={TriangleAlert} label={log.problem} status="complete" />}
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
                {groupIndex === groups.length - 1 && !running && <ChainOfThoughtStep icon={Clapperboard} label={copy.hostWork} status="active" />}
              </div>
            ))}
            {groups.length === 0 && <ChainOfThoughtStep icon={SparklesIcon} label={copy.reading} status="active" />}
            <div ref={end} />
          </div>
        </ChainOfThoughtContent>
      </ChainOfThought>
    </section>
  );
}
