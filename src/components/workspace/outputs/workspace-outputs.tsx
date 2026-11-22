"use client";

import { ChevronDown, Lock, Monitor, MoreHorizontal, Plus, RectangleHorizontal, Smartphone, Square } from "lucide-react";
import { WorkspaceSettingsHeading } from "@/components/workspace/settings/workspace-settings-heading";
import { useLocale } from "@/lib/locale-provider";
import { WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { FormatDelivery } from "./format-delivery";

type OutputFormat = { id: number; name: string; width: number; height: number; enabled: boolean };

function shapeIcon(format: OutputFormat) {
  return format.width > format.height ? RectangleHorizontal : format.width < format.height ? Smartphone : Square;
}

export function WorkspaceOutputs({ workspace }: { workspace: { id: number; name: string } }) {
  const [formats, setFormats] = useState<OutputFormat[]>([]);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [attempt, setAttempt] = useState(0);
  // `null` closes the format dialog, "new" adds one, a format edits it.
  const [editing, setEditing] = useState<OutputFormat | "new" | null>(null);
  const [removing, setRemoving] = useState<OutputFormat | null>(null);
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const [pending, setPending] = useState(false);
  const [failure, setFailure] = useState<"save" | "remove" | "toggle" | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const { messages } = useLocale();
  const copy = messages.outputs;

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    invoke<OutputFormat[]>("list_output_formats", { workspaceId: workspace.id }).then(
      (rows) => {
        if (!active) return;
        setFormats(rows);
        setExpanded((current) => current.size ? current : new Set(rows.filter((row) => row.enabled).slice(0, 1).map((row) => row.id)));
        setStatus("ready");
      },
      () => { if (active) setStatus("error"); },
    );
    return () => { active = false; };
  }, [workspace.id, attempt]);

  async function write(format: OutputFormat | null, settings: Omit<OutputFormat, "id">) {
    const saved = await invoke<OutputFormat>(format ? "update_output_format" : "create_output_format", {
      workspaceId: workspace.id, ...(format && { formatId: format.id }), settings,
    });
    setFormats((current) => format ? current.map((row) => row.id === saved.id ? saved : row) : [...current, saved]);
    return saved;
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending || editing === null) return;
    const fields = new FormData(event.currentTarget);
    const name = String(fields.get("name") ?? "").trim();
    if (!name) return;
    const format = editing === "new" ? null : editing;
    setPending(true);
    setFailure(null);
    try {
      const saved = await write(format, {
        name, width: Number(fields.get("width")), height: Number(fields.get("height")), enabled: format?.enabled ?? true,
      });
      if (!format) setExpanded((current) => new Set(current).add(saved.id));
      setEditing(null);
    } catch {
      setFailure("save");
    } finally {
      setPending(false);
    }
  }

  async function toggle(format: OutputFormat, enabled: boolean) {
    if (pending) return;
    setPending(true);
    setFailure(null);
    try {
      await write(format, { name: format.name, width: format.width, height: format.height, enabled });
    } catch {
      setFailure("toggle");
    } finally {
      setPending(false);
    }
  }

  async function remove() {
    if (pending || !removing) return;
    setPending(true);
    setFailure(null);
    try {
      await invoke<boolean>("remove_output_format", { workspaceId: workspace.id, formatId: removing.id });
      setFormats((current) => current.filter((row) => row.id !== removing.id));
      setRemoving(null);
    } catch {
      setFailure("remove");
    } finally {
      setPending(false);
    }
  }

  const current = editing === "new" ? null : editing;

  return (
    <WorkspaceShell>
      <WorkspaceSettingsHeading workspace={workspace} current="outputs" headingRef={heading} />
      <section className="flex min-w-0 flex-col gap-5" aria-label={copy.list}>
        <p className="flex items-start gap-3 rounded-xl bg-secondary p-4 text-sm">
          <Lock className="mt-0.5 size-4 shrink-0" aria-hidden="true" />{copy.alwaysShips}
        </p>
        {status === "loading" && <Skeleton className="h-32 w-full" role="status" aria-label={copy.loading} />}
        {status === "error" && (
          <Alert variant="destructive">
            <AlertDescription>{copy.loadError}</AlertDescription>
            <Button type="button" variant="outline" onClick={() => { setStatus("loading"); setAttempt((value) => value + 1); }}>{copy.retry}</Button>
          </Alert>
        )}
        {failure === "toggle" && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
        {status === "ready" && formats.length === 0 && (
          <Empty><EmptyHeader><EmptyMedia variant="icon"><Monitor /></EmptyMedia><EmptyTitle>{copy.empty}</EmptyTitle></EmptyHeader></Empty>
        )}
        {status === "ready" && formats.map((format) => {
          const Icon = shapeIcon(format);
          const open = format.enabled && expanded.has(format.id);
          return (
            <Collapsible key={format.id} open={open} className="rounded-xl border bg-card"
              onOpenChange={(next) => setExpanded((value) => { const set = new Set(value); if (next) set.add(format.id); else set.delete(format.id); return set; })}>
              <div className="flex items-center gap-3 p-4 sm:px-5">
                <CollapsibleTrigger disabled={!format.enabled}
                  className="group flex min-w-0 flex-1 items-center gap-3 rounded-md text-start outline-none focus-visible:outline-2 focus-visible:outline-ring disabled:cursor-default">
                  <Icon className="size-5 shrink-0" aria-hidden="true" />
                  <span className="min-w-0">
                    <h2 className="text-[1.35rem] leading-tight wrap-anywhere">{format.name}</h2>
                    <span className="block text-sm text-muted-foreground">
                      {format.enabled ? copy.formatHint.replace("{width}", String(format.width)).replace("{height}", String(format.height)) : copy.formatOff}
                    </span>
                  </span>
                  {format.enabled && <ChevronDown aria-hidden="true" className="ms-1 size-4 shrink-0 text-muted-foreground transition-transform duration-150 group-data-[state=open]:rotate-180 motion-reduce:transition-none" />}
                </CollapsibleTrigger>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button type="button" variant="ghost" size="icon-sm" disabled={pending} aria-label={copy.moreFor.replace("{format}", format.name)}><MoreHorizontal /></Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem onSelect={() => { setFailure(null); setEditing(format); }}>{copy.edit}</DropdownMenuItem>
                    <DropdownMenuItem variant="destructive" onSelect={() => { setFailure(null); setRemoving(format); }}>{copy.remove}</DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
                <Switch checked={format.enabled} disabled={pending} aria-label={copy.makeFormat.replace("{format}", format.name)}
                  onCheckedChange={(checked) => void toggle(format, checked)} />
              </div>
              <CollapsibleContent className="border-t">
                {open && <FormatDelivery workspaceId={workspace.id} format={format} embedded />}
              </CollapsibleContent>
            </Collapsible>
          );
        })}
        {status === "ready" && (
          <button type="button" onClick={() => { setFailure(null); setEditing("new"); }}
            className="flex items-center justify-center gap-2 rounded-xl border border-dashed border-line-strong p-4 text-sm font-semibold text-muted-foreground transition-colors hover:border-primary hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring">
            <Plus className="size-4" aria-hidden="true" />{copy.addFormat}
          </button>
        )}
      </section>

      <Dialog open={editing !== null} onOpenChange={(next) => { if (!next && !pending) setEditing(null); }}>
        <DialogContent className="sm:max-w-md">
          <form key={current?.id ?? "new"} onSubmit={save} aria-busy={pending}>
            <DialogHeader>
              <DialogTitle>{current ? copy.editTitle : copy.create}</DialogTitle>
              <DialogDescription>{copy.formatDialogHint}</DialogDescription>
            </DialogHeader>
            <fieldset disabled={pending} className="my-5">
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="format-name">{copy.name}</FieldLabel>
                  <Input id="format-name" name="name" defaultValue={current?.name} placeholder={copy.namePlaceholder} required autoFocus />
                </Field>
                <div className="grid grid-cols-2 gap-3">
                  <Field>
                    <FieldLabel htmlFor="format-width">{copy.width}</FieldLabel>
                    <Input id="format-width" name="width" type="number" min={1} max={4294967295} defaultValue={current?.width ?? 1080} required />
                  </Field>
                  <Field>
                    <FieldLabel htmlFor="format-height">{copy.height}</FieldLabel>
                    <Input id="format-height" name="height" type="number" min={1} max={4294967295} defaultValue={current?.height ?? 1080} required />
                  </Field>
                </div>
                {failure === "save" && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
              </FieldGroup>
            </fieldset>
            <DialogFooter>
              <Button type="button" variant="ghost" disabled={pending} onClick={() => setEditing(null)}>{copy.cancel}</Button>
              <Button type="submit" disabled={pending}>{current ? copy.save : copy.create}</Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <AlertDialog open={removing !== null} onOpenChange={(next) => { if (!next && !pending) setRemoving(null); }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{copy.remove} · {removing?.name}</AlertDialogTitle>
            <AlertDialogDescription>{copy.confirm}</AlertDialogDescription>
          </AlertDialogHeader>
          {failure === "remove" && <Alert variant="destructive"><AlertDescription>{copy.removeError}</AlertDescription></Alert>}
          <AlertDialogFooter>
            <AlertDialogCancel disabled={pending}>{copy.cancel}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" disabled={pending}
              className={cn("bg-destructive! text-white!")} onClick={(event) => { event.preventDefault(); void remove(); }}>{copy.remove}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </WorkspaceShell>
  );
}
