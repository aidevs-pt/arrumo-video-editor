"use client";

import { Markdown } from "@tiptap/markdown";
import { EditorContent, useEditor } from "@tiptap/react";
import { StarterKit } from "@tiptap/starter-kit";

const extensions = [
  StarterKit.configure({ link: false, underline: false }),
  Markdown,
];

export default function ScriptReviewText({ markdown }: { markdown: string }) {
  const editor = useEditor({
    extensions,
    content: markdown,
    contentType: "markdown",
    editable: false,
    immediatelyRender: false,
  });

  if (!editor) return <p className="max-w-[72ch] wrap-anywhere whitespace-pre-wrap">{markdown}</p>;
  return (
    <EditorContent editor={editor}
      className="max-w-[72ch] wrap-anywhere leading-relaxed [&_.tiptap]:flex [&_.tiptap]:flex-col [&_.tiptap]:gap-3 [&_blockquote]:border-s-2 [&_blockquote]:ps-3 [&_h1]:text-xl [&_h2]:text-lg [&_h3]:font-bold [&_ol]:list-decimal [&_ol]:ps-5 [&_ul]:list-disc [&_ul]:ps-5" />
  );
}
