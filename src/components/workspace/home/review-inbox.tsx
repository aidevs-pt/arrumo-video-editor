import { invoke } from "@tauri-apps/api/core";
import { Bell, CircleAlert } from "lucide-react";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuLabel, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useLocale } from "@/lib/locale-provider";

type Review = { run_id: number; title: string };

export function ReviewInbox({ workspaceId }: { workspaceId: number }) {
  const router = useRouter();
  const { messages } = useLocale();
  const copy = messages.studio;
  const [reviews, setReviews] = useState<Review[]>();
  const [failed, setFailed] = useState(false);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const rows = await invoke<Review[]>("pending_generation_reviews", { workspaceId });
        if (active) { setReviews(rows); setFailed(false); }
      } catch {
        if (active) setFailed(true);
      }
      if (active) timer = setTimeout(refresh, 5000);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [open, workspaceId]);

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger asChild>
        <Button type="button" variant="ghost" size="icon" aria-label={copy.reviewInbox} className="relative">
          <Bell />
          {!!reviews?.length && <span aria-hidden="true" className="absolute end-1.5 top-1.5 size-2 rounded-full bg-primary ring-2 ring-background" />}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-72 max-w-[calc(100vw-1.5rem)]">
        <DropdownMenuLabel className="flex items-center justify-between gap-2">
          {copy.reviewInbox}
          {!!reviews?.length && <Badge variant="secondary" className="tabular-nums">{reviews.length}</Badge>}
        </DropdownMenuLabel>
        {failed ? <p className="p-2 text-sm text-muted-foreground">{messages.queue.loadError}</p> : reviews?.length ? (
          <ScrollArea className={reviews.length > 5 ? "h-56" : "max-h-56"}>
            <DropdownMenuGroup>
              {reviews.map((review) => (
                <DropdownMenuItem key={review.run_id}
                  onSelect={() => router.push(`/generation-run?workspace=${workspaceId}&run=${review.run_id}`)}>
                  <CircleAlert aria-hidden="true" /><span className="truncate">{review.title}</span>
                </DropdownMenuItem>
              ))}
            </DropdownMenuGroup>
          </ScrollArea>
        ) : <p className="p-2 text-sm text-muted-foreground">
          {reviews ? copy.noReviews : messages.queue.loading}
        </p>}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
