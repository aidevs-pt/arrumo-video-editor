"use client";

import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

/** Colors that stay distinct from each other and from the cobalt rail. */
export const SWATCHES = ["#E86AA8", "#4BA3F2", "#3DBB6B", "#E55D4D", "#C68820", "#138778", "#6974D6", "#AB5980"];

/** A preset color chosen at random, for things that should be told apart from the start. */
export function randomSwatch() {
  return SWATCHES[Math.floor(Math.random() * SWATCHES.length)];
}

/** Preset swatches, a color well for any other color, and an optional way to clear it.
 * `value` is `#RRGGBB` or null; `onChange` receives the same. */
export function ColorPicker({ value, onChange, label, customLabel, noneLabel, disabled }: {
  value: string | null;
  onChange: (color: string | null) => void;
  label: string;
  customLabel: string;
  /** Omit to require a color. */
  noneLabel?: string;
  disabled?: boolean;
}) {
  const current = value?.toUpperCase() ?? null;
  const [custom, setCustom] = useState(current ?? "#2337B4");
  useEffect(() => { if (current) setCustom(current); }, [current]);
  const isCustom = current !== null && !SWATCHES.includes(current);

  return <div className="flex flex-col gap-3">
    <div role="radiogroup" aria-label={label} className="grid grid-cols-8 gap-1.5">
      {SWATCHES.map((swatch) => {
        const selected = current === swatch;
        return <button key={swatch} type="button" role="radio" aria-checked={selected} aria-label={swatch} disabled={disabled}
          onClick={() => onChange(swatch)}
          className={cn("aspect-square min-w-7 rounded-[4px] outline-offset-2 transition-transform hover:scale-105 focus-visible:outline-2 focus-visible:outline-attention disabled:opacity-50 motion-reduce:transition-none",
            selected && "outline-2 outline-foreground")} style={{ backgroundColor: swatch }} />;
      })}
    </div>
    <div className="flex items-center gap-2">
      <label className={cn("flex flex-1 items-center gap-2 text-sm", disabled && "opacity-50")}>
        <input type="color" value={custom} disabled={disabled}
          className={cn("size-8 cursor-pointer rounded-[4px] border bg-transparent p-0.5", isCustom && "outline-2 outline-offset-2 outline-foreground")}
          onChange={(event) => setCustom(event.target.value.toUpperCase())}
          onBlur={() => { if (custom !== current) onChange(custom); }} />
        {customLabel}
        {isCustom && <span className="font-mono text-xs text-muted-foreground">{current}</span>}
      </label>
      {noneLabel && <Button type="button" size="sm" variant="ghost" onClick={() => onChange(null)} disabled={disabled || current === null}>{noneLabel}</Button>}
    </div>
  </div>;
}
