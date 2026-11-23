# Arrumo — DESIGN.md (Azulejo)

Visual system for Arrumo AI Video Editor (Tauri + Next.js + shadcn, themed per Impeccable). Canvas, page "Azulejo": https://claude.ai/artifact/Lb9j2eyiiR4TgJxyMA7GiT
Replaces the first dark/coral direction, which was rejected for reading as AI-generated.

## Idea
From the brief: Portuguese *arrumar* (put in order), video editing, the face logo. Cobalt azulejo blue is the brand and the agent's world; tram yellow appears only when something needs the user; the workspace is cool paper.

## Color (OKLCH tokens)
- Cobalt `#2337B4` — brand, the one primary button per screen, the agent. Deep cobalt (cobalt mixed toward navy) floods the generation screen.
- Tram yellow `oklch(85.5% .16 86)` — attention only: counts that need you, staged decisions, the playhead, warning banners, focus ring.
- Ink `oklch(23% .05 268)` (navy, never #111) / secondary `oklch(40% .04 268)` / tertiary `oklch(50% .035 268)`.
- Paper `oklch(97.6% .006 265)` (cool, never cream), sheet `oklch(99.4% .003 265)`, sunk `oklch(94.6% .011 265)`, mist lines `oklch(89.5% .015 265)`.
- Decisions: approve `oklch(50% .13 150)`, changes = yellow, reject `oklch(52% .19 27)`; always with icon + word.
- Profile colors are data only (squares, bars, calendar).

## Type
- Anybody (variable width): headlines, wide and heavy like film title cards. H1 48px wght 850 wdth 135%; page H1 34px; H2 21px wdth 112%.
- Atkinson Hyperlegible Next: everything else, 15px body, tabular numbers.
- No monospace, no all-caps labels, no eyebrows, no "A · B · C" meta strings, no sparkle icons (the agent's icon is the face).

## Structure
- Cobalt rail. The active item is a paper tab that joins the page it opens.
- Lists with hairlines over card grids; sheets (12px) only for real groups; controls 8px; thumbnails 4px; toggles and markers squared.
- Layer gauge: 4 stacked bars, one filled, + "App / Workspace / Profile / Video" next to every inherited value.
- Thumbnails: flat field, condensed Anybody title, creator silhouette.
- Steppers are segmented tracks (filled = done, cobalt = current).

## Inside the agent (the one bold moment)
- Generation screens turn deep cobalt. The plan is an editing timeline (stage lanes, step clips, time ruler) with a yellow "now" playhead, the only thing that moves on its own (one 1.1s ease-out-quart slide on load).
- The agent's current thought is shown as a subtitle caption next to the face mark, whose eyes look at the work.
- The user's review sits on a paper sheet on top of the cobalt; reasoning, tool calls and the queue stay on cobalt.

## Interaction
- Decisions staged until "Send N decisions" in a navy tray; batch bar with a shared comment; comments show "not sent yet".
- Motion only answers actions (120–160ms), except the playhead. Respect reduced motion.
- Copy: sentence case, buttons name the result ("Generate a Tool reviews video", "Send 2 decisions", "Replace everything").
