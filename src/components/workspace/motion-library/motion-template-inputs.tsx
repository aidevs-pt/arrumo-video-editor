"use client";

import { invoke } from "@tauri-apps/api/core";
import { Check, TextCursorInput, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useLocale } from "@/lib/locale-provider";

type Input = {
  key: string;
  label: string;
  kind: "text" | "number";
  default: string;
  maxLength: number;
  tips: string;
  dos: string[];
  donts: string[];
};

/** What a template asks each video to fill in, with the guidance videos follow. */
export function MotionTemplateInputs({ workspaceId, itemId, revision }: { workspaceId: number; itemId: number; revision: unknown }) {
  const { messages: { motionLibrary: copy } } = useLocale();
  const [inputs, setInputs] = useState<Input[]>([]);

  useEffect(() => {
    let active = true;
    invoke<Input[]>("motion_template_inputs", { workspaceId, itemId })
      .then((entries) => { if (active) setInputs(entries); }, () => { if (active) setInputs([]); });
    return () => { active = false; };
  }, [workspaceId, itemId, revision]);

  if (!inputs.length) return null;
  return (
    <section aria-labelledby="motion-template-inputs" className="flex flex-col gap-2">
      <h3 id="motion-template-inputs" className="font-bold">{copy.templateInputs}</h3>
      <p className="text-sm text-muted-foreground">{copy.templateHint}</p>
      <ul className="flex flex-col divide-y rounded-lg border">
        {inputs.map((input) => (
          <li key={input.key} className="flex flex-col gap-1.5 px-3 py-2.5 text-sm">
            <p className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
              <TextCursorInput className="size-4 shrink-0 self-center text-primary" aria-hidden="true" />
              <span className="font-semibold">{input.label}</span>
              <span className="text-xs text-muted-foreground">
                {input.kind === "number" ? copy.templateNumber : copy.templateLimit.replace("{count}", String(input.maxLength))}</span>
            </p>
            {input.default && <p className="text-muted-foreground wrap-anywhere">{copy.templateExample.replace("{value}", input.default)}</p>}
            {input.tips && <p className="wrap-anywhere">{input.tips}</p>}
            {(input.dos.length > 0 || input.donts.length > 0) && <ul className="flex flex-col gap-1">
              {input.dos.map((line) => <li key={`do-${line}`} className="flex gap-1.5">
                <Check className="mt-0.5 size-3.5 shrink-0 text-approval" aria-label={copy.templateDo} />{line}</li>)}
              {input.donts.map((line) => <li key={`dont-${line}`} className="flex gap-1.5">
                <X className="mt-0.5 size-3.5 shrink-0 text-destructive" aria-label={copy.templateDont} />{line}</li>)}
            </ul>}
          </li>
        ))}
      </ul>
    </section>
  );
}
