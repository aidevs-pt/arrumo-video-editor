"use client";

import { invoke } from "@tauri-apps/api/core";
import { Check, Copy, FileUp, Zap } from "lucide-react";
import { useEffect, useId, useRef, useState, type FormEvent, type ReactNode } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";
import { usePluginText } from "@/lib/plugin-text";

export type ActionInputKind = "text" | "longText" | "number" | "boolean" | "choice" | "language" | "subtitles" | "json";
export type ActionPlacement = "draft" | "generation" | "completed-video";
export type ActionInput = {
  id: string;
  label: string;
  description?: string;
  kind: ActionInputKind;
  required?: boolean;
  choices?: string[];
};
export type ActionHook = { event: "video-completed" | "generation-failed" | "publish-date-reached"; enabledBy: string };
export type InstalledAction = {
  pluginId: string;
  pluginName: string;
  packageSha256: string;
  action: { id: string; label: string; description: string; inputs?: ActionInput[]; placements?: ActionPlacement[]; hooks?: ActionHook[] };
};
export type ActionJob = {
  id: string;
  pluginId: string;
  actionId: string;
  state: "queued" | "running" | "completed" | "failed";
  result: string | null;
};

/** What the place hosting an action can offer its form, such as the subtitle tracks of the video in view. */
export type ActionContext = {
  /** Lists the subtitle tracks the creator can pick; called when a form asks for subtitles. */
  subtitleTracks?: () => Promise<SubtitleTrack[]>;
};
export type SubtitleTrack = { id: string; label: string; load: () => Promise<string> };

const MAX_INPUT = 65_536;
/** Common subtitle and dubbing languages, named in the creator's language. */
const LANGUAGES = ["en", "es", "pt", "fr", "de", "it", "nl", "pl", "sv", "ru", "uk", "tr", "ar", "hi", "ja", "ko", "zh", "id"];

type Values = Record<string, string | boolean>;

/** Runs a plugin action from the form its manifest declares, or from a JSON object when it declares none. */
export function ActionForm({ entry, workspaceId, context = {}, onChanged }: {
  entry: InstalledAction;
  workspaceId: number | null;
  context?: ActionContext;
  onChanged?: () => void;
}) {
  const pluginText = usePluginText();
  const { messages: { pluginActions: copy } } = useLocale();
  const id = useId();
  const inputs = (entry.action.inputs ?? []).map((input) => pluginText.actionInput(entry.pluginId, entry.action.id, { ...input, description: input.description ?? "" }));
  const [values, setValues] = useState<Values>(() => Object.fromEntries(inputs.map((input) => [input.id, input.kind === "boolean" ? false : ""])));
  const [raw, setRaw] = useState("{}");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string>();
  const [output, setOutput] = useState<string>();

  function set(key: string, value: string | boolean) {
    setValues((current) => ({ ...current, [key]: value }));
    setError(undefined);
    setOutput(undefined);
  }

  function build(): string | undefined {
    if (!inputs.length) {
      try { JSON.parse(raw); } catch { setError(copy.invalidInput); return; }
      return raw;
    }
    const input: Record<string, unknown> = {};
    for (const field of inputs) {
      const value = values[field.id];
      if (field.kind === "boolean") { input[field.id] = value === true; continue; }
      const text = typeof value === "string" ? value : "";
      if (!text.trim()) {
        if (field.required) { setError(copy.required.replace("{field}", field.label)); return; }
        continue;
      }
      if (field.kind === "number") {
        const number = Number(text);
        if (!Number.isFinite(number)) { setError(copy.invalidNumber.replace("{field}", field.label)); return; }
        input[field.id] = number;
      } else if (field.kind === "json") {
        try { input[field.id] = JSON.parse(text); } catch { setError(copy.invalidJson.replace("{field}", field.label)); return; }
      } else {
        input[field.id] = text;
      }
    }
    const json = JSON.stringify(input);
    if (json.length > MAX_INPUT) { setError(copy.invalidInput); return; }
    return json;
  }

  async function run(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    // The sheet is portalled, but React still bubbles its submit to a form around the placement.
    event.stopPropagation();
    if (pending) return;
    setError(undefined);
    setOutput(undefined);
    const inputJson = build();
    if (inputJson === undefined) return;
    setPending(true);
    try {
      const job = crypto.randomUUID();
      await invoke<void>("queue_plugin_action_job", { request: {
        id: job, workspaceId, inputJson, actionId: entry.action.id,
        pluginId: entry.pluginId, packageSha256: entry.packageSha256,
      } });
      onChanged?.();
      const done = await invoke<ActionJob>("process_plugin_action_job", { workspaceId, id: job });
      if (done.state === "failed") setError(copy.runError);
      if (done.state === "completed") setOutput(done.result ?? "");
    } catch (reason) {
      setError(reason === "plugin_action_input_invalid" ? copy.invalidInput : copy.runError);
    } finally {
      onChanged?.();
      setPending(false);
    }
  }

  return <form className="flex flex-col gap-4" onSubmit={run} aria-busy={pending} noValidate>
    <p className="text-sm text-muted-foreground">{pluginText.action(entry.pluginId, entry.action.id, "description", entry.action.description)}</p>
    {entry.action.hooks?.map((hook) => <Alert key={hook.event}>
      <Zap />
      <AlertDescription>{copy.hook[hook.event].replace("{setting}", pluginText.settingLabel(entry.pluginId, hook.enabledBy))}</AlertDescription>
    </Alert>)}
    {inputs.length ? <FieldGroup className="gap-4">
      {inputs.map((input) => <InputField key={input.id} id={`${id}-${input.id}`} input={input} value={values[input.id]}
        disabled={pending} context={context} onChange={(value) => set(input.id, value)} />)}
    </FieldGroup> : <Field>
      <FieldLabel htmlFor={`${id}-raw`}>{copy.input}</FieldLabel>
      <Textarea id={`${id}-raw`} className="max-h-60 font-mono" spellCheck={false} maxLength={MAX_INPUT} disabled={pending}
        value={raw} onChange={(event) => { setRaw(event.target.value); setError(undefined); setOutput(undefined); }} />
    </Field>}
    <Button type="submit" className="self-start" disabled={pending}>{pending ? copy.running : copy.run}</Button>
    {error && <Alert variant="destructive"><AlertDescription>{error}</AlertDescription></Alert>}
    {output !== undefined && <ActionResult result={output} />}
    <span role="status" className="sr-only">{output !== undefined ? copy.finished : ""}</span>
  </form>;
}

function InputField({ id, input, value, disabled, context, onChange }: {
  id: string;
  input: ActionInput & { description: string; choiceLabels: Record<string, string> };
  value: string | boolean | undefined;
  disabled: boolean;
  context: ActionContext;
  onChange: (value: string | boolean) => void;
}) {
  const { locale, messages: { pluginActions: copy } } = useLocale();
  const text = typeof value === "string" ? value : "";
  const label = <FieldLabel htmlFor={id}>
    {input.label}{!input.required && input.kind !== "boolean" && <span className="font-normal text-muted-foreground"> · {copy.optional}</span>}
  </FieldLabel>;
  const description = input.description ? <FieldDescription id={`${id}-description`}>{input.description}</FieldDescription> : null;
  const described = input.description ? `${id}-description` : undefined;

  if (input.kind === "boolean") return <Field orientation="horizontal">
    <Checkbox id={id} checked={value === true} disabled={disabled} aria-describedby={described} onCheckedChange={(checked) => onChange(checked === true)} />
    <div className="flex flex-col gap-1">{label}{description}</div>
  </Field>;

  if (input.kind === "subtitles") return <SubtitlesField id={id} label={label} description={description} described={described}
    value={text} disabled={disabled} listTracks={context.subtitleTracks} onChange={onChange} />;

  let control;
  if (input.kind === "choice" || input.kind === "language") {
    const names = new Intl.DisplayNames([locale], { type: "language" });
    const options = input.kind === "choice"
      ? (input.choices ?? []).map((choice) => [choice, input.choiceLabels[choice] ?? choice])
      : LANGUAGES.map((code) => [code, names.of(code) ?? code]).sort((a, b) => a[1].localeCompare(b[1], locale));
    control = <NativeSelect id={id} className="w-full" value={text} disabled={disabled} aria-describedby={described}
      onChange={(event) => onChange(event.target.value)}>
      <NativeSelectOption value="">{input.kind === "language" ? copy.chooseLanguage : copy.chooseOption}</NativeSelectOption>
      {options.map(([option, name]) => <NativeSelectOption key={option} value={option}>{name}</NativeSelectOption>)}
    </NativeSelect>;
  } else if (input.kind === "longText" || input.kind === "json") {
    control = <Textarea id={id} value={text} disabled={disabled} aria-describedby={described} maxLength={MAX_INPUT}
      className={input.kind === "json" ? "max-h-60 font-mono" : "max-h-60"} spellCheck={input.kind !== "json"}
      onChange={(event) => onChange(event.target.value)} />;
  } else {
    control = <Input id={id} value={text} disabled={disabled} aria-describedby={described}
      inputMode={input.kind === "number" ? "decimal" : undefined} maxLength={input.kind === "number" ? 32 : 2000}
      onChange={(event) => onChange(event.target.value)} />;
  }
  return <Field>{label}{control}{description}</Field>;
}

function SubtitlesField({ id, label, description, described, value, disabled, listTracks, onChange }: {
  id: string;
  label: ReactNode;
  description: ReactNode;
  described?: string;
  value: string;
  disabled: boolean;
  listTracks?: ActionContext["subtitleTracks"];
  onChange: (value: string) => void;
}) {
  const { messages: { pluginActions: copy } } = useLocale();
  const file = useRef<HTMLInputElement>(null);
  const [tracks, setTracks] = useState<SubtitleTrack[]>([]);
  const [source, setSource] = useState("");
  const [loading, setLoading] = useState(!!listTracks);
  const [problem, setProblem] = useState<string>();
  const loaded = useRef<string>(undefined);

  async function choose(track: string, from = tracks) {
    setSource(track);
    setProblem(undefined);
    onChange("");
    const chosen = from.find((entry) => entry.id === track);
    if (!chosen) return;
    setLoading(true);
    try {
      loaded.current = track;
      const subtitles = await chosen.load();
      if (loaded.current === track) onChange(subtitles);
    } catch {
      setProblem(copy.trackError);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (!listTracks) return;
    let active = true;
    listTracks().then((found) => {
      if (!active) return;
      setTracks(found);
      if (found[0]) void choose(found[0].id, found);
      else setLoading(false);
    }, () => { if (active) { setLoading(false); setProblem(copy.trackError); } });
    return () => { active = false; };
    // Lists the tracks once, when the form opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function open(files: FileList | null) {
    const chosen = files?.[0];
    if (!chosen) return;
    if (chosen.size > MAX_INPUT) { setProblem(copy.fileError); return; }
    try {
      onChange(await chosen.text());
      setProblem(undefined);
    } catch {
      setProblem(copy.fileError);
    }
  }

  return <Field data-invalid={!!problem}>
    {label}
    {tracks.length > 0 && <NativeSelect id={id} className="w-full" value={source} disabled={disabled || loading}
      aria-label={copy.subtitlesSource} onChange={(event) => void choose(event.target.value)}>
      {tracks.map((track) => <NativeSelectOption key={track.id} value={track.id}>{track.label}</NativeSelectOption>)}
      <NativeSelectOption value="">{copy.subtitlesOwn}</NativeSelectOption>
    </NativeSelect>}
    {!loading && (!tracks.length || source === "") && <>
      <Textarea id={tracks.length ? `${id}-text` : id} value={value} disabled={disabled} aria-describedby={described}
        className="max-h-60 font-mono text-xs" spellCheck={false} maxLength={MAX_INPUT} placeholder={"1\n00:00:00,000 --> 00:00:02,000\n…"}
        onChange={(event) => onChange(event.target.value)} />
      <input ref={file} type="file" accept=".srt,application/x-subrip,text/plain" className="sr-only" tabIndex={-1}
        onChange={(event) => { void open(event.target.files); event.target.value = ""; }} />
      <Button type="button" variant="outline" size="sm" className="self-start" disabled={disabled} onClick={() => file.current?.click()}>
        <FileUp data-icon="inline-start" />{copy.openFile}
      </Button>
    </>}
    {source !== "" && value && <p className="text-xs text-muted-foreground tabular-nums">{copy.cues.replace("{count}", String(value.split(/\n\s*\n/).filter((cue) => cue.trim()).length))}</p>}
    {description}
    {problem && <p role="alert" className="text-sm text-destructive">{problem}</p>}
  </Field>;
}

/** A plugin's result: text fields readable on their own, anything else as formatted JSON. */
export function ActionResult({ result }: { result: string }) {
  const { messages: { pluginActions: copy } } = useLocale();
  let parsed: unknown = result;
  try { parsed = JSON.parse(result); } catch { /* shown as text */ }
  const body = parsed && typeof parsed === "object" && !Array.isArray(parsed) && "ok" in parsed && "result" in parsed
    ? (parsed as { result: unknown }).result : parsed;
  const fields = body && typeof body === "object" && !Array.isArray(body)
    ? Object.entries(body as Record<string, unknown>) : [[copy.result, body] as const];
  return <section className="flex min-w-0 flex-col gap-3" aria-label={copy.result}>
    <h3 className="text-sm font-bold">{copy.result}</h3>
    {fields.map(([name, value]) => <ResultBlock key={name} name={name}
      text={typeof value === "string" ? value : JSON.stringify(value, null, 2)} />)}
  </section>;
}

function ResultBlock({ name, text }: { name: string; text: string }) {
  const { messages: { pluginActions: copy } } = useLocale();
  const [copied, setCopied] = useState(false);
  return <div className="flex min-w-0 flex-col gap-1.5">
    <div className="flex items-center justify-between gap-2">
      <span className="text-xs font-bold text-muted-foreground uppercase">{name}</span>
      <Button type="button" variant="ghost" size="sm" onClick={() => void navigator.clipboard.writeText(text).then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      }, () => undefined)}>
        {copied ? <Check data-icon="inline-start" /> : <Copy data-icon="inline-start" />}{copied ? copy.copied : copy.copy}
      </Button>
    </div>
    <pre tabIndex={0} className="max-h-72 overflow-auto rounded-md border bg-muted p-3 text-xs whitespace-pre-wrap">{text}</pre>
  </div>;
}
