import { WorkspaceOutputs } from "@/components/workspace/outputs/workspace-outputs";
import { WorkspaceCatalogView } from "@/components/workspace/home/workspace-catalog-view";

export default function Page() {
  return <WorkspaceCatalogView view={WorkspaceOutputs} />;
}
