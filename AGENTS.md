<!-- BEGIN:nextjs-agent-rules -->

## This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->

## Desktop interface

Use Tailwind utilities for component styles and compose shared shadcn primitives. Keep theme tokens and document-wide defaults in `src/app/globals.css`. Put recurring workspace layouts in `src/components/workspace`.

Store user-facing strings in `locales/<language-tag>.json` and read them through the locale provider. Use the English catalog as the key reference.

## Domain organization

Keep `src/app` for Next.js routes, layouts, and application-wide configuration. Put workspace features in named domain folders under `src/components/workspace`, with layout primitives shared at the workspace level. Keep UI state and components close to the workflow they support. Use descriptive feature boundaries in Rust as well, with application rules in shared domain modules and transport adapters calling those modules.
