"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { useLocale } from "@/lib/locale-provider";
import { PLUGINS_CHANGED, PluginIcon, usePluginText } from "@/lib/plugin-text";
import { cn } from "@/lib/utils";

import { ActionForm, type ActionContext, type ActionPlacement, type InstalledAction } from "./action-form";

/** The installed plugin actions declared for one place in the app, each opening its form in a side sheet.
 * Renders nothing when no plugin offers an action here. */
export function PlacedPluginActions({ workspaceId, placement, context, className }: {
  workspaceId: number;
  placement: ActionPlacement;
  context?: ActionContext;
  className?: string;
}) {
  const pluginText = usePluginText();
  const { messages: { pluginActions: copy } } = useLocale();
  const [actions, setActions] = useState<InstalledAction[]>([]);
  const [open, setOpen] = useState<InstalledAction>();
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

  if (!actions.length) return null;
  const label = (entry: InstalledAction) => pluginText.action(entry.pluginId, entry.action.id, "label", entry.action.label);

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
    <Sheet open={!!open} onOpenChange={(value) => { if (!value) setOpen(undefined); }}>
      <SheetContent className={cn("w-full gap-0 overflow-y-auto sm:max-w-lg")}>
        {open && <>
          <SheetHeader className="flex-row items-center gap-3 border-b pe-12">
            <PluginIcon pluginId={open.pluginId} className="size-9" />
            <div className="min-w-0">
              <SheetTitle>{label(open)}</SheetTitle>
              <SheetDescription>{pluginText.name(open.pluginId, open.pluginName)}</SheetDescription>
            </div>
          </SheetHeader>
          <div className="p-4">
            <ActionForm key={`${open.pluginId}/${open.action.id}`} entry={open} workspaceId={workspaceId} context={context} />
          </div>
        </>}
      </SheetContent>
    </Sheet>
  </Card>;
}
