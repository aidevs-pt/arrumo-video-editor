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
    <div className="flex min-w-0 flex-col gap-2">
      <Button type="button" size="sm" variant="outline" className="self-start" aria-pressed={showSource}
        disabled={props.disabled} onClick={() => setShowSource((current) => !current)}>
        {copy.source}
      </Button>
      {showSource ? (
        <Textarea id={props.id} aria-label={props.label} className="min-h-56 font-mono" value={props.value}
          disabled={props.disabled} onChange={(event) => props.onChange(event.target.value)} />
      ) : <RichEditor {...props} />}
    </div>
  );
}
