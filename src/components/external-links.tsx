"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";

/** Opens web links in the system browser; the webview would otherwise navigate away from the app. */
export function ExternalLinks() {
  useEffect(() => {
    function onClick(event: MouseEvent) {
      if (event.defaultPrevented || event.button !== 0) return;
      const anchor = (event.target as Element | null)?.closest?.("a[href]");
      if (!(anchor instanceof HTMLAnchorElement)) return;
      if (!/^https?:$/.test(anchor.protocol) || anchor.origin === window.location.origin) return;
      event.preventDefault();
      void invoke("open_external_link", { url: anchor.href }).catch(() => {});
    }
    document.addEventListener("click", onClick, true);
    return () => document.removeEventListener("click", onClick, true);
  }, []);
  return null;
}
