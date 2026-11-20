"use client";

import { createContext, useContext, useEffect, useState, type ReactNode } from "react";

export type TrailItem = { label: string; href?: string };

type TrailState = { items: TrailItem[] | null; setItems: (items: TrailItem[] | null) => void };

const Trail = createContext<TrailState>({ items: null, setItems: () => {} });

export function StudioTrailProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<TrailItem[] | null>(null);
  return <Trail.Provider value={{ items, setItems }}>{children}</Trail.Provider>;
}

export function useStudioTrailItems() {
  return useContext(Trail).items;
}

/** Replaces the section breadcrumb with a more specific trail while the calling page is mounted. */
export function useStudioTrail(items: TrailItem[] | null) {
  const { setItems } = useContext(Trail);
  const key = JSON.stringify(items);
  useEffect(() => {
    setItems(key === "null" ? null : JSON.parse(key));
    return () => setItems(null);
  }, [key, setItems]);
}
