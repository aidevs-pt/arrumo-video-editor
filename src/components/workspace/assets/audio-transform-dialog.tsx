"use client";

import { invoke } from "@tauri-apps/api/core";
import { AudioLines, Info } from "lucide-react";
import { useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { useLocale } from "@/lib/locale-provider";

export type AudioTransformProvider = {
  provider: string;
  transform: "voice-change" | "voice-isolation";
  minInputSeconds: number | null;
  maxInputSeconds: number;
};

/** Sends an asset's sound to a provider that changes or isolates its voice, making a new asset. */
export function AudioTransformDialog({ workspaceId, assetId, assetName, offer, disabled, onCreated }: {
  workspaceId: number;
  assetId: number;
  assetName: string;
  offer: AudioTransformProvider;
  disabled?: boolean;
  onCreated: (id: number) => void;
}) {
  const { messages } = useLocale();
  const copy = messages.assetLibrary.audioTransform;
  const text = offer.transform === "voice-change" ? copy.change : copy.isolate;
  const [open, setOpen] = useState(false);
  const [voice, setVoice] = useState("");
  const [state, setState] = useState<"idle" | "working" | "done" | "error">("idle");
  const [error, setError] = useState("");
  const limits = offer.minInputSeconds
    ? copy.limitsBoth.replace("{min}", String(offer.minInputSeconds)).replace("{max}", String(offer.maxInputSeconds))
    : copy.limitsMax.replace("{max}", String(offer.maxInputSeconds));

  async function run() {
    setState("working");
    setError("");
    try {
      const id = await invoke<number>("transform_audio_asset", {
        workspaceId, assetId, transform: offer.transform, provider: offer.provider, voice: voice.trim() || null,
      });
      setState("done");
      onCreated(id);
    } catch (reason) {
      setError(typeof reason === "string" ? reason : "");
      setState("error");
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => { if (state !== "working") { setOpen(next); if (next) setState("idle"); } }}>
      <DialogTrigger asChild>
        <Button variant="outline" size="sm" disabled={disabled}><AudioLines data-icon="inline-start" />{text.action}</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{text.title}</DialogTitle>
          <DialogDescription>{text.description.replace("{name}", assetName)}</DialogDescription>
        </DialogHeader>
        <Alert><Info /><AlertDescription>{limits} {copy.cost}</AlertDescription></Alert>
        {offer.transform === "voice-change" && <Field>
          <FieldLabel htmlFor="transform-voice">{copy.voice}</FieldLabel>
          <Input id="transform-voice" value={voice} maxLength={256} disabled={state === "working"}
            placeholder={copy.voicePlaceholder} onChange={(event) => setVoice(event.target.value)} />
          <FieldDescription>{copy.voiceHint}</FieldDescription>
        </Field>}
        {state === "error" && <Alert variant="destructive"><AlertDescription>{error || copy.failed}</AlertDescription></Alert>}
        {state === "done" && <Alert><AlertDescription>{copy.done}</AlertDescription></Alert>}
        <DialogFooter>
          <DialogClose asChild><Button variant="outline" disabled={state === "working"}>{state === "done" ? copy.close : copy.cancel}</Button></DialogClose>
          {state !== "done" && <Button disabled={state === "working"} onClick={() => void run()}>
            {state === "working" ? copy.working : text.confirm}
          </Button>}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
