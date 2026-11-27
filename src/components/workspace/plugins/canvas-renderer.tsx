"use client";

import { invoke } from "@tauri-apps/api/core";
import { Download, LoaderCircle, ShieldCheck, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";

type CanvasStatus = {
  pluginId: string | null;
  supported: boolean;
  downloadBytes: number | null;
  installed: boolean;
  sandbox: "browser" | "bubblewrap" | null;
  ready: boolean;
  problem: string | null;
  libraries: { id: string; version: string; license: string }[];
};

/** The headless browser a canvas renderer plugin draws pages with: download it, see how pages
 * are isolated, or remove it. */
export function CanvasRenderer({ pluginId }: { pluginId: string }) {
  const { locale, messages: { canvasRenderer: copy } } = useLocale();
  const [status, setStatus] = useState<CanvasStatus>();
  const [busy, setBusy] = useState<"install" | "remove">();
  const [error, setError] = useState<string>();

  useEffect(() => {
    let active = true;
    invoke<CanvasStatus>("get_canvas_renderer").then((next) => { if (active) setStatus(next); }, () => {});
    return () => { active = false; };
  }, []);

  if (!status) return <Skeleton className="h-28" />;
  if (status.pluginId !== pluginId) return null;

  async function change(action: "install" | "remove") {
    setBusy(action);
    setError(undefined);
    try {
      setStatus(await invoke<CanvasStatus>(action === "install" ? "install_canvas_renderer" : "remove_canvas_renderer"));
    } catch {
      setError(action === "install" ? copy.failed : copy.removeFailed);
    } finally {
      setBusy(undefined);
    }
  }

  const size = new Intl.NumberFormat(locale, { style: "unit", unit: "megabyte", maximumFractionDigits: 0 })
    .format((status.downloadBytes ?? 0) / 1_000_000);

  return (
    <section className="flex flex-col gap-4 rounded-xl border bg-card p-5">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="flex max-w-2xl flex-col gap-1">
          <h2 className="text-[1.35rem]">{copy.title}</h2>
          {!status.supported ? <p className="text-sm text-muted-foreground">{copy.unsupported}</p>
            : status.ready ? <p className="flex items-start gap-2 text-sm text-muted-foreground">
              <ShieldCheck className="mt-0.5 size-4 shrink-0 text-primary" aria-hidden />
              <span>{copy.ready} {status.sandbox === "bubblewrap" ? copy.sandboxBubblewrap : copy.sandboxBrowser}</span>
            </p>
            : status.installed && status.problem ? <p className="text-sm text-muted-foreground">{copy.needsBubblewrap}</p>
            : <p className="text-sm text-muted-foreground">{copy.missing.replace("{size}", size)}</p>}
        </div>
        {status.supported && (status.installed
          ? <Button variant="outline" disabled={busy !== undefined} onClick={() => change("remove")}>
            {busy === "remove" ? <LoaderCircle data-icon="inline-start" className="animate-spin" /> : <Trash2 data-icon="inline-start" />}
            {copy.remove}
          </Button>
          : <Button disabled={busy !== undefined} onClick={() => change("install")}>
            {busy === "install" ? <LoaderCircle data-icon="inline-start" className="animate-spin" /> : <Download data-icon="inline-start" />}
            {busy === "install" ? copy.downloading : copy.download}
          </Button>)}
      </div>
      {error && <Alert variant="destructive"><AlertDescription>{error}</AlertDescription></Alert>}
      {status.libraries.length > 0 && <div className="flex flex-wrap items-center gap-2 text-sm">
        <span className="font-semibold">{copy.libraries}</span>
        {status.libraries.map((library) => <Badge key={library.id} variant="secondary">
          {library.id} {library.version} · {library.license}
        </Badge>)}
      </div>}
    </section>
  );
}
