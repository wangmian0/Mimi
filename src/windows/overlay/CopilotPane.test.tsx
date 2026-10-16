// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { CopilotOverlayState } from "../../lib/copilot";
import { setStoredUiLanguage } from "../../lib/i18n";
import { CopilotPane } from "./CopilotPane";
import { receiveCopilotState } from "./useCopilotState";

let host: HTMLDivElement;
let root: ReturnType<typeof createRoot>;
const english = "Could you please confirm the exact payment date and the amount recorded on your account?";
const chinese = "您能确认账户记录的确切付款日期和金额吗？";
const ready: CopilotOverlayState = {
  revision: 1, available: true, unavailableReason: null, active: true, phase: "ready", speakers: [],
  suggestion: { id: 1, english, chinese, isFinal: true }, errorLabel: null,
};

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  setStoredUiLanguage("en");
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove(); setStoredUiLanguage("system"); vi.unstubAllGlobals();
});

it("keeps the full English and Chinese suggestion in a minimum-height pane", async () => {
  await act(async () => root.render(<CopilotPane height={156} />));
  await act(async () => receiveCopilotState(ready));
  const pane = host.querySelector<HTMLElement>('[data-testid="copilot-pane"]')!;
  expect(pane.classList.contains("copilot-pane--tight")).toBe(true);
  expect(pane.getAttribute("aria-label")).toBe("AI suggestions");
  expect(pane.querySelector(".copilot-pane__english")?.textContent).toBe(english);
  expect(pane.querySelector(".copilot-pane__chinese")?.textContent).toBe(chinese);
  expect(pane.querySelector(".copilot-pane__heading")).toBeNull();
  expect(pane.querySelector(".copilot-pane__status")).toBeNull();
});

it("maps known machine labels and unknown details to local copy without echoing raw text", async () => {
  await act(async () => root.render(<CopilotPane height={156} />));
  await act(async () => receiveCopilotState({ ...ready, phase: "error", errorLabel: "openai_compatible.rejected(code=429)" }));
  expect(host.querySelector('[role="status"]')?.textContent).toBe("Suggestion service temporarily unavailable");
  expect(host.textContent).not.toContain("openai_compatible");
  await act(async () => receiveCopilotState({ ...ready, revision: 2, errorLabel: "apple_translation_assets_missing" }));
  expect(host.querySelector('[role="status"]')?.textContent).toBe("Local Chinese translation unavailable");
  await act(async () => receiveCopilotState({ ...ready, revision: 3, errorLabel: "private provider detail" }));
  expect(host.querySelector('[role="status"]')?.textContent).toBe("Suggestions are temporarily unavailable");
  expect(host.textContent).not.toContain("private provider detail");
});
