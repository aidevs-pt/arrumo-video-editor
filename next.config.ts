import type { NextConfig } from "next";

// The dev indicator sits bottom-right so it never covers the sidebar's agent card.
const config: NextConfig = { output: "export", devIndicators: { position: "bottom-right" } };

export default config;
