import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

/** Where an agent's CLI was found, and the program chosen for it in settings. */
export type AgentCliLocation = { found: string | null; chosen: string | null };

export function agentCliLocation(provider: string) {
  return invoke<AgentCliLocation>("agent_cli_location", { provider });
}

/** Asks for the agent's program file and saves it. `undefined` when the person cancels. */
export async function chooseAgentProgram(provider: string, title: string): Promise<AgentCliLocation | undefined> {
  const path = await open({ multiple: false, directory: false, title });
  if (typeof path !== "string") return undefined;
  return invoke<AgentCliLocation>("set_agent_cli_path", { provider, path });
}

/** Forgets the chosen program, so the CLI is found on the PATH again. */
export function resetAgentProgram(provider: string) {
  return invoke<AgentCliLocation>("set_agent_cli_path", { provider, path: null });
}
