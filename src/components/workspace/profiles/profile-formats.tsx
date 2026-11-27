"use client";

import { useLocale } from "@/lib/locale-provider";
import { WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { Monitor, Plus, RectangleHorizontal, Smartphone, Square } from "lucide-react";
import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Empty, EmptyContent, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { LayerGauge } from "@/components/workspace/layer-gauge";
import { FormatDelivery } from "@/components/workspace/outputs/format-delivery";

type ProfileFormat = {
  id: number;
  name: string;
  width: number;
  height: number;
  workspace_width: number;
  workspace_height: number;
  overrides: { width: number | null; height: number | null };
  enabled: boolean;
};

export function ProfileFormats({
  workspaceId, profile, header, onOutputs,
}: {
  workspaceId: number;
  profile: { id: number; name: string };

  header: ReactNode;
  onOutputs: () => void;
}) {
  const [selectedId, setSelectedId] = useState<number>();
  const [formats, setFormats] = useState<ProfileFormat[]>();
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [pending, setPending] = useState<number | null>(null);
  const [result, setResult] = useState<{ id: number; saved: boolean } | null>(null);
  const [switchError, setSwitchError] = useState<"lastFormat" | "switchError">();
  const { messages } = useLocale();
  const copy = messages.profileFormats;

  useEffect(() => {
    let current = true;
    invoke<ProfileFormat[]>("list_profile_formats", { workspaceId, profileId: profile.id }).then(
      (rows) => { if (current) setFormats(rows); },
      () => { if (current) setLoadError(true); },
    );
    return () => { current = false; };
  }, [workspaceId, profile.id, attempt]);

  async function save(event: FormEvent<HTMLFormElement>, formatId: number) {
    event.preventDefault();
    if (pending !== null) return;
    const data = new FormData(event.currentTarget);
    const width = data.get("width");
    const height = data.get("height");
    const overrides = { width: width ? Number(width) : null, height: height ? Number(height) : null };
    setResult(null);
    setPending(formatId);
    try {
      await invoke("set_profile_format_overrides", {
        workspaceId,
        profileId: profile.id,
        formatId,
        overrides,
      });
      setFormats((current) => current?.map((format) => format.id === formatId ? {
        ...format,
        overrides,
        width: overrides.width ?? format.workspace_width,
        height: overrides.height ?? format.workspace_height,
      } : format));
      setResult({ id: formatId, saved: true });
    } catch {
      setResult({ id: formatId, saved: false });
    } finally {
      setPending(null);
    }
  }

  async function setEnabled(format: ProfileFormat, enabled: boolean) {
    setSwitchError(undefined);
    if (!enabled && !formats?.some((other) => other.enabled && other.id !== format.id)) {
      setSwitchError("lastFormat");
      return;
    }
    try {
      setFormats(await invoke<ProfileFormat[]>("set_profile_format_enabled", { workspaceId, profileId: profile.id, formatId: format.id, enabled }));
    } catch {
      setSwitchError("switchError");
    }
  }

  const selected = formats?.find((format) => format.id === selectedId) ?? formats?.[0];

  return (
    <WorkspaceShell>
      {header}
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((value) => value + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : !formats ? (
        <Skeleton className="h-44 w-full" role="status" aria-label={copy.loading} />
      ) : formats.length === 0 || !selected ? (
        <Empty>
          <EmptyHeader><EmptyMedia variant="icon"><Monitor /></EmptyMedia><EmptyTitle>{copy.empty}</EmptyTitle></EmptyHeader>
          <EmptyContent><Button type="button" variant="outline" onClick={onOutputs}>{copy.manage}</Button></EmptyContent>
        </Empty>
      ) : (
        <div className="flex flex-col gap-6">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <ToggleGroup type="single" variant="outline" value={String(selected.id)} aria-label={copy.title}
              onValueChange={(value) => { if (value) { setResult(null); setSwitchError(undefined); setSelectedId(Number(value)); } }}>
              {formats.map((format) => {
                const Icon = format.width > format.height ? RectangleHorizontal : format.width < format.height ? Smartphone : Square;
                return <ToggleGroupItem key={format.id} value={String(format.id)} className="px-3">
                  <Icon aria-hidden="true" /><span className={cn(!format.enabled && "text-muted-foreground line-through")}>{format.name}</span>
                  {!format.enabled && <span className="text-xs text-muted-foreground">{copy.offBadge}</span>}
                </ToggleGroupItem>;
              })}
            </ToggleGroup>
            <p className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
              {copy.fromWorkspace}
              <Button type="button" size="sm" variant="outline" onClick={onOutputs} disabled={pending !== null}>
                <Plus data-icon="inline-start" />{copy.addFormat}
              </Button>
            </p>
          </div>
          {selected.enabled ? <FormatDelivery key={selected.id} workspaceId={workspaceId} profileId={profile.id} profileName={profile.name}
            format={selected} embedded before={<>{switchCard(selected)}{sizeCard(selected)}</>} /> : switchCard(selected)}
        </div>
      )}
    </WorkspaceShell>
  );

  function switchCard(format: ProfileFormat) {
    return (
      <div className="flex flex-col gap-2 rounded-xl border bg-card p-4 sm:p-5">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <p id={`profile-format-on-${format.id}`} className="font-semibold">{copy.madeHere.replace("{format}", format.name)}</p>
            <p className="text-sm text-muted-foreground">{format.enabled ? copy.madeHereHint : copy.offNotice.replace("{format}", format.name)}</p>
          </div>
          <Switch aria-labelledby={`profile-format-on-${format.id}`} checked={format.enabled}
            onCheckedChange={(enabled) => void setEnabled(format, enabled)} />
        </div>
        {switchError && <p role="alert" className="text-sm text-destructive">{copy[switchError]}</p>}
      </div>
    );
  }

  function sizeCard(format: ProfileFormat) {
    const inherited = format.overrides.width === null && format.overrides.height === null;
    return (
      <form onSubmit={(event) => void save(event, format.id)} aria-labelledby={`profile-format-${format.id}`}
        className="rounded-xl border bg-card p-4 sm:p-5" onChange={() => { if (result?.id === format.id) setResult(null); }}>
        <div className="mb-3 flex flex-wrap items-center gap-3">
          <h3 id={`profile-format-${format.id}`} className="min-w-0 flex-1 font-semibold">{copy.size}</h3>
          <LayerGauge layer={inherited ? "workspace" : "profile"} />
        </div>
        <p id="format-inheritance" className="mb-3 text-sm text-muted-foreground">{copy.inherit}</p>
        <fieldset disabled={pending !== null}>
          <FieldGroup className="grid grid-cols-2 items-end gap-3 sm:grid-cols-[repeat(2,minmax(0,10rem))_auto]">
            <Field>
              <FieldLabel htmlFor={`profile-format-width-${format.id}`}>{copy.width}</FieldLabel>
              <Input id={`profile-format-width-${format.id}`} name="width" type="number" min={1} max={4294967295}
                aria-describedby="format-inheritance" placeholder={String(format.workspace_width)}
                defaultValue={format.overrides.width ?? ""} className="tabular-nums" />
            </Field>
            <Field>
              <FieldLabel htmlFor={`profile-format-height-${format.id}`}>{copy.height}</FieldLabel>
              <Input id={`profile-format-height-${format.id}`} name="height" type="number" min={1} max={4294967295}
                aria-describedby="format-inheritance" placeholder={String(format.workspace_height)}
                defaultValue={format.overrides.height ?? ""} className="tabular-nums" />
            </Field>
            <Button type="submit" variant="outline" className="col-span-full justify-self-start sm:col-span-1">{copy.save}</Button>
          </FieldGroup>
        </fieldset>
        {result?.id === format.id && (
          result.saved ? <p role="status" className="mt-2 text-sm text-muted-foreground">{copy.saved}</p>
            : <p role="alert" className="mt-2 text-sm text-destructive">{copy.saveError}</p>
        )}
      </form>
    );
  }
}
