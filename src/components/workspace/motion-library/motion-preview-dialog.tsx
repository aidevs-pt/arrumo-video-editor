import { invoke } from "@tauri-apps/api/core";
import { Play } from "lucide-react";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { useLocale } from "@/lib/locale-provider";

type Preview = { video_url: string; sound_url: string | null };

export function MotionPreviewDialog({ workspaceId, profileId, itemId, title }: {
  workspaceId: number;
  profileId: number | null;
  itemId: number;
  title: string;
}) {
  const { messages } = useLocale();
  const copy = messages.motionLibrary;
  const [open, setOpen] = useState(false);
  const [preview, setPreview] = useState<Preview>();
  const [videoError, setVideoError] = useState(false);
  const [soundError, setSoundError] = useState(false);

  useEffect(() => {
    if (!open) return;
    let active = true;
    invoke<Preview>("preview_motion_library_item", { workspaceId, profileId, itemId }).then(
      (value) => { if (active) setPreview(value); },
      () => { if (active) setVideoError(true); },
    );
    return () => { active = false; };
  }, [open, workspaceId, profileId, itemId]);

  return (
    <Dialog open={open} onOpenChange={(next) => {
      setPreview(undefined);
      setVideoError(false);
      setSoundError(false);
      setOpen(next);
    }}>
      <DialogTrigger asChild>
        <Button type="button" size="sm" variant="outline" aria-label={`${copy.playPreview} · ${title}`}>
          <Play data-icon="inline-start" />{copy.playPreview}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-h-[85dvh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="wrap-anywhere pe-6">{title}</DialogTitle>
          <DialogDescription>{copy.previewDescription}</DialogDescription>
        </DialogHeader>
        {videoError ? <Alert variant="destructive"><AlertDescription>{copy.previewError}</AlertDescription></Alert>
          : preview ? <video aria-label={copy.video} src={preview.video_url} controls loop playsInline preload="metadata"
              className="aspect-video max-h-[55dvh] w-full rounded-md bg-black" onError={() => setVideoError(true)} />
            : <p role="status" className="text-sm text-muted-foreground">{copy.loadingPreview}</p>}
        {preview?.sound_url && <div className="flex flex-col gap-2">
          <p className="font-medium">{copy.audio}</p>
          {soundError ? <Alert variant="destructive"><AlertDescription>{copy.soundPreviewError}</AlertDescription></Alert>
            : <audio aria-label={copy.audio} src={preview.sound_url} controls preload="none" className="w-full"
                onError={() => setSoundError(true)} />}
        </div>}
      </DialogContent>
    </Dialog>
  );
}
