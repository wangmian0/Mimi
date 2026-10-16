import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { isTauri } from "./ipc";
import type { AudioSource } from "./types";

export type CopilotSpeaker = "me" | "other";
export interface CopilotSpeakerLabel { audioSource: AudioSource; createdAt: number; speaker: CopilotSpeaker }
export type CopilotPhase = "idle" | "warming" | "waiting" | "streaming" | "ready" | "error";
export interface CopilotSuggestion { id: number; english: string; chinese: string | null; isFinal: boolean }
export interface CopilotOverlayState {
  revision: number;
  available: boolean;
  unavailableReason: "disabled" | "modelMissing" | "voiceprintMissing" | "microphoneRequired" | null;
  active: boolean;
  phase: CopilotPhase;
  speakers: CopilotSpeakerLabel[];
  suggestion: CopilotSuggestion | null;
  errorLabel: string | null;
}
export interface CopilotScenario { id: string; name: string; prompt: string; background: string; strategy: string }
export interface CopilotModelRef { profileId: string; route: "openAICompatible" | "chatMock" }
export interface CopilotConfig { enabled: boolean; model: CopilotModelRef | null; activeScenarioId: string | null; scenarios: CopilotScenario[] }
export interface CopilotModelOption { profileId: string; profileName: string; route: CopilotModelRef["route"]; routeName: string | null; model: string; hasApiKey: boolean }
export type CopilotScenarioPatch = Partial<Pick<CopilotScenario, "name" | "prompt" | "background" | "strategy">>;
export interface VoiceprintStatus { voiceprint: "missing" | "ready" | "recording" | "invalid"; durationMs?: number; updatedAtMs?: number; model: "missing" | "downloading" | "ready" | "failed" }
export interface VoiceprintRecordingEvent { state: "recording" | "saved" | "failed"; elapsedMs: number; peakDbfs: number; error?: string }

export const COPILOT_UNAVAILABLE: CopilotOverlayState = {
  revision: 0, available: false, unavailableReason: "disabled", active: false,
  phase: "idle", speakers: [], suggestion: null, errorLabel: null,
};
export const COPILOT_CONFIG_DEFAULT: CopilotConfig = { enabled: false, model: null, activeScenarioId: null, scenarios: [] };
export const VOICEPRINT_MISSING: VoiceprintStatus = { voiceprint: "missing", model: "missing" };

export function copilotGetState(): Promise<CopilotOverlayState> {
  return isTauri ? invoke("copilot_get_state") : Promise.resolve(COPILOT_UNAVAILABLE);
}
export function copilotSetActive(active: boolean): Promise<CopilotOverlayState> {
  return isTauri ? invoke("copilot_set_active", { active }) : Promise.resolve(COPILOT_UNAVAILABLE);
}
export function listenCopilotState(handler: (state: CopilotOverlayState) => void): Promise<UnlistenFn> {
  return isTauri ? listen<CopilotOverlayState>("copilot-state", event => handler(event.payload)) : Promise.resolve(() => {});
}
export function copilotConfigGet(): Promise<CopilotConfig> {
  return isTauri ? invoke("copilot_config_get") : Promise.resolve(COPILOT_CONFIG_DEFAULT);
}
export function copilotConfigSetEnabled(enabled: boolean): Promise<CopilotConfig> {
  return isTauri ? invoke("copilot_config_set_enabled", { enabled }) : Promise.resolve(COPILOT_CONFIG_DEFAULT);
}
export function copilotModelOptions(): Promise<CopilotModelOption[]> {
  return isTauri ? invoke("copilot_model_options") : Promise.resolve([]);
}
export function copilotSelectModel(model: CopilotModelRef | null): Promise<CopilotConfig> {
  return isTauri ? invoke("copilot_select_model", { model }) : Promise.resolve(COPILOT_CONFIG_DEFAULT);
}
export function copilotScenarioCreate(name: string): Promise<CopilotConfig> {
  return isTauri ? invoke("copilot_scenario_create", { name }) : Promise.resolve(COPILOT_CONFIG_DEFAULT);
}
export function copilotScenarioUpdate(id: string, patch: CopilotScenarioPatch): Promise<CopilotConfig> {
  return isTauri ? invoke("copilot_scenario_update", { id, patch }) : Promise.resolve(COPILOT_CONFIG_DEFAULT);
}
export function copilotScenarioDelete(id: string): Promise<CopilotConfig> {
  return isTauri ? invoke("copilot_scenario_delete", { id }) : Promise.resolve(COPILOT_CONFIG_DEFAULT);
}
export function copilotScenarioSelect(id: string): Promise<CopilotConfig> {
  return isTauri ? invoke("copilot_scenario_select", { id }) : Promise.resolve(COPILOT_CONFIG_DEFAULT);
}
export function speakerVoiceprintStatus(): Promise<VoiceprintStatus> {
  return isTauri ? invoke("speaker_voiceprint_status") : Promise.resolve(VOICEPRINT_MISSING);
}
export function speakerVoiceprintImport(path: string): Promise<VoiceprintStatus> {
  return isTauri ? invoke("speaker_voiceprint_import", { path }) : Promise.resolve(VOICEPRINT_MISSING);
}
export function speakerVoiceprintRecordStart(maxSeconds?: number): Promise<void> {
  return isTauri ? invoke("speaker_voiceprint_record_start", { maxSeconds }) : Promise.resolve();
}
export function speakerVoiceprintRecordStop(): Promise<VoiceprintStatus> {
  return isTauri ? invoke("speaker_voiceprint_record_stop") : Promise.resolve(VOICEPRINT_MISSING);
}
export function speakerVoiceprintRecordCancel(): Promise<void> {
  return isTauri ? invoke("speaker_voiceprint_record_cancel") : Promise.resolve();
}
export function listenCopilotConfigChanged(handler: (config: CopilotConfig) => void): Promise<UnlistenFn> {
  return isTauri ? listen<CopilotConfig>("copilot-config-changed", event => handler(event.payload)) : Promise.resolve(() => {});
}
export function listenVoiceprintRecording(handler: (state: VoiceprintRecordingEvent) => void): Promise<UnlistenFn> {
  return isTauri ? listen<VoiceprintRecordingEvent>("speaker://voiceprint-recording", event => handler(event.payload)) : Promise.resolve(() => {});
}
