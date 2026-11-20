"use client";

import { StudioDashboard } from "@/components/workspace/home/studio-dashboard";
import { WorkspaceHome } from "@/components/workspace/home/workspace-home";
import { useWorkspaceCatalog } from "@/components/workspace/home/workspace-router";

export default function Page() {
  const { workspace } = useWorkspaceCatalog();
  return workspace ? <StudioDashboard key={workspace.id} workspace={workspace} /> : <WorkspaceHome />;
}
