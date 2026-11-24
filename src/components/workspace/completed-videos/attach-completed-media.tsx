import { invoke } from "@tauri-apps/api/core";
import { Paperclip } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { AssetPicker } from "@/components/workspace/assets/asset-picker";
import { useLocale } from "@/lib/locale-provider";

/** Attaches a library file to a finished video's format, picked by its preview and description. */
export function AttachCompletedMedia({ workspaceId, runId, formatId, formatName, attached, onAttached }: {
  workspaceId: number;
  runId: number;
  formatId: number;
  formatName: string;
  attached: readonly number[];
  onAttached: () => void;
}) {
  const { messages: { completedVideo: copy } } = useLocale();
  const [open, setOpen] = useState(false);

  async function attach([assetId]: number[]) {
    await invoke<boolean>("attach_completed_media", { workspaceId, runId, formatId, assetId });
    onAttached();
  }

  return <>
    <Button size="sm" variant="outline" onClick={() => setOpen(true)}><Paperclip data-icon="inline-start" />{copy.attachFile}</Button>
    <AssetPicker workspaceId={workspaceId} open={open} onOpenChange={setOpen} title={copy.attachFile}
      description={copy.attachFileDescription.replace("{format}", () => formatName)}
      value={[]} exclude={[...attached]} confirmLabel={copy.attachFile} onConfirm={attach} />
  </>;
}
