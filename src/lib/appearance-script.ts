// Kept out of the client provider's module: a server component that imports a value from a
// "use client" module receives a client reference, not the string.

export const appearanceStorageKey = "arrumo:appearance";

/** Runs before hydration so the first paint already uses the saved theme and motion choice, and
 * marks macOS, whose scrollbars keep their system look. */
export const appearanceScript = `try{if(/Mac/.test(navigator.platform||navigator.userAgent))document.documentElement.dataset.platform="mac"}catch(e){}try{var a=JSON.parse(localStorage.getItem("${appearanceStorageKey}")||"{}"),d=document.documentElement,m=matchMedia("(prefers-color-scheme: dark)").matches;if(a.theme==="dark"||(a.theme!=="light"&&m))d.classList.add("dark");if(a.motion==="reduce"||(a.motion!=="full"&&matchMedia("(prefers-reduced-motion: reduce)").matches))d.dataset.reduceMotion=""}catch(e){}`;
