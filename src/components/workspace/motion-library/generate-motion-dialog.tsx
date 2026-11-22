"use client";

import { invoke } from "@tauri-apps/api/core";
import { Sparkles, WandSparkles } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";
import { Input } from "@/components/ui/input";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";
import { AgentLogs } from "@/components/workspace/generation/agent-logs";
import { useLocale } from "@/lib/locale-provider";

type Failure = "agent_unavailable" | "agent_timed_out" | "agent_failed" | "motion_generation_failed";

/** Asks the workspace's agent for a new animation, or a changed copy of `basedOn`. */
export function GenerateMotionDialog<Item>({ workspaceId, profileId, basedOn, disabled, onCreate }: {
  workspaceId: number;
  profileId: number | null;
  basedOn?: { id: number; title: string };
  disabled?: boolean;
  onCreate: (item: Item) => void;
}) {
  const { messages: { motionLibrary: copy } } = useLocale();
  const [open, setOpen] = useState(false);
  const [working, setWorking] = useState(false);
  const [failure, setFailure] = useState<Failure>();

  async function generate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const request = String(new FormData(event.currentTarget).get("request") ?? "").trim();
    if (!request || working) return;
    setWorking(true);
    setFailure(undefined);
    try {
      const item = await invoke<Item>("generate_motion_library_item", {
        workspaceId, profileId, request, basedOn: basedOn?.id ?? null,
      });
      onCreate(item);
      setOpen(false);
    } catch (error) {
      setFailure(typeof error === "string" && error in copy.generateErrors ? error as Failure : "motion_generation_failed");
    } finally {
      setWorking(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => { if (!working) { setFailure(undefined); setOpen(next); } }}>
      <DialogTrigger asChild>
        <Button type="button" variant={basedOn ? "outline" : "default"} disabled={disabled}>
          {basedOn ? <WandSparkles data-icon="inline-start" /> : <Sparkles data-icon="inline-start" />}
          {basedOn ? copy.changeWithAgent : copy.generate}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-h-[85dvh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{basedOn ? copy.changeWithAgent : copy.generate}</DialogTitle>
          <DialogDescription>
            {basedOn ? copy.changeDescription.replace("{title}", basedOn.title) : copy.generateDescription}
          </DialogDescription>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={generate}>
          <Field>
            <FieldLabel htmlFor="motion-request">{basedOn ? copy.changeRequest : copy.request}</FieldLabel>
            <Textarea id="motion-request" name="request" required maxLength={4000} rows={5} disabled={working}
              placeholder={basedOn ? copy.changePlaceholder : copy.requestPlaceholder} />
            <FieldDescription>{copy.requestHint}</FieldDescription>
          </Field>
          {working && <p role="status" className="text-sm text-muted-foreground">{copy.generating}</p>}
          {failure && <Alert variant="destructive"><AlertDescription>{copy.generateErrors[failure]}</AlertDescription></Alert>}
          <DialogFooter className="flex-wrap gap-2 sm:justify-between">
            <AgentLogs workspaceId={workspaceId} />
            <span className="flex gap-2">
              <Button type="button" variant="outline" disabled={working} onClick={() => setOpen(false)}>{copy.cancel}</Button>
              <Button type="submit" disabled={working}>{working ? copy.generatingShort : basedOn ? copy.changeSubmit : copy.generateSubmit}</Button>
            </span>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** The library's describe-and-make bar: one line to the workspace's agent, which adds a new animation. */
export function MotionPromptBar<Item>({ workspaceId, profileId, disabled, onCreate }: {
  workspaceId: number;
  profileId: number | null;
  disabled?: boolean;
  onCreate: (item: Item) => void;
}) {
  const { messages: { motionLibrary: copy } } = useLocale();
  const [request, setRequest] = useState("");
  const [working, setWorking] = useState(false);
  const [failure, setFailure] = useState<Failure>();
  const [agent, setAgent] = useState<string>();

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<{ effective_provider: string | null }>("get_agent_settings", { workspaceId }),
      invoke<{ id: string; label: string }[]>("list_agent_plugins"),
    ]).then(([settings, plugins]) => {
      if (active) setAgent(plugins.find(({ id }) => id === settings.effective_provider)?.label);
    }, () => {});
    return () => { active = false; };
  }, [workspaceId]);

  async function generate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!request.trim() || working) return;
    setWorking(true);
    setFailure(undefined);
    try {
      onCreate(await invoke<Item>("generate_motion_library_item", { workspaceId, profileId, request: request.trim(), basedOn: null }));
      setRequest("");
    } catch (error) {
      setFailure(typeof error === "string" && error in copy.generateErrors ? error as Failure : "motion_generation_failed");
    } finally {
      setWorking(false);
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <form onSubmit={generate} aria-busy={working}
        className="flex flex-wrap items-center gap-3 rounded-xl border-2 border-primary/25 bg-card p-2 ps-4 focus-within:border-primary/60">
        <Sparkles className="size-4 shrink-0 text-primary" aria-hidden="true" />
        <Input value={request} onChange={(event) => setRequest(event.target.value)} maxLength={4000} disabled={disabled || working}
          aria-label={copy.request} placeholder={copy.barPlaceholder}
          className="h-10 min-w-48 flex-1 border-0 bg-transparent px-0 shadow-none focus-visible:ring-0 dark:bg-transparent" />
        <Button type="submit" disabled={disabled || working || !request.trim()} className="max-sm:w-full">
          {working ? copy.generatingShort : agent ? copy.makeWith.replace("{agent}", agent) : copy.generateSubmit}
        </Button>
      </form>
      {working && <p role="status" className="text-sm text-muted-foreground">{copy.generating}</p>}
      {failure && <Alert variant="destructive"><AlertDescription>{copy.generateErrors[failure]}</AlertDescription></Alert>}
    </div>
  );
}
