"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Input } from "@/components/ui/input";
import { useLocale } from "@/lib/locale-provider";

type Notices = {
  ffmpeg: { version: string; release: string; url: string } | null;
  packages: { name: string; version: string; license: string; url: string; kind: "rust" | "npm" | "font"; texts: string[] }[];
  texts: Record<string, string>;
};

const SOURCE = "https://github.com/aidevs-pt/arrumo-video-editor";

function External({ href, children }: { href: string; children: React.ReactNode }) {
  return <a href={href} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1 font-semibold text-primary underline underline-offset-4">{children}<ArrowUpRight aria-hidden="true" className="size-3.5" /></a>;
}

/** The app's version, its license and source, and the notices of everything it includes. */
export function AboutSettings() {
  const { messages: { about: copy } } = useLocale();
  const [version, setVersion] = useState<string>();
  const [notices, setNotices] = useState<Notices | null>();
  const [query, setQuery] = useState("");

  useEffect(() => {
    let active = true;
    invoke<{ version: string }>("update_status").then((status) => { if (active) setVersion(status.version); }, () => {});
    fetch("/third-party-licenses.json")
      .then((response) => (response.ok ? response.json() : null))
      .then((value: Notices | null) => { if (active) setNotices(value); }, () => { if (active) setNotices(null); });
    return () => { active = false; };
  }, []);

  const packages = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return (notices?.packages ?? []).filter((pkg) => !needle || pkg.name.toLowerCase().includes(needle) || pkg.license.toLowerCase().includes(needle));
  }, [notices, query]);

  return (
    <div className="flex flex-col gap-8">
      <section aria-labelledby="about-app" className="flex flex-col gap-2 text-sm">
        <h2 id="about-app" className="text-[1.5rem]">Arrumo AI Video Editor{version && <span className="ms-2 text-base font-normal text-muted-foreground">{version}</span>}</h2>
        <p>{copy.copyright}</p>
        <p className="max-w-[70ch] text-muted-foreground">{copy.license}</p>
        <div className="flex flex-wrap gap-x-5 gap-y-1">
          <External href={SOURCE}>{copy.source}</External>
          <External href="https://www.gnu.org/licenses/agpl-3.0.html">{copy.licenseLink}</External>
          <External href="https://aidevs.pt">aidevs.pt</External>
        </div>
      </section>

      {notices?.ffmpeg && <section aria-labelledby="about-ffmpeg" className="flex flex-col gap-2 text-sm">
        <h2 id="about-ffmpeg" className="text-[1.25rem]">FFmpeg</h2>
        <p className="max-w-[70ch] text-muted-foreground">{copy.ffmpeg.replace("{version}", notices.ffmpeg.version)}</p>
        <div><External href={notices.ffmpeg.url}>{copy.ffmpegSource}</External></div>
      </section>}

      <section aria-labelledby="about-notices" className="flex flex-col gap-3 text-sm">
        <h2 id="about-notices" className="text-[1.25rem]">{copy.notices}</h2>
        {notices === null ? <p className="text-muted-foreground">{copy.noticesMissing}</p>
          : notices === undefined ? null : <>
            <p className="max-w-[70ch] text-muted-foreground">{copy.noticesIntro.replace("{count}", String(notices.packages.length))}</p>
            <Input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={copy.search} aria-label={copy.search} className="max-w-sm" />
            <ul className="max-h-[28rem] divide-y overflow-y-auto rounded-lg border">
              {packages.map((pkg) => (
                <li key={`${pkg.kind}:${pkg.name}@${pkg.version}`}>
                  <details className="group">
                    <summary className="flex cursor-pointer flex-wrap items-baseline gap-x-2 px-3 py-2 hover:bg-muted/50">
                      <span className="font-medium">{pkg.name}</span>
                      {pkg.version && <span className="text-muted-foreground">{pkg.version}</span>}
                      <span className="ms-auto text-muted-foreground">{pkg.license}</span>
                    </summary>
                    <div className="flex flex-col gap-3 px-3 pb-3">
                      <External href={pkg.url}>{copy.project}</External>
                      {pkg.texts.length ? pkg.texts.map((id) => (
                        <pre key={id} className="max-h-64 overflow-auto rounded-md bg-muted p-3 font-mono text-xs whitespace-pre-wrap">{notices.texts[id]}</pre>
                      )) : <p className="text-muted-foreground">{copy.noText}</p>}
                    </div>
                  </details>
                </li>
              ))}
            </ul>
          </>}
      </section>
    </div>
  );
}
