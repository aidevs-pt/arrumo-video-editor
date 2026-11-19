"use client";

import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";

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
  requestsWorkspaceContext: boolean;
};

type Placement = "settings" | "workspace" | "completed_video";

export function PluginSurfaces({ placement, revision = "", workspaceId }: {
  placement: Placement;
  revision?: string;
  workspaceId?: number;
}) {
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
          const key = `${surface.pluginId}:${surface.id}:${surface.packageSha256}:${workspaceId ?? "app"}`;
          return <AccordionItem key={key} value={key}>
            <AccordionTrigger className="wrap-anywhere">
              <span>{surface.title}<span className="block text-xs text-muted-foreground">{surface.pluginName}</span></span>
            </AccordionTrigger>
            <AccordionContent><PluginDocument surface={surface} workspaceId={workspaceId} /></AccordionContent>
          </AccordionItem>;
        })}
      </Accordion>
    </CardContent>
  </Card>;
}

function PluginDocument({ surface, workspaceId }: { surface: Surface; workspaceId?: number }) {
  const { messages: { pluginSurfaces: copy } } = useLocale();
  const [loaded, setLoaded] = useState(false);
  const [bridgeReady, setBridgeReady] = useState(false);
  const [nonce] = useState(() => crypto.randomUUID());
  const frame = useRef<HTMLIFrameElement>(null);
  const handled = useRef(false);
  const bridge = surface.requestsWorkspaceContext && workspaceId !== undefined;
  const { pluginId, id: surfaceId, packageSha256 } = surface;
  const address = convertFileSrc(`${pluginId}.${surfaceId}.${packageSha256}`, "arrumo-panel");

  useEffect(() => {
    if (!bridge) return;
    let active = true;
    function handle(event: MessageEvent) {
      if (handled.current || event.source !== frame.current?.contentWindow || event.origin !== "null") return;
      const { data, ports } = event;
      if (data?.type !== "arrumo:workspace-context" || data.nonce !== nonce || ports.length !== 1) return;
      handled.current = true;
      const [port] = ports;
      void invoke<string | null>("read_plugin_surface_context", { pluginId, surfaceId, packageSha256, workspaceId }).then(
        (context) => { if (active) port.postMessage({ ok: true, context }); },
        () => { if (active) port.postMessage({ ok: false, error: "context_unavailable" }); },
      ).finally(() => port.close());
    }
    window.addEventListener("message", handle);
    setBridgeReady(true);
    return () => { active = false; window.removeEventListener("message", handle); };
  }, [bridge, nonce, packageSha256, pluginId, surfaceId, workspaceId]);

  return (
    <div>
      {!loaded && <Skeleton className="h-24" aria-label={copy.loading} />}
      {(!bridge || bridgeReady) && <iframe ref={frame} title={surface.title} sandbox="allow-scripts" referrerPolicy="no-referrer"
        src={bridge ? `${address}#${nonce}` : address} className="h-72 w-full rounded-md border bg-background"
        aria-busy={!loaded} onLoad={() => setLoaded(true)} />}
    </div>
  );
}
