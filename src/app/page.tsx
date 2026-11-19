"use client";

import { invoke, isTauri } from "@tauri-apps/api/core";
import { motion, useReducedMotion } from "motion/react";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { WorkspaceOutputs } from "./workspace-outputs";
import { WorkspaceProfiles } from "./workspace-profiles";
import { workspaceCopy, workspaceLocale } from "./workspace-copy";

type Workspace = { id: number; name: string };
type Status = "loading" | "ready" | "error" | "browser";

export default function Page() {
  const [locale, setLocale] = useState<keyof typeof workspaceCopy>("en");
  const [status, setStatus] = useState<Status>("loading");
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [showOutputs, setShowOutputs] = useState(false);
  const [selected, setSelected] = useState<Workspace | null>(null);
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const reducedMotion = useReducedMotion();
  const copy = workspaceCopy[locale];

  async function loadWorkspaces() {
    if (!isTauri()) {
      setStatus("browser");
      return;
    }
    setStatus("loading");
    try {
      setWorkspaces(await invoke<Workspace[]>("list_workspaces"));
      setStatus("ready");
    } catch {
      setStatus("error");
    }
  }

  useEffect(() => {
    const currentLocale = workspaceLocale(navigator.language);
    document.documentElement.lang = currentLocale;
    setLocale(currentLocale);
    void loadWorkspaces();
  }, []);

  async function createWorkspace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!name.trim() || saving || status !== "ready") return;
    setSaving(true);
    setSaveError(false);
    try {
      const workspace = await invoke<Workspace>("create_workspace", { name });
      setWorkspaces((current) => [...current, workspace]);
      setName("");
    } catch {
      setSaveError(true);
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <header className="app-bar">
        {/* The original SVG carries the supplied Arrumo identity. */}
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img src="/arrumo-icon.svg" alt="" width={28} height={28} />
        <span>Arrumo</span>
      </header>
      {selected && showOutputs ? (
        <WorkspaceOutputs
          key={selected.id}
          workspace={selected}
          locale={locale}
          onBack={() => setShowOutputs(false)}
        />
      ) : selected ? (
        <WorkspaceProfiles
          key={selected.id}
          workspace={selected}
          locale={locale}
          onBack={() => setSelected(null)}
          onOutputs={() => setShowOutputs(true)}
        />
      ) : (
        <main className="workspace-shell">
          <div className="workspace-heading">
            <h1>{copy.title}</h1>
            <p>{copy.intro}</p>
          </div>
          <div className="workspace-layout">
            <section className="workspace-list" aria-labelledby="workspace-list-title">
              <h2 id="workspace-list-title">{copy.list}</h2>
              {status === "loading" && <p role="status">{copy.loading}</p>}
              {status === "browser" && <p role="status">{copy.desktop}</p>}
              {status === "error" && (
                <Alert variant="destructive">
                  <AlertDescription>{copy.loadError}</AlertDescription>
                  <Button type="button" variant="outline" onClick={loadWorkspaces}>{copy.retry}</Button>
                </Alert>
              )}
              {status === "ready" && (workspaces.length ? (
                <ul className="workspace-rows" aria-live="polite">
                  {workspaces.map((workspace) => (
                    <motion.li
                      key={workspace.id}
                      initial={reducedMotion ? false : { opacity: 0.75, y: 5 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ duration: 0.22, ease: "easeOut" }}
                    >
                      <Button className="workspace-open" variant="ghost" onClick={() => setSelected(workspace)}>
                        {workspace.name}
                      </Button>
                    </motion.li>
                  ))}
                </ul>
              ) : (
                <Empty className="workspace-empty">
                  <EmptyHeader>
                    <EmptyTitle>{copy.empty}</EmptyTitle>
                    <EmptyDescription>{copy.emptyHint}</EmptyDescription>
                  </EmptyHeader>
                </Empty>
              ))}
            </section>
            <form className="workspace-form" onSubmit={createWorkspace} aria-labelledby="new-workspace-title">
              <h2 id="new-workspace-title">{copy.form}</h2>
              <FieldGroup>
                <Field data-disabled={status !== "ready" || saving}>
                  <FieldLabel htmlFor="workspace-name">{copy.name}</FieldLabel>
                  <Input
                    id="workspace-name"
                    value={name}
                    onChange={(event) => setName(event.target.value)}
                    disabled={status !== "ready" || saving}
                    required
                  />
                </Field>
                {saveError && <Alert variant="destructive" role="alert"><AlertDescription>{copy.createError}</AlertDescription></Alert>}
                <Button type="submit" disabled={status !== "ready" || saving || !name.trim()}>
                  {saving ? copy.creating : copy.create}
                </Button>
              </FieldGroup>
            </form>
          </div>
        </main>
      )}
    </>
  );
}
