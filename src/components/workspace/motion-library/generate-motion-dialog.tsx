"use client";

import { invoke } from "@tauri-apps/api/core";
import { Sparkles, WandSparkles } from "lucide-react";
import { useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";
import { AgentLogs } from "@/components/workspace/generation/agent-logs";
import { useLocale } from "@/lib/locale-provider";

type Failure = "agent_unavailable" | "agent_timed_out" | "agent_failed" | "motion_generation_failed";

/** Asks the workspace's agent for a new animation, or a changed copy of `basedOn`. */
export function GenerateMotionDialog<Item>({ workspaceId, profileId, basedOn, disabled, onCreate, className }: {
  className?: string;
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
        <Button type="button" variant={basedOn ? "outline" : "default"} disabled={disabled} className={className}>
          {basedOn ? <WandSparkles data-icon="inline-start" /> : <Sparkles data-icon="inline-start" />}
          {basedOn ? copy.changeWithAgent : copy.generate}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-h-[85dvh] overflow-y-auto sm:max-w-xl">
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
            <AgentLogs workspaceId={workspaceId} size="default" />
            <span className="flex gap-2">
              <Button type="button" variant="outline" disabled={working} onClick={() => setOpen(false)}>{copy.cancel}</Button>
              <Button type="submit" disabled={working}>{basedOn ? <WandSparkles data-icon="inline-start" /> : <Sparkles data-icon="inline-start" />}{working ? copy.generatingShort : basedOn ? copy.changeSubmit : copy.generateSubmit}</Button>
            </span>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
