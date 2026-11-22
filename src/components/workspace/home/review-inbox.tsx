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

type Inbox = { reviews: RunNotice[]; completions: (RunNotice & { unread: boolean })[] };

type InboxState = { workspaceId: number; inbox?: Inbox; working?: number; failed: boolean };
type RunState = { state: string; pending_review_id: number | null; progress?: { stage: string } | null };

const workingStates = new Set(["queued", "running"]);

export function useGenerationInbox(workspaceId: number | undefined) {
  const [state, setState] = useState<InboxState>();

  useEffect(() => {
    if (workspaceId === undefined) return;
    const id = workspaceId;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const [rows, runs] = await Promise.all([
          invoke<Inbox>("generation_inbox", { workspaceId: id }),
          invoke<RunState[]>("list_generation_runs", { workspaceId: id }),
        ]);
        const working = runs.filter((run) => workingStates.has(run.state) && run.pending_review_id === null
          && !(run.state === "queued" && run.progress?.stage === "verify_delivery")).length;
        if (active) setState({ workspaceId: id, inbox: rows, working, failed: false });
      } catch {
        if (active) setState((current) => ({
          workspaceId: id,
          inbox: current?.workspaceId === id ? current.inbox : undefined,
          working: current?.workspaceId === id ? current.working : undefined,
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
  const attentionCount = reviews.length + completions.filter((notice) => notice.unread).length;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button type="button" variant="outline" size="icon" aria-label={attentionCount ? `${copy.notifications}, ${copy.notificationAttention}` : copy.notifications} className="relative border-border bg-card hover:border-primary">
          <Bell />
          {attentionCount > 0 && <span aria-hidden="true" className="absolute end-1.5 top-1.5 size-2 rounded-[2px] bg-attention ring-2 ring-background" />}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-72 max-w-[calc(100vw-1.5rem)]">
        <DropdownMenuLabel className="flex items-center justify-between gap-2">
          {copy.notifications}
          {attentionCount > 0 && <Badge variant="secondary" className="bg-attention text-ink tabular-nums">{attentionCount}</Badge>}
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
                    <CircleCheck aria-hidden="true" /><span className="min-w-0 flex-1 truncate">{completion.title}</span>
                    {completion.unread && <>
                      <span aria-hidden="true" className="size-1.5 shrink-0 rounded-[2px] bg-attention" />
                      <span className="sr-only">{copy.unreadCompletion}</span>
                    </>}
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
