"use client";

import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { defaultLocale, preferredLocale, type LocaleCatalog, type LocaleMessages } from "./locales";

const LocaleContext = createContext<{ locale: string; messages: LocaleMessages; setLocale: (locale: string) => void } | null>(null);

export function LocaleProvider({ catalog, children }: { catalog: LocaleCatalog; children: ReactNode }) {
  const [locale, updateLocale] = useState(defaultLocale);
  useEffect(() => {
    let saved: string | null = null;
    try { saved = localStorage.getItem("arrumo-locale"); } catch {}
    const nextLocale = saved && Object.hasOwn(catalog, saved) ? saved : preferredLocale(navigator.languages, catalog);
    document.documentElement.lang = nextLocale;
    updateLocale(nextLocale);
  }, [catalog]);

  function setLocale(nextLocale: string) {
    if (!Object.hasOwn(catalog, nextLocale)) return;
    document.documentElement.lang = nextLocale;
    updateLocale(nextLocale);
    try { localStorage.setItem("arrumo-locale", nextLocale); } catch {}
  }

  return (
    <LocaleContext.Provider value={{ locale, messages: catalog[locale], setLocale }}>
      {children}
    </LocaleContext.Provider>
  );
}

export function useLocale() {
  const locale = useContext(LocaleContext);
  if (!locale) throw new Error("LocaleProvider is missing");
  return locale;
}
