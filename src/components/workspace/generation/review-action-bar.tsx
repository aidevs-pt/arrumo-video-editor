"use client";

import { Check, ChevronUp, MessageSquare, PencilLine, X, type LucideIcon } from "lucide-react";
import { useId, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

export type BulkDecision = "approved" | "revise" | "rejected";

export const decisionStyles = [
  { value: "approved", icon: Check, fill: "bg-approval text-white hover:bg-approval hover:text-white", marker: "bg-approval" },
  { value: "revise", icon: PencilLine, fill: "bg-attention text-ink hover:bg-attention hover:text-ink", marker: "bg-attention" },
  { value: "rejected", icon: X, fill: "bg-destructive text-white hover:bg-destructive hover:text-white", marker: "bg-destructive" },
] as const;

/** Which items a bulk decision applies to: the selection, else the undecided ones, else all of them. */
export function bulkTargets<T>(items: T[], decided: (item: T) => boolean, selected?: (item: T) => boolean) {
  const chosen = selected ? items.filter(selected) : [];
  if (chosen.length) return chosen;
  const rest = items.filter((item) => !decided(item));
  return rest.length ? rest : items;
}

/** One item's decision as three icon toggles with tooltips, so each card stays calm. */
export function DecisionToggles({ value, label, disabled, onChange, className }: {
  value?: BulkDecision | "";
  label: string;
  disabled?: boolean;
  onChange: (decision: BulkDecision) => void;
  className?: string;
}) {
  const { messages: { productionPlan: copy } } = useLocale();
  return (
    <TooltipProvider delayDuration={250}>
      <div role="group" aria-label={copy.decisionFor.replace("{label}", label)}
        className={cn("inline-flex items-center gap-0.5 rounded-lg border bg-card p-0.5", className)}>
        {decisionStyles.map(({ value: option, icon: Icon, fill }) => {
          const active = value === option;
          return <Tooltip key={option}>
            <TooltipTrigger asChild>
              <Button type="button" size="icon-sm" variant="ghost" disabled={disabled} aria-pressed={active}
                aria-label={copy[`item_${option}`]} className={cn(active && fill)} onClick={() => onChange(option)}>
                <Icon />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{copy[`item_${option}`]}</TooltipContent>
          </Tooltip>;
        })}
      </div>
    </TooltipProvider>
  );
}

export type BarMenuItem = { label: string; icon?: LucideIcon; destructive?: boolean; onSelect: () => void };

/**
 * The review bar that stays at the bottom of the screen while the creator goes through the items:
 * the status, the overall notes, one menu of bulk decisions and Send. The menu says what it acts on,
 * so choices made one by one are never overwritten by "all".
 */
export function ReviewActionBar({ total, decided, selected = 0, reviewing, onDecide, notes, onNotes, notesLabel, status, actions, onClearSelection, menuItems = [], className }: {
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
  /** More actions for the whole review, shown at the end of the menu. */
  menuItems?: BarMenuItem[];
  className?: string;
}) {
  const { messages: { productionPlan: copy } } = useLocale();
  const notesId = useId();
  const [notesOpen, setNotesOpen] = useState(false);
  const scope = selected > 0 ? "selected" : decided > 0 && decided < total ? "rest" : "batch";
  const bulk = Boolean(onDecide && total > 0);
  const notesLabelNow = notesOpen ? copy.hideNotes : notes.trim() ? copy.editNotes : copy.addNotes;
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
        <TooltipProvider delayDuration={250}>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button type="button" size="icon" variant="ghost" aria-expanded={notesOpen} aria-label={notesLabelNow} disabled={reviewing}
                onClick={() => setNotesOpen(!notesOpen)}
                className={cn("relative text-white/85 hover:bg-white/10 hover:text-white", notesOpen && "bg-white/10")}>
                <MessageSquare />
                {notes.trim() && !notesOpen && <span aria-hidden="true" className="absolute top-1.5 end-1.5 size-2 rounded-full bg-attention" />}
              </Button>
            </TooltipTrigger>
            <TooltipContent>{notesLabelNow}</TooltipContent>
          </Tooltip>
        </TooltipProvider>
        {(bulk || menuItems.length > 0) && <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button type="button" variant="outline" disabled={reviewing}
              className="border-white/40 bg-transparent text-white hover:bg-white/10 hover:text-white data-[state=open]:bg-white/10">
              {bulk ? copy[`${scope}_menu`].replace("{count}", String(selected)) : copy.moreActions}<ChevronUp data-icon="inline-end" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent side="top" align="end" className="min-w-56">
            {bulk && <>
              <DropdownMenuLabel>{copy[`${scope}_menu`].replace("{count}", String(selected))}</DropdownMenuLabel>
              {decisionStyles.map(({ value, icon: Icon }) => (
                <DropdownMenuItem key={value} onSelect={() => onDecide!(value)}><Icon />{copy[`item_${value}`]}</DropdownMenuItem>
              ))}
              {selected > 0 && onClearSelection && <DropdownMenuItem onSelect={onClearSelection}>{copy.clearOutputSelection}</DropdownMenuItem>}
            </>}
            {menuItems.length > 0 && <>
              {bulk && <DropdownMenuSeparator />}
              {menuItems.map(({ label, icon: Icon, destructive, onSelect }) => (
                <DropdownMenuItem key={label} variant={destructive ? "destructive" : "default"} onSelect={onSelect}>{Icon && <Icon />}{label}</DropdownMenuItem>
              ))}
            </>}
          </DropdownMenuContent>
        </DropdownMenu>}
        {actions}
      </div>
    </div>
  );
}
