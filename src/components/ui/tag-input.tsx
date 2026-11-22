"use client";

import { X } from "lucide-react";
import { useId, useState, type KeyboardEvent } from "react";
import { cn } from "@/lib/utils";

/**
 * A list of short values entered one at a time: Enter or a comma adds one, Backspace on an empty
 * field removes the last, and each chip has its own remove button. With `name`, the values are
 * also submitted with the surrounding form, joined by commas.
 */
export function TagInput({ id, name, value, onChange, placeholder, suggestions = [], disabled, removeLabel, className, maxLength = 60 }: {
  id?: string;
  name?: string;
  value: string[];
  onChange: (value: string[]) => void;
  placeholder?: string;
  suggestions?: string[];
  disabled?: boolean;
  /** Accessible label for a chip's remove button; `{tag}` is replaced with the value. */
  removeLabel: string;
  className?: string;
  maxLength?: number;
}) {
  const [draft, setDraft] = useState("");
  const listId = useId();
  const has = (tag: string) => value.some((existing) => existing.toLowerCase() === tag.toLowerCase());

  function add(text: string) {
    const tags = text.split(",").map((tag) => tag.trim()).filter((tag) => tag && !has(tag));
    if (tags.length) onChange([...value, ...tags.filter((tag, index) => tags.indexOf(tag) === index)]);
    setDraft("");
  }

  function keyDown(event: KeyboardEvent<HTMLInputElement>) {
    if ((event.key === "Enter" || event.key === ",") && draft.trim()) {
      event.preventDefault();
      add(draft);
    } else if (event.key === "Backspace" && !draft && value.length) {
      onChange(value.slice(0, -1));
    }
  }

  const open = suggestions.filter((tag) => !has(tag));
  return (
    <div className={cn(
      "flex min-h-9 w-full flex-wrap items-center gap-1.5 rounded-md border border-input bg-transparent px-2 py-1.5 text-sm shadow-xs transition-[color,box-shadow]",
      "focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/50",
      disabled && "pointer-events-none opacity-50", className)}>
      {value.map((tag) => (
        <span key={tag} className="inline-flex max-w-full items-center gap-1 rounded-md bg-secondary py-0.5 ps-2 pe-1 text-sm font-medium text-secondary-foreground">
          <span className="truncate">{tag}</span>
          <button type="button" disabled={disabled} aria-label={removeLabel.replace("{tag}", tag)}
            className="grid size-4 place-items-center rounded-sm text-muted-foreground hover:bg-background hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring"
            onClick={() => onChange(value.filter((existing) => existing !== tag))}>
            <X className="size-3" aria-hidden="true" />
          </button>
        </span>
      ))}
      <input id={id} value={draft} disabled={disabled} maxLength={maxLength} list={open.length ? listId : undefined}
        placeholder={value.length ? undefined : placeholder}
        className="min-w-24 flex-1 bg-transparent py-0.5 outline-none placeholder:text-muted-foreground/60"
        onChange={(event) => {
          const text = event.target.value;
          if (text.includes(",")) add(text);
          else setDraft(text);
        }}
        onKeyDown={keyDown}
        onBlur={() => { if (draft.trim()) add(draft); }} />
      {open.length > 0 && <datalist id={listId}>{open.map((tag) => <option key={tag} value={tag} />)}</datalist>}
      {name && <input type="hidden" name={name} value={value.join(", ")} />}
    </div>
  );
}
