"use client";

import dynamic from "next/dynamic";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";

const RichEditor = dynamic(() => import("./rich-editor").then((module) => module.RichEditor), {
  ssr: false,
});

export type MarkdownEditorProps = {
  id: string;
  label: string;
  value: string;
  disabled?: boolean;
  onChange: (value: string) => void;
};

export function MarkdownEditor(props: MarkdownEditorProps) {
  const [showSource, setShowSource] = useState(false);
  const { messages: { markdownEditor: copy } } = useLocale();

  return (
    <div className="relative min-w-0">
      <Button type="button" size="xs" variant="ghost" aria-pressed={showSource}
        className="absolute end-2 top-2 z-[1] text-muted-foreground aria-pressed:bg-secondary aria-pressed:text-foreground"
        disabled={props.disabled} onClick={() => setShowSource((current) => !current)}>
        {copy.source}
      </Button>
      {showSource ? (
        <Textarea id={props.id} aria-label={props.label} className="min-h-56 pt-11" value={props.value}
          disabled={props.disabled} onChange={(event) => props.onChange(event.target.value)} />
      ) : <RichEditor {...props} />}
    </div>
  );
}
