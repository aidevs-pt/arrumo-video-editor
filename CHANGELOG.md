# Changelog

What changed in each release of Arrumo AI Video Editor.

## 0.1.14 (2026-11-27)

### New

- Draw 3D scenes, charts, particles, routes, Lottie animations and shader effects in animations with the new Canvas layers plugin, rendered in an isolated browser with your brand colours (#109)
- Reusable canvas components the agent writes once and other animations import (#109)
- Sharper guides for the agent, a guide for the look and motion of a video, and a guide per kind of video: explainer, data story, promo, short vertical, talking head, titles and animated short (#109)
- Change animations in place and bring back earlier versions (#104)
- Animation templates whose text each video fills in, with a preview of every use (#104)
- Select and delete several animations at once (#104)
- Go back to any finished step of a video, and decide all, the rest or a selection at once (#103)
- Switch formats off per profile (#106)
- Optionally delete uploaded clips once their video is finished (#107)
- Use your installed agent plugins with or without file and command access, and see your agent plan's usage, read between videos (#102)
- Choose the theme, usage data and crash reports when Arrumo first opens (#102)

### Fixes

- The review bar stays in view while you scroll (#103)
- Right-click in dialogs and menus no longer opens the menu of the item underneath (#105)
- Icon buttons match the height of labelled buttons, and detail panels keep their title and actions in view (#105)
- The usage table keeps its size while it loads (#102)
- The macOS disk image no longer replaces its volume icon (#108)

## 0.1.13 (2026-11-26)

### New

- Deliver videos queued without subtitles without waiting for them (#101)

### Fixes

- Generate subtitles by default and never fail a run for verifying early (#99)

## 0.1.12 (2026-11-26)

### New

- Group by cause, report silent failures, and send no location (#97)

### Fixes

- Keep the lease while a cut renders its animations (#95)

## 0.1.11 (2026-11-26)

### Fixes

- Say why a render failed and report the whole error (#93)
- Show the signing client's output when Windows signing fails (#91)

## 0.1.10 (2026-11-26)

### New

- Read the PATH again when checking for an agent's CLI (#86)

### Fixes

- Apply an agent switch right away and to videos in progress (#84)

## 0.1.9 (2026-11-25)

### New

- Improve content generation guidance and styling (#80)
- Stop synthesizing animation sounds with FFmpeg (#78)

### Fixes

- Keep every agent session in the progress log without gaps (#76)

## 0.1.8 (2026-11-25)

### New

- Step timings and warnings under the usage data setting (#70)

### Fixes

- Stop storyboards with music and no voice from failing at cleanup (#73)
- Give macOS the same themed scrollbars as Windows (#72)
- Apply the setting and scrubbing to unparsed interface reports (#68)

## 0.1.7 (2026-11-25)

### New

- Install-box volume icon, Windows setup icon and a taller Retina disk image background (#52)

### Fixes

- Say what and where every failure happened; macOS scrollbars (#51)
- Stop step restarts, keep one agent session per round (#50)
- Let bundled provider adapters grow to 256 KiB (#49)

## 0.1.6 (2026-11-25)

### New

- Portrait voice-over, new animations in cuts, review aids, platform notes (#47)
- Let creators set how much media the agent may order (#46)

## 0.1.5 (2026-11-25)

### New

- Preview planned assets and fold the agent's raw output (#45)
- Order media from any step, and add voice-over and sound effects to previews (#44)

### Fixes

- Use the current model, effort and web access in every agent step (#43)

## 0.1.4 (2026-11-24)

### New

- Let the agent order generated clips for a video (#40)

### Fixes

- Correct text layout and review animations before showing them (#41)
- Dark theme selections and sidebar, and wide windows (#42)
- Send the agent's model and effort, finish publishing text, detailed failure reports (#39)
- Count an install only when its database is created (#38)

## 0.1.3 (2026-11-24)

### New

- Pick assets from a picker that shows what each one is (#35)
- Describe assets with the agent (#33)
- Removal releases settings, bulk removal, and says what keeps an asset (#31)

### Fixes

- Stop the run filter tabs from scrolling vertically (#34)
- Count installs once, drop video_queued, add animation_generated (#32)
- Clearer inherited outputs on profiles; header stays above focused content (#30)
- Keep focus rings whole inside accordions (#29)

## 0.1.2 (2026-11-24)

### Fixes

- Stop a lingering shell helper from blocking the app's launch (#28)
- Installer title, no console windows for FFmpeg, capped text areas in dialogs (#27)

## 0.1.1 (2026-11-24)

### Fixes

- Up-to-date check before the first full release, plugin tour on new installs (#24)
- Brand font previews, agent-only providers when not installed, and reports of handled failures (#23)
- Keep dialog and menu actions within their space in every language (#22)
- Render animations on Windows and macOS (#21)

## 0.1.0 (2026-11-24)

The first release of Arrumo AI Video Editor.
