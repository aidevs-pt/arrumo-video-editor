"use client";

import { Check, MessageSquare, PencilLine, X } from "lucide-react";
import { useId, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

export type BulkDecision = "approved" | "revise" | "rejected";

const bulk = [
  { value: "approved", icon: Check },
  { value: "revise", icon: PencilLine },
  { value: "rejected", icon: X },
] as const;

/** Which items a bulk decision applies to: the selection, else the undecided ones, else all of them. */
export function bulkTargets<T>(items: T[], decided: (item: T) => boolean, selected?: (item: T) => boolean) {
  const chosen = selected ? items.filter(selected) : [];
  if (chosen.length) return chosen;
  const rest = items.filter((item) => !decided(item));
  return rest.length ? rest : items;
}

/**
 * The review bar that stays at the bottom of the screen while the creator goes through the items:
 * bulk decisions, the overall notes and Send. Bulk buttons say what they act on, so choices
 * made one by one are never overwritten by "all".
 */
export function ReviewActionBar({ total, decided, selected = 0, reviewing, onDecide, notes, onNotes, notesLabel, status, actions, onClearSelection, className }: {
  total: number;
  decided: number;
  selected?: number;
  reviewing: boolean;
  onDecide?: (decision: BulkDecision) => void;
  notes: string;
  onNotes: (notes: string) => void;
  notesLabel: string;
  status: ReactNode;
  actions: ReactNode;
  onClearSelection?: () => void;
  className?: string;
}) {
  const { messages: { productionPlan: copy } } = useLocale();
  const notesId = useId();
  const [notesOpen, setNotesOpen] = useState(false);
  const scope = selected > 0 ? "selected" : decided > 0 && decided < total ? "rest" : "batch";
  return (
    <div className={cn("sticky bottom-4 z-[1] flex flex-col gap-2 rounded-lg bg-ink px-3 py-2 text-white shadow-lg", className)}>
      {notesOpen && <div className="flex flex-col gap-1.5 pt-1">
        <label htmlFor={notesId} className="text-sm font-semibold">{notesLabel}</label>
        <Textarea id={notesId} autoFocus value={notes} maxLength={4096} disabled={reviewing}
          onChange={(event) => onNotes(event.target.value)}
          className="min-h-20 border-white/30 bg-white/10 text-white placeholder:text-white/60" />
      </div>}
      <div className="flex flex-wrap items-center gap-2">
        <div className="me-auto min-w-0">{status}</div>
        {onDecide && total > 0 && <div role="group" aria-label={selected > 0 ? copy.selectedOutputCount.replace("{count}", String(selected)) : copy.batchDecisions}
          className="flex flex-wrap items-center gap-1.5">
          {selected > 0 && <span className="px-1 text-sm">{copy.selectedOutputCount.replace("{count}", String(selected))}</span>}
          {bulk.map(({ value, icon: Icon }) => (
            <Button key={value} type="button" size="sm" variant="outline" disabled={reviewing} onClick={() => onDecide(value)}
              className="border-white/40 bg-transparent text-white hover:bg-white/10 hover:text-white">
              <Icon data-icon="inline-start" />{copy[`${scope}_${value}`]}
            </Button>
          ))}
          {selected > 0 && onClearSelection && <Button type="button" size="sm" variant="ghost" disabled={reviewing} onClick={onClearSelection}
            className="text-white/80 hover:bg-white/10 hover:text-white">{copy.clearOutputSelection}</Button>}
        </div>}
        <Button type="button" size="sm" variant="ghost" aria-expanded={notesOpen} aria-controls={notesId} disabled={reviewing}
          onClick={() => setNotesOpen(!notesOpen)}
          className={cn("text-white/85 hover:bg-white/10 hover:text-white", notes.trim() && "text-attention")}>
          <MessageSquare data-icon="inline-start" />{notesOpen ? copy.hideNotes : notes.trim() ? copy.editNotes : copy.addNotes}
        </Button>
        {actions}
      </div>
    </div>
  );
}
