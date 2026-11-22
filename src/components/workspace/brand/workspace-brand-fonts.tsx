"use client";

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FileType, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { useLocale } from "@/lib/locale-provider";

type Role = "heading" | "body";
type Weight = "regular" | "bold";
type BrandFont = { role: Role; weight: Weight; family: string; file_name: string; sha256: string };

const ROLES: Role[] = ["heading", "body"];
const WEIGHTS: Weight[] = ["regular", "bold"];

/** The heading and body fonts animations are set in, each with a regular and a bold file. */
export function WorkspaceBrandFonts({ workspaceId }: { workspaceId: number }) {
  const { messages } = useLocale();
  const copy = messages.brand.fonts;
  const [fonts, setFonts] = useState<BrandFont[]>();
  const [loaded, setLoaded] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState<string>();
  const [error, setError] = useState<string>();

  useEffect(() => {
    let active = true;
    invoke<BrandFont[]>("get_brand_fonts", { workspaceId }).then(
      (value) => { if (active) setFonts(value); },
      () => { if (active) setError(copy.errors.brand_fonts_load_failed); },
    );
    return () => { active = false; };
  }, [workspaceId, copy.errors.brand_fonts_load_failed]);

  useEffect(() => {
    let active = true;
    for (const font of fonts ?? []) {
      if (loaded.has(font.sha256)) continue;
      invoke<string>("brand_font_data", { workspaceId, sha256: font.sha256 })
        .then((url) => new FontFace(`arrumo-${font.sha256}`, `url(${url})`).load())
        .then((face) => {
          document.fonts.add(face);
          if (active) setLoaded((current) => new Set(current).add(font.sha256));
        })
        .catch(() => {});
    }
    return () => { active = false; };
  }, [fonts, loaded, workspaceId]);

  const find = (role: Role, weight: Weight) => fonts?.find((font) => font.role === role && font.weight === weight);
  /** The file a role draws with in a weight, following the same fallbacks as the renderer. */
  const drawn = (role: Role, weight: Weight): BrandFont | undefined =>
    find(role, weight) ?? find(role, "regular") ?? (role === "heading" ? drawn("body", weight) : undefined);

  async function choose(role: Role, weight: Weight) {
    const source = await open({ title: copy.choose, filters: [{ name: copy.fileType, extensions: ["ttf", "otf"] }] });
    if (typeof source !== "string") return;
    setBusy(`${role}-${weight}`);
    setError(undefined);
    try {
      const font = await invoke<BrandFont>("import_brand_font", { workspaceId, role, weight, source });
      setFonts((current) => [...(current ?? []).filter((item) => item.role !== role || item.weight !== weight), font]);
    } catch (reason) {
      const code = typeof reason === "string" ? reason : "";
      setError(copy.errors[code as keyof typeof copy.errors] ?? copy.errors.brand_font_save_failed);
    } finally {
      setBusy(undefined);
    }
  }

  async function remove(role: Role, weight: Weight) {
    setBusy(`${role}-${weight}`);
    setError(undefined);
    try {
      await invoke<boolean>("remove_brand_font", { workspaceId, role, weight });
      setFonts((current) => current?.filter((item) => item.role !== role || item.weight !== weight));
    } catch {
      setError(copy.errors.brand_font_remove_failed);
    } finally {
      setBusy(undefined);
    }
  }

  return (
    <section aria-labelledby="brand-fonts" className="mt-12">
      <h2 id="brand-fonts" className="text-[1.75rem]">{copy.title}</h2>
      <p className="mt-1 max-w-prose text-muted-foreground">{copy.description}</p>
      {error && <p role="alert" className="mt-3 text-destructive">{error}</p>}
      {!fonts ? !error && <p role="status" className="mt-4">{copy.loading}</p> : (
        <div className="mt-4 grid gap-4 md:grid-cols-2">
          {ROLES.map((role) => {
            const regular = drawn(role, "regular");
            const bold = drawn(role, "bold");
            const family = (font?: BrandFont) => font && loaded.has(font.sha256) ? `"arrumo-${font.sha256}", sans-serif` : undefined;
            return (
              <article key={role} aria-labelledby={`brand-font-${role}`} className="flex flex-col gap-4 rounded-lg border bg-card p-4">
                <header>
                  <h3 id={`brand-font-${role}`} className="font-semibold">{copy.roles[role]}</h3>
                  <p className="text-sm text-muted-foreground">{copy.uses[role]}</p>
                </header>
                <div className="rounded-md bg-secondary px-4 py-3" aria-hidden="true">
                  <p className="truncate text-3xl" style={{ fontFamily: family(bold) }}>{copy.specimenTitle}</p>
                  <p className="truncate text-base" style={{ fontFamily: family(regular) }}>{copy.specimenBody}</p>
                </div>
                {!find(role, "regular") && !find(role, "bold") && <p className="text-sm text-muted-foreground">
                  {role === "heading" && find("body", "regular") ? copy.usesBody : copy.usesSystem}
                </p>}
                <ul className="flex flex-col divide-y">
                  {WEIGHTS.map((weight) => {
                    const font = find(role, weight);
                    const working = busy === `${role}-${weight}`;
                    return (
                      <li key={weight} className="flex items-center gap-3 py-2">
                        <div className="min-w-0 flex-1">
                          <p className="text-sm font-medium">{copy.weights[weight]}</p>
                          <p className="truncate text-sm text-muted-foreground" title={font?.file_name}>
                            {font ? `${font.family} · ${font.file_name}` : copy.notSet}
                          </p>
                        </div>
                        <Button type="button" variant="outline" size="sm" disabled={!!busy}
                          aria-label={`${font ? copy.replace : copy.add}: ${copy.roles[role]}, ${copy.weights[weight]}`}
                          onClick={() => void choose(role, weight)}>
                          <FileType data-icon="inline-start" />{working ? copy.saving : font ? copy.replace : copy.add}
                        </Button>
                        {font && <Button type="button" variant="ghost" size="icon-sm" disabled={!!busy}
                          aria-label={`${copy.remove}: ${copy.roles[role]}, ${copy.weights[weight]}`}
                          title={copy.remove} onClick={() => void remove(role, weight)}><Trash2 /></Button>}
                      </li>
                    );
                  })}
                </ul>
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}
