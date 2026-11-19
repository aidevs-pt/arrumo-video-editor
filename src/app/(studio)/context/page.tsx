import { WorkspaceContext } from "@/components/workspace/context/workspace-context";
import { WorkspaceCatalogView } from "@/components/workspace/home/workspace-catalog-view";

export default function Page() {
  return <WorkspaceCatalogView view={WorkspaceContext} />;
}
