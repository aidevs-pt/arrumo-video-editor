"use client";

import { invoke } from "@tauri-apps/api/core";
import { Info, Languages } from "lucide-react";
import { useMemo, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { useLocale } from "@/lib/locale-provider";
import { soundLength } from "./audio-transform-dialog";

export type DubbingProvider = { provider: string; languages: string[]; maxInputSeconds: number };

/** Dubs an asset's speech into another language with a provider, making a new asset. */
export function DubDialog({ workspaceId, assetId, assetName, offer, disabled, onCreated }: {
  workspaceId: number;
  assetId: number;
  assetName: string;
  offer: DubbingProvider;
  disabled?: boolean;
  onCreated: (id: number) => void;
}) {
  const { locale, messages } = useLocale();
  const copy = messages.assetLibrary.dub;
  const shared = messages.assetLibrary.audioTransform;
  const languages = useMemo(() => {
    const names = new Intl.DisplayNames([locale], { type: "language" });
    return offer.languages
      .map((code) => ({ code, name: names.of(code) ?? code }))
      .sort((a, b) => a.name.localeCompare(b.name, locale));
  }, [offer.languages, locale]);
  const [open, setOpen] = useState(false);
  const [language, setLanguage] = useState(offer.languages.includes(locale) ? locale : offer.languages[0]);
  const [state, setState] = useState<"idle" | "working" | "done" | "error">("idle");
  const [error, setError] = useState("");

  async function run() {
    setState("working");
    setError("");
    try {
      onCreated(await invoke<number>("dub_asset", { workspaceId, assetId, language, provider: offer.provider }));
      setState("done");
    } catch (reason) {
      setError(typeof reason === "string" ? reason : "");
      setState("error");
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => { if (state !== "working") { setOpen(next); if (next) setState("idle"); } }}>
      <DialogTrigger asChild>
        <Button variant="outline" size="sm" disabled={disabled}><Languages data-icon="inline-start" />{copy.action}</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{copy.title}</DialogTitle>
          <DialogDescription>{copy.description.replace("{name}", assetName)}</DialogDescription>
        </DialogHeader>
        <Alert><Info /><AlertDescription>
          {shared.limitsMax.replace("{max}", soundLength(offer.maxInputSeconds, locale))} {copy.wait} {shared.cost}
        </AlertDescription></Alert>
        <Field>
          <FieldLabel htmlFor="dub-language">{copy.language}</FieldLabel>
          <NativeSelect id="dub-language" value={language} disabled={state === "working"}
            onChange={(event) => setLanguage(event.target.value)}>
            {languages.map((entry) => <NativeSelectOption key={entry.code} value={entry.code}>{entry.name}</NativeSelectOption>)}
          </NativeSelect>
        </Field>
        {state === "error" && <Alert variant="destructive"><AlertDescription>{error || shared.failed}</AlertDescription></Alert>}
        {state === "done" && <Alert><AlertDescription>{shared.done}</AlertDescription></Alert>}
        <DialogFooter>
          <DialogClose asChild><Button variant="outline" disabled={state === "working"}>{state === "done" ? shared.close : shared.cancel}</Button></DialogClose>
          {state !== "done" && <Button disabled={state === "working"} onClick={() => void run()}>
            {state === "working" ? copy.working : copy.confirm}
          </Button>}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
