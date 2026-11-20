"use client";

import { useEffect, useState } from "react";
import { useLocale } from "@/lib/locale-provider";

/** On Linux, playback goes through the system's GStreamer, which often lacks an H.264 decoder. */
export function CodecHint() {
  const { messages } = useLocale();
  const [linux, setLinux] = useState(false);
  useEffect(() => setLinux(/Linux/.test(navigator.userAgent) && !/Android/.test(navigator.userAgent)), []);
  if (!linux) return null;
  return <span className="mt-1 block">{messages.media.linuxCodecs}</span>;
}
