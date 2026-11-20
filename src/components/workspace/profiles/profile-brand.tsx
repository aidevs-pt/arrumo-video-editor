"use client";

import { invoke } from "@tauri-apps/api/core";
import Link from "next/link";
import { useEffect, useState } from "react";
import { Skeleton } from "@/components/ui/skeleton";
import { AssetThumbnail } from "@/components/workspace/assets/asset-thumbnail";
import { profileHref, workspaceHref } from "@/components/workspace/home/workspace-links";
import { LayerGauge } from "@/components/workspace/layer-gauge";
import { useLocale } from "@/lib/locale-provider";
import type { Profile } from "./profile-route";

type Association = { asset_id: number; role: "identity" | "style" };
type ProfileAsset = { id: number; name: string; kind: "image" | "audio" | "video" };
type Brand = { colors: string[]; images: Association[]; profileImages: ProfileAsset[] };

export function ProfileBrand({ workspaceId, profile }: {
  workspaceId: number;
  profile: Profile;
  onUpdate: (profile: Profile) => void;
}) {
  const { messages } = useLocale();
  const copy = messages.profileHub;
  const [brand, setBrand] = useState<Brand>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<string[]>("get_brand_colors", { workspaceId }),
      invoke<Association[]>("list_brand_assets", { workspaceId }),
      invoke<ProfileAsset[]>("list_profile_assets", { workspaceId, profileId: profile.id }),
    ]).then(([colors, images, shared]) => {
      if (!active) return;
      const seen = new Set<number>();
      const profileImages = shared.filter((asset) => asset.kind === "image" && !seen.has(asset.id) && seen.add(asset.id));
      setBrand({ colors, images: images.filter((image, index) => images.findIndex(({ asset_id }) => asset_id === image.asset_id) === index), profileImages });
    }, () => { if (active) setFailed(true); });
    return () => { active = false; };
  }, [profile.id, workspaceId]);

  if (failed) return <p role="alert" className="text-destructive">{copy.loadError}</p>;
  if (!brand) return <Skeleton className="h-48" role="status" aria-label={messages.queue.loading} />;

  return (
    <div className="flex max-w-4xl flex-col gap-10">
      <section className="rounded-xl bg-secondary p-5">
        <div className="mb-1 flex flex-wrap items-center justify-between gap-2">
          <LayerGauge layer="workspace" />
          <Link href={workspaceHref("brand", workspaceId)} className="font-semibold text-primary underline underline-offset-4">{copy.editWorkspaceBrand}</Link>
        </div>
        <h2 className="mb-1 text-[1.5rem]">{copy.workspaceBrand}</h2>
        <p className="mb-4 text-muted-foreground">{copy.workspaceBrandHint.replace("{profile}", profile.name)}</p>
        {brand.colors.length ? <ul className="mb-4 flex flex-wrap gap-2" aria-label={messages.brand.colors}>
          {brand.colors.map((color) => (
            <li key={color} className="flex items-center gap-2 rounded-lg border bg-card py-1.5 ps-1.5 pe-3 text-sm tabular-nums">
              <span aria-hidden="true" className="size-6 rounded-[4px] border" style={{ backgroundColor: color }} />{color}
            </li>
          ))}
        </ul> : <p className="mb-4 text-sm text-muted-foreground">{copy.noBrandColors}</p>}
        {brand.images.length > 0 && <ul className="grid grid-cols-[repeat(auto-fill,minmax(7rem,1fr))] gap-2">
          {brand.images.map((image) => (
            <li key={image.asset_id}><AssetThumbnail workspaceId={workspaceId} assetId={image.asset_id} kind="image" className="aspect-square w-full" /></li>
          ))}
        </ul>}
      </section>
      <section>
        <div className="mb-1 flex flex-wrap items-center justify-between gap-2">
          <LayerGauge layer="profile" />
          <Link href={`${profileHref(workspaceId, profile.id)}&tab=assets`} className="font-semibold text-primary underline underline-offset-4">{messages.profiles.assets}</Link>
        </div>
        <h2 className="mb-1 text-[1.5rem]">{copy.profileImages.replace("{profile}", profile.name)}</h2>
        <p className="mb-4 text-muted-foreground">{copy.profileImagesHint}</p>
        {brand.profileImages.length ? <ul className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-3">
          {brand.profileImages.map((image) => (
            <li key={image.id} className="min-w-0">
              <AssetThumbnail workspaceId={workspaceId} assetId={image.id} kind="image" className="aspect-square w-full" />
              <p className="mt-1 truncate text-sm">{image.name}</p>
            </li>
          ))}
        </ul> : <p className="border-y py-4 text-muted-foreground">{copy.noProfileImages}</p>}
      </section>
    </div>
  );
}
