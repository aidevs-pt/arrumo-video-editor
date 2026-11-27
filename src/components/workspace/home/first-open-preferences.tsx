"use client";

import { invoke } from "@tauri-apps/api/core";
import { Settings } from "lucide-react";
import { useEffect, useId, useState } from "react";

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { useAppearance } from "@/lib/appearance";
import { useLocale } from "@/lib/locale-provider";

type Privacy = { usageData: boolean; crashReports: boolean; usageAvailable: boolean; crashAvailable: boolean };

/** The few choices a creator may want before anything else: theme, usage data and crash reports.
 * Everything here is also in Settings › General. */
export function FirstOpenPreferences() {
  const { messages: { onboarding: copy, settings: text } } = useLocale();
  const appearance = useAppearance();
  const id = useId();
  const [privacy, setPrivacy] = useState<Privacy>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    invoke<Privacy>("read_privacy_settings").then((value) => { if (active) setPrivacy(value); }, () => { if (active) setFailed(true); });
    return () => { active = false; };
  }, []);

  async function save(change: Partial<Privacy>) {
    if (!privacy) return;
    const next = { ...privacy, ...change };
    setPrivacy(next);
    try {
      setPrivacy(await invoke<Privacy>("write_privacy_settings", { usageData: next.usageData, crashReports: next.crashReports }));
      setFailed(false);
    } catch {
      setPrivacy(privacy);
      setFailed(true);
    }
  }

  const row = (key: "usageData" | "crashReports", label: string, hint: string) => (
    <div className="flex items-start justify-between gap-4">
      <div className="min-w-0">
        <p id={`${id}-${key}`} className="font-semibold">{label}</p>
        <p className="text-xs text-muted-foreground">{hint}</p>
      </div>
      <Switch aria-labelledby={`${id}-${key}`} checked={privacy?.[key] ?? false} disabled={!privacy}
        onCheckedChange={(value) => void save({ [key]: value })} />
    </div>
  );

  return (
    <Popover>
      <TooltipProvider delayDuration={250}>
        <Tooltip>
          <TooltipTrigger asChild>
            <PopoverTrigger asChild>
              <button type="button" aria-label={copy.preferences}
                className="grid size-6 place-items-center rounded-sm text-white/85 hover:text-white focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring">
                <Settings aria-hidden="true" className="size-4" />
              </button>
            </PopoverTrigger>
          </TooltipTrigger>
          <TooltipContent>{copy.preferences}</TooltipContent>
        </Tooltip>
      </TooltipProvider>
      <PopoverContent align="start" className="flex w-80 flex-col gap-4 text-sm">
        <p className="font-heading text-base font-bold">{copy.preferences}</p>
        <div className="flex flex-col gap-2">
          <p id={`${id}-theme`} className="font-semibold">{text.theme}</p>
          <ToggleGroup type="single" variant="outline" size="sm" aria-labelledby={`${id}-theme`} value={appearance.theme}
            onValueChange={(value) => { if (value) appearance.setTheme(value as typeof appearance.theme); }}>
            <ToggleGroupItem value="system">{text.themeSystem}</ToggleGroupItem>
            <ToggleGroupItem value="light">{text.themeLight}</ToggleGroupItem>
            <ToggleGroupItem value="dark">{text.themeDark}</ToggleGroupItem>
          </ToggleGroup>
        </div>
        {row("usageData", text.usageData, text.usageDataHint)}
        {row("crashReports", text.crashReports, text.crashReportsHint)}
        {failed && <p role="alert" className="text-destructive">{text.privacyError}</p>}
        <p className="text-xs text-muted-foreground">{copy.preferencesLater}</p>
      </PopoverContent>
    </Popover>
  );
}
