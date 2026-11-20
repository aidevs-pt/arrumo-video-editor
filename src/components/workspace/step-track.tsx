import { cn } from "@/lib/utils";

/** Equal bars for a fixed sequence: done in ink, current in cobalt, upcoming in mist. */
export function StepTrack({ count, current, label, className }: {
  count: number;
  current: number;
  label: string;
  className?: string;
}) {
  return (
    <div role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={count} aria-valuenow={Math.min(current, count)}
      className={cn("flex gap-1", className)}>
      {Array.from({ length: count }, (_, index) => (
        <span key={index} className={cn("h-1.5 flex-1 rounded-[2px]",
          index < current ? "bg-ink" : index === current ? "bg-primary" : "bg-border")} />
      ))}
    </div>
  );
}
