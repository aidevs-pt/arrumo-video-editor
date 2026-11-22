import type { Metadata } from "next";
import { PluginTextProvider } from "@/lib/plugin-text";
import "./globals.css";
import { getLocaleCatalog } from "@/lib/locale-catalog";
import { AppearanceProvider, appearanceScript } from "@/lib/appearance";
import { LocaleProvider } from "@/lib/locale-provider";
import { Anybody, Atkinson_Hyperlegible_Next } from "next/font/google";
import { cn } from "@/lib/utils";

const anybody = Anybody({ subsets: ["latin"], axes: ["wdth"], variable: "--font-anybody" });
const atkinson = Atkinson_Hyperlegible_Next({ subsets: ["latin"], adjustFontFallback: false, variable: "--font-atkinson" });

export const metadata: Metadata = {
  title: "Arrumo Video Editor",
  icons: "/arrumo-icon.svg",
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" className={cn("font-sans", anybody.variable, atkinson.variable)} suppressHydrationWarning>
      <head><script dangerouslySetInnerHTML={{ __html: appearanceScript }} /></head>
      <body><AppearanceProvider><LocaleProvider catalog={getLocaleCatalog()}><PluginTextProvider>{children}</PluginTextProvider></LocaleProvider></AppearanceProvider></body>
    </html>
  );
}
