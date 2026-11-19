"use client";

import { invoke } from "@tauri-apps/api/core";
import { FileArchive, PackagePlus } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";
import { PluginRegistryBrowser } from "./plugin-registry-browser";
import { PluginPreferences, type PluginSettingsManifest } from "./plugin-preferences";

type Plugin = {
  bundled: boolean;
  manifest: PluginSettingsManifest & { description: string };
  package?: { sha256: string; release?: { version: string } };
};

export function PluginCatalog() {
  const { messages: { pluginCatalog: copy, workspaces: workspaceCopy } } = useLocale();
  const [plugins, setPlugins] = useState<Plugin[]>();
  const [error, setError] = useState<"loadError" | "importError" | "removeError">();
  const [notice, setNotice] = useState<"imported" | "removed">();
  const [pending, setPending] = useState(false);

  const load = useCallback(async () => {
    setError(undefined);
    try {
      setPlugins(await invoke<Plugin[]>("list_plugin_catalog"));
    } catch {
      setError("loadError");
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  async function importPackage() {
    setPending(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const plugin = await invoke<Plugin | null>("import_plugin_package", { pickerTitle: copy.chooseFile });
      if (plugin) {
        await load();
        setNotice("imported");
      }
    } catch {
      setError("importError");
    } finally {
      setPending(false);
    }
  }

  async function removePackage(id: string) {
    setPending(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const removed = await invoke<boolean>("remove_plugin_package", { id });
      await load();
      if (removed) setNotice("removed");
    } catch {
      setError("removeError");
    } finally {
      setPending(false);
    }
  }

  return (
    <>
      {plugins && <PluginPreferences manifests={plugins.map(({ manifest }) => manifest)} workspaceId={null} />}
      <Card>
        <CardHeader>
          <CardTitle>{copy.title}</CardTitle>
          <CardDescription>{copy.description}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="outline" className="self-start" disabled={pending || !plugins}>
                <PackagePlus data-icon="inline-start" />
                {copy.import}
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>{copy.import}</AlertDialogTitle>
                <AlertDialogDescription>{copy.warning}</AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>{copy.cancel}</AlertDialogCancel>
                <AlertDialogAction onClick={() => void importPackage()}>{copy.chooseFile}</AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
          {error && (
            <Alert variant="destructive">
              <AlertDescription>{copy[error]}</AlertDescription>
              {error === "loadError" && <Button variant="outline" onClick={() => void load()}>{workspaceCopy.retry}</Button>}
            </Alert>
          )}
          <div role="status" className={pending || notice ? "text-sm text-muted-foreground" : "sr-only"}>
            {pending ? copy.working : notice && copy[notice]}
          </div>
          {!plugins && !error && <Skeleton className="h-24" aria-label={copy.loading} />}
          {plugins && (
            <ul className="divide-y">
              {plugins.map(({ bundled, manifest, package: stored }) => (
                <li key={manifest.id} className="flex flex-wrap items-start gap-3 py-3 first:pt-0 last:pb-0">
                  <FileArchive aria-hidden="true" className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
                  <div className="min-w-32 flex-1 wrap-anywhere">
                    <h3 className="text-sm font-medium">{manifest.name}</h3>
                    <p className="text-sm text-muted-foreground">{manifest.description}</p>
                    {stored?.release && <p className="text-xs text-muted-foreground">{stored.release.version}</p>}
                  </div>
                  {bundled ? <Badge variant="secondary">{copy.bundled}</Badge> : (
                    <AlertDialog>
                      <AlertDialogTrigger asChild>
                        <Button variant="ghost" size="sm" disabled={pending}
                          aria-label={copy.removeNamed.replace("{name}", () => manifest.name)}>{copy.remove}</Button>
                      </AlertDialogTrigger>
                      <AlertDialogContent>
                        <AlertDialogHeader>
                          <AlertDialogTitle className="wrap-anywhere">{copy.removeNamed.replace("{name}", () => manifest.name)}</AlertDialogTitle>
                          <AlertDialogDescription>{copy.removeDescription}</AlertDialogDescription>
                        </AlertDialogHeader>
                        <AlertDialogFooter>
                          <AlertDialogCancel>{copy.cancel}</AlertDialogCancel>
                          <AlertDialogAction variant="destructive" onClick={() => void removePackage(manifest.id)}>
                            {copy.remove}
                          </AlertDialogAction>
                        </AlertDialogFooter>
                      </AlertDialogContent>
                    </AlertDialog>
                  )}
                </li>
              ))}
            </ul>
          )}
          {plugins && <PluginRegistryBrowser installedIds={plugins.map(({ manifest }) => manifest.id)} onInstalled={load} />}
        </CardContent>
      </Card>
    </>
  );
}
