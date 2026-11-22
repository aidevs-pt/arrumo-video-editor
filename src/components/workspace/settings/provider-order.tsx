"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowDown, ArrowUp, RotateCcw } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import { useEffect, useState } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useLocale } from "@/lib/locale-provider";
import type { PluginSettingDeclaration } from "./plugin-preferences";

type Provider = { id: string; name: string; bundled: boolean };
type Preferences = {
  available: Provider[];
  app: string[] | null;
  workspaceOverride: string[] | null;
  effective: string[];
  source: string;
};
type Manifest = {
  id: string;
  name: string;
  settings?: PluginSettingDeclaration[];
  capabilities?: Record<string, { name: string; description: string }>;
  agentProvider?: { cli?: { imageRun?: unknown } };
};
type CatalogEntry = { bundled: boolean; manifest: Manifest };
type KeyState = "none" | "saved" | "missing";

/** Capabilities creators choose an order for; agents are chosen on the Agent tab. */
const SHOWN = ["transcription", "image-generation", "text-to-speech", "video-generation", "subtitle-translation"];

/** Which plugin does each job, in the order Arrumo tries them, for the app or one workspace. */
export function ProviderOrder({ catalog, workspaceId }: { catalog: CatalogEntry[]; workspaceId: number | null }) {
  const { messages: { providerOrder: copy } } = useLocale();
  const reduceMotion = useReducedMotion();
  const [preferences, setPreferences] = useState<Record<string, Preferences>>();
  const [keys, setKeys] = useState<Record<string, KeyState>>({});
  const [failed, setFailed] = useState(false);
  const [pending, setPending] = useState<string>();

  useEffect(() => {
    let active = true;
    invoke<Record<string, Preferences>>("list_provider_preferences", { workspaceId }).then(
      (result) => { if (active) setPreferences(result); },
      () => { if (active) setFailed(true); },
    );
    // One at a time: the system credential store does not always answer parallel reads.
    void (async () => {
      for (const { manifest } of catalog) {
        const secret = manifest.settings?.find((setting) => setting.kind === "secret");
        let state: KeyState = "none";
        if (secret) {
          try {
            const status = await invoke<{ source: string | null }>("read_plugin_credential_status",
              { pluginId: manifest.id, settingId: secret.id, workspaceId });
            state = status.source ? "saved" : "missing";
          } catch {
            continue;
          }
        }
        if (!active) return;
        setKeys((current) => ({ ...current, [manifest.id]: state }));
      }
    })();
    return () => { active = false; };
  }, [catalog, workspaceId]);

  async function save(capability: string, order: string[] | null) {
    setPending(capability);
    setFailed(false);
    try {
      setPreferences(await invoke<Record<string, Preferences>>("write_provider_preferences", { capability, workspaceId, order }));
    } catch {
      setFailed(true);
    } finally {
      setPending(undefined);
    }
  }

  if (!preferences) return null;
  const capabilities = SHOWN.filter((id) => (preferences[id]?.available.length ?? 0) > 0);
  if (capabilities.length === 0) return null;

  function note(capability: string, id: string) {
    const manifest = catalog.find((entry) => entry.manifest.id === id)?.manifest;
    if (capability === "image-generation" && manifest?.agentProvider?.cli?.imageRun) return copy.noExtraKey;
    if (capability === "subtitle-translation" && manifest?.agentProvider) return copy.agentSignIn;
    return { none: copy.noKey, saved: copy.keySaved, missing: copy.keyNeeded, undefined: "" }[String(keys[id]) as KeyState | "undefined"];
  }

  return (
    <section aria-labelledby="provider-order" className="flex flex-col gap-4">
      <h2 id="provider-order" className="text-[1.5rem]">{copy.title}</h2>
      <p className="text-muted-foreground">{workspaceId === null ? copy.appIntro : copy.workspaceIntro}</p>
      {failed && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
      <div className="grid gap-6 md:grid-cols-2">
        {capabilities.map((capability) => {
          const entry = preferences[capability];
          const names = new Map(entry.available.map((provider) => [provider.id, provider.name]));
          const order = entry.effective.filter((id) => names.has(id));
          const custom = workspaceId === null ? entry.app !== null : entry.workspaceOverride !== null;
          const label = catalog.flatMap(({ manifest }) => manifest.capabilities?.[capability]?.name ?? []).at(0);
          const move = (index: number, by: number) => {
            const next = [...order];
            [next[index], next[index + by]] = [next[index + by], next[index]];
            void save(capability, next);
          };
          return (
            <div key={capability} className="flex flex-col gap-2">
              <div className="flex items-center justify-between gap-2">
                <h3 className="font-semibold">{copy.capabilities[capability as keyof typeof copy.capabilities] ?? label ?? capability}</h3>
                {custom && <Button variant="ghost" size="sm" disabled={pending === capability} onClick={() => void save(capability, null)}>
                  <RotateCcw aria-hidden="true" />{workspaceId === null ? copy.resetApp : copy.resetWorkspace}
                </Button>}
              </div>
              <ol className="divide-y rounded-xl border bg-card">
                {order.map((id, index) => (
                  <motion.li key={id} layout={!reduceMotion} transition={{ duration: 0.2 }}
                    className="flex items-center gap-3 px-3 py-2.5">
                    <span className="w-4 text-sm tabular-nums text-muted-foreground">{index + 1}</span>
                    <span className="flex-1 font-semibold">{names.get(id)}</span>
                    <span className="text-sm text-muted-foreground">{note(capability, id)}</span>
                    {order.length > 1 && <span className="flex">
                      <Button variant="ghost" size="icon" className="size-7" disabled={index === 0 || pending === capability}
                        aria-label={copy.moveUp.replace("{provider}", names.get(id) ?? id)} onClick={() => move(index, -1)}>
                        <ArrowUp aria-hidden="true" />
                      </Button>
                      <Button variant="ghost" size="icon" className="size-7" disabled={index === order.length - 1 || pending === capability}
                        aria-label={copy.moveDown.replace("{provider}", names.get(id) ?? id)} onClick={() => move(index, 1)}>
                        <ArrowDown aria-hidden="true" />
                      </Button>
                    </span>}
                  </motion.li>
                ))}
              </ol>
              <p className="text-sm text-muted-foreground">
                {copy.hints[capability as keyof typeof copy.hints]}
                {!custom && workspaceId !== null && <> {copy.inherited}</>}
              </p>
            </div>
          );
        })}
      </div>
    </section>
  );
}
