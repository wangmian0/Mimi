// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useStore } from "../../lib/store";
import { shareUnchangedSubtitleHistory } from "../../lib/sessionSnapshot";
import type { SessionStateEvent } from "../../lib/types";
import { OverlayWindow } from "./OverlayWindow";
import { receiveCopilotState } from "./useCopilotState";
import type { CopilotOverlayState } from "../../lib/copilot";

const timelineRender = vi.hoisted(() => vi.fn());
vi.mock("./Timeline", async () => {
  const { memo } = await import("react");
  return { Timeline: memo((props: { blocks: { source: string | null; translation: string | null }[] }) => {
    timelineRender();
    return <div data-testid="timeline">{props.blocks.map((block) => `${block.source ?? ""} ${block.translation ?? ""}`).join("\n")}</div>;
  }) };
});
vi.mock("./PulseRing", () => ({ PulseRing: () => null }));
vi.mock("./ResizeHandles", () => ({ ResizeHandles: () => null }));

const original = useStore.getState();
const session: SessionStateEvent = {
  ...original.session, status: { kind: "listening" }, isActive: true,
  detectedLanguage: "en", isTranslationPending: true,
  subtitles: {
    source: { text: "Synthetic live source", isFinal: false },
    translation: { text: "Synthetic settled translation", isFinal: false },
    history: [
      { source: "Synthetic confirmed first", translation: "Synthetic first translation", createdAt: 1 },
      { source: "Synthetic confirmed second", translation: "Synthetic second translation", createdAt: 2 },
    ],
  },
};
let host: HTMLDivElement, root: Root;
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.stubGlobal("matchMedia", () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  vi.useFakeTimers();
  timelineRender.mockClear();
  useStore.setState({ ...original, session, settings: { ...original.settings, sourceLanguage: "en", targetLanguage: "zh", subtitleDisplayMode: "translation", pulseAnimation: false, subtitleAnimation: false } }, true);
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount()); host.remove(); useStore.setState(original, true);
  vi.useRealTimers(); vi.unstubAllGlobals();
});

async function publish(snapshot: SessionStateEvent) {
  await act(async () => useStore.setState((state) => ({ session: shareUnchangedSubtitleHistory(state.session, snapshot) })));
}

it("keeps Timeline unchanged during draft churn and updates for settled text and real history edits", async () => {
  await act(async () => root.render(<OverlayWindow />));
  expect(timelineRender).toHaveBeenCalledOnce();
  for (let index = 1; index <= 50; index++) {
    const incoming = JSON.parse(JSON.stringify(session)) as SessionStateEvent;
    incoming.subtitles.translation.text = `Synthetic translation draft ${index}`;
    await publish(incoming);
  }
  expect(timelineRender).toHaveBeenCalledOnce();
  expect(host.textContent).not.toContain("Synthetic translation draft 50");
  await act(async () => vi.advanceTimersByTime(400));
  expect(timelineRender).toHaveBeenCalledTimes(2);
  expect(host.textContent).toContain("Synthetic translation draft 50");

  const revised = JSON.parse(JSON.stringify(useStore.getState().session)) as SessionStateEvent;
  revised.subtitles.history[0].translation = "Synthetic corrected first translation";
  await publish(revised);
  expect(timelineRender).toHaveBeenCalledTimes(3);
  expect(host.textContent).toContain("Synthetic corrected first translation");

  const trimmed = { ...revised, subtitles: { ...revised.subtitles, history: revised.subtitles.history.slice(1) } };
  await publish(trimmed);
  expect(timelineRender).toHaveBeenCalledTimes(4);
  expect(host.textContent).not.toContain("Synthetic corrected first translation");

  await publish({ ...trimmed, subtitles: { ...trimmed.subtitles, history: [] } });
  expect(timelineRender).toHaveBeenCalledTimes(5);
  expect(host.textContent).not.toContain("Synthetic second translation");
});

it("stabilizes both input drafts without rebuilding their shared history on every IPC event", async () => {
  const tracks = (["system", "microphone"] as const).map(audioSource => ({
    audioSource, source: { text: `Synthetic ${audioSource}`, isFinal: false },
    translation: { text: `Synthetic ${audioSource} settled`, isFinal: false },
    history: session.subtitles.history.map(pair => ({ ...pair, audioSource })),
    detectedLanguage: "en", isTranslationPending: true, isTranslationTimedOut: false,
  }));
  const dual = { ...session, subtitles: { ...session.subtitles, tracks } };
  await publish(dual);
  await act(async () => root.render(<OverlayWindow />));
  expect(timelineRender).toHaveBeenCalledOnce();
  for (let index = 1; index <= 50; index++) {
    const incoming = JSON.parse(JSON.stringify(dual)) as SessionStateEvent;
    incoming.subtitles.tracks![index % 2].translation.text = `Synthetic updated draft ${index}`;
    await publish(incoming);
  }
  expect(timelineRender).toHaveBeenCalledOnce();
  await act(async () => vi.advanceTimersByTime(400));
  expect(timelineRender).toHaveBeenCalledTimes(2);
  expect(host.textContent).toContain("Synthetic updated draft 50");
});

it("does not redraw Timeline for streamed suggestion tokens with unchanged speaker labels", async () => {
  await act(async () => root.render(<OverlayWindow />));
  const initial: CopilotOverlayState = {
    revision: 1, available: true, unavailableReason: null, active: true, phase: "streaming",
    speakers: [{ audioSource: "system", createdAt: 1, speaker: "other" }],
    suggestion: { id: 1, english: "Please", chinese: null, isFinal: false }, errorLabel: null,
  };
  await act(async () => receiveCopilotState(initial));
  const renders = timelineRender.mock.calls.length;
  expect(host.querySelector('[data-testid="copilot-pane"]')).not.toBeNull();
  for (let revision = 2; revision <= 30; revision++) {
    await act(async () => receiveCopilotState({ ...initial, revision,
      speakers: [...initial.speakers], suggestion: { ...initial.suggestion!, english: `Please ${revision}` } }));
  }
  expect(timelineRender).toHaveBeenCalledTimes(renders);
  expect(host.textContent).toContain("Please 30");
  await act(async () => receiveCopilotState({ ...initial, revision: 31, active: false,
    suggestion: { ...initial.suggestion!, english: "Stale secret suggestion" } }));
  expect(host.querySelector('[data-testid="copilot-pane"]')).toBeNull();
  expect(host.textContent).not.toContain("Stale secret suggestion");
});
