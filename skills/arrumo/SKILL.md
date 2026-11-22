---
name: arrumo
description: Make and edit videos with the Arrumo desktop editor through its local MCP server. Import footage, write a brief, queue a run, review each stage (script, assets, previews, final render) and export the verified files. Arrumo does the cutting, rendering and loudness; you supply judgement, review notes and taste.
---

# Arrumo

Arrumo is a local video editor. A *run* takes a draft (a brief plus optional footage) through stages: **script → assets → previews → final render → delivery check**. Its own agent produces each stage; in guided mode every stage waits for a review. You drive the run and review it like a careful editor would.

## Connect

Add a stdio MCP server that runs `arrumo-workspaces <absolute-database-path> mcp` (see [Connect to Arrumo](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/en/developer/connect.md)). The desktop app keeps its database at `workspaces.sqlite` in its data folder, `pt.aidevs.arrumo` under the platform's app-data directory. Most tools take `workspaceId`; start with `list_workspaces`.

## Principles

1. **Ask, confirm, then run.** Look at the footage and the workspace first, ask questions shaped by the material, and confirm the plan in plain words before queuing anything.
2. **Audio leads.** Cuts come from speech boundaries and pauses. Arrumo already transcribes footage, shows the agent phrase-level timings, moves any cut edge off a spoken word, fades every cut and evens out the final loudness to -14 LUFS. Don't ask its agent for these again.
3. **Review like a critic.** Each review note should name a moment (a time or script line), say what is wrong and what would fix it. "Make it better" teaches nothing.
4. **Teach the workspace once.** Feedback that applies to every video (tone, pacing, words to avoid) belongs in `add_workspace_feedback`, not repeated in each review.
5. **Never guess what kind of video it is.** Look first, ask second, edit last.
6. **Read the craft notes.** `list_editing_guides` and `get_editing_guide` return the guides Arrumo's own agent follows at each stage (script, visuals, animation, cutting, storyboard, portrait cuts, publishing text, review). Read the one for the stage you are reviewing so your notes and its work pull the same way.

## The process

1. **Inventory.** `list_workspace_assets`, `get_workspace_context`, `list_profiles`, `list_output_formats`, `list_workspace_feedback`. Import new footage with `import_workspace_asset` (absolute local paths only).
2. **Converse.** Collect what the material needs: purpose, audience, length, formats, must-keep and must-cut moments, tone, animation and subtitle wishes.
3. **Propose.** Four to eight sentences: shape, takes, pacing, animations, formats, length. Wait for a yes.
4. **Draft and queue.** `create_video_draft` with `title`, `intent` (the brief), `source_ids` in viewing order and an optional `profile_id`. Then `enqueue_video_draft` with the `draftId` and a fresh `requestKey`; reuse that key only to retry the same submission. Leave `autopilot` off unless the creator asked for it.
5. **Review each stage.** Poll `get_production_progress`. When a result is pending:
   - Script, assets and supplemental items: read the result, then `review_production_result` with `approved`, `revise` (with notes) or `rejected`. On assets approval, `saveMotionIndexes` chooses which new animations join the library.
   - Previews: one decision per format with `review_preview_formats`. Approved formats are kept; only formats marked `revise` are rendered again.
   - Read `get_agent_log` when a result surprises you, to see what Arrumo's agent was asked.
6. **Check the preview before approving.** For footage runs, compare the cuts with the transcript: no clipped first or last words, no repeated take, punchlines and reactions kept whole, no long dead air, the video ends on a finished thought. For storyboards, check that each scene's text is readable at phone size and stays on screen long enough to read. At each doubtful moment, `render_frame_strip` draws frames above the waveform for a stretch of up to 120 s of a preview (`runId`, `formatId`, `previewAttempt`), a final video or a footage asset into a PNG you can look at. Use it at decision points, not to scan a whole video, and say what you could not check.
7. **Deliver.** After the delivery check passes, `get_completed_video` lists the files; `export_generation_delivery` writes them to a ZIP at an absolute path the creator chose.
8. **Remember.** Save run-specific lessons with `add_completed_video_feedback` and lasting ones with `add_workspace_feedback`.

## Library media

Generate images, speech or video clips for the library with `generate_library_images`, `generate_library_speech` and `generate_library_video`. `list_video_models` shows each video provider's models and how many reference images or videos each one takes; pass workspace asset IDs as `references`. Generated media lands in the asset library with notes saying which provider and model made it. `list_speech_models` lists voice models, `transform_audio_asset` changes a voice or isolates it from noise, and `dub_asset` dubs an asset's speech into another language (`list_audio_transforms` and `list_dubbing_providers` show what the installed plugins offer).

## Subtitles in the picture

A format whose delivery sets `burned_captions` also ships a copy with subtitles drawn in. Its `caption_style` (font, size, colours, outline or box, position, margin) is set per format with `set_format_delivery`; a profile (`set_profile_delivery`) or one video (`format_overrides`) changes only the fields it names. `preview_caption_style` draws a style on a sample frame so you can check it before a run.

## Hard rules

- Use only workspace asset IDs and absolute local paths. Never pass URLs where a path is expected.
- Never paste API keys into tool arguments, notes or feedback. Keys live in **Settings › Services and keys** and the tools never return them.
- Check `get_provider_usage` before a long run. A provider at 20% or less may stop the run partway.
- A failed stage records why in `get_generation_activity`. Fix the cause, then call `control_generation` with `action: "retry"`; don't queue a duplicate draft.
