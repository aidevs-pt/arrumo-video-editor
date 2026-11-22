# Create and export your first video

[Español](../es/first-video.md) · [Português](../pt/first-video.md)

## Set up a workspace

Create a workspace and choose the editing agent available on your machine. If you plan to generate subtitles from audio, configure a transcription provider that supplies timestamps. A credential for one provider does not give access to another.

Each workspace starts with its own icon and color. To change them, or to show an image such as your channel avatar, choose **Customize workspace** in the workspace menu.

Open **Outputs** and check which formats are enabled. Each enabled format needs its own final MP4 and SRT for the delivery to pass verification.

## Save and queue a draft

1. Open **Videos** and go to **Drafts**. Add recordings to a draft or choose **Start from scratch** and describe what you want the video to say.
2. Give the draft a title and add any instructions that affect the edit. Save it so you can return to it before starting production.
3. Choose **Generate** on the draft. Use the run options to choose providers and decide whether to use **Autopilot**. Autopilot advances through review gates. A run can still wait for rendering or files.
4. Turn on **Generate missing subtitles automatically** if you want Arrumo to send audio from each rendered format missing an SRT to a provider selected for the run. This option is off by default.
5. Select **Generate this video**. Open **Generating** to follow the run.

## Review and finish the delivery

For a guided run, open **Review run** when a stage needs a decision. Check the available script, media, or preview. Stage your decisions and send them when you are ready. If you want a revision, describe the change in the stage’s notes before sending.

At **Assets and animations**, the agent also designs animations for the video, such as a title card, lower thirds or an end card, and Arrumo renders them for you to review. A video started from scratch has no footage, so its preview is a storyboard of animated scenes built from the approved script and animations. Its captions come from the text on screen. If a text-to-speech provider such as ElevenLabs is set up, the storyboard is narrated and its captions follow the narration.

Formats that ask for portrait cuts get short vertical videos taken from the approved video, each with its own thumbnails, ready to review with the titles and descriptions.

To see what the agent was asked and what it printed, open **Agent logs** next to the run's activity. A failed step shows its error output there.

At the final render stage, use **Render final video** when the action is available. In **Verify delivery**, use **Generate subtitles** or **Import SRT** for any format still missing captions. An imported SRT must be UTF-8 and its cues must fit within that format’s video duration.

If automatic transcription fails or was interrupted, you can retry explicitly with **Generate subtitles** or import a file. An automatic attempt is not resent after interruption because the provider may already have processed it. Existing subtitles are preserved by generation.

The desktop verifies the delivery when all enabled formats have managed MP4 and SRT files. If the run stays queued at verification, check for a missing file on another format.

## Open and export the result

Go to **Videos** → **Completed** and open the finished video. Choose an output to preview it or open **Subtitle text** to inspect its cues. Use **Export ZIP** to save the verified delivery files to a new ZIP.

## Back up and restore

**Settings › Import and export** saves everything (settings, workspaces, profiles, assets, animations, finished videos and installed plugins) to one `.zip`. API keys are not included; they stay in this computer's credential store, so add them again on a new computer.

To restore, choose a backup in the same place. Arrumo checks every file against the backup's checksums first, then shows what it contains (workspaces, plugins, size) and asks before replacing anything; **Export mine first** saves your current data before you decide. It restarts to restore, and keeps the data it replaced in a folder named `workspaces.sqlite.before-import-…` next to your data, in case you need it back.

## Data and privacy

Arrumo counts which features are used and reports crashes, to learn what to improve. Both are on by default and can be turned off in **Settings › General › Data and privacy**. They are tied to a random ID for this install, never to you, and never include your videos, files, text or keys. Builds you compile yourself send nothing.

## New plugins

When plugins arrive, from a ZIP, the registry or an Arrumo update, Arrumo introduces them in a short tour, one page each. Skip it whenever you like; **Settings › Plugins › Take the plugin tour** shows every plugin's introduction again.
