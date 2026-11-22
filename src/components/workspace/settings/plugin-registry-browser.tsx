"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, ChevronDown } from "lucide-react";
import { useState, type FormEvent } from "react";

import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";

type Release = {
  version: string;
  manifestSchemaVersion: number;
  packageUrl: string;
  sha256: string;
};
type RegistryPlugin = { id: string; repository: string; releases: Release[] };
type Listing = {
  registry: { plugins: RegistryPlugin[] };
  supportedManifestSchemaVersions: number[];
  updateVersions: Record<string, string[]>;
};

export function PluginRegistryBrowser({
  installedIds,
  onInstalled,
}: {
  installedIds: string[];
  onInstalled: () => Promise<void>;
}) {
  const { messages: { pluginRegistry: copy, pluginCatalog } } = useLocale();
  const [url, setUrl] = useState("");
  const [listing, setListing] = useState<(Listing & { url: string })>();
  const [pending, setPending] = useState<"loading" | "installing">();
  const [error, setError] = useState<"loadError" | "installError" | "sourceError">();
  const [success, setSuccess] = useState<"installed" | "updated">();

  async function browse(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const catalogUrl = url.trim();
    setListing(undefined);
    setError(undefined);
    setSuccess(undefined);
    setPending("loading");
    try {
      const index = await invoke<Listing>("browse_plugin_registry", { catalogUrl });
      setListing({ ...index, url: catalogUrl });
    } catch {
      setError("loadError");
    } finally {
      setPending(undefined);
    }
  }

  async function install(plugin: RegistryPlugin, release: Release, catalogUrl: string, update: boolean) {
    setError(undefined);
    setSuccess(undefined);
    setPending("installing");
    try {
      await invoke("install_registry_plugin", { catalogUrl, id: plugin.id, repository: plugin.repository, release, update });
      await onInstalled();
      if (update) setListing(undefined);
      setSuccess(update ? "updated" : "installed");
    } catch {
      setError("installError");
    } finally {
      setPending(undefined);
    }
  }

  async function openRepository(repository: string) {
    setError(undefined);
    try {
      await invoke("open_plugin_repository", { url: repository });
    } catch {
      setError("sourceError");
    }
  }

  return (
    <>
      <Separator />
      <Collapsible>
        <CollapsibleTrigger asChild>
          <Button variant="outline">
            {copy.browse}
            <ChevronDown data-icon="inline-end" />
          </Button>
        </CollapsibleTrigger>
        <CollapsibleContent className="flex flex-col gap-4 data-closed:hidden">
          <form className="mt-3 flex flex-col items-start gap-2 sm:flex-row sm:items-end" onSubmit={browse}>
            <Field>
              <FieldLabel htmlFor="plugin-registry-url">{copy.url}</FieldLabel>
              <Input id="plugin-registry-url" type="url" value={url} maxLength={2048} required
                onChange={(event) => { setUrl(event.target.value); setListing(undefined); setError(undefined); setSuccess(undefined); }}
                disabled={Boolean(pending)} />
            </Field>
            <Button type="submit" disabled={Boolean(pending)}>{copy.load}</Button>
          </form>
          {error && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
          <div role="status" className={pending || success ? "text-sm text-muted-foreground" : "sr-only"}>
            {pending ? copy[pending] : success === "updated" ? copy.updated : success === "installed" && pluginCatalog.imported}
          </div>
          {pending === "loading" && <Skeleton className="h-16" />}
          {listing && (listing.registry.plugins.length === 0 ? (
            <p className="text-sm text-muted-foreground">{copy.empty}</p>
          ) : (
            <Accordion type="single" collapsible>
              {listing.registry.plugins.map((plugin) => {
                const update = installedIds.includes(plugin.id);
                const versions = listing.updateVersions[plugin.id] ?? [];
                const releases = update
                  ? plugin.releases.filter((release) => versions.includes(release.version))
                  : plugin.releases;
                const action = update ? copy.update : copy.install;
                return (
                  <AccordionItem key={plugin.id} value={plugin.id}>
                    <AccordionTrigger className="wrap-anywhere">{plugin.id}</AccordionTrigger>
                    <AccordionContent>
                      <Button variant="link" size="sm" className="mb-1 px-0" onClick={() => void openRepository(plugin.repository)}>
                        {copy.source}<ArrowUpRight data-icon="inline-end" />
                      </Button>
                      {update && releases.length === 0 ? <p className="text-sm text-muted-foreground">{copy.installed}</p>
                        : releases.length === 0 ? <p className="text-sm text-muted-foreground">{copy.noReleases}</p>
                        : (
                          <ul className="divide-y">
                            {releases.map((release) => (
                              <li key={release.version} className="flex flex-wrap items-center justify-between gap-2 py-2">
                                <span className="min-w-0 wrap-anywhere">{release.version}</span>
                                {!listing.supportedManifestSchemaVersions.includes(release.manifestSchemaVersion)
                                  ? <span className="text-xs text-muted-foreground">{copy.incompatible}</span>
                                  : (
                                    <AlertDialog>
                                      <AlertDialogTrigger asChild>
                                        <Button variant="outline" size="sm" disabled={Boolean(pending)}>{action}</Button>
                                      </AlertDialogTrigger>
                                      <AlertDialogContent>
                                        <AlertDialogHeader>
                                          <AlertDialogTitle className="wrap-anywhere">{(update ? copy.confirmUpdate : copy.confirmInstall).replace("{name}", plugin.id).replace("{version}", release.version)}</AlertDialogTitle>
                                          <AlertDialogDescription>{pluginCatalog.warning}</AlertDialogDescription>
                                        </AlertDialogHeader>
                                        <dl className="grid gap-1 rounded-lg border bg-secondary/50 p-3 text-sm">
                                          <dt className="font-semibold">{copy.repository}</dt>
                                          <dd className="flex min-w-0 items-center gap-2">
                                            <span className="min-w-0 truncate text-muted-foreground" title={plugin.repository}>{plugin.repository}</span>
                                            <Button type="button" variant="link" size="sm" className="h-auto shrink-0 px-0" onClick={() => void openRepository(plugin.repository)}>
                                              {copy.source}<ArrowUpRight data-icon="inline-end" />
                                            </Button>
                                          </dd>
                                        </dl>
                                        <p className="text-sm text-muted-foreground">{copy.tourNote}</p>
                                        <AlertDialogFooter>
                                          <AlertDialogCancel>{pluginCatalog.cancel}</AlertDialogCancel>
                                          <AlertDialogAction onClick={() => void install(plugin, release, listing.url, update)}>
                                            {copy.checkedInstall}
                                          </AlertDialogAction>
                                        </AlertDialogFooter>
                                      </AlertDialogContent>
                                    </AlertDialog>
                                  )}
                              </li>
                            ))}
                          </ul>
                        )}
                    </AccordionContent>
                  </AccordionItem>
                );
              })}
            </Accordion>
          ))}
        </CollapsibleContent>
      </Collapsible>
    </>
  );
}
