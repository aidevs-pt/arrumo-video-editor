import { Fragment } from "react";

const TOKEN = /(\/\*[\s\S]*?\*\/|\/\/[^\n]*)|(`(?:\\[\s\S]|[^`\\])*`|"(?:\\.|[^"\\\n])*"|'(?:\\.|[^'\\\n])*')|\b(\d+(?:\.\d+)?)\b|\b(import|export|default|from|const|let|var|function|return|if|else|for|while|of|in|new|true|false|null|undefined|this|typeof|async|await|class|extends|break|continue|switch|case)\b|\b([A-Za-z_$][\w$]*)(?=\s*\()/g;
const STYLES = ["text-white/50 italic", "text-[#f2c66d]", "text-[#ff9e80]", "text-[#9fb4ff]", "text-[#7fe0c3]"];

/** Colours JavaScript for reading: comments, strings, numbers, keywords and calls. */
export function HighlightedCode({ source }: { source: string }) {
  const parts: React.ReactNode[] = [];
  let last = 0;
  for (const match of source.matchAll(TOKEN)) {
    const at = match.index ?? 0;
    if (at > last) parts.push(source.slice(last, at));
    const group = match.slice(1).findIndex((value) => value !== undefined);
    parts.push(<span key={at} className={STYLES[group]}>{match[0]}</span>);
    last = at + match[0].length;
  }
  parts.push(source.slice(last));
  return <>{parts.map((part, index) => <Fragment key={index}>{part}</Fragment>)}</>;
}
