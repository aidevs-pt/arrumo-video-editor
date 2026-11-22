"use client";

import dynamic from "next/dynamic";

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

/** Rich text editing for creators; the Markdown it produces is what agents read. */
export function MarkdownEditor(props: MarkdownEditorProps) {
  return <div className="relative min-w-0"><RichEditor {...props} /></div>;
}
