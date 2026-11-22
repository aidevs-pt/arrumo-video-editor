"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { useLocale } from "@/lib/locale-provider";
import { PLUGINS_CHANGED, PluginIcon, usePluginText } from "@/lib/plugin-text";

import { ActionForm, type ActionContext, type ActionPlacement, type InstalledAction } from "./action-form";

/** The installed plugin actions declared for one place in the app, refreshed when plugins change. */
export function usePlacedActions(workspaceId: number, placement: ActionPlacement) {
  const [actions, setActions] = useState<InstalledAction[]>([]);
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    const changed = () => setRevision((value) => value + 1);
    window.addEventListener(PLUGINS_CHANGED, changed);
    return () => window.removeEventListener(PLUGINS_CHANGED, changed);
  }, []);

  useEffect(() => {
    let active = true;
    invoke<InstalledAction[]>("list_plugin_actions", { workspaceId }).then(
      (items) => { if (active) setActions(items.filter((entry) => entry.action.placements?.includes(placement))); },
      () => { if (active) setActions([]); },
    );
    return () => { active = false; };
  }, [workspaceId, placement, revision]);

  return actions;
}

export function usePlacedActionLabel() {
  const pluginText = usePluginText();
  return (entry: InstalledAction) => pluginText.action(entry.pluginId, entry.action.id, "label", entry.action.label);
}

/** One action's form in a side sheet. */
export function ActionSheet({ workspaceId, entry, context, onClose }: {
  workspaceId: number;
  entry: InstalledAction | undefined;
  context?: ActionContext;
  onClose: () => void;
}) {
  const pluginText = usePluginText();
  const label = usePlacedActionLabel();
  return <Sheet open={!!entry} onOpenChange={(value) => { if (!value) onClose(); }}>
    <SheetContent className="w-full gap-0 overflow-y-auto sm:max-w-lg">
      {entry && <>
        <SheetHeader className="flex-row items-center gap-3 border-b pe-12">
          <PluginIcon pluginId={entry.pluginId} className="size-9" />
          <div className="min-w-0">
            <SheetTitle>{label(entry)}</SheetTitle>
            <SheetDescription>{pluginText.name(entry.pluginId, entry.pluginName)}</SheetDescription>
          </div>
        </SheetHeader>
        <div className="p-4">
          <ActionForm key={`${entry.pluginId}/${entry.action.id}`} entry={entry} workspaceId={workspaceId} context={context} />
        </div>
      </>}
    </SheetContent>
  </Sheet>;
}

/** The actions plugins offer in one place, each opening its form in a side sheet.
 * Renders nothing when no plugin offers an action here. */
export function PlacedPluginActions({ workspaceId, placement, context, className }: {
  workspaceId: number;
  placement: ActionPlacement;
  context?: ActionContext;
  className?: string;
}) {
  const { messages: { pluginActions: copy } } = useLocale();
  const actions = usePlacedActions(workspaceId, placement);
  const label = usePlacedActionLabel();
  const [open, setOpen] = useState<InstalledAction>();

  if (!actions.length) return null;
  return <Card size="sm" className={className}>
    <CardHeader>
      <CardTitle>{copy.placedTitle}</CardTitle>
      <CardDescription>{copy.placedDescription.replace("{place}", copy.place[placement])}</CardDescription>
    </CardHeader>
    <CardContent className="flex flex-wrap gap-2">
      {actions.map((entry) => (
        <Button key={`${entry.pluginId}/${entry.action.id}`} type="button" variant="outline" className="h-auto justify-start gap-2.5 py-1.5 ps-1.5"
          onClick={() => setOpen(entry)}>
          <PluginIcon pluginId={entry.pluginId} className="size-6" />
          <span className="truncate">{label(entry)}</span>
        </Button>
      ))}
    </CardContent>
    <ActionSheet workspaceId={workspaceId} entry={open} context={context} onClose={() => setOpen(undefined)} />
  </Card>;
}
