"use client";

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";

type AvailableUpdate = { version: string; currentVersion: string; notes: string | null };
type UpdateStatus = { enabled: boolean; version: string; store: boolean };

/** The settings page offers an update it found through the same prompt. */
const OFFERED = "arrumo:update-offered";

/** Asks whether to install an update the desktop host found, now or when Arrumo quits. */
export function UpdatePrompt() {
  const { messages: { updates: copy } } = useLocale();
  const [update, setUpdate] = useState<AvailableUpdate>();
  const [state, setState] = useState<"idle" | "installing" | "scheduled" | "failed">("idle");

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    const offer = (next: AvailableUpdate) => { if (active) { setUpdate(next); setState("idle"); } };
    const onOffered = (event: Event) => offer((event as CustomEvent<AvailableUpdate>).detail);
    window.addEventListener(OFFERED, onOffered);
    void listen<AvailableUpdate>("update-available", (event) => offer(event.payload))
      .then((unlisten) => { if (active) unsubscribe = unlisten; else unlisten(); })
      .catch(() => {});
    return () => { active = false; unsubscribe?.(); window.removeEventListener(OFFERED, onOffered); };
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
    <AlertDialog open={update !== undefined} onOpenChange={(open) => { if (!open && !busy) setUpdate(undefined); }}>
      <AlertDialogContent className="data-[size=default]:max-w-[calc(100vw-2rem)] data-[size=default]:sm:max-w-lg">
        <AlertDialogHeader>
          <AlertDialogTitle>{copy.title.replace("{version}", update?.version ?? "")}</AlertDialogTitle>
          <AlertDialogDescription>
            {state === "scheduled" ? copy.scheduled : copy.description.replace("{current}", update?.currentVersion ?? "")}
          </AlertDialogDescription>
        </AlertDialogHeader>
        {update?.notes && state !== "scheduled" && <div className="max-h-48 overflow-y-auto rounded-lg bg-muted p-3 text-sm whitespace-pre-wrap">{update.notes}</div>}
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
