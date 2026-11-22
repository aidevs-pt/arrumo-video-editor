"use client";

import { usePluginText } from "@/lib/plugin-text";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useLocale } from "@/lib/locale-provider";

type ContextRequest = {
  pluginId: string;
  pluginName: string;
  packageSha256: string;
  granted: boolean;
};

export function PluginGrants({ workspace }: { workspace: { id: number; name: string } }) {
  const pluginText = usePluginText();
  const { messages: { pluginGrants: copy, workspaces, pluginCatalog } } = useLocale();
  const [requests, setRequests] = useState<ContextRequest[]>([]);
  const [attempt, setAttempt] = useState(0);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<"loadError" | "writeError">();
  const [notice, setNotice] = useState<"grantedNotice" | "revokedNotice">();
  const workspaceId = workspace.id;

  useEffect(() => {
    let active = true;
    invoke<ContextRequest[]>("list_plugin_context_requests", { workspaceId }).then(
      (items) => { if (active) setRequests(items); },
      () => { if (active) setError("loadError"); },
    );
    return () => { active = false; };
  }, [workspaceId, attempt]);

  function refresh() {
    setError(undefined);
    setAttempt((current) => current + 1);
  }

  async function change({ pluginId, packageSha256, granted }: ContextRequest) {
    setPending(true);
    setError(undefined);
    setNotice(undefined);
    try {
      await invoke("apply_plugin_context_grant", { pluginId, packageSha256, workspaceId, allowed: !granted });
      setRequests((current) => current.map((entry) =>
        entry.pluginId === pluginId && entry.packageSha256 === packageSha256
          ? { ...entry, granted: !granted } : entry,
      ));
      setNotice(granted ? "revokedNotice" : "grantedNotice");
    } catch {
      setError("writeError");
    } finally {
      setPending(false);
    }
  }

  if (!requests.length && !error) return null;

  return <Card>
    <CardHeader>
      <CardTitle>{copy.title}</CardTitle>
      <CardDescription>{copy.description}</CardDescription>
    </CardHeader>
    <CardContent className="flex flex-col gap-3">
      {error && <Alert variant="destructive">
        <AlertDescription>{copy[error]}</AlertDescription>
        <Button type="button" variant="outline" onClick={refresh}>{workspaces.retry}</Button>
      </Alert>}
      <ul className="divide-y">
        {requests.map((request) => (
          <li key={request.pluginId + ":" + request.packageSha256} className="flex flex-wrap items-center gap-2 py-2">
            <span className="min-w-28 flex-1 text-sm font-medium wrap-anywhere">{pluginText.name(request.pluginId, request.pluginName)}</span>
            <Badge variant="secondary">{request.granted ? copy.allowed : copy.denied}</Badge>
            {request.granted ? (
              <Button size="sm" variant="outline" disabled={pending || Boolean(error)} onClick={() => void change(request)}>{copy.revoke}</Button>
            ) : (
              <AlertDialog>
                <AlertDialogTrigger asChild>
                  <Button size="sm" variant="outline" disabled={pending || Boolean(error)}>{copy.grant}</Button>
                </AlertDialogTrigger>
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>{copy.grant}</AlertDialogTitle>
                    <AlertDialogDescription>
                      {copy.confirm.replace(/\{(plugin|workspace)\}/g, (_, name) => name === "plugin" ? pluginText.name(request.pluginId, request.pluginName) : workspace.name)}
                    </AlertDialogDescription>
                  </AlertDialogHeader>
                  <AlertDialogFooter>
                    <AlertDialogCancel>{pluginCatalog.cancel}</AlertDialogCancel>
                    <AlertDialogAction onClick={() => void change(request)}>{copy.allow}</AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            )}
          </li>
        ))}
      </ul>
      <p role="status" className={notice ? "text-sm text-muted-foreground" : "sr-only"}>{notice && copy[notice]}</p>
    </CardContent>
  </Card>;
}
