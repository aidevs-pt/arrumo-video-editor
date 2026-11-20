import { WorkspaceCatalogView } from "@/components/workspace/home/workspace-catalog-view";
import { WorkspaceMotionLibrary } from "@/components/workspace/motion-library/workspace-motion-library";

export default function Page() {
  return <WorkspaceCatalogView view={WorkspaceMotionLibrary} />;
}
