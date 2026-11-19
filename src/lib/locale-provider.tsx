"use client";

import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { defaultLocale, preferredLocale, type LocaleCatalog, type LocaleMessages } from "./locales";

const LocaleContext = createContext<{ locale: string; messages: LocaleMessages } | null>(null);

export function LocaleProvider({ catalog, children }: { catalog: LocaleCatalog; children: ReactNode }) {
  const [locale, setLocale] = useState(defaultLocale);
  useEffect(() => {
    const nextLocale = preferredLocale(navigator.languages, catalog);
    document.documentElement.lang = nextLocale;
    setLocale(nextLocale);
  }, [catalog]);

  return (
    <LocaleContext.Provider value={{ locale, messages: catalog[locale] }}>
      {children}
    </LocaleContext.Provider>
  );
}

export function useLocale() {
  const locale = useContext(LocaleContext);
  if (!locale) throw new Error("LocaleProvider is missing");
  return locale;
}
