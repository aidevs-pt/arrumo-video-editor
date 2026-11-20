"use client";

import { invoke } from "@tauri-apps/api/core";
import { Film, Layers, Search } from "lucide-react";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Command, CommandDialog, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from "@/components/ui/command";
import { runTitle, type Run } from "@/components/workspace/generation/generation-run-row";
import { useLocale } from "@/lib/locale-provider";
import { completedVideoHref, profileHref } from "./workspace-links";

type SearchProfile = { id: number; name: string };
type SearchResults = { profiles: SearchProfile[]; runs: Run[] };

export function WorkspaceSearch({ workspaceId }: { workspaceId: number }) {
  const router = useRouter();
  const { messages } = useLocale();
  const copy = messages.dashboard;
  const [open, setOpen] = useState(false);
  const [results, setResults] = useState<SearchResults>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    function onShortcut(event: KeyboardEvent) {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "k") return;
      event.preventDefault();
      setOpen((current) => !current);
    }
    window.addEventListener("keydown", onShortcut);
    return () => window.removeEventListener("keydown", onShortcut);
  }, []);

  useEffect(() => {
    if (!open) return;
    let active = true;
    Promise.all([
      invoke<SearchProfile[]>("list_profiles", { workspaceId }),
      invoke<Run[]>("list_generation_runs", { workspaceId }),
    ]).then(
      ([profiles, runs]) => { if (active) { setResults({ profiles, runs }); setFailed(false); } },
      () => { if (active) setFailed(true); },
    );
    return () => { active = false; };
  }, [open, workspaceId]);

  function go(path: string) {
    setOpen(false);
    router.push(path);
  }

  return <>
    <Button type="button" size="sm" variant="ghost" onClick={() => setOpen(true)} aria-label={copy.search} aria-keyshortcuts="Control+k Meta+k">
      <Search data-icon="inline-start" />
      <span className="hidden sm:inline">{copy.search}</span>
      <kbd className="ms-4 hidden rounded-sm bg-secondary px-1.5 font-sans text-xs text-muted-foreground lg:inline">Ctrl K</kbd>
    </Button>
    <CommandDialog open={open} onOpenChange={setOpen} title={copy.search} description={copy.searchHint}>
      <Command>
        <CommandInput aria-label={copy.searchHint} placeholder={copy.searchHint} />
        <CommandList>
          <CommandEmpty>{failed ? messages.queue.loadError : results ? copy.noMatches : messages.queue.loading}</CommandEmpty>
          <CommandGroup heading={messages.profiles.title}>
            {results?.profiles.map((profile) => (
              <CommandItem key={profile.id} value={`profile-${profile.id}`} keywords={[profile.name]} onSelect={() => go(profileHref(workspaceId, profile.id))}>
                <Layers />{profile.name}
              </CommandItem>
            ))}
          </CommandGroup>
          <CommandGroup heading={messages.studio.videos}>
            {results?.runs.map((run) => (
              <CommandItem key={run.id} value={`run-${run.id}`} keywords={[runTitle(run)]}
                onSelect={() => go(run.state === "completed" ? completedVideoHref(workspaceId, run.id) : `/generation-run?workspace=${workspaceId}&run=${run.id}`)}>
                <Film />{runTitle(run)}
              </CommandItem>
            ))}
          </CommandGroup>
        </CommandList>
      </Command>
    </CommandDialog>
  </>;
}
