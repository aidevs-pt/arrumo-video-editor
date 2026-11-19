import { invoke } from "@tauri-apps/api/core";
import { BarChart3, ListChecks } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { useLocale } from "@/lib/locale-provider";
import type { WorkspaceView } from "./workspace-frame";

export function WorkspaceOverview({ workspaceId, view, onNavigate }: {
  workspaceId: number;
  view: WorkspaceView;
  onNavigate: (view: WorkspaceView) => void;
}) {
  const { messages } = useLocale();
  const [counts, setCounts] = useState<{ profiles: number; assets: number; formats: number; reviews: number }>();

  useEffect(() => {
    let active = true;
    const viewport = window.matchMedia("(min-width: 1280px)");
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      if (!viewport.matches || !active) return;
      const args = { workspaceId };
      try {
        const [profiles, assets, outputs, runs] = await Promise.all([
          invoke<unknown[]>("list_profiles", args),
          invoke<unknown[]>("list_assets", args),
          invoke<unknown[]>("list_output_formats", args),
          invoke<unknown[]>("pending_generation_reviews", args),
        ]);
        if (active) setCounts({ profiles: profiles.length, assets: assets.length, formats: outputs.length,
          reviews: runs.length });
      } catch {
        if (active) setCounts(undefined);
      } finally {
        if (active && viewport.matches) timer = setTimeout(refresh, 5000);
      }
    }
    function onResize() {
      clearTimeout(timer);
      if (viewport.matches) void refresh();
    }
    void refresh();
    viewport.addEventListener("change", onResize);
    return () => {
      active = false;
      clearTimeout(timer);
      viewport.removeEventListener("change", onResize);
    };
  }, [workspaceId, view]);

  const stats = [
    { label: messages.profiles.title, value: counts?.profiles },
    { label: messages.assets.title, value: counts?.assets },
    { label: messages.studio.formats, value: counts?.formats },
  ];
  return (
    <>
      <Card>
        <CardHeader>
          <h2 className="flex items-center gap-2 text-sm font-semibold">
            <BarChart3 className="size-4 text-primary" aria-hidden="true" />{messages.studio.overview}
          </h2>
        </CardHeader>
        <CardContent>
          <dl className="grid grid-cols-3 gap-2">
            {stats.map(({ label, value }) => (
              <div className="flex min-w-0 flex-col-reverse" key={label}>
                <dt className="wrap-anywhere text-xs text-muted-foreground">{label}</dt>
                <dd className="text-xl font-semibold tabular-nums">{value ?? "—"}</dd>
              </div>
            ))}
          </dl>
        </CardContent>
      </Card>
      {Boolean(counts?.reviews) && (
        <Card>
          <CardHeader>
            <h2 className="flex items-center gap-2 text-sm font-semibold">
              <ListChecks className="size-4 text-primary" aria-hidden="true" />{messages.productionPlan.awaitingReview}
            </h2>
          </CardHeader>
          <CardContent>
            <Button variant="outline" className="justify-start" onClick={() => onNavigate("generations")}>
              {messages.queue.title} <span className="tabular-nums">({counts?.reviews})</span>
            </Button>
          </CardContent>
        </Card>
      )}
    </>
  );
}
