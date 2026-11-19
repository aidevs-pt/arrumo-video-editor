import { Suspense, type ReactNode } from "react";
import { WorkspaceRouter } from "@/components/workspace/home/workspace-router";

export default function StudioLayout({ children }: { children: ReactNode }) {
  return <Suspense><WorkspaceRouter>{children}</WorkspaceRouter></Suspense>;
}
