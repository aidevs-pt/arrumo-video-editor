import type { Metadata } from "next";
import "./globals.css";
import { getLocaleCatalog } from "@/lib/locale-catalog";
import { LocaleProvider } from "@/lib/locale-provider";
import { Geist } from "next/font/google";
import { cn } from "@/lib/utils";

const geist = Geist({subsets:['latin'],variable:'--font-geist'});

export const metadata: Metadata = {
  title: "Arrumo Video Editor",
  icons: "/arrumo-icon.svg",
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="en" className={cn("font-sans", geist.variable)}><body><LocaleProvider catalog={getLocaleCatalog()}>{children}</LocaleProvider></body></html>;
}
