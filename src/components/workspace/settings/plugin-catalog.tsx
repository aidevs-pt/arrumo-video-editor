"use client";

import { PluginIcon, usePluginText } from "@/lib/plugin-text";
import { invoke } from "@tauri-apps/api/core";
import { GitBranch, PackagePlus, Sparkles } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import {
  Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { PluginActions } from "@/components/workspace/plugins/plugin-actions";
import { PluginSurfaces } from "@/components/workspace/plugins/plugin-surfaces";
import { announcePluginChanges, replayPluginTour } from "@/components/workspace/plugins/plugin-tour";
import { useLocale } from "@/lib/locale-provider";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { PluginRegistryBrowser } from "./plugin-registry-browser";
import type { PluginSettingsManifest } from "./plugin-preferences";

type Plugin = {
  bundled: boolean;
  active: boolean;
  manifest: PluginSettingsManifest & { description: string };
  package?: { sha256: string; release?: { version: string } };
};

export function PluginCatalog() {
  const { messages: { pluginCatalog: copy, pluginDetail: detailCopy, workspaces: workspaceCopy, pluginTour: tourCopy } } = useLocale();
  const pluginText = usePluginText();
  const query = useSearchParams();
  const detailHref = (id: string) => {
    const params = new URLSearchParams(query.toString());
    params.set("section", "plugins");
    params.set("plugin", id);
    return `/settings?${params}`;
  };
  const [plugins, setPlugins] = useState<Plugin[]>();
  const [error, setError] = useState<"loadError" | "importError" | "removeError">();
  const [notice, setNotice] = useState<"imported" | "removed">();
  const [repository, setRepository] = useState("");
  const [repositoryOpen, setRepositoryOpen] = useState(false);
  const [repositoryError, setRepositoryError] = useState<string>();
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
        announcePluginChanges();
      }
    } catch {
      setError("importError");
    } finally {
      setPending(false);
    }
  }

  async function installRepository() {
    setPending(true);
    setRepositoryError(undefined);
    setNotice(undefined);
    try {
      await invoke<Plugin>("install_plugin_repository", { repository: repository.trim() });
      await load();
      setNotice("imported");
      setRepositoryOpen(false);
      setRepository("");
      announcePluginChanges();
    } catch (reason) {
      setRepositoryError(typeof reason === "string" && reason ? reason : copy.importError);
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
      <Card>
        <CardHeader>
          <CardTitle>{copy.title}</CardTitle>
          <CardDescription>{copy.description}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="flex flex-wrap gap-2">
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="outline" disabled={pending || !plugins}>
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
          <Dialog open={repositoryOpen} onOpenChange={(next) => { if (!pending) { setRepositoryOpen(next); setRepositoryError(undefined); } }}>
            <DialogTrigger asChild>
              <Button variant="outline" disabled={pending || !plugins}>
                <GitBranch data-icon="inline-start" />
                {copy.fromRepository}
              </Button>
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>{copy.fromRepository}</DialogTitle>
                <DialogDescription>{copy.warning}</DialogDescription>
              </DialogHeader>
              <form className="flex flex-col gap-4" onSubmit={(event) => { event.preventDefault(); void installRepository(); }}>
                <Field>
                  <FieldLabel htmlFor="plugin-repository">{copy.repositoryLabel}</FieldLabel>
                  <Input id="plugin-repository" value={repository} disabled={pending} maxLength={300} inputMode="url"
                    placeholder="https://github.com/owner/plugin" onChange={(event) => setRepository(event.target.value)} />
                  <FieldDescription>{copy.repositoryHint}</FieldDescription>
                </Field>
                {repositoryError && <Alert variant="destructive"><AlertDescription>{repositoryError}</AlertDescription></Alert>}
                <DialogFooter>
                  <DialogClose asChild><Button type="button" variant="outline" disabled={pending}>{copy.cancel}</Button></DialogClose>
                  <Button type="submit" disabled={pending || !repository.trim().startsWith("https://github.com/")}>
                    {pending ? copy.working : copy.installRepository}
                  </Button>
                </DialogFooter>
              </form>
            </DialogContent>
          </Dialog>
          <Button variant="ghost" onClick={() => replayPluginTour()} disabled={!plugins}>
            <Sparkles data-icon="inline-start" />
            {tourCopy.replay}
          </Button>
          </div>
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
              {plugins.map(({ bundled, active, manifest: declared, package: stored }) => {
                const manifest = { ...declared, name: pluginText.name(declared.id, declared.name), description: pluginText.description(declared.id, declared.description) };
                return (
                <li key={manifest.id} className="flex flex-wrap items-start gap-3 py-3 first:pt-0 last:pb-0">
                  <PluginIcon pluginId={manifest.id} className="size-9" />
                  <div className="min-w-32 flex-1 wrap-anywhere">
                    <h3 className="text-sm font-semibold"><Link className="hover:underline" href={detailHref(manifest.id)}>{manifest.name}</Link></h3>
                    <p className="text-sm text-muted-foreground">{manifest.description}</p>
                    {stored?.release && <p className="text-xs text-muted-foreground">{stored.release.version}</p>}
                    {pluginText.iconNotice(manifest.id) && <p className="mt-1 text-xs text-muted-foreground">{pluginText.iconNotice(manifest.id)}</p>}
                  </div>
                  {!active && <Badge variant="outline">{detailCopy.off}</Badge>}
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
                );
              })}
            </ul>
          )}
          {plugins && <PluginRegistryBrowser installedIds={plugins.map(({ manifest }) => manifest.id)} onInstalled={async () => { await load(); announcePluginChanges(); }} />}
        </CardContent>
      </Card>
      {plugins && <PluginActions workspaceId={null}
        revision={plugins.map((plugin) => plugin.package?.sha256 ?? "").join(":")} />}
      {plugins && <PluginSurfaces placement="settings"
        revision={plugins.map((plugin) => plugin.package?.sha256 ?? "").join(":")} />}
    </>
  );
}
