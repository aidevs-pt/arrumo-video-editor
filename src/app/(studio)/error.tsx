"use client";

import { useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { useLocale } from "@/lib/locale-provider";
import { reportError } from "@/lib/report-error";

export default function StudioError({ error, reset }: { error: Error & { digest?: string }; reset: () => void }) {
  const { messages: { errorBoundary: copy } } = useLocale();
  useEffect(() => reportError(error), [error]);
  return (
    <Empty className="min-h-[60vh]">
      <EmptyHeader>
        <EmptyTitle>{copy.title}</EmptyTitle>
        <EmptyDescription>{copy.body}</EmptyDescription>
      </EmptyHeader>
      <Button onClick={reset}>{copy.retry}</Button>
    </Empty>
  );
}
