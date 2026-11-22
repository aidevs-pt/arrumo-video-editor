import { invoke } from "@tauri-apps/api/core";
import { Bell } from "lucide-react";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { USAGE_DISMISSED, dismissKey, dismissUsage, useProviderUsage, wasDismissed } from "@/components/workspace/usage/provider-usage";
import { usePluginText } from "@/lib/plugin-text";
import { useLocale } from "@/lib/locale-provider";

type RunNotice = { run_id: number; title: string; profile: string | null; at: number };

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

/** "4 min", "2 h", "3 d" in the app's language. */
function ago(at: number, locale: string) {
  const minutes = Math.max(0, Math.round((Date.now() / 1000 - at) / 60));
  const [value, unit] = minutes < 60 ? [Math.max(1, minutes), "minute"] : minutes < 60 * 24 ? [Math.round(minutes / 60), "hour"] : [Math.round(minutes / 1440), "day"];
  return new Intl.NumberFormat(locale, { style: "unit", unit, unitDisplay: "narrow" }).format(value);
}

type Entry = { key: string; kind: "review" | "completed"; notice: RunNotice; unread: boolean; href: string };

const SEEN_REVIEWS = "arrumo-seen-reviews";
const reviewKey = (workspaceId: number, notice: RunNotice) => `${workspaceId}:${notice.run_id}:${notice.at}`;

function seenReviews(): Set<string> {
  try { return new Set(JSON.parse(localStorage.getItem(SEEN_REVIEWS) ?? "[]") as string[]); } catch { return new Set(); }
}

function rememberReviews(keys: string[]) {
  try {
    const all = [...new Set([...seenReviews(), ...keys])].slice(-500);
    localStorage.setItem(SEEN_REVIEWS, JSON.stringify(all));
  } catch {}
}

export function ReviewInbox({ workspaceId, state }: { workspaceId: number; state?: InboxState }) {
  const router = useRouter();
  const { locale, messages } = useLocale();
  const copy = messages.studio;
  const inbox = state?.inbox;
  const failed = state?.failed;
  const [readHere, setReadHere] = useState<Set<number>>(new Set());
  const [seen, setSeen] = useState<Set<string>>(() => seenReviews());
  const reviews = (inbox?.reviews ?? []).map((notice) => ({ ...notice, unread: !seen.has(reviewKey(workspaceId, notice)) }));
  const unreadReviews = reviews.filter((notice) => notice.unread);
  const completions = (inbox?.completions ?? []).map((notice) => ({ ...notice, unread: notice.unread && !readHere.has(notice.run_id) }));
  const usage = useProviderUsage(workspaceId);
  const pluginText = usePluginText();
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());
  useEffect(() => {
    const changed = (event: Event) => setDismissed((current) => new Set([...current, ...(event as CustomEvent<string[]>).detail]));
    window.addEventListener(USAGE_DISMISSED, changed);
    return () => window.removeEventListener(USAGE_DISMISSED, changed);
  }, []);
  const lowProviders = usage?.providers.filter((entry) => (entry.level === "warning" || entry.level === "danger") && entry.remainingPercent !== null)
    .filter((entry) => { const key = dismissKey(workspaceId, entry); return !dismissed.has(key) && !wasDismissed(key); }) ?? [];
  const attentionCount = unreadReviews.length + completions.filter((notice) => notice.unread).length + lowProviders.length;
  const unreadCompletions = completions.filter((notice) => notice.unread);
  const entries: Entry[] = [
    ...reviews.map((notice) => ({ key: `r${notice.run_id}`, kind: "review" as const, notice, unread: notice.unread, href: `/generation-run?workspace=${workspaceId}&run=${notice.run_id}` })),
    ...completions.map((notice) => ({ key: `c${notice.run_id}`, kind: "completed" as const, notice, unread: notice.unread, href: `/completed-video?workspace=${workspaceId}&run=${notice.run_id}` })),
  ].sort((a, b) => b.notice.at - a.notice.at);

  async function markAllRead() {
    dismissUsage(lowProviders.map((entry) => dismissKey(workspaceId, entry)));
    const reviewKeys = unreadReviews.map((notice) => reviewKey(workspaceId, notice));
    rememberReviews(reviewKeys);
    setSeen((current) => new Set([...current, ...reviewKeys]));
    const ids = unreadCompletions.map((notice) => notice.run_id);
    setReadHere((current) => new Set([...current, ...ids]));
    await invoke("read_all_completion_notifications", { workspaceId }).catch(() => setReadHere((current) => {
      const next = new Set(current);
      for (const id of ids) next.delete(id);
      return next;
    }));
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button type="button" variant="outline" size="icon" aria-label={attentionCount ? `${copy.notifications}, ${copy.notificationAttention}` : copy.notifications} className="relative size-9 border-border bg-card hover:border-primary hover:bg-card hover:text-foreground aria-expanded:border-primary aria-expanded:bg-card">
          <Bell />
          {attentionCount > 0 && <span aria-hidden="true" className="absolute end-1.5 top-1.5 size-2 rounded-[2px] bg-attention ring-2 ring-background" />}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-96 max-w-[calc(100vw-1.5rem)] p-0">
        <div className="flex items-center justify-between gap-3 border-b px-4 py-3">
          <h2 className="font-heading text-lg leading-none font-[850] [font-variation-settings:'wdth'_120]">{copy.notifications}</h2>
          <Button type="button" variant="outline" size="sm" disabled={!unreadCompletions.length && !unreadReviews.length && !lowProviders.length} onClick={(event) => { event.preventDefault(); void markAllRead(); }}>
            {copy.markAllRead}
          </Button>
        </div>
        {failed && <p className="px-4 py-3 text-sm text-muted-foreground">{messages.queue.loadError}</p>}
        {entries.length + lowProviders.length > 0 ? (
          <ScrollArea className={entries.length + lowProviders.length > 4 ? "h-[26rem]" : undefined}>
            {lowProviders.map((entry) => (
              <DropdownMenuItem key={`u${entry.pluginId}`} onSelect={() => router.push(`/settings?workspace=${workspaceId}&scope=workspace&section=services`)}
                className={cn("flex-col items-stretch gap-1 rounded-none border-b px-4 py-3", entry.level === "danger" ? "bg-destructive/8 focus:bg-destructive/12" : "bg-attention/15 focus:bg-attention/25")}>
                <span className="font-semibold">{copy.noticeUsage.replace("{provider}", pluginText.name(entry.pluginId, usage?.names[entry.pluginId] ?? entry.pluginId))
                  .replace("{percent}", new Intl.NumberFormat(locale, { style: "percent", maximumFractionDigits: 0 }).format(entry.remainingPercent! / 100))}</span>
                <span>{copy.noticeUsageBody}</span>
                <span className="text-xs text-muted-foreground">{copy.opensServices}</span>
              </DropdownMenuItem>
            ))}
            {entries.map(({ key, kind, notice, unread, href }) => (
              <DropdownMenuItem key={key} onSelect={() => {
                if (kind === "review") {
                  const opened = reviewKey(workspaceId, notice);
                  rememberReviews([opened]);
                  setSeen((current) => new Set([...current, opened]));
                }
                router.push(href);
              }}
                className={cn("flex-col items-stretch gap-1 rounded-none border-b px-4 py-3 last:border-b-0", unread && "bg-primary/8 focus:bg-primary/12")}>
                <span className="flex items-baseline justify-between gap-3">
                  <span className="font-semibold">{kind === "review" ? copy.noticeReview : copy.noticeCompleted}</span>
                  <span className="shrink-0 text-xs text-muted-foreground tabular-nums">{ago(notice.at, locale)}</span>
                </span>
                <span className="truncate">{notice.title}</span>
                <span className="text-xs text-muted-foreground">
                  {notice.profile
                    ? copy.opensIn.replace("{profile}", notice.profile).replace("{place}", kind === "review" ? copy.placeGeneration : copy.placeFinished)
                    : kind === "review" ? copy.opensGeneration : copy.opensFinished}
                </span>
                {unread && <span className="sr-only">{kind === "review" ? copy.reviewInbox : copy.unreadCompletion}</span>}
              </DropdownMenuItem>
            ))}
          </ScrollArea>
        ) : !failed && <p className="px-4 py-3 text-sm text-muted-foreground">
          {inbox ? copy.noNotifications : messages.queue.loading}
        </p>}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
