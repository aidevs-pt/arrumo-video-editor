"use client";

import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Download, RotateCcw, ShieldCheck, Upload } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { useLocale } from "@/lib/locale-provider";

type Summary = { appVersion: string; createdAt: number; workspaces: string[]; plugins: number; files: number; bytes: number };
type ImportError = "backup_not_arrumo" | "backup_damaged" | "backup_newer" | "backup_import_failed";

function size(bytes: number, locale: string) {
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
  return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value)} ${units[unit]}`;
}

/** Full backups: export everything to one ZIP, or replace everything here with one. */
export function BackupSettings() {
  const { locale, messages: { backup: copy } } = useLocale();
  const [exporting, setExporting] = useState(false);
  const [exported, setExported] = useState<Summary>();
  const [exportFailed, setExportFailed] = useState(false);
  const [checking, setChecking] = useState(false);
  const [staged, setStaged] = useState<Summary | "earlier">();
  const [importError, setImportError] = useState<ImportError>();
  const [archiveName, setArchiveName] = useState("");
  const [current, setCurrent] = useState<number>();
  const restoring = useRef(false);

  useEffect(() => {
    let active = true;
    invoke<boolean>("pending_backup_import").then((pending) => { if (active && pending) setStaged("earlier"); }, () => {});
    return () => { active = false; };
  }, []);

  async function exportBackup() {
    const day = new Date().toISOString().slice(0, 10);
    const destination = await save({ defaultPath: `arrumo-backup-${day}.zip`, filters: [{ name: "Arrumo backup", extensions: ["zip"] }] });
    if (!destination) return;
    setExporting(true);
    setExported(undefined);
    setExportFailed(false);
    try {
      setExported(await invoke<Summary>("export_backup", { destination }));
    } catch {
      setExportFailed(true);
    } finally {
      setExporting(false);
    }
  }

  async function chooseBackup() {
    const archive = await open({ multiple: false, directory: false, filters: [{ name: "Arrumo backup", extensions: ["zip"] }] });
    if (typeof archive !== "string") return;
    setChecking(true);
    setImportError(undefined);
    setArchiveName(archive.split(/[\\/]/).pop() ?? archive);
    invoke<unknown[]>("list_workspaces").then((list) => setCurrent(list.length), () => setCurrent(undefined));
    try {
      setStaged(await invoke<Summary>("stage_backup_import", { archive }));
    } catch (error) {
      setImportError(typeof error === "string" && error in copy.errors ? error as ImportError : "backup_import_failed");
    } finally {
      setChecking(false);
    }
  }

  async function cancel() {
    await invoke("cancel_backup_import").catch(() => {});
    setStaged(undefined);
  }

  const made = staged && staged !== "earlier"
    ? new Intl.DateTimeFormat(locale, { dateStyle: "long", timeStyle: "short" }).format(new Date(staged.createdAt * 1000))
    : "";

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <section aria-labelledby="backup-export" className="flex flex-col gap-3 rounded-xl border bg-card p-5">
        <h2 id="backup-export" className="font-semibold">{copy.exportTitle}</h2>
        <p className="text-sm text-muted-foreground">{copy.exportBody}</p>
        <p className="text-sm text-muted-foreground">{copy.checksum}</p>
        <Button variant="outline" className="mt-auto" disabled={exporting} onClick={() => void exportBackup()}>
          <Download aria-hidden="true" />{exporting ? copy.exporting : copy.export}
        </Button>
        {exported && <p role="status" className="text-sm">
          {copy.exported.replace("{files}", String(exported.files)).replace("{size}", size(exported.bytes, locale))}
        </p>}
        {exportFailed && <Alert variant="destructive"><AlertDescription>{copy.exportError}</AlertDescription></Alert>}
      </section>

      <section aria-labelledby="backup-import" className="flex flex-col gap-3 rounded-xl border border-destructive/40 bg-card p-5">
        <h2 id="backup-import" className="font-semibold">{copy.importTitle}</h2>
        <p className="text-sm text-muted-foreground">{copy.importBody}</p>
        <Button variant="outline" className="mt-auto" disabled={checking} onClick={() => void chooseBackup()}>
          <Upload aria-hidden="true" />{checking ? copy.checking : copy.choose}
        </Button>
        {importError && <Alert variant="destructive"><AlertDescription>{copy.errors[importError]}</AlertDescription></Alert>}
        {staged === "earlier" && <Alert>
          <AlertDescription>{copy.stagedEarlier}</AlertDescription>
          <div className="mt-2 flex flex-wrap gap-2">
            <Button size="sm" variant="destructive" onClick={() => void invoke("restart_to_restore")}><RotateCcw aria-hidden="true" />{copy.restart}</Button>
            <Button size="sm" variant="outline" onClick={() => void cancel()}>{copy.cancel}</Button>
          </div>
        </Alert>}
      </section>

      <AlertDialog open={staged !== undefined && staged !== "earlier"} onOpenChange={(openDialog) => { if (!openDialog && !restoring.current) void cancel(); }}>
        <AlertDialogContent className="max-w-[calc(100%-2rem)]! sm:max-w-xl! sm:p-6">
          <AlertDialogHeader>
            <AlertDialogTitle className="font-heading text-3xl leading-none font-[850] [font-variation-settings:'wdth'_120]">{copy.confirmTitle}</AlertDialogTitle>
            <AlertDialogDescription className="sr-only">{copy.replaceWarning}</AlertDialogDescription>
          </AlertDialogHeader>
          {staged && staged !== "earlier" && <>
            <div className="flex items-start gap-3 rounded-lg bg-secondary p-4 text-sm">
              <ShieldCheck className="mt-0.5 size-5 shrink-0 text-approval" aria-hidden="true" />
              <p><strong>{copy.checksumMatches}</strong> {copy.unchanged.replace("{file}", archiveName)}</p>
            </div>
            <dl className="grid grid-cols-[1fr_auto] gap-x-6 gap-y-2.5 text-sm">
              <dt className="text-muted-foreground">{copy.rowWorkspaces}</dt>
              <dd className="text-end font-semibold">{(current === undefined ? copy.workspacesValue : copy.workspacesReplacing)
                .replace("{count}", String(staged.workspaces.length)).replace("{current}", String(current ?? 0))}</dd>
              <dt className="text-muted-foreground">{copy.rowPlugins}</dt>
              <dd className="text-end font-semibold">{staged.plugins ? copy.pluginsValue.replace("{count}", String(staged.plugins)) : copy.pluginsUnknown}</dd>
              <dt className="text-muted-foreground">{copy.rowFiles}</dt>
              <dd className="text-end font-semibold">{size(staged.bytes, locale)}</dd>
              <dt className="text-muted-foreground">{copy.rowMade}</dt>
              <dd className="text-end font-semibold">{copy.madeValue.replace("{date}", made).replace("{version}", staged.appVersion)}</dd>
              <dt className="text-muted-foreground">{copy.rowKeys}</dt>
              <dd className="text-end font-semibold">{copy.keysValue}</dd>
            </dl>
            <p className="text-sm font-semibold text-destructive">{copy.replaceWarning}</p>
          </>}
          <AlertDialogFooter className="items-center sm:-mx-6 sm:-mb-6">
            <AlertDialogCancel variant="ghost">{copy.cancel}</AlertDialogCancel>
            <Button variant="outline" disabled={exporting} onClick={() => void exportBackup()}>
              <Download data-icon="inline-start" />{exporting ? copy.exporting : copy.exportFirst}
            </Button>
            <AlertDialogAction variant="destructive" className="bg-destructive! text-white! hover:bg-destructive/90!" onClick={() => { restoring.current = true; void invoke("restart_to_restore"); }}>{copy.restart}</AlertDialogAction>
          </AlertDialogFooter>
          {exported && <p role="status" className="text-sm">{copy.exported.replace("{files}", String(exported.files)).replace("{size}", size(exported.bytes, locale))}</p>}
          {exportFailed && <Alert variant="destructive"><AlertDescription>{copy.exportError}</AlertDescription></Alert>}
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
