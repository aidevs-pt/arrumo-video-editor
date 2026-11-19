"use client";

import { invoke } from "@tauri-apps/api/core";
import { ChevronDown } from "lucide-react";
import { useState } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Field, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";

type Provider = { id: string; name: string };
type Preferences = { available: Provider[]; effective: string[] };

export function RunProviders({ workspaceId, disabled }: { workspaceId: number; disabled: boolean }) {
  const { messages: { runProviders: copy, workspaces } } = useLocale();
  const [preferences, setPreferences] = useState<Record<string, Preferences>>();
  const [overrides, setOverrides] = useState<Record<string, string[]>>({});
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);

  async function load() {
    setLoading(true);
    setFailed(false);
    try {
      setPreferences(await invoke<Record<string, Preferences>>("list_provider_preferences", { workspaceId }));
    } catch {
      setFailed(true);
    } finally {
      setLoading(false);
    }
  }

  function toggle(capability: string, provider: string, selected: boolean) {
    setOverrides((current) => {
      const order = Object.hasOwn(current, capability) ? current[capability] : preferences?.[capability].effective ?? [];
      return { ...current, [capability]: selected ? [...order, provider] : order.filter((id) => id !== provider) };
    });
  }

  function inherit(capability: string) {
    setOverrides((current) => Object.fromEntries(Object.entries(current).filter(([id]) => id !== capability)));
  }

  const capabilities = Object.entries(preferences ?? {}).filter(([id, { available }]) => id !== "agents" && available.length > 0);

  return (
    <Collapsible onOpenChange={(open) => { if (open && !preferences && !loading) void load(); }}>
      <input type="hidden" name="provider-overrides" value={JSON.stringify(overrides)} />
      <CollapsibleTrigger asChild>
        <Button type="button" variant="outline" disabled={disabled}>
          {copy.title}<ChevronDown data-icon="inline-end" />
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent className="mt-3 flex flex-col gap-4 data-closed:hidden">
        {failed && <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => void load()}>{workspaces.retry}</Button>
        </Alert>}
        {loading && <Skeleton className="h-20" aria-label={copy.loading} />}
        {preferences && capabilities.length === 0 && <p className="text-sm text-muted-foreground">{copy.empty}</p>}
        {capabilities.map(([capability, { available, effective }]) => {
          const hasOverride = Object.hasOwn(overrides, capability);
          const order = hasOverride ? overrides[capability] : effective;
          const providers = [
            ...order.flatMap((id) => available.filter((provider) => provider.id === id)),
            ...available.filter((provider) => !order.includes(provider.id)),
          ];
          const labels: Record<string, string> = copy.capabilities;
          return (
            <FieldSet key={capability} className="gap-2">
              <FieldLegend variant="label">{Object.hasOwn(labels, capability) ? labels[capability] : capability}</FieldLegend>
              {providers.map(({ id, name }) => (
                <Field key={id} orientation="horizontal">
                  <Checkbox id={`run-provider:${capability}:${id}`} checked={order.includes(id)}
                    disabled={disabled} onCheckedChange={(checked) => toggle(capability, id, checked === true)} />
                  <FieldLabel htmlFor={`run-provider:${capability}:${id}`}>{name}</FieldLabel>
                </Field>
              ))}
              {hasOverride && <Button type="button" variant="ghost" size="sm" className="self-start"
                onClick={() => inherit(capability)} disabled={disabled}>{copy.inherit}</Button>}
            </FieldSet>
          );
        })}
      </CollapsibleContent>
    </Collapsible>
  );
}
