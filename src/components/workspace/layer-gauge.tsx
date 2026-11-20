import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

const layers = ["app", "workspace", "profile", "video"] as const;

export function LayerGauge({ layer }: { layer: (typeof layers)[number] }) {
  const { messages } = useLocale();
  return <span className="inline-flex items-center gap-2 text-xs font-semibold text-muted-foreground">
    <span aria-hidden="true" className="grid w-3.5 gap-0.5">
      {layers.map((level) => <span key={level} className={cn("h-0.5 rounded-[1px] bg-line-strong", level === layer && "bg-primary")} />)}
    </span>
    {messages.layers[layer]}
  </span>;
}
