"use client";

import { invoke } from "@tauri-apps/api/core";
import { Star } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { useLocale } from "@/lib/locale-provider";

const REPOSITORY = "https://github.com/aidevs-pt/arrumo-video-editor";
const KEY = "arrumo-star-prompt";
/** How many more completed videos "Remind me later" waits for. */
const LATER = 5;

type Saved = { finished?: boolean; remindAt?: number };

function read(): Saved {
  try { return JSON.parse(localStorage.getItem(KEY) ?? "{}") as Saved; } catch { return {}; }
}

function write(saved: Saved) {
  try { localStorage.setItem(KEY, JSON.stringify(saved)); } catch {}
}

/** Once per launch, after the first completed video, asks kindly for a GitHub star. */
export function StarPrompt({ workspaceIds }: { workspaceIds: number[] }) {
  const { messages } = useLocale();
  const copy = messages.starPrompt;
  const [open, setOpen] = useState(false);
  const [completed, setCompleted] = useState(0);
  const [checked, setChecked] = useState(false);

  useEffect(() => {
    if (checked || !workspaceIds.length) return;
    setChecked(true);
    const saved = read();
    if (saved.finished) return;
    Promise.all(workspaceIds.map((workspaceId) =>
      invoke<{ state: string }[]>("list_generation_runs", { workspaceId }).catch(() => [])))
      .then((lists) => {
        const done = lists.flat().filter((run) => run.state === "completed").length;
        setCompleted(done);
        if (done >= (saved.remindAt ?? 1)) setOpen(true);
      });
  }, [checked, workspaceIds]);

  function finish() {
    write({ finished: true });
    setOpen(false);
  }

  function later() {
    write({ remindAt: completed + LATER });
    setOpen(false);
  }

  function star() {
    void invoke("open_external_link", { url: REPOSITORY }).catch(() => {});
    finish();
  }

  return (
    <Dialog open={open} onOpenChange={(next) => { if (!next) finish(); }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2"><Star className="size-5 text-attention" aria-hidden="true" />{copy.title}</DialogTitle>
          <DialogDescription>{completed > 1 ? copy.bodyMany.replace("{count}", String(completed)) : copy.body}</DialogDescription>
        </DialogHeader>
        {/* Stacked, so the three choices fit in every language and keep one size. */}
        <DialogFooter className="flex-col sm:flex-col sm:justify-start">
          <Button onClick={star}><Star data-icon="inline-start" />{copy.star}</Button>
          <Button variant="outline" onClick={later}>{copy.later}</Button>
          <Button variant="ghost" onClick={finish}>{copy.never}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
