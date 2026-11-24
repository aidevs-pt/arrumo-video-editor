"use client";

import { CircleCheck, LoaderCircle, TriangleAlert, X } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useLocale } from "@/lib/locale-provider";
import type { Asset, AssetOrganization } from "./workspace-assets";

type SkipReason = "agent_cannot_see" | "unsupported_format" | "nothing_to_read" | "unavailable" | "not_described";
export type OmittedAsset = { assetId: number; name: string; reason: SkipReason };
export type DescriptionPlan = { eligible: number[]; omitted: OmittedAsset[] };
export type DescribedAssets = { assets: Asset[]; organization: Record<number, AssetOrganization>; omitted: OmittedAsset[] };
/** `label` is the asset's name when there is one, otherwise how many. */
export type DescriptionState =
  | { phase: "running" | "done"; label: string | number; omitted: OmittedAsset[] }
  | { phase: "none" | "error" | "unavailable"; omitted: OmittedAsset[] };

/** Says what the agent is describing, which assets it left out and why, and how it went. */
export function DescriptionNotice({ state, onClose }: { state: DescriptionState; onClose: () => void }) {
  const { locale, messages } = useLocale();
  const copy = messages.assetLibrary;
  const skipped = new Intl.ListFormat(locale, { type: "conjunction" })
    .format(state.omitted.map((entry) => `${entry.name} (${copy.skipReason[entry.reason]})`));
  const counted = (one: string, many: string, label: string | number) =>
    typeof label === "string" ? one.replace("{name}", () => label) : many.replace("{count}", String(label));
  const headline = state.phase === "running" ? counted(copy.describingOne, copy.describingMany, state.label)
    : state.phase === "done" ? counted(copy.describedOne, copy.describedMany, state.label)
      : state.phase === "none" ? copy.describeNone.replace("{list}", () => skipped)
        : state.phase === "unavailable" ? copy.describeUnavailable : copy.describeError;
  const warning = state.phase === "none" || state.phase === "error" || state.phase === "unavailable";
  const Icon = state.phase === "running" ? LoaderCircle : warning ? TriangleAlert : CircleCheck;
  return (
    <Alert role={warning ? "alert" : "status"} variant={state.phase === "error" ? "destructive" : "default"}>
      <Icon className={state.phase === "running" ? "motion-safe:animate-spin" : undefined} aria-hidden="true" />
      <AlertDescription className="flex items-start gap-2">
        <span className="min-w-0 flex-1">
          <span className="font-semibold text-foreground">{headline}</span>
          {state.phase !== "none" && state.omitted.length > 0 && <> {copy.describeSkipped.replace("{list}", () => skipped)}</>}
        </span>
        {state.phase !== "running" && <Button variant="ghost" size="icon-sm" aria-label={copy.dismissNotice} onClick={onClose}><X /></Button>}
      </AlertDescription>
    </Alert>
  );
}
