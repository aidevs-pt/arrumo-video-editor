"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, X } from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useCallback, useEffect, useState, useSyncExternalStore } from "react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { useLocale } from "@/lib/locale-provider";
import { PLUGINS_CHANGED, PluginIcon, usePluginText } from "@/lib/plugin-text";

type Localized = Record<string, string>;
type Stop = { pluginId: string; name: string; headline: Localized; body: Localized };

const REPLAY = "arrumo:plugin-tour";

/** Asks the tour host to introduce plugins that arrived since the creator last looked. */
export function announcePluginChanges() {
  window.dispatchEvent(new Event(PLUGINS_CHANGED));
}

/** Whether the tour is still being checked for or is showing, so other prompts can wait for it. */
let touring = true;
const tourListeners = new Set<() => void>();
function setTouring(value: boolean) {
  if (touring === value) return;
  touring = value;
  for (const listener of tourListeners) listener();
}
function subscribeTouring(listener: () => void) {
  tourListeners.add(listener);
  return () => { tourListeners.delete(listener); };
}
export function usePluginTourActive() {
  return useSyncExternalStore(subscribeTouring, () => touring, () => true);
}

/** Replays the introductions of the given plugins, or of every plugin that has one. */
export function replayPluginTour(ids: string[] = []) {
  window.dispatchEvent(new CustomEvent<string[]>(REPLAY, { detail: ids }));
}

/** The shared plugin tour: one page per new plugin, from the introduction each plugin declares.
 * Mounted once in the studio frame. */
export function PluginTourHost() {
  const [stops, setStops] = useState<Stop[]>([]);
  const [replaying, setReplaying] = useState(false);
  const [checked, setChecked] = useState(false);

  const checkPending = useCallback(async () => {
    try {
      const pending = await invoke<Stop[]>("pending_plugin_tour");
      if (pending.length > 0) { setReplaying(false); setStops(pending); }
    } catch {
      // The tour is a courtesy; failing to read it must not block the app.
    } finally {
      setChecked(true);
    }
  }, []);

  useEffect(() => { setTouring(!checked || stops.length > 0); }, [checked, stops.length]);

  useEffect(() => {
    void checkPending();
    const replay = (event: Event) => {
      const ids = (event as CustomEvent<string[]>).detail ?? [];
      invoke<Stop[]>("plugin_tour", { ids }).then((all) => {
        if (all.length > 0) { setReplaying(true); setStops(all); }
      }, () => undefined);
    };
    const changed = () => void checkPending();
    window.addEventListener(PLUGINS_CHANGED, changed);
    window.addEventListener(REPLAY, replay);
    return () => {
      window.removeEventListener(PLUGINS_CHANGED, changed);
      window.removeEventListener(REPLAY, replay);
    };
  }, [checkPending]);

  function finish() {
    const ids = stops.map((stop) => stop.pluginId);
    setStops([]);
    void invoke("finish_plugin_tour", { ids }).catch(() => undefined);
  }

  return stops.length > 0 ? <PluginTour key={stops.map((stop) => stop.pluginId).join()} stops={stops} replaying={replaying} onFinish={finish} /> : null;
}

function PluginTour({ stops, replaying, onFinish }: { stops: Stop[]; replaying: boolean; onFinish: () => void }) {
  const { locale, messages: { pluginTour: copy } } = useLocale();
  const reduceMotion = useReducedMotion();
  const pluginText = usePluginText();
  const [index, setIndex] = useState(0);
  const name = (entry: Stop) => pluginText.name(entry.pluginId, entry.name);
  const stop = stops[index];
  const previous = stops[index - 1];
  const next = stops[index + 1];
  const text = (value: Localized) => value[locale] ?? value.en;
  const heading = stops.length === 1
    ? (replaying ? name(stop) : copy.newPlugin)
    : (replaying ? copy.plugins : copy.newPlugins).replace("{current}", String(index + 1)).replace("{total}", String(stops.length));

  return (
    <Dialog open onOpenChange={(open) => { if (!open) onFinish(); }}>
      <DialogContent showCloseButton={false} className="gap-0 overflow-hidden rounded-xl border-0 p-0 sm:max-w-3xl">
        <div className="bg-primary px-6 pt-6 pb-0 text-primary-foreground sm:px-8 sm:pt-8">
          <div className="flex items-center justify-between gap-4">
            <p className="font-bold">{heading}</p>
            <DialogClose asChild>
              <Button variant="ghost" size="icon" className="text-primary-foreground hover:bg-white/15 hover:text-primary-foreground" aria-label={copy.close}>
                <X />
              </Button>
            </DialogClose>
          </div>
          {stops.length > 1 && (
            <ol className="mt-5 flex gap-2" aria-label={heading}>
              {stops.map((entry, position) => (
                <li key={entry.pluginId} className="h-1 flex-1 rounded-full transition-colors"
                  style={{ background: position < index ? "var(--yellow)" : position === index ? "var(--on-cobalt)" : "color-mix(in oklab, var(--on-cobalt) 30%, transparent)" }}>
                  <span className="sr-only">{name(entry)}</span>
                </li>
              ))}
            </ol>
          )}
          <div className="relative mt-6 grid h-44 place-items-center overflow-hidden rounded-t-lg bg-[var(--ink)] sm:h-56" aria-hidden="true">
            <div className="relative size-32 rounded-full border border-dashed border-white/35 sm:size-40">
              <motion.div className="absolute inset-0"
                animate={reduceMotion ? undefined : { rotate: 360 }}
                transition={{ duration: 14, ease: "linear", repeat: Infinity }}>
                <span className="absolute top-1/2 -left-2 size-4 -translate-y-1/2 rounded-sm bg-primary" />
              </motion.div>
              <span className="absolute inset-0 m-auto grid size-14 place-items-center rounded-xl bg-white/10">
                <PluginIcon pluginId={stop.pluginId} tone="dark" className="size-10" />
              </span>
            </div>
          </div>
        </div>
        <div className="px-6 pt-6 pb-6 sm:px-8 sm:pb-8">
          <AnimatePresence mode="wait" initial={false}>
            <motion.div key={stop.pluginId}
              initial={reduceMotion ? false : { opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={reduceMotion ? undefined : { opacity: 0, y: -8 }}
              transition={{ duration: 0.18 }}>
              <p className="text-sm text-muted-foreground">{name(stop)}</p>
              <DialogTitle className="mt-1 font-heading text-[clamp(1.75rem,4vw,2.5rem)] leading-tight font-[820] [font-variation-settings:'wdth'_125]">{text(stop.headline)}</DialogTitle>
              <DialogDescription className="mt-3 text-base text-foreground/80">{text(stop.body)}</DialogDescription>
            </motion.div>
          </AnimatePresence>
          <div className="mt-6 flex flex-wrap items-center gap-2">
            {previous && (
              <Button variant="ghost" onClick={() => setIndex(index - 1)}>
                <ArrowLeft data-icon="inline-start" />{name(previous)}
              </Button>
            )}
            <div className="ms-auto flex flex-wrap items-center gap-2">
              {next && <Button variant="ghost" onClick={onFinish}>{copy.skip}</Button>}
              <Button onClick={() => (next ? setIndex(index + 1) : onFinish())}>
                {next ? copy.next.replace("{name}", name(next)) : copy.done}
              </Button>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
