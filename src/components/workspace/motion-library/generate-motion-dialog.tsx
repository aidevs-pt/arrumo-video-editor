"use client";

import { invoke } from "@tauri-apps/api/core";
import { Blocks, Sparkles, WandSparkles } from "lucide-react";
import { useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { AgentLogs } from "@/components/workspace/generation/agent-logs";
import { useLocale } from "@/lib/locale-provider";

type Failure = "agent_unavailable" | "agent_timed_out" | "agent_failed" | "motion_generation_failed";

/** Asks the workspace's agent for a new animation, transition or canvas component, or a changed
 * copy of `basedOn`. */
export function GenerateMotionDialog<Item>({ workspaceId, profileId, basedOn, components = false, disabled, onCreate, className }: {
  className?: string;
  workspaceId: number;
  profileId: number | null;
  basedOn?: { id: number; title: string };
  /** Whether a new item may be a canvas component, which needs the canvas renderer. */
  components?: boolean;
  disabled?: boolean;
  onCreate: (item: Item) => void;
}) {
  const { messages: { motionLibrary: copy } } = useLocale();
  const [open, setOpen] = useState(false);
  const [working, setWorking] = useState(false);
  const [failure, setFailure] = useState<Failure>();
  const [choice, setChoice] = useState<"motion" | "component">("motion");
  const making = components && !basedOn && choice === "component";

  async function generate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const request = String(new FormData(event.currentTarget).get("request") ?? "").trim();
    if (!request || working) return;
    setWorking(true);
    setFailure(undefined);
    try {
      const item = making
        ? await invoke<Item>("generate_canvas_component", { workspaceId, profileId, request })
        : await invoke<Item>("generate_motion_library_item", { workspaceId, profileId, request, basedOn: basedOn?.id ?? null });
      onCreate(item);
      setOpen(false);
    } catch (error) {
      setFailure(typeof error === "string" && error in copy.generateErrors ? error as Failure : "motion_generation_failed");
    } finally {
      setWorking(false);
    }
  }

  const Icon = basedOn ? WandSparkles : making ? Blocks : Sparkles;
  const action = basedOn ? copy.changeWithAgent : copy.generate;
  return (
    <Dialog open={open} onOpenChange={(next) => { if (!working) { setFailure(undefined); setOpen(next); } }}>
      <DialogTrigger asChild>
        <Button type="button" variant={basedOn ? "outline" : "default"} disabled={disabled} className={className}>
          {basedOn ? <WandSparkles data-icon="inline-start" /> : <Sparkles data-icon="inline-start" />}{action}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-h-[85dvh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{action}</DialogTitle>
          <DialogDescription>
            {basedOn ? copy.changeDescription.replace("{title}", basedOn.title) : making ? copy.componentDescription : copy.generateDescription}
          </DialogDescription>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={generate}>
          {components && !basedOn && <Field>
            <FieldLabel id="motion-make-label">{copy.whatToMake}</FieldLabel>
            <ToggleGroup type="single" variant="outline" value={choice} aria-labelledby="motion-make-label" disabled={working}
              className="grid w-full grid-cols-1 gap-2 sm:grid-cols-2"
              onValueChange={(value) => { if (value) setChoice(value as typeof choice); }}>
              {(["motion", "component"] as const).map((value) => {
                const ItemIcon = value === "motion" ? Sparkles : Blocks;
                return <ToggleGroupItem key={value} value={value}
                  className="h-auto flex-col items-start gap-1 rounded-lg! px-3 py-2.5 text-start whitespace-normal">
                  <span className="flex items-center gap-2 font-semibold"><ItemIcon aria-hidden="true" />{value === "motion" ? copy.makeMotion : copy.makeComponent}</span>
                  <span className="text-sm font-normal text-muted-foreground">{value === "motion" ? copy.makeMotionHint : copy.makeComponentHint}</span>
                </ToggleGroupItem>;
              })}
            </ToggleGroup>
          </Field>}
          <Field>
            <FieldLabel htmlFor="motion-request">{basedOn ? copy.changeRequest : making ? copy.componentRequest : copy.request}</FieldLabel>
            <Textarea id="motion-request" name="request" required maxLength={4000} rows={5} disabled={working}
              placeholder={basedOn ? copy.changePlaceholder : making ? copy.componentPlaceholder : copy.requestPlaceholder} />
            <FieldDescription>{copy.requestHint}</FieldDescription>
          </Field>
          {working && <p role="status" className="text-sm text-muted-foreground">{making ? copy.componentGenerating : copy.generating}</p>}
          {failure && <Alert variant="destructive"><AlertDescription>{copy.generateErrors[failure]}</AlertDescription></Alert>}
          <DialogFooter className="flex-wrap gap-2 sm:justify-between">
            <AgentLogs workspaceId={workspaceId} size="default" />
            <span className="flex gap-2">
              <Button type="button" variant="outline" disabled={working} onClick={() => setOpen(false)}>{copy.cancel}</Button>
              <Button type="submit" disabled={working}><Icon data-icon="inline-start" />{working ? copy.generatingShort : basedOn ? copy.changeSubmit : making ? copy.componentSubmit : copy.generateSubmit}</Button>
            </span>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
