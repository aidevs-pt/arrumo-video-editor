/** The face that covers the app while it starts. It is plain markup in the prerendered page, so it
 * paints before React, the language and the workspaces load; `markAppReady` fades it out.
 * The face parts keep the IDs from the supplied logo. */
export function LaunchScreen() {
  return (
    <div id="arrumo-launch" aria-hidden="true">
      <svg viewBox="156 156 712 712" className="arrumo-launch-face">
        <rect x="156" y="156" width="458.667" height="205.333" rx="102.667" id="long-left-eyebrow" />
        <rect x="662.667" y="156" width="205.333" height="205.333" rx="64" id="short-right-eyebrow" />
        <rect x="156" y="409.333" width="205.333" height="205.333" rx="64" id="left-eye" />
        <rect x="409.333" y="409.333" width="205.333" height="458.667" rx="102.667" id="long-nose" />
        <rect x="662.667" y="409.333" width="205.333" height="205.333" rx="64" id="right-eye" />
        <rect x="156" y="662.667" width="205.333" height="205.333" rx="64" id="left-cheek" />
        <rect x="662.667" y="662.667" width="205.333" height="205.333" rx="64" id="right-cheek" />
      </svg>
    </div>
  );
}

/** Lets the launch face finish its greeting (about a second after launch), then fades it out. */
export function markAppReady() {
  const root = document.documentElement;
  if (root.dataset.appReady !== undefined) return;
  const wait = root.dataset.noSplash !== undefined ? 0 : Math.max(0, 1300 - performance.now());
  window.setTimeout(() => { root.dataset.appReady = ""; }, wait);
}
