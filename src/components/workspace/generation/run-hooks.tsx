"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { Field, FieldDescription, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";
import { Switch } from "@/components/ui/switch";
import type { InstalledAction } from "@/components/workspace/plugins/action-form";
import { useLocale } from "@/lib/locale-provider";
import { PluginIcon, usePluginText } from "@/lib/plugin-text";

/** The plugin hooks switched on for this workspace that run when this video finishes, each of
 * which the creator can skip for this one video. Submits the skipped ones as `skip-hooks` JSON. */
export function RunHooks({ workspaceId, disabled }: { workspaceId: number; disabled: boolean }) {
  const { messages: { runHooks: copy } } = useLocale();
  const pluginText = usePluginText();
  const [hooks, setHooks] = useState<InstalledAction[]>([]);
  const [skipped, setSkipped] = useState<string[]>([]);

  useEffect(() => {
    let active = true;
    (async () => {
      const actions = await invoke<InstalledAction[]>("list_plugin_actions", { workspaceId });
      const on = await Promise.all(actions.filter((entry) => entry.action.hooks?.length).map(async (entry) => {
        const settings = await invoke<Record<string, { effective?: string | null }>>("read_plugin_settings", { pluginId: entry.pluginId, workspaceId });
        return entry.action.hooks!.some((hook) => settings[hook.enabledBy]?.effective === "true") ? entry : null;
      }));
      if (active) setHooks(on.filter((entry): entry is InstalledAction => entry !== null));
    })().catch(() => { if (active) setHooks([]); });
    return () => { active = false; };
  }, [workspaceId]);

  if (!hooks.length) return null;
  return <FieldSet>
    <FieldLegend variant="label">{copy.title}</FieldLegend>
    <FieldDescription>{copy.description}</FieldDescription>
    <input type="hidden" name="skip-hooks" value={JSON.stringify(skipped)} />
    {hooks.map((entry) => {
      const key = `${entry.pluginId}/${entry.action.id}`;
      const id = `run-hook-${key.replace(/[^a-z0-9-]/gi, "-")}`;
      return <Field key={key} orientation="horizontal">
        <Switch id={id} checked={!skipped.includes(key)} disabled={disabled}
          onCheckedChange={(on) => setSkipped((current) => on ? current.filter((item) => item !== key) : [...current, key])} />
        <FieldLabel htmlFor={id} className="flex items-center gap-2 font-normal">
          <PluginIcon pluginId={entry.pluginId} className="size-5" />
          {pluginText.action(entry.pluginId, entry.action.id, "label", entry.action.label)}
          <span className="text-muted-foreground">· {pluginText.name(entry.pluginId, entry.pluginName)}</span>
        </FieldLabel>
      </Field>;
    })}
  </FieldSet>;
}
