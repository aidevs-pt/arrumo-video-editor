"use client";

import { MotionConfig } from "motion/react";
import { createContext, useContext, useEffect, useState, type ReactNode } from "react";

export type ThemePreference = "system" | "light" | "dark";
export type MotionPreference = "system" | "reduce" | "full";
type Appearance = {
  theme: ThemePreference;
  motion: MotionPreference;
  systemReducesMotion: boolean;
  setTheme: (theme: ThemePreference) => void;
  setMotion: (motion: MotionPreference) => void;
};

const storageKey = "arrumo:appearance";
const Context = createContext<Appearance | null>(null);

/** Runs before hydration so the first paint already uses the saved theme and motion choice. */
export const appearanceScript = `try{var a=JSON.parse(localStorage.getItem("${storageKey}")||"{}"),d=document.documentElement,m=matchMedia("(prefers-color-scheme: dark)").matches;if(a.theme==="dark"||(a.theme!=="light"&&m))d.classList.add("dark");if(a.motion==="reduce"||(a.motion!=="full"&&matchMedia("(prefers-reduced-motion: reduce)").matches))d.dataset.reduceMotion=""}catch(e){}`;

function read(): { theme: ThemePreference; motion: MotionPreference } {
  try {
    const saved = JSON.parse(localStorage.getItem(storageKey) ?? "{}");
    return {
      theme: saved.theme === "light" || saved.theme === "dark" ? saved.theme : "system",
      motion: saved.motion === "reduce" || saved.motion === "full" ? saved.motion : "system",
    };
  } catch {
    return { theme: "system", motion: "system" };
  }
}

function useMediaQuery(query: string) {
  const [matches, setMatches] = useState(false);
  useEffect(() => {
    const media = matchMedia(query);
    const update = () => setMatches(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, [query]);
  return matches;
}

export function AppearanceProvider({ children }: { children: ReactNode }) {
  const [preferences, setPreferences] = useState<{ theme: ThemePreference; motion: MotionPreference }>({ theme: "system", motion: "system" });
  const systemDark = useMediaQuery("(prefers-color-scheme: dark)");
  const systemReducesMotion = useMediaQuery("(prefers-reduced-motion: reduce)");
  const dark = preferences.theme === "dark" || (preferences.theme === "system" && systemDark);
  const reduce = preferences.motion === "reduce" || (preferences.motion === "system" && systemReducesMotion);

  useEffect(() => { setPreferences(read()); }, []);
  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", dark);
    if (reduce) root.dataset.reduceMotion = "";
    else delete root.dataset.reduceMotion;
  }, [dark, reduce]);

  function update(change: Partial<typeof preferences>) {
    setPreferences((current) => {
      const next = { ...current, ...change };
      try { localStorage.setItem(storageKey, JSON.stringify(next)); } catch {}
      return next;
    });
  }

  return (
    <Context.Provider value={{
      ...preferences, systemReducesMotion,
      setTheme: (theme) => update({ theme }), setMotion: (motion) => update({ motion }),
    }}>
      <MotionConfig reducedMotion={reduce ? "always" : "never"}>{children}</MotionConfig>
    </Context.Provider>
  );
}

export function useAppearance() {
  const value = useContext(Context);
  if (!value) throw new Error("AppearanceProvider is missing");
  return value;
}
