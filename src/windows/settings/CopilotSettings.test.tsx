// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setStoredUiLanguage } from "../../lib/i18n";
import { CopilotSettings } from "./CopilotSettings";

const api = vi.hoisted(() => ({
  get: vi.fn(), options: vi.fn(), status: vi.fn(), update: vi.fn(), select: vi.fn(),
  importVoice: vi.fn(), recordStart: vi.fn(), recordStop: vi.fn(), recordCancel: vi.fn(),
  recordingHandler: null as null | ((event: { state: "recording" | "saved" | "failed"; elapsedMs: number; peakDbfs: number; error?: string }) => void),
}));
vi.mock("../../lib/copilot", () => ({
  COPILOT_CONFIG_DEFAULT: { enabled: false, model: null, activeScenarioId: null, scenarios: [] },
  VOICEPRINT_MISSING: { voiceprint: "missing", model: "missing" },
  copilotConfigGet: api.get, copilotModelOptions: api.options, speakerVoiceprintStatus: api.status,
  copilotScenarioUpdate: api.update, copilotScenarioSelect: api.select,
  speakerVoiceprintImport: api.importVoice, speakerVoiceprintRecordStart: api.recordStart,
  speakerVoiceprintRecordStop: api.recordStop, speakerVoiceprintRecordCancel: api.recordCancel,
  copilotConfigSetEnabled: vi.fn(), copilotSelectModel: vi.fn(), copilotScenarioCreate: vi.fn(),
  copilotScenarioDelete: vi.fn(), listenCopilotConfigChanged: async () => () => {},
  listenVoiceprintRecording: async (handler: typeof api.recordingHandler) => {
    api.recordingHandler = handler; return () => { api.recordingHandler = null; };
  },
}));

const config = {
  enabled: true, model: { profileId: "p1", route: "openAICompatible" },
  activeScenarioId: "s1", scenarios: [
    { id: "s1", name: "First", prompt: "initial", background: "", strategy: "" },
    { id: "s2", name: "Second", prompt: "other", background: "", strategy: "" },
  ],
};
let host: HTMLDivElement, root: Root;
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  setStoredUiLanguage("en");
  for (const mock of [api.get, api.options, api.status, api.update, api.select,
    api.importVoice, api.recordStart, api.recordStop, api.recordCancel]) mock.mockReset();
  api.recordingHandler = null;
  api.get.mockResolvedValue(config);
  api.options.mockResolvedValue([{ profileId: "p1", profileName: "Saved profile", route: "openAICompatible",
    routeName: "DeepSeek", model: "chat-model", hasApiKey: false }]);
  api.status.mockResolvedValue({ voiceprint: "ready", model: "ready" });
  api.update.mockImplementation(async (id, patch) => ({
    ...config, scenarios: config.scenarios.map(s => s.id === id ? { ...s, ...patch } : s),
  }));
  api.select.mockImplementation(async id => ({ ...config, activeScenarioId: id }));
  api.importVoice.mockResolvedValue({ voiceprint: "ready", model: "ready" });
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount()); host.remove(); setStoredUiLanguage("system");
  vi.unstubAllGlobals();
});
async function render(sessionIsActive = false) {
  await act(async () => root.render(<CopilotSettings visible sessionIsActive={sessionIsActive} onOpenModels={() => {}} />));
}

it("shows saved model and speaker state while blocking recording during a session", async () => {
  await render(true);
  expect(host.textContent).toContain("DeepSeek");
  expect(host.textContent).toContain("Voiceprint ready");
  expect(host.textContent).toContain("Speaker model ready");
  expect([...host.querySelectorAll<HTMLButtonElement>("button")].find(button => button.textContent?.includes("Record voiceprint"))?.disabled).toBe(true);
  const importButton = [...host.querySelectorAll<HTMLButtonElement>("button")].find(button => button.textContent?.includes("Import WAV"))!;
  await act(async () => importButton.click());
  expect(api.importVoice).toHaveBeenCalledExactlyOnceWith("");
});

it("keeps separate mounted drafts when switching scenarios and saves multiline text", async () => {
  await render();
  const first = host.querySelector<HTMLTextAreaElement>("#copilot-s1-prompt")!;
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(first, "line one\nline two");
    first.dispatchEvent(new Event("input", { bubbles: true }));
    first.dispatchEvent(new FocusEvent("focusout", { bubbles: true }));
  });
  expect(api.update).toHaveBeenCalledWith("s1", { prompt: "line one\nline two" });
  const picker = host.querySelector<HTMLButtonElement>('[role="combobox"][aria-label="Scenario"]')!;
  await act(async () => picker.click());
  const second = [...document.querySelectorAll<HTMLElement>('[role="option"]')].find(option => option.textContent?.includes("Second"))!;
  await act(async () => second.click());
  expect(api.select).toHaveBeenCalledWith("s2");
  expect(host.querySelector("#copilot-s2-prompt")).not.toBeNull();
  expect(first.closest(".copilot-scenario-fields")?.hasAttribute("hidden")).toBe(true);
  expect(first.value).toBe("line one\nline two");
});

it("refreshes after automatic recording stop and treats user cancellation as neutral", async () => {
  api.status.mockResolvedValueOnce({ voiceprint: "recording", model: "ready" })
    .mockResolvedValue({ voiceprint: "ready", model: "ready" });
  await render();
  expect(host.textContent).toContain("Recording");
  await act(async () => api.recordingHandler?.({
    state: "saved", elapsedMs: 60_000, peakDbfs: -12,
  }));
  expect(host.textContent).toContain("Voiceprint ready");
  await act(async () => api.recordingHandler?.({
    state: "failed", elapsedMs: 0, peakDbfs: -80, error: "speaker_record_cancelled",
  }));
  expect(host.querySelector("#copilot-voiceprint [role='alert']")).toBeNull();
});
