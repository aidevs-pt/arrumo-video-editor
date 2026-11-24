import type { Metadata } from "next";
import { PluginTextProvider } from "@/lib/plugin-text";
import "./globals.css";
import { getLocaleCatalog } from "@/lib/locale-catalog";
import { AppearanceProvider } from "@/lib/appearance";
import { appearanceScript } from "@/lib/appearance-script";
import { LocaleProvider } from "@/lib/locale-provider";
import { Anybody, Atkinson_Hyperlegible_Next } from "next/font/google";
import { cn } from "@/lib/utils";
import { LaunchScreen } from "@/components/launch-screen";
import { ExternalLinks } from "@/components/external-links";
import { CloseGuard } from "@/components/close-guard";
import { UpdatePrompt } from "@/components/app-updates";
import { MediaToolsCheck } from "@/components/media-tools-check";

const anybody = Anybody({ subsets: ["latin"], axes: ["wdth"], variable: "--font-anybody" });
const atkinson = Atkinson_Hyperlegible_Next({ subsets: ["latin"], adjustFontFallback: false, variable: "--font-atkinson" });

export const metadata: Metadata = {
  title: "Arrumo AI Video Editor",
  icons: "/arrumo-icon.svg",
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" className={cn("font-sans", anybody.variable, atkinson.variable)} suppressHydrationWarning>
      <head><script dangerouslySetInnerHTML={{ __html: `${appearanceScript};setTimeout(function(){document.documentElement.dataset.appReady=""},12000)` }} /></head>
      <body><LaunchScreen /><ExternalLinks /><AppearanceProvider><LocaleProvider catalog={getLocaleCatalog()}><PluginTextProvider>{children}<CloseGuard /><UpdatePrompt /><MediaToolsCheck /></PluginTextProvider></LocaleProvider></AppearanceProvider></body>
    </html>
  );
}
