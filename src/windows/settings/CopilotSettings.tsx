import { useEffect, useRef, useState } from "react";
import { ProviderIcon } from "../../components/ProviderIcon";
import { Select } from "../../components/Select";
import { Switch } from "../../components/Switch";
import { Icon } from "../../components/Icon";
import {
  COPILOT_CONFIG_DEFAULT, VOICEPRINT_MISSING, copilotConfigGet, copilotConfigSetEnabled,
  copilotModelOptions, copilotScenarioCreate, copilotScenarioDelete, copilotScenarioSelect,
  copilotScenarioUpdate, copilotSelectModel, listenCopilotConfigChanged, listenVoiceprintRecording,
  speakerVoiceprintImport, speakerVoiceprintRecordCancel, speakerVoiceprintRecordStart,
  speakerVoiceprintRecordStop, speakerVoiceprintStatus,
  type CopilotConfig, type CopilotModelOption, type CopilotModelRef, type VoiceprintStatus,
} from "../../lib/copilot";
import { SettingsConfirmation } from "./DestructiveConfirmation";
import { InlineFeedback, SettingsRow, SettingsSection } from "./SettingsPrimitives";
import { CopilotTextField } from "./CopilotTextField";
import { copilotSettingsCopy } from "./copilotSettingsCopy";
import { useSettingsToast } from "./useSettingsToast";
import "./copilot-settings.css";

function modelValue(model: CopilotModelRef) { return `${model.profileId}\u0000${model.route}`; }

export function CopilotSettings({ visible, sessionIsActive, onOpenModels }: {
  visible: boolean; sessionIsActive: boolean; onOpenModels: () => void;
}) {
  const copy = copilotSettingsCopy();
  const [config, setConfig] = useState<CopilotConfig>(COPILOT_CONFIG_DEFAULT);
  const [options, setOptions] = useState<CopilotModelOption[]>([]);
  const [voice, setVoice] = useState<VoiceprintStatus>(VOICEPRINT_MISSING);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const [modelError, setModelError] = useState(false);
  const [voiceError, setVoiceError] = useState(false);
  const [busy, setBusy] = useState(false);
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [nameError, setNameError] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const queue = useRef<Promise<void>>(Promise.resolve());
  const mounted = useRef(true);
  const initialized = useRef(false);
  const { beginToast, runWithToast } = useSettingsToast();

  const mutate = (action: () => Promise<CopilotConfig>) => {
    const next = queue.current.then(action);
    queue.current = next.then(() => {}, () => {});
    return next.then(value => { if (mounted.current) setConfig(value); return value; });
  };
  const load = async () => {
    setLoading(true); setLoadError(false);
    try { const value = await copilotConfigGet(); if (mounted.current) setConfig(value); }
    catch { if (mounted.current) setLoadError(true); }
    finally { if (mounted.current) setLoading(false); }
  };
  const loadModels = async () => {
    try { const value = await copilotModelOptions(); if (mounted.current) { setOptions(value); setModelError(false); } }
    catch { if (mounted.current) setModelError(true); }
  };
  const loadVoice = async () => {
    try { const value = await speakerVoiceprintStatus(); if (mounted.current) { setVoice(value); setVoiceError(false); } }
    catch { if (mounted.current) setVoiceError(true); }
  };
  useEffect(() => {
    mounted.current = true;
    let disposed = false;
    let unlistenConfig: (() => void) | undefined;
    let unlistenVoice: (() => void) | undefined;
    void listenCopilotConfigChanged(value => { if (!disposed) setConfig(value); }).then(fn => {
      if (!disposed) unlistenConfig = fn; else fn();
    }).catch(() => { if (!disposed) setLoadError(true); });
    void listenVoiceprintRecording(event => {
      if (disposed) return;
      setElapsed(event.elapsedMs);
      if (event.state !== "recording") {
        void loadVoice().then(() => {
          if (!disposed && event.state === "failed" && event.error !== "speaker_record_cancelled") setVoiceError(true);
        });
      }
    }).then(fn => { if (!disposed) unlistenVoice = fn; else fn(); })
      .catch(() => { if (!disposed) setVoiceError(true); });
    return () => { disposed = true; mounted.current = false; unlistenConfig?.(); unlistenVoice?.(); };
  }, []);
  useEffect(() => {
    if (!visible) return;
    queueMicrotask(() => {
      if (!mounted.current) return;
      if (!initialized.current) { initialized.current = true; void load(); }
      void loadModels(); void loadVoice();
    });
  }, [visible]);

  const selected = config.model ? modelValue(config.model) : "";
  const selectedExists = options.some(option => modelValue(option) === selected);
  const modelOptions = options.map(option => ({
    value: modelValue(option),
    label: option.routeName || `${option.profileName} · ${option.model}`,
    description: option.routeName ? `${option.profileName} · ${option.model}` : undefined,
    icon: <ProviderIcon provider={option.route} size={32} />,
  }));
  const activeId = config.activeScenarioId ?? config.scenarios[0]?.id ?? "";
  const selectScenario = (id: string) => { void runWithToast(() => mutate(() => copilotScenarioSelect(id)), copy.operationFailed); };
  const createScenario = async () => {
    const name = newName.trim();
    if (!name) { setNameError(true); return; }
    setBusy(true);
    const notify = beginToast();
    try { await mutate(() => copilotScenarioCreate(name)); setCreating(false); setNewName(""); setNameError(false); }
    catch { notify(copy.operationFailed, true); }
    finally { setBusy(false); }
  };
  const deleteScenario = async () => {
    if (!deleteId) return;
    setBusy(true);
    const notify = beginToast();
    try { await mutate(() => copilotScenarioDelete(deleteId)); setDeleteId(null); }
    catch { notify(copy.operationFailed, true); }
    finally { setBusy(false); }
  };
  const voiceAction = async (action: () => Promise<unknown>) => {
    setBusy(true); setVoiceError(false);
    try { await action(); await loadVoice(); }
    catch { setVoiceError(true); }
    finally { setBusy(false); }
  };

  return <div className="copilot-settings">
    {loading ? <InlineFeedback tone="info">{copy.loading}</InlineFeedback> : loadError ? <InlineFeedback tone="error">{copy.operationFailed} <button type="button" className="settings-link" onClick={() => { void load(); }}>{copy.retry}</button></InlineFeedback> : <>
      <SettingsSection id="copilot-enable" title={copy.title} hideHeading>
        <SettingsRow label={copy.enabled} description={copy.enabledHelp}>
          <Switch checked={config.enabled} aria-label={copy.enabled}
            onChange={enabled => { void runWithToast(() => mutate(() => copilotConfigSetEnabled(enabled)), copy.operationFailed); }} />
        </SettingsRow>
        <SettingsRow label={copy.model} description={copy.modelHelp}
          feedback={modelError ? <InlineFeedback tone="error">{copy.modelError} <button type="button" className="settings-link" onClick={() => { void loadModels(); }}>{copy.retry}</button></InlineFeedback> : config.model && !selectedExists ? <InlineFeedback tone="error">{copy.modelRemoved}</InlineFeedback> : null}>
          <div className="copilot-inline-actions">
            {options.length ? <Select label={copy.model} value={selected} valueLabel={config.model ? copy.modelRemoved : copy.modelMissing}
              options={[{ value: "", label: copy.modelMissing }, ...modelOptions]}
              onChange={value => {
                const model = options.find(option => modelValue(option) === value);
                void runWithToast(() => mutate(() => copilotSelectModel(model ? { profileId: model.profileId, route: model.route } : null)), copy.operationFailed);
              }} /> : <span>{copy.modelMissing}</span>}
            <button type="button" className="settings-button settings-button--quiet settings-button--compact" onClick={onOpenModels}>{copy.addModel}</button>
          </div>
        </SettingsRow>
      </SettingsSection>

      <SettingsSection id="copilot-scenarios" title={copy.scenario}>
        <SettingsRow label={copy.scenario}>
          <div className="copilot-inline-actions">
            {config.scenarios.length ? <Select label={copy.scenario} value={activeId}
              options={config.scenarios.map(s => ({ value: s.id, label: s.name }))}
              onChange={selectScenario} /> : <span>{copy.scenarioMissing}</span>}
            <button type="button" className="settings-button settings-button--quiet settings-button--compact"
              disabled={config.scenarios.length >= 20} onClick={() => setCreating(true)}>
              <Icon name="plus" />{copy.create}
            </button>
            {activeId && <button type="button" className="settings-button settings-button--danger settings-button--compact"
              onClick={() => setDeleteId(activeId)}><Icon name="trash" />{copy.delete}</button>}
          </div>
        </SettingsRow>
        {config.scenarios.map(scenario => <div key={scenario.id} hidden={scenario.id !== activeId} className="copilot-scenario-fields">
          {(["prompt", "background", "strategy"] as const).map(field => <CopilotTextField key={field}
            id={`copilot-${scenario.id}-${field}`} label={copy[field]} help={copy[`${field}Help`]}
            value={scenario[field]} visible={visible && scenario.id === activeId}
            onSave={value => mutate(() => copilotScenarioUpdate(scenario.id, { [field]: value }))} />)}
        </div>)}
      </SettingsSection>

      <SettingsSection id="copilot-voiceprint" title={copy.voiceprint}>
        <SettingsRow label={copy.voiceprint} description={copy.voiceprintHelp}
          feedback={voiceError ? <InlineFeedback tone="error">{copy.voiceError} <button type="button" className="settings-link" onClick={() => { void loadVoice(); }}>{copy.retry}</button></InlineFeedback> : sessionIsActive ? <InlineFeedback tone="info">{copy.sessionActive}</InlineFeedback> : null}>
          <div className="copilot-voiceprint-status">
            <span>{voice.voiceprint === "ready" ? copy.voiceReady : voice.voiceprint === "invalid" ? copy.voiceInvalid : voice.voiceprint === "recording" ? `${copy.recording} · ${Math.floor(elapsed / 1000)}s` : copy.voiceMissing}</span>
            <span>{voice.model === "ready" ? copy.modelReady : voice.model === "downloading" ? copy.modelDownloading : voice.model === "failed" ? copy.modelFailed : copy.modelMissingSpeaker}</span>
          </div>
        </SettingsRow>
        <SettingsRow label={copy.import} description={copy.recordHelp}>
          <div className="copilot-inline-actions">
            <button type="button" className="settings-button settings-button--quiet settings-button--compact"
              disabled={busy || voice.voiceprint === "recording"} onClick={() => { void voiceAction(() => speakerVoiceprintImport("")); }}>
              <Icon name="download" />{copy.import}
            </button>
            {voice.voiceprint === "recording" ? <>
              <button type="button" className="settings-button settings-button--quiet settings-button--compact" disabled={busy}
                onClick={() => { void voiceAction(() => speakerVoiceprintRecordStop()); }}>{copy.stop}</button>
              <button type="button" className="settings-button settings-button--quiet settings-button--compact" disabled={busy}
                onClick={() => { void voiceAction(() => speakerVoiceprintRecordCancel()); }}>{copy.cancel}</button>
            </> : <button type="button" className="settings-button settings-button--quiet settings-button--compact"
              disabled={busy || sessionIsActive} onClick={() => { void voiceAction(async () => { await speakerVoiceprintRecordStart(60); setVoice(current => ({ ...current, voiceprint: "recording" })); }); }}>
              <Icon name="microphone" />{copy.record}
            </button>}
          </div>
        </SettingsRow>
      </SettingsSection>
    </>}
    {creating && <SettingsConfirmation message={copy.createTitle} variant="default" confirmLabel={copy.createConfirm}
      disabled={busy} onCancel={() => { setCreating(false); setNameError(false); }} onConfirm={() => { void createScenario(); }}>
      <input autoFocus value={newName} maxLength={64} placeholder={copy.createPlaceholder}
        aria-label={copy.createTitle} aria-invalid={nameError || undefined}
        onChange={event => { setNewName(event.target.value); setNameError(false); }} />
      {nameError && <InlineFeedback tone="error">{copy.nameRequired}</InlineFeedback>}
    </SettingsConfirmation>}
    {deleteId && <SettingsConfirmation message={copy.deleteConfirm} disabled={busy}
      onCancel={() => setDeleteId(null)} onConfirm={() => { void deleteScenario(); }} />}
  </div>;
}
