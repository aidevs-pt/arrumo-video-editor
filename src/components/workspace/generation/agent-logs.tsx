"use client";

import { invoke } from "@tauri-apps/api/core";
import { Copy, ScrollText } from "lucide-react";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle, SheetTrigger } from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useLocale } from "@/lib/locale-provider";

type LogStatus = "running" | "succeeded" | "failed" | "timed_out" | "unavailable";

type LogSummary = {
  id: number;
  runId: number | null;
  stage: string;
  attempt: number | null;
  provider: string;
  status: LogStatus;
  startedAt: number;
  durationMs: number | null;
  problem: string | null;
};

type Log = LogSummary & {
  exitCode: number | null;
  command: string;
  prompt: string;
  output: string;
  errors: string;
  answer: string | null;
};

const REFRESH_MS = 2000;

/** Opens the agent runs of one generation run, or of the whole workspace when `runId` is omitted. */
export function AgentLogs({ workspaceId, runId, className, size = "sm" }: { workspaceId: number; runId?: number; className?: string; size?: "sm" | "default" }) {
  const { messages: { agentLogs: copy } } = useLocale();
  const [open, setOpen] = useState(false);
  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetTrigger asChild>
        <Button type="button" variant="outline" size={size} className={className}>
          <ScrollText aria-hidden="true" />{copy.open}
        </Button>
      </SheetTrigger>
      <SheetContent side="right" className="w-full gap-0 data-[side=right]:sm:max-w-3xl">
        <SheetHeader>
          <SheetTitle>{copy.title}</SheetTitle>
          <SheetDescription>{copy.description}</SheetDescription>
        </SheetHeader>
        {open && <AgentLogBrowser workspaceId={workspaceId} runId={runId} />}
      </SheetContent>
    </Sheet>
  );
}

function AgentLogBrowser({ workspaceId, runId }: { workspaceId: number; runId?: number }) {
  const { locale, messages: { agentLogs: copy } } = useLocale();
  const [logs, setLogs] = useState<LogSummary[]>();
  const [selected, setSelected] = useState<number>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const items = await invoke<LogSummary[]>("agent_logs", { workspaceId, runId: runId ?? null });
        if (!active) return;
        setLogs(items);
        setFailed(false);
        setSelected((current) => current ?? items[0]?.id);
      } catch {
        if (active) setFailed(true);
      }
      if (active) timer = setTimeout(refresh, REFRESH_MS);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [workspaceId, runId]);

  if (failed && !logs) return <Alert variant="destructive" className="m-4"><AlertDescription>{copy.error}</AlertDescription></Alert>;
  if (!logs) return <Skeleton className="m-4 h-24" aria-label={copy.loading} />;
  if (!logs.length) return <p className="px-4 text-sm text-muted-foreground">{copy.empty}</p>;

  const time = (seconds: number) => new Intl.DateTimeFormat(locale, { dateStyle: "short", timeStyle: "medium" }).format(new Date(seconds * 1000));
  return (
    <div className="grid min-h-0 flex-1 grid-rows-[auto_1fr] gap-3 px-4 pb-4 md:grid-cols-[14rem_1fr] md:grid-rows-1">
      <ol className="flex max-h-48 flex-col gap-1 scroll-fade overflow-y-auto md:max-h-none" aria-label={copy.runs}>
        {logs.map((log) => (
          <li key={log.id}>
            <button type="button" onClick={() => setSelected(log.id)} aria-current={selected === log.id}
              className="flex w-full flex-col items-start gap-1 rounded-md border px-3 py-2 text-start text-sm hover:bg-muted aria-[current=true]:border-primary aria-[current=true]:bg-muted">
              <span className="flex w-full items-center justify-between gap-2">
                <span className="font-medium">{copy.stages[log.stage as keyof typeof copy.stages] ?? log.stage}</span>
                <StatusBadge status={log.status} />
              </span>
              <span className="text-xs text-muted-foreground">{time(log.startedAt)}{log.attempt ? ` · ${copy.attempt.replace("{attempt}", String(log.attempt))}` : ""}</span>
            </button>
          </li>
        ))}
      </ol>
      {selected !== undefined && <AgentLogDetail workspaceId={workspaceId} logId={selected} />}
    </div>
  );
}

function StatusBadge({ status }: { status: LogStatus }) {
  const { messages: { agentLogs: copy } } = useLocale();
  const variant = status === "succeeded" ? "secondary" : status === "running" ? "outline" : "destructive";
  return <Badge variant={variant}>{copy.status[status]}</Badge>;
}

function AgentLogDetail({ workspaceId, logId }: { workspaceId: number; logId: number }) {
  const { messages: { agentLogs: copy } } = useLocale();
  const [log, setLog] = useState<Log | null>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    setLog(undefined);
    async function refresh() {
      try {
        const item = await invoke<Log | null>("agent_log", { workspaceId, logId });
        if (!active) return;
        setLog(item);
        setFailed(false);
        if (item?.status === "running") timer = setTimeout(refresh, REFRESH_MS);
      } catch {
        if (active) setFailed(true);
      }
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [workspaceId, logId]);

  if (failed) return <Alert variant="destructive"><AlertDescription>{copy.error}</AlertDescription></Alert>;
  if (log === undefined) return <Skeleton className="h-48" aria-label={copy.loading} />;
  if (log === null) return <p className="text-sm text-muted-foreground">{copy.empty}</p>;

  const seconds = log.durationMs === null ? null : (log.durationMs / 1000).toFixed(1);
  return (
    <section className="flex min-h-0 min-w-0 flex-col gap-3" aria-live="polite">
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
        <dt className="text-muted-foreground">{copy.agent}</dt><dd>{log.provider}</dd>
        {seconds && <><dt className="text-muted-foreground">{copy.duration}</dt><dd>{copy.seconds.replace("{seconds}", seconds)}</dd></>}
        {log.exitCode !== null && <><dt className="text-muted-foreground">{copy.exitCode}</dt><dd>{log.exitCode}</dd></>}
        {log.problem && <><dt className="text-muted-foreground">{copy.problem}</dt><dd className="text-destructive">{log.problem}</dd></>}
      </dl>
      <Tabs defaultValue={log.errors && log.status !== "succeeded" ? "errors" : "output"} className="min-h-0 flex-1">
        <TabsList className="flex-wrap">
          <TabsTrigger value="output">{copy.output}</TabsTrigger>
          <TabsTrigger value="errors">{copy.errors}</TabsTrigger>
          <TabsTrigger value="prompt">{copy.prompt}</TabsTrigger>
          {log.answer && <TabsTrigger value="answer">{copy.answer}</TabsTrigger>}
          <TabsTrigger value="command">{copy.command}</TabsTrigger>
        </TabsList>
        <LogText value="output" text={log.output} />
        <LogText value="errors" text={log.errors} />
        <LogText value="prompt" text={log.prompt} />
        {log.answer && <LogText value="answer" text={log.answer} />}
        <LogText value="command" text={log.command} />
      </Tabs>
    </section>
  );
}

function LogText({ value, text }: { value: string; text: string }) {
  const { messages: { agentLogs: copy } } = useLocale();
  const [copied, setCopied] = useState(false);
  return (
    <TabsContent value={value} className="relative min-h-0">
      {text ? (
        <>
          <Button type="button" variant="ghost" size="icon-sm" className="absolute end-2 top-2" aria-label={copied ? copy.copied : copy.copy}
            onClick={() => void navigator.clipboard.writeText(text).then(() => { setCopied(true); setTimeout(() => setCopied(false), 1500); })}>
            <Copy aria-hidden="true" />
          </Button>
          <pre className="max-h-[60dvh] overflow-auto rounded-md bg-muted p-3 pe-10 font-mono text-xs leading-relaxed whitespace-pre-wrap wrap-anywhere">{text}</pre>
        </>
      ) : <p className="text-sm text-muted-foreground">{copy.nothing}</p>}
    </TabsContent>
  );
}
