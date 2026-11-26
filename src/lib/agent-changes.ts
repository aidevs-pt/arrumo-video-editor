import { useEffect, useState } from "react";

import { PLUGINS_CHANGED } from "@/lib/plugin-text";

const AGENT_CHANGED = "arrumo:agent-changed";

/** Tells the open screens that the selected agent changed. */
export function announceAgentChange() {
  window.dispatchEvent(new Event(AGENT_CHANGED));
}

/** A number that changes whenever the selected agent may have, for effects that show it. */
export function useAgentChanges() {
  const [version, setVersion] = useState(0);
  useEffect(() => {
    const changed = () => setVersion((current) => current + 1);
    window.addEventListener(AGENT_CHANGED, changed);
    window.addEventListener(PLUGINS_CHANGED, changed);
    return () => {
      window.removeEventListener(AGENT_CHANGED, changed);
      window.removeEventListener(PLUGINS_CHANGED, changed);
    };
  }, []);
  return version;
}
