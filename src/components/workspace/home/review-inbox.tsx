import { invoke } from "@tauri-apps/api/core";
import { Bell, CircleAlert, CircleCheck } from "lucide-react";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuLabel, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useLocale } from "@/lib/locale-provider";

type RunNotice = { run_id: number; title: string };

type Inbox = { reviews: RunNotice[]; completions: RunNotice[] };

type InboxState = { workspaceId: number; inbox?: Inbox; failed: boolean };

export function useGenerationInbox(workspaceId: number | undefined) {
  const [state, setState] = useState<InboxState>();

  useEffect(() => {
    if (workspaceId === undefined) return;
    const id = workspaceId;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const rows = await invoke<Inbox>("generation_inbox", { workspaceId: id });
        if (active) setState({ workspaceId: id, inbox: rows, failed: false });
      } catch {
        if (active) setState((current) => ({
          workspaceId: id,
          inbox: current?.workspaceId === id ? current.inbox : undefined,
          failed: true,
        }));
      }
      if (active) timer = setTimeout(refresh, 5000);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [workspaceId]);

  return state?.workspaceId === workspaceId ? state : undefined;
}

export function ReviewInbox({ workspaceId, state }: { workspaceId: number; state?: InboxState }) {
  const router = useRouter();
  const { messages } = useLocale();
  const copy = messages.studio;
  const inbox = state?.inbox;
  const failed = state?.failed;
  const reviews = inbox?.reviews ?? [];
  const completions = inbox?.completions ?? [];

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button type="button" variant="ghost" size="icon" aria-label={copy.notifications} className="relative">
          <Bell />
          {reviews.length > 0 && <span aria-hidden="true" className="absolute end-1.5 top-1.5 size-2 rounded-full bg-primary ring-2 ring-background" />}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-72 max-w-[calc(100vw-1.5rem)]">
        <DropdownMenuLabel className="flex items-center justify-between gap-2">
          {copy.notifications}
          {reviews.length > 0 && <Badge variant="secondary" className="tabular-nums">{reviews.length}</Badge>}
        </DropdownMenuLabel>
        {failed && <p className="p-2 text-sm text-muted-foreground">{messages.queue.loadError}</p>}
        {reviews.length + completions.length > 0 ? (
          <ScrollArea className={reviews.length + completions.length > 4 ? "h-60" : "max-h-60"}>
            {reviews.length > 0 && (
              <DropdownMenuGroup aria-label={copy.reviewInbox}>
                <DropdownMenuLabel className="text-xs text-muted-foreground">{copy.reviewInbox}</DropdownMenuLabel>
                {reviews.map((review) => (
                  <DropdownMenuItem key={review.run_id}
                    onSelect={() => router.push(`/generation-run?workspace=${workspaceId}&run=${review.run_id}`)}>
                    <CircleAlert aria-hidden="true" /><span className="truncate">{review.title}</span>
                  </DropdownMenuItem>
                ))}
              </DropdownMenuGroup>
            )}
            {completions.length > 0 && (
              <DropdownMenuGroup aria-label={copy.recentCompletions}>
                <DropdownMenuLabel className="text-xs text-muted-foreground">{copy.recentCompletions}</DropdownMenuLabel>
                {completions.map((completion) => (
                  <DropdownMenuItem key={completion.run_id}
                    onSelect={() => router.push(`/completed-video?workspace=${workspaceId}&run=${completion.run_id}`)}>
                    <CircleCheck aria-hidden="true" /><span className="truncate">{completion.title}</span>
                  </DropdownMenuItem>
                ))}
              </DropdownMenuGroup>
            )}
          </ScrollArea>
        ) : !failed && <p className="p-2 text-sm text-muted-foreground">
          {inbox ? copy.noNotifications : messages.queue.loading}
        </p>}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
