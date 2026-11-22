import { invoke } from "@tauri-apps/api/core";
import { usePluginText } from "@/lib/plugin-text";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { useLocale } from "@/lib/locale-provider";

type CatalogEntry = {
  manifest: { id: string; name: string; capabilities: Record<string, unknown> };
  package?: { sha256: string };
};

export function MotionRendererAction({ workspaceId, runId, available }: {
  workspaceId: number;
  runId: number;
  available: boolean;
}) {
  const pluginText = usePluginText();
  const { messages: { motionLibrary: copy } } = useLocale();
  const [entries, setEntries] = useState<CatalogEntry[]>([]);
  const [selected, setSelected] = useState("");
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);
  const activeId = selected || (entries.length === 1 ? entries[0].manifest.id : "");

  useEffect(() => {
    let active = true;
    invoke<boolean>("desktop_motion_renderer_available").then(async (enabled) => {
      if (!enabled || !active) return;
      const catalog = await invoke<CatalogEntry[]>("list_plugin_catalog");
      if (active) setEntries(catalog.filter(({ manifest, package: revision }) =>
        revision && Object.hasOwn(manifest.capabilities, "motion-render"),
      ));
    }).catch(() => {});
    return () => { active = false; };
  }, []);

  async function render(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const entry = entries.find(({ manifest }) => manifest.id === activeId);
    if (!entry?.package || !available || pending) return;
    setPending(true);
    setFailed(false);
    try {
      await invoke("render_desktop_run_motion", {
        workspaceId, runId, pluginId: entry.manifest.id, digest: entry.package.sha256,
      });
    } catch {
      setFailed(true);
    } finally {
      setPending(false);
    }
  }

  if (!entries.length) return null;
  return (
    <form onSubmit={render} className="flex flex-col items-start gap-3 border-t pt-3">
      <Field>
        <FieldLabel htmlFor={`motion-renderer-${runId}`}>{copy.renderer}</FieldLabel>
        <NativeSelect id={`motion-renderer-${runId}`} value={activeId}
          onChange={(event) => setSelected(event.target.value)} disabled={!available || pending}>
          <NativeSelectOption value="">{copy.selectRenderer}</NativeSelectOption>
          {entries.map(({ manifest }) => <NativeSelectOption key={manifest.id} value={manifest.id}>{pluginText.name(manifest.id, manifest.name)}</NativeSelectOption>)}
        </NativeSelect>
        <FieldDescription>{copy.rendererHint}</FieldDescription>
      </Field>
      <Button type="submit" size="sm" disabled={!activeId || !available || pending}>
        {pending ? copy.renderingMotion : copy.renderMotion}
      </Button>
      {failed && <Alert variant="destructive"><AlertDescription>{copy.rendererFailed}</AlertDescription></Alert>}
    </form>
  );
}
