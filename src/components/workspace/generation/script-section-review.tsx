"use client";

import { Check, MessageSquare, PencilLine, Trash2, X } from "lucide-react";
import { useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";
import { bulkTargets, DecisionToggles, ReviewActionBar } from "./review-action-bar";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

export type ScriptSection = { heading: string; timing: string; narration: string; idea: string };
type Decision = "approved" | "revise" | "rejected";
type Staged = { decisions: Record<number, Decision>; notes: Record<number, string>; overall: string };

const options = [
  { value: "approved", icon: Check, fill: "bg-approval text-white hover:bg-approval hover:text-white", marker: "bg-approval" },
  { value: "revise", icon: PencilLine, fill: "bg-attention text-ink hover:bg-attention hover:text-ink", marker: "bg-attention" },
  { value: "rejected", icon: X, fill: "bg-destructive text-white hover:bg-destructive hover:text-white", marker: "bg-destructive" },
] as const;

export function isScriptSections(value: unknown): value is ScriptSection[] {
  return Array.isArray(value) && value.length > 0 && value.every((section) =>
    section && typeof section === "object" && typeof section.heading === "string" && typeof section.narration === "string");
}

/** Turns staged section decisions into one stage review the agent can act on. */
export function compileScriptReview(sections: ScriptSection[], staged: Staged, labels: Record<Decision, string>, heading: string) {
  const decided = Object.entries(staged.decisions).map(([index, decision]) => ({ index: Number(index), decision }));
  const needsWork = decided.some(({ decision }) => decision !== "approved");
  const lines = sections.flatMap((section, index) => {
    const decision = staged.decisions[index];
    const note = staged.notes[index]?.trim();
    if (!decision && !note) return [];
    return [`- ${section.heading}: ${labels[decision ?? "revise"]}${note ? `. ${note}` : ""}`];
  });
  const notes = [lines.length ? `${heading}\n${lines.join("\n")}` : "", staged.overall.trim()].filter(Boolean).join("\n\n");
  return { decision: needsWork || (!decided.length && notes) ? "revise" as const : "approved" as const, notes };
}

export function ScriptSectionReview({ storageKey, sections, agent, reviewing, reviewError, onSend }: {
  storageKey: string;
  sections: ScriptSection[];
  agent?: string;
  reviewing: boolean;
  reviewError: boolean;
  onSend: (decision: Decision, notes: string) => Promise<boolean>;
}) {
  const { messages } = useLocale();
  const copy = messages.scriptReview;
  const plan = messages.productionPlan;
  const agentName = agent ?? plan.agentFallback;
  const [staged, setStaged] = useState<Staged>(() => {
    try {
      const saved = JSON.parse(sessionStorage.getItem(storageKey) ?? "null");
      if (saved?.decisions && saved?.notes) return { overall: "", ...saved };
    } catch {}
    return { decisions: {}, notes: {}, overall: "" };
  });
  const [selected, setSelected] = useState<number[]>([]);
  const [shared, setShared] = useState("");
  const [openNotes, setOpenNotes] = useState<number[]>([]);
  const count = Object.keys(staged.decisions).length;
  const labels = { approved: copy.approved, revise: copy.revise, rejected: copy.rejected };

  function update(next: Staged) {
    setStaged(next);
    try { sessionStorage.setItem(storageKey, JSON.stringify(next)); } catch {}
  }

  function decide(indexes: number[], decision: Decision, note?: string, bulk = false) {
    const decisions = { ...staged.decisions };
    const notes = { ...staged.notes };
    for (const index of indexes) {
      if (!bulk && indexes.length === 1 && decisions[index] === decision && !note) delete decisions[index];
      else decisions[index] = decision;
      if (note?.trim()) notes[index] = note.trim();
    }
    update({ ...staged, decisions, notes });
  }

  async function send(decision?: Decision) {
    const review = decision === "rejected"
      ? { decision, notes: staged.overall.trim() }
      : compileScriptReview(sections, staged, labels, copy.notesHeading);
    if (await onSend(review.decision, review.notes)) {
      try { sessionStorage.removeItem(storageKey); } catch {}
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2 rounded-lg bg-secondary px-3 py-2" role="group" aria-label={copy.selected.replace("{count}", String(selected.length))}>
        <label className="flex items-center gap-2 text-sm font-semibold">
          <Checkbox checked={selected.length === sections.length ? true : selected.length ? "indeterminate" : false}
            onCheckedChange={(checked) => setSelected(checked === true ? sections.map((_, index) => index) : [])} />
          {selected.length === 1 ? copy.selectedOne : selected.length ? copy.selected.replace("{count}", String(selected.length)) : copy.selectAll}
        </label>
        {selected.length > 0 && <>
          <Input value={shared} onChange={(event) => setShared(event.target.value)} placeholder={copy.sharedComment}
            aria-label={copy.sharedComment} className="h-8 min-w-48 flex-1 bg-card" />
        </>}
      </div>
      <ol className="flex flex-col gap-2">
        {sections.map((section, index) => {
          const decision = staged.decisions[index];
          const note = staged.notes[index] ?? "";
          const chosen = selected.includes(index);
          const marker = options.find((option) => option.value === decision)?.marker;
          return (
            <li key={index} className={cn("grid gap-x-4 gap-y-2 rounded-lg border-2 border-transparent p-3 sm:grid-cols-[auto_8rem_minmax(0,1fr)]",
              chosen && "border-primary")}>
              <Checkbox className="mt-1" checked={chosen} aria-label={copy.selectSection.replace("{heading}", section.heading)}
                onCheckedChange={(checked) => setSelected((current) => checked === true ? [...current, index] : current.filter((value) => value !== index))} />
              <div>
                <p className="font-bold">{section.heading}</p>
                {section.timing && <p className="text-sm text-muted-foreground tabular-nums">{section.timing}</p>}
              </div>
              <div className="flex min-w-0 flex-col gap-2 sm:col-start-3">
                <p className="leading-relaxed wrap-anywhere">{section.narration}</p>
                {section.idea && <p className="text-sm text-muted-foreground"><span className="font-semibold text-primary">
                  {agent ? copy.idea.replace("{agent}", agent) : copy.ideaFallback}</span> {section.idea}</p>}
                {note && !openNotes.includes(index) && (
                  <p className="flex items-start gap-2 rounded-md bg-accent px-3 py-2 text-sm">
                    <MessageSquare className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
                    <span className="min-w-0 flex-1 wrap-anywhere">{note}</span>
                    <span className="shrink-0 text-muted-foreground">{copy.notSent}</span>
                  </p>
                )}
                {openNotes.includes(index) && (
                  <Textarea autoFocus aria-label={copy.commentFor.replace("{heading}", section.heading)} value={note} maxLength={2000}
                    onChange={(event) => update({ ...staged, notes: { ...staged.notes, [index]: event.target.value } })}
                    onBlur={() => setOpenNotes((current) => current.filter((value) => value !== index))} />
                )}
                <div className="flex flex-wrap items-center gap-2">
                  <span className="me-auto flex items-center gap-1.5 text-sm font-semibold">
                    {marker && <span aria-hidden="true" className={cn("size-2.5 rounded-[2px]", marker)} />}
                    {decision ? labels[decision] : <span className="rounded-md bg-secondary px-2 py-0.5 font-normal text-muted-foreground">{copy.noDecision}</span>}
                  </span>
                  <TooltipProvider delayDuration={250}>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button type="button" size="icon-sm" variant="ghost" disabled={reviewing} aria-label={copy.commentFor.replace("{heading}", section.heading)}
                          onClick={() => setOpenNotes((current) => [...current, index])}><MessageSquare /></Button>
                      </TooltipTrigger>
                      <TooltipContent>{copy.comment}</TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                  <DecisionToggles value={decision} label={section.heading} disabled={reviewing} onChange={(value) => decide([index], value)} />
                </div>
              </div>
            </li>
          );
        })}
      </ol>
      {reviewError && <Alert variant="destructive"><AlertDescription>{plan.reviewError}</AlertDescription></Alert>}
      <ReviewActionBar total={sections.length} decided={count} selected={selected.length} reviewing={reviewing}
        onDecide={(decision) => {
          const indexes = sections.map((_, index) => index);
          decide(bulkTargets(indexes, (index) => Boolean(staged.decisions[index]), (index) => selected.includes(index)), decision, shared, true);
          setSelected([]);
          setShared("");
        }}
        onClearSelection={() => setSelected([])}
        notes={staged.overall} onNotes={(overall) => update({ ...staged, overall })} notesLabel={copy.overall}
        status={<p className="text-sm" role="status" title={count ? copy.stagedHint.replace("{agent}", agentName) : copy.nothingStaged}>
          {count ? <span className="font-bold text-attention">{count === 1 ? copy.stagedOne : copy.staged.replace("{count}", String(count))}</span> : copy.noDecisionsYet}
        </p>}
        menuItems={[{ label: copy.rejectAll, icon: Trash2, destructive: true, onSelect: () => void send("rejected") }]}
        actions={<>
          <Button type="button" disabled={reviewing || (!count && !staged.overall.trim())} onClick={() => void send()}
            className="bg-attention text-ink hover:bg-[oklch(90%_.13_86)] disabled:bg-white/15 disabled:text-white/70 disabled:opacity-100">
            {count === 0 ? plan.sendNothing : count === 1 ? copy.sendOne : copy.send.replace("{count}", String(count))}
          </Button>
        </>} />
    </div>
  );
}
