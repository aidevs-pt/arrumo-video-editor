"use client";

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useLaunchDialog } from "@/lib/launch-dialogs";
import { useLocale } from "@/lib/locale-provider";

type AvailableUpdate = { version: string; currentVersion: string; notes: string | null };
type UpdateStatus = { enabled: boolean; version: string; store: boolean };

/** The settings page offers an update it found through the same prompt. */
const OFFERED = "arrumo:update-offered";
/** How long the interface must stay up before the launch counts as working. */
const LAUNCH_SETTLED_MS = 30_000;

/** Asks whether to install an update the desktop host found, now or when Arrumo quits. */
export function UpdatePrompt() {
  const { messages: { updates: copy } } = useLocale();
  const [update, setUpdate] = useState<AvailableUpdate>();
  const [state, setState] = useState<"idle" | "installing" | "scheduled" | "failed">("idle");
  const open = useLaunchDialog("update", update !== undefined);

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    const offer = (next: AvailableUpdate) => { if (active) { setUpdate(next); setState("idle"); } };
    const onOffered = (event: Event) => offer((event as CustomEvent<AvailableUpdate>).detail);
    window.addEventListener(OFFERED, onOffered);
    void invoke("launch_ready", { settled: false }).catch(() => {});
    const settled = setTimeout(() => void invoke("launch_ready", { settled: true }).catch(() => {}), LAUNCH_SETTLED_MS);
    void listen<AvailableUpdate>("update-available", (event) => offer(event.payload))
      .then((unlisten) => { if (active) unsubscribe = unlisten; else unlisten(); })
      .catch(() => {});
    return () => { active = false; clearTimeout(settled); unsubscribe?.(); window.removeEventListener(OFFERED, onOffered); };
  }, []);

  async function install(now: boolean) {
    setState("installing");
    try {
      await invoke("install_update", { now });
      setState("scheduled");
    } catch {
      setState("failed");
    }
  }

  const busy = state === "installing";
  return (
    <AlertDialog open={open} onOpenChange={(open) => { if (!open && !busy) setUpdate(undefined); }}>
      <AlertDialogContent className="max-h-[calc(100svh-2rem)] overflow-y-auto data-[size=default]:max-w-[calc(100vw-2rem)] data-[size=default]:sm:max-w-lg">
        <AlertDialogHeader>
          <AlertDialogTitle>{copy.title.replace("{version}", update?.version ?? "")}</AlertDialogTitle>
          <AlertDialogDescription>
            {state === "scheduled" ? copy.scheduled : copy.description.replace("{current}", update?.currentVersion ?? "")}
          </AlertDialogDescription>
        </AlertDialogHeader>
        {update?.notes && state !== "scheduled" && <ReleaseNotes notes={update.notes} />}
        {update && state !== "scheduled" && <a href={`https://github.com/aidevs-pt/arrumo-video-editor/releases/tag/v${update.version}`} target="_blank" rel="noreferrer" className="w-fit text-sm font-semibold text-primary underline underline-offset-4">{copy.whatsNew}</a>}
        {state === "failed" && <Alert variant="destructive"><AlertDescription>{copy.failed}</AlertDescription></Alert>}
        <AlertDialogFooter className="sm:flex-wrap">
          {state === "scheduled" ? <AlertDialogAction>{copy.done}</AlertDialogAction> : <>
            <AlertDialogCancel disabled={busy} className="sm:me-auto">{copy.later}</AlertDialogCancel>
            <Button variant="outline" disabled={busy} onClick={() => void install(false)}>{copy.onQuit}</Button>
            <Button disabled={busy} onClick={() => void install(true)}>{busy ? copy.installing : copy.now}</Button>
          </>}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

/** The changelog the release workflow writes: `### Section` headings and `- item` lines. */
function ReleaseNotes({ notes }: { notes: string }) {
  const blocks: { heading?: string; items: string[]; text: string[] }[] = [{ items: [], text: [] }];
  for (const line of notes.split("\n").map((value) => value.trim()).filter(Boolean)) {
    if (line.startsWith("### ")) blocks.push({ heading: line.slice(4), items: [], text: [] });
    else if (line.startsWith("- ")) blocks[blocks.length - 1].items.push(line.slice(2).replace(/^\*\*(.+?)\*\*\s*/, "$1 "));
    else blocks[blocks.length - 1].text.push(line);
  }
  return (
    <div className="flex max-h-56 flex-col gap-3 overflow-y-auto rounded-lg bg-muted p-3 text-sm">
      {blocks.filter((block) => block.heading || block.items.length || block.text.length).map((block, index) => (
        <div key={index} className="flex flex-col gap-1">
          {block.heading && <p className="font-semibold">{block.heading}</p>}
          {block.text.map((text) => <p key={text}>{text}</p>)}
          {block.items.length > 0 && <ul className="list-disc ps-5">{block.items.map((item) => <li key={item}>{item}</li>)}</ul>}
        </div>
      ))}
    </div>
  );
}

/** The version this is, and a manual check for updates. */
export function UpdateSettings() {
  const { messages: { updates: copy } } = useLocale();
  const [status, setStatus] = useState<UpdateStatus>();
  const [check, setCheck] = useState<"idle" | "checking" | "current" | "failed">("idle");

  useEffect(() => {
    let active = true;
    invoke<UpdateStatus>("update_status").then((value) => { if (active) setStatus(value); }, () => {});
    return () => { active = false; };
  }, []);

  async function checkNow() {
    setCheck("checking");
    try {
      const update = await invoke<AvailableUpdate | null>("check_for_update");
      if (update) {
        setCheck("idle");
        window.dispatchEvent(new CustomEvent(OFFERED, { detail: update }));
      } else {
        setCheck("current");
      }
    } catch {
      setCheck("failed");
    }
  }

  return (
    <section aria-labelledby="settings-updates">
      <h2 id="settings-updates" className="mb-3 text-[1.5rem]">{copy.heading}</h2>
      {status ? <div className="flex flex-wrap items-center justify-between gap-4 rounded-xl border bg-card px-5 py-4">
        <div className="min-w-0 flex-1 basis-64">
          <p className="font-semibold">{copy.version.replace("{version}", status.version)}</p>
          <p className="text-sm text-muted-foreground" role="status">
            {!status.enabled ? (status.store ? copy.store : copy.notInThisBuild)
              : check === "current" ? copy.current : check === "failed" ? copy.checkFailed : copy.automatic}
          </p>
        </div>
        {status.enabled && <Button variant="outline" disabled={check === "checking"} onClick={() => void checkNow()}>
          {check === "checking" ? copy.checking : copy.check}
        </Button>}
      </div> : <Skeleton className="h-20" />}
    </section>
  );
}
