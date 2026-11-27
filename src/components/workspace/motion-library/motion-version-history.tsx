"use client";

import { invoke } from "@tauri-apps/api/core";
import { ChevronDown, History, RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { IconAction } from "@/components/workspace/icon-action";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

import { MotionPreviewDialog } from "./motion-preview-dialog";

type Version = {
  version: number;
  current: boolean;
  title: string;
  change_note: string;
  restored_from: number | null;
  replaced_at: string | null;
};

/**
 * The states an animation went through: each change made with the agent, or new media chosen by
 * hand, keeps the one before. Any of them can be watched and brought back.
 */
export function MotionVersionHistory<Item>({ workspaceId, itemId, revision, disabled, onRestored }: {
  workspaceId: number;
  itemId: number;
  /** Changes when the item changes, so the list follows. */
  revision: unknown;
  disabled?: boolean;
  onRestored: (item: Item) => void;
}) {
  const { messages: { motionLibrary: copy }, locale } = useLocale();
  const [versions, setVersions] = useState<Version[]>();
  const [restoring, setRestoring] = useState<number>();
  const [failed, setFailed] = useState(false);
  const [reload, setReload] = useState(0);

  useEffect(() => {
    let active = true;
    invoke<Version[]>("list_motion_library_versions", { workspaceId, itemId })
      .then((entries) => { if (active) setVersions(entries); }, () => { if (active) setVersions([]); });
    return () => { active = false; };
  }, [workspaceId, itemId, revision, reload]);

  async function restore(version: number) {
    setRestoring(version);
    setFailed(false);
    try {
      onRestored(await invoke<Item>("restore_motion_library_version", { workspaceId, itemId, version }));
      setReload((value) => value + 1);
    } catch {
      setFailed(true);
    } finally {
      setRestoring(undefined);
    }
  }

  if (!versions || versions.length < 2) return null;
  const when = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" });
  const describe = (entry: Version) => entry.restored_from !== null
    ? copy.versionRestored.replace("{version}", String(entry.restored_from))
    : entry.change_note ? `“${entry.change_note}”` : entry.version === 1 ? copy.versionFirst : copy.versionByHand;

  return (
    <Collapsible className="border-t pt-4">
      <CollapsibleTrigger asChild>
        <Button type="button" variant="ghost" className="group -mx-2 w-[calc(100%+1rem)] justify-start px-2">
          <History data-icon="inline-start" />{copy.versions.replace("{count}", String(versions.length))}
          <ChevronDown data-icon="inline-end" className="ms-auto transition-transform duration-150 group-data-[state=open]:rotate-180 motion-reduce:transition-none" />
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <ol className="mt-2 flex flex-col">
          {versions.map((entry) => (
            <li key={entry.version} className={cn("flex items-start gap-2 border-b py-2.5 last:border-b-0", entry.current && "text-foreground")}>
              <div className="min-w-0 flex-1">
                <p className="flex flex-wrap items-center gap-2 font-semibold">
                  {copy.versionLabel.replace("{version}", String(entry.version))}
                  {entry.current && <span className="rounded-md bg-secondary px-1.5 py-0.5 text-xs">{copy.versionCurrent}</span>}
                </p>
                <p className="line-clamp-2 text-sm text-muted-foreground wrap-anywhere">{describe(entry)}</p>
                {entry.replaced_at && <p className="text-xs text-muted-foreground tabular-nums">
                  {copy.versionReplaced.replace("{date}", when.format(new Date(entry.replaced_at)))}</p>}
              </div>
              {!entry.current && <>
                <MotionPreviewDialog compact workspaceId={workspaceId} itemId={itemId} version={entry.version}
                  title={`${entry.title} · ${copy.versionLabel.replace("{version}", String(entry.version))}`} />
                <IconAction label={copy.versionRestore.replace("{version}", String(entry.version))} variant="ghost"
                  disabled={disabled || restoring !== undefined} onClick={() => void restore(entry.version)}>
                  <RotateCcw className={cn(restoring === entry.version && "animate-spin motion-reduce:animate-none")} />
                </IconAction>
              </>}
            </li>
          ))}
        </ol>
        {failed && <p role="alert" className="text-sm text-destructive">{copy.versionRestoreError}</p>}
      </CollapsibleContent>
    </Collapsible>
  );
}
