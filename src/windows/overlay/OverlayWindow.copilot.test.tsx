// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setStoredUiLanguage } from "../../lib/i18n";
import { useStore } from "../../lib/store";
import type { CopilotOverlayState } from "../../lib/copilot";
import { receiveCopilotState } from "./useCopilotState";
import { OverlayWindow } from "./OverlayWindow";

vi.mock("./PulseRing", () => ({ PulseRing: () => null }));
vi.mock("./ResizeHandles", () => ({ ResizeHandles: () => null }));
const original = useStore.getState();
let host: HTMLDivElement;
let root: Root;
const available: CopilotOverlayState = {
  revision: 1, available: true, unavailableReason: null, active: false, phase: "idle", speakers: [],
  suggestion: null, errorLabel: null,
};

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.stubGlobal("matchMedia", () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  Object.defineProperty(HTMLElement.prototype, "scrollTo", { configurable: true, value: () => {} });
  setStoredUiLanguage("en");
  useStore.setState({ ...original,
    session: { ...original.session, isActive: true, status: { kind: "listening" }, subtitles: {
      source: { text: "", isFinal: false }, translation: { text: "", isFinal: false },
      history: [{ audioSource: "microphone", source: "Hello", translation: "你好", createdAt: 100 }],
    } },
    settings: { ...original.settings, audioInput: "microphone", sourceLanguage: "en", targetLanguage: "zh",
      subtitleDisplayMode: "bilingual", pulseAnimation: false, subtitleAnimation: false },
  }, true);
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});

afterEach(async () => {
  await act(async () => root.unmount());
  host.remove(); useStore.setState(original, true); setStoredUiLanguage("system");
  Reflect.deleteProperty(HTMLElement.prototype, "scrollTo"); vi.unstubAllGlobals();
});

it("shows the switch only when enabled, splits the actual DOM, and clears suggestions when off", async () => {
  await act(async () => root.render(<OverlayWindow />));
  expect(host.querySelector('[data-testid="toggle-copilot"]')).toBeNull();
  expect(host.querySelector(".copilot-reading-half")).toBeNull();
  await act(async () => receiveCopilotState(available));
  const button = host.querySelector<HTMLButtonElement>('[data-testid="toggle-copilot"]')!;
  expect(button.getAttribute("aria-pressed")).toBe("false");
  expect(host.querySelector(".copilot-reading-half")).toBeNull();
  await act(async () => receiveCopilotState({ ...available, revision: 2, active: true, phase: "ready",
    speakers: [
      { audioSource: "system", createdAt: 100, speaker: "me" },
      { audioSource: "microphone", createdAt: 100, speaker: "other" },
    ], suggestion: { id: 5, english: "Could you repeat that?", chinese: "能请您重复吗？", isFinal: true } }));
  expect(button.getAttribute("aria-pressed")).toBe("true");
  expect(host.querySelector(".copilot-reading-half .overlay-timeline")).not.toBeNull();
  expect(host.querySelector('[data-testid="copilot-pane"]')?.textContent).toContain("Could you repeat that?");
  expect(host.querySelector('[data-testid="copilot-pane"]')?.textContent).toContain("能请您重复吗？");
  expect(host.querySelector('[data-utterance-id="microphone:history-100"] [data-speaker="other"]')?.textContent).toBe("Other");
  expect(host.querySelector('[data-speaker="me"]')).toBeNull();
  await act(async () => receiveCopilotState({ ...available, revision: 3, active: false,
    suggestion: { id: 5, english: "Must not remain", chinese: null, isFinal: true } }));
  expect(button.getAttribute("aria-pressed")).toBe("false");
  expect(host.querySelector(".copilot-reading-half")).toBeNull();
  expect(host.querySelector('[data-testid="copilot-pane"]')).toBeNull();
  expect(host.textContent).not.toContain("Must not remain");
});
