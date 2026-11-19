import { WorkspaceAssets } from "@/components/workspace/assets/workspace-assets";
import { WorkspaceCatalogView } from "@/components/workspace/home/workspace-catalog-view";

export default function Page() {
  return <WorkspaceCatalogView view={WorkspaceAssets} />;
}
