"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { AlertDialog, AlertDialogAction, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { useLaunchDialog } from "@/lib/launch-dialogs";
import { useLocale } from "@/lib/locale-provider";

type Announcement = { id: string; text: Record<string, string>; link?: string | null };

/** A message from the Arrumo team, signed and checked by the desktop, shown once at launch. */
export function AnnouncementPrompt() {
  const { locale, messages: { announcements: copy } } = useLocale();
  const [announcement, setAnnouncement] = useState<Announcement>();
  const open = useLaunchDialog("announcement", announcement !== undefined);

  useEffect(() => {
    let active = true;
    void invoke<Announcement | null>("pending_announcement")
      .then((next) => { if (active && next) setAnnouncement(next); })
      .catch(() => {});
    return () => { active = false; };
  }, []);

  function close() {
    if (announcement) void invoke("dismiss_announcement", { id: announcement.id }).catch(() => {});
    setAnnouncement(undefined);
  }

  const text = announcement && (announcement.text[locale] ?? announcement.text.en);
  return (
    <AlertDialog open={open} onOpenChange={(next) => { if (!next) close(); }}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{copy.title}</AlertDialogTitle>
          <AlertDialogDescription className="whitespace-pre-line">{text}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          {announcement?.link && <Button variant="outline" asChild><a href={announcement.link} target="_blank" rel="noreferrer">{copy.more}</a></Button>}
          <AlertDialogAction onClick={close}>{copy.close}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
