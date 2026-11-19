# Arrumo Video Studio

## Visual direction

Use the supplied reference set in `../reference-style` as the source for the studio's visual language. The workspace is bright and utilitarian, with white working surfaces, quiet gray rails and forms, ink-black typography and icon tiles, and vermilion for decisions and navigation. Preserve the supplied Arrumo icon and its SVG component IDs. The reference layouts guide the interface structure, density, and hierarchy.

## Shell

- Keep workspace switching and workflow navigation in a persistent left rail. Group Context, Brand, Assets, and Outputs under Workspace. Give Profiles, Drafts, and Generations their own destinations.
- Use the compact top bar for location and the main navigation action. Use the dark icon tile and a single strong title at the start of each working view.
- Reserve the right rail for information that helps the current task, such as actual workspace counts or runs awaiting review. Keep the main column usable at the minimum window width. The overview rail appears from 1200 px, and the left navigation reflows above content below 800 px.
- Use native window chrome. Keep one content scroll flow and let cards and forms grow for localized labels.

## Tokens and type

- Define semantic shadcn tokens in `src/app/globals.css`. The light palette uses white for content, `#f8f8f7` for the rail, `#f7f7f6` for form surfaces, `#171719` for ink, and `#e2e2e3` for dividers.
- Use `#c92c1b` for primary controls and selected destinations, with white text. Use the pale accent surface and its darker foreground for workspace tabs and selection. Keep destructive feedback on its own semantic token.
- Use Geist for body and heading text. Page titles are bold, compact, and approximately 26–34 px. Section headings are 16 px, and secondary text uses the muted foreground token. Use tabular numerals for counts, durations, and hex labels.

## Components and behavior

- Compose the shared shadcn Button, Card, Field, Input, Native Select, Alert, and Empty primitives. Put reusable variants in the primitives and layout utilities in the owning component. Use `src/components/workspace/layout.tsx` for shared page structure.
- Use thin borders, a single radius family, and spacing to distinguish groups. Show brand colors as swatches, and use creator-owned media when the current view can display it.
- Use red sparingly for the current destination and the action that advances the work. Choose outline buttons for secondary actions, ghost buttons for inline controls, and explicit alert states for failures.
- Keep keyboard focus visible on controls and respect reduced motion. Store user-facing text in the English, Spanish, and Portuguese catalogs. Verify changes at wide and minimum window widths with real data and useful empty states.
- Keep feature UI in named folders under `src/components/workspace`. Add workflow steps, agent state, and output previews as their corresponding capabilities become available.
