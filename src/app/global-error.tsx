"use client";

import { useEffect } from "react";
import { reportError } from "@/lib/report-error";
import "./globals.css";

// Shown when the app's own layout fails, so it cannot use the app's translations.
const COPY = {
  en: { title: "Arrumo hit a problem", body: "The error was recorded. Try again, or restart Arrumo if it keeps happening.", retry: "Try again" },
  es: { title: "Arrumo ha tenido un problema", body: "Se ha registrado el error. Vuelve a intentarlo o reinicia Arrumo si sigue ocurriendo.", retry: "Volver a intentarlo" },
  pt: { title: "O Arrumo teve um problema", body: "O erro foi registado. Tenta novamente ou reinicia o Arrumo se continuar a acontecer.", retry: "Tentar novamente" },
};

export default function GlobalError({ error, reset }: { error: Error & { digest?: string }; reset: () => void }) {
  useEffect(() => reportError(error), [error]);
  const language = (typeof navigator === "undefined" ? "en" : navigator.language.slice(0, 2)) as keyof typeof COPY;
  const copy = COPY[language] ?? COPY.en;
  return (
    <html lang={language in COPY ? language : "en"}>
      <body className="flex min-h-screen items-center justify-center bg-background p-4 text-foreground">
        <main className="flex max-w-md flex-col items-start gap-3">
          <h1 className="text-2xl font-semibold">{copy.title}</h1>
          <p className="text-muted-foreground">{copy.body}</p>
          <button type="button" className="rounded-md bg-primary px-4 py-2 text-primary-foreground" onClick={reset}>{copy.retry}</button>
        </main>
      </body>
    </html>
  );
}
