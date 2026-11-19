"use client";

import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";

type Surface = {
  id: string;
  title: string;
  pluginId: string;
  pluginName: string;
  packageSha256: string;
};

type Placement = "settings" | "workspace" | "completed_video";

export function PluginSurfaces({ placement, revision = "" }: { placement: Placement; revision?: string }) {
  const [surfaces, setSurfaces] = useState<Surface[]>([]);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const { messages: { pluginSurfaces: copy, workspaces } } = useLocale();

  useEffect(() => {
    if (!isTauri()) return;
    let active = true;
    invoke<Surface[]>("list_plugin_surfaces", { placement }).then(
      (items) => { if (active) { setSurfaces(items); setFailed(false); } },
      () => { if (active) { setSurfaces([]); setFailed(true); } },
    );
    return () => { active = false; };
  }, [placement, revision, attempt]);

  if (failed) return <Alert variant="destructive">
    <AlertDescription>{copy.listError}</AlertDescription>
    <Button variant="outline" onClick={() => setAttempt((value) => value + 1)}>{workspaces.retry}</Button>
  </Alert>;
  if (!surfaces.length) return null;

  return <Card>
    <CardHeader><CardTitle>{copy.title}</CardTitle></CardHeader>
    <CardContent>
      <Accordion type="single" collapsible>
        {surfaces.map((surface) => {
          const key = `${surface.pluginId}:${surface.id}:${surface.packageSha256}`;
          return <AccordionItem key={key} value={key}>
            <AccordionTrigger className="wrap-anywhere">
              <span>{surface.title}<span className="block text-xs text-muted-foreground">{surface.pluginName}</span></span>
            </AccordionTrigger>
            <AccordionContent><PluginDocument surface={surface} /></AccordionContent>
          </AccordionItem>;
        })}
      </Accordion>
    </CardContent>
  </Card>;
}

function PluginDocument({ surface }: { surface: Surface }) {
  const { messages: { pluginSurfaces: copy } } = useLocale();
  const [loaded, setLoaded] = useState(false);
  const address = convertFileSrc(`${surface.pluginId}.${surface.id}.${surface.packageSha256}`, "arrumo-panel");
  return (
    <div>
      {!loaded && <Skeleton className="h-24" aria-label={copy.loading} />}
      <iframe title={surface.title} sandbox="allow-scripts" referrerPolicy="no-referrer"
        src={address} className="h-72 w-full rounded-md border bg-background"
        aria-busy={!loaded} onLoad={() => setLoaded(true)} />
    </div>
  );
}
