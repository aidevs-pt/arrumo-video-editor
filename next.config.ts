import type { NextConfig } from "next";

// The dev indicator sits bottom-right so it never covers the sidebar's agent card.
// ARRUMO_HIDE_DEV_INDICATOR=1 hides it for screenshots and recordings.
const config: NextConfig = {
  output: "export",
  devIndicators: process.env.ARRUMO_HIDE_DEV_INDICATOR ? false : { position: "bottom-right" },
};

export default config;
