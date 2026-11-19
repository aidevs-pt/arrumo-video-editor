import { WorkspaceBrand } from "@/components/workspace/brand/workspace-brand";
import { WorkspaceCatalogView } from "@/components/workspace/home/workspace-catalog-view";

export default function Page() {
  return <WorkspaceCatalogView view={WorkspaceBrand} />;
}
