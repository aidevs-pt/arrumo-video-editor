"use client";

import { Check, Code, Copy } from "lucide-react";
import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from "@/components/ui/input-group";
import { useLocale } from "@/lib/locale-provider";

import { HighlightedCode } from "./code-highlight";
import { MotionClip } from "./motion-clip";

export type CanvasComponent = {
  slug: string;
  source: string;
  options: { name: string; description: string }[];
  libraries: string[];
};

/** What a canvas component is and does: its demo playing on a loop, how pages import it and its
 * options. Changes go through the agent. */
export function ComponentDetails({ workspaceId, profileId, itemId, title, component, revision, thumbnail }: {
  workspaceId: number;
  profileId: number | null;
  itemId: number;
  title: string;
  component: CanvasComponent;
  /** Changes when the item does, so the demo reloads. */
  revision: unknown;
  thumbnail: { kind: "image" | "video"; assetId: number };
}) {
  const { messages: { motionLibrary: copy } } = useLocale();
  const [copied, setCopied] = useState(false);
  const importPath = `workspace/${component.slug}`;

  function copyImport() {
    void navigator.clipboard.writeText(importPath).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }, () => {});
  }

  return (
    <div className="flex flex-col gap-4">
      <MotionClip mode="loop" workspaceId={workspaceId} profileId={profileId} itemId={itemId} title={title}
        revision={revision} thumbnail={thumbnail} />
      <p className="text-sm text-muted-foreground">{copy.componentIntro}</p>
      <div>
        <h3 className="mb-1 font-bold"><label htmlFor="component-import-path">{copy.importPath}</label></h3>
        <InputGroup>
          <InputGroupInput id="component-import-path" readOnly value={importPath} className="font-mono"
            onFocus={(event) => event.currentTarget.select()} />
          <InputGroupAddon align="inline-end">
            <InputGroupButton size="icon-sm" aria-label={copied ? copy.copied : copy.copyImport} title={copied ? copy.copied : copy.copyImport}
              onClick={copyImport}>{copied ? <Check /> : <Copy />}</InputGroupButton>
          </InputGroupAddon>
        </InputGroup>
      </div>
      <div>
        <h3 className="mb-1 font-bold">{copy.options}</h3>
        {component.options.length ? <dl className="flex flex-col gap-1.5 text-sm">
          {component.options.map((option) => <div key={option.name}>
            <dt className="font-mono font-semibold">{option.name}</dt>
            <dd className="text-muted-foreground wrap-anywhere">{option.description}</dd>
          </div>)}
        </dl> : <p className="text-sm text-muted-foreground">{copy.noOptions}</p>}
      </div>
      {component.libraries.length > 0 && <p className="flex flex-wrap items-center gap-1.5 text-sm">
        <span className="font-bold">{copy.usesLibraries}</span>
        {component.libraries.map((library) => <Badge key={library} variant="secondary">{library}</Badge>)}
      </p>}
      <ComponentCode title={title} source={component.source} />
    </div>
  );
}

/** The component's code in a read-only dialog, opened from a small icon button. */
export function ComponentCode({ title, source, disabled }: { title: string; source: string; disabled?: boolean }) {
  const { messages: { motionLibrary: copy } } = useLocale();
  const [copied, setCopied] = useState(false);

  function copyCode() {
    void navigator.clipboard.writeText(source).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }, () => {});
  }

  return (
    <Dialog>
      <DialogTrigger asChild>
        <Button type="button" variant="link" size="sm" disabled={disabled} className="h-auto self-start px-0! text-muted-foreground">
          <Code data-icon="inline-start" />{copy.viewCode}
        </Button>
      </DialogTrigger>
      <DialogContent className="flex max-h-[85dvh] flex-col sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>{copy.codeTitle.replace("{title}", title)}</DialogTitle>
          <DialogDescription>{copy.codeDescription}</DialogDescription>
        </DialogHeader>
        <pre tabIndex={0} aria-label={copy.codeTitle.replace("{title}", title)}
          className="min-h-0 flex-1 overflow-auto rounded-lg bg-ink p-4 font-mono text-sm leading-relaxed text-white">
          <code><HighlightedCode source={source} /></code>
        </pre>
        <div className="flex justify-end">
          <Button type="button" variant="outline" onClick={copyCode}>
            {copied ? <Check data-icon="inline-start" /> : <Copy data-icon="inline-start" />}
            {copied ? copy.copied : copy.copyCode}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
