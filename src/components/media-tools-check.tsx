"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { AlertDialog, AlertDialogAction, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { useLaunchDialog } from "@/lib/launch-dialogs";
import { useLocale } from "@/lib/locale-provider";

type MediaToolsStatus = { ffmpeg: boolean; ffprobe: boolean };

/** Says at launch when FFmpeg or ffprobe doesn't start, since nothing renders without them. */
export function MediaToolsCheck() {
  const { messages: { mediaTools: copy } } = useLocale();
  const [missing, setMissing] = useState<string[]>([]);
  const open = useLaunchDialog("media-tools", missing.length > 0);

  useEffect(() => {
    let active = true;
    void invoke<MediaToolsStatus>("media_tools_status")
      .then((status) => {
        if (!active) return;
        setMissing([!status.ffmpeg && "FFmpeg", !status.ffprobe && "ffprobe"].filter((tool): tool is string => Boolean(tool)));
      })
      .catch(() => {});
    return () => { active = false; };
  }, []);

  return (
    <AlertDialog open={open} onOpenChange={(open) => { if (!open) setMissing([]); }}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{copy.title}</AlertDialogTitle>
          <AlertDialogDescription>{copy.description.replace("{tools}", missing.join(", "))}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogAction>{copy.done}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
