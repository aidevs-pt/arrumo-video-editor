"use client";

import type { ComponentType } from "react";
import type { Workspace } from "./workspace-frame";
import { useWorkspaceCatalog } from "./workspace-router";

export function WorkspaceCatalogView({ view: View }: { view: ComponentType<{ workspace: Workspace }> }) {
  const { workspace } = useWorkspaceCatalog();
  return workspace ? <View key={workspace.id} workspace={workspace} /> : null;
}
