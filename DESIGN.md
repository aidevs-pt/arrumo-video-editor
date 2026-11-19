# Arrumo Video Studio

## Product and visual direction

Arrumo is a desktop production tool for directing Codex or Claude Code through repeatable video workflows. The five workspace references in `../reference-style` establish the visual system. Keep the supplied Arrumo icon and its SVG identifiers. Use white for the working canvas, a quiet gray rail and panel surface, ink-black type, and vermilion for the current destination, review attention, and primary commit.

Give each surface a clear job. The asset library puts media and usage notes first. Draft composition starts with clips and their order, then profile and guidance. A generation review leads with the current artifact and its decision, with stage progress and real activity nearby. A completed video leads with playback and export, with metadata, activity and feedback in secondary views.

## Desktop shell

- Use the shadcn Sidebar with grouped workspace and production navigation, a workspace switcher, and a color-coded profile hierarchy. On narrow windows, open navigation in a Sheet. Keep the toolbar and location breadcrumbs consistent across routes.
- Keep Workspace settings (Context, Brand, Assets, Outputs) distinct from production (Drafts, Generations, Completed videos). The New video action opens the composer. Link attention items to the specific run.
- Use a contextual inspector on wide displays. On setup screens this can show relevant workspace context. On a run, show its stage and activity. Keep primary controls available when the inspector stacks or is absent.
- Use native window chrome. Use a contained scrolling region for long popups or media lists. Keep basic navigation and decisions keyboard accessible at the minimum window width.

## Tokens and components

- Define semantic tokens in `src/app/globals.css`. The light palette uses white for content, `#f8f8f7` for navigation, `#f7f7f6` for forms, `#171719` for ink, and `#e2e2e3` for dividers.
- Use `#c92c1b` for primary actions and the related tinted surface for selection. Reserve destructive styling for the destructive action. Use outline controls for secondary actions and ghost controls for low-emphasis inline actions.
- Use Geist. The page title is 28 px, section titles are 16 px, normal interface text is 14 px, and compact metadata is 12–13 px. Allow labels to wrap. Use tabular numerals for counts and durations and keep prose near 65 characters per line.
- Prefer the installed shadcn primitives for navigation, cards, tables, disclosures, forms, dialogs, selects, confirmation, feedback, and states. Compose them in their owning domain. Use shared variants and tokens for common treatment.
- Use border, spacing, and typographic weight to separate groups. Use real media previews when available. Use a file-type icon when the format has no rendered preview.
- Show real readiness, provider, and review states. Use Skeleton, Empty, or Alert with appropriate recovery. Keep stable file and entity names in accessible labels.
- Use Motion for purposeful transitions and honor reduced motion. Keep user-facing strings in the English, Spanish, and Portuguese catalogs.

## Release verification

Exercise the first-run path, recorded and brief-only drafts, generation and review, playback and export, workspace switching, and error recovery with the real desktop host. Check keyboard navigation and window resizing. Visible agent connection and worker progress must come from the actual runtime. Outstanding orchestration, onboarding, provider, plugin, and release work is tracked in the root TODO.
