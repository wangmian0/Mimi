import { useEffect, useRef, useState } from "react";
import { InlineFeedback } from "./SettingsPrimitives";
import { SettingsHelp } from "./SettingsHelp";
import { I18N } from "../../lib/i18n";
import { copilotSettingsCopy } from "./copilotSettingsCopy";

/** Keeps one mounted draft per scenario field; delayed saves never replace typed text. */
export function CopilotTextField({ id, label, help, value, visible, onSave }: {
  id: string; label: string; help: string; value: string; visible: boolean;
  onSave: (value: string) => Promise<unknown>;
}) {
  const copy = copilotSettingsCopy();
  const [draft, setDraft] = useState(value);
  const [error, setError] = useState(false);
  const draftRef = useRef(value);
  const confirmed = useRef(value);
  const composing = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const inFlight = useRef(false);
  const mounted = useRef(true);
  const onSaveRef = useRef(onSave);
  const flushRef = useRef<() => Promise<void>>(async () => {});

  const clearTimer = () => { if (timer.current) clearTimeout(timer.current); timer.current = null; };
  const flush = async () => {
    clearTimer();
    if (composing.current || inFlight.current || draftRef.current === confirmed.current) return;
    const submitted = draftRef.current;
    inFlight.current = true;
    let succeeded = false;
    try {
      await onSaveRef.current(submitted);
      succeeded = true;
      confirmed.current = submitted;
      if (mounted.current) setError(false);
    } catch {
      if (mounted.current) setError(true);
    } finally {
      inFlight.current = false;
      if (mounted.current && succeeded && draftRef.current !== submitted) {
        timer.current = setTimeout(() => { void flush(); }, 350);
      }
    }
  };
  const schedule = () => {
    clearTimer();
    if (!composing.current) timer.current = setTimeout(() => { void flush(); }, 350);
  };
  useEffect(() => { onSaveRef.current = onSave; }, [onSave]);
  useEffect(() => { flushRef.current = flush; });
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; clearTimer(); }; }, []);
  useEffect(() => { if (!visible) void flushRef.current(); }, [visible]);

  return <div className="copilot-text-field">
    <div className="copilot-text-field__label"><label htmlFor={id}>{label}</label><SettingsHelp text={help} label={I18N.settings.helpLabel} /></div>
    <textarea id={id} value={draft} rows={5}
      aria-label={label} aria-describedby={`${id}-help`}
      aria-invalid={error || undefined}
      onChange={event => {
        const next = Array.from(event.currentTarget.value).slice(0, 8000).join("");
        draftRef.current = next;
        setDraft(draftRef.current);
        setError(false);
        schedule();
      }}
      onBlur={() => { void flush(); }}
      onCompositionStart={() => { composing.current = true; clearTimer(); }}
      onCompositionEnd={event => {
        composing.current = false;
        draftRef.current = Array.from(event.currentTarget.value).slice(0, 8000).join("");
        setDraft(draftRef.current);
        schedule();
      }} />
    <span id={`${id}-help`} className="settings-sr-only">{help}</span>
    {error && <div className="copilot-text-field__feedback"><InlineFeedback tone="error">{copy.saveFailed} <button className="settings-link" type="button" onClick={() => { void flush(); }}>{copy.retry}</button></InlineFeedback></div>}
  </div>;
}
