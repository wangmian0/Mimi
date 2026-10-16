import { useSyncExternalStore } from "react";
import {
  COPILOT_UNAVAILABLE, copilotGetState, copilotSetActive, listenCopilotState,
  type CopilotOverlayState, type CopilotSpeakerLabel,
} from "../../lib/copilot";

type ChromeState = Pick<CopilotOverlayState, "available" | "unavailableReason" | "active" | "speakers">;
let state: CopilotOverlayState = COPILOT_UNAVAILABLE;
let chrome: ChromeState = state;
let received = false;
const subscribers = new Set<() => void>();
const chromeSubscribers = new Set<() => void>();
let unlisten: (() => void) | undefined;
let generation = 0;

function sameSpeakers(a: readonly CopilotSpeakerLabel[], b: readonly CopilotSpeakerLabel[]) {
  return a === b || a.length === b.length && a.every((label, index) =>
    label.audioSource === b[index].audioSource && label.createdAt === b[index].createdAt && label.speaker === b[index].speaker);
}

export function receiveCopilotState(next: CopilotOverlayState | null | undefined) {
  if (!next || !Number.isFinite(next.revision) || received && next.revision <= state.revision) return;
  received = true;
  const speakers = next.active ? next.speakers : [];
  const stableSpeakers = sameSpeakers(state.speakers, speakers) ? state.speakers : speakers;
  const active = next.available && next.active;
  const accepted = { ...next, active, speakers: stableSpeakers,
    suggestion: active ? next.suggestion : null, errorLabel: active ? next.errorLabel : null };
  const chromeChanged = chrome.available !== accepted.available || chrome.unavailableReason !== accepted.unavailableReason
    || chrome.active !== accepted.active || chrome.speakers !== accepted.speakers;
  state = accepted;
  subscribers.forEach(notify => notify());
  if (chromeChanged) {
    chrome = { available: accepted.available, unavailableReason: accepted.unavailableReason,
      active: accepted.active, speakers: accepted.speakers };
    chromeSubscribers.forEach(notify => notify());
  }
}

function subscribeToNative() {
  const currentGeneration = ++generation;
  const accept = (next: CopilotOverlayState | null | undefined) => {
    if (currentGeneration === generation) receiveCopilotState(next);
  };
  const read = () => { void copilotGetState().then(accept).catch(() => {}); };
  void listenCopilotState(accept).then(remove => {
    if (currentGeneration !== generation) { remove(); return; }
    unlisten = remove;
    read();
  }).catch(read);
}

function subscribe(set: Set<() => void>, notify: () => void) {
  const wasEmpty = subscribers.size + chromeSubscribers.size === 0;
  set.add(notify);
  if (wasEmpty) subscribeToNative();
  return () => {
    set.delete(notify);
    if (subscribers.size + chromeSubscribers.size === 0) {
      generation++;
      unlisten?.();
      unlisten = undefined;
      state = COPILOT_UNAVAILABLE;
      chrome = state;
      received = false;
    }
  };
}

const subscribeChrome = (notify: () => void) => subscribe(chromeSubscribers, notify);
const subscribeFull = (notify: () => void) => subscribe(subscribers, notify);
const getChrome = () => chrome;
const getFull = () => state;

export function useCopilotState() { return useSyncExternalStore(subscribeChrome, getChrome, getChrome); }
export function useCopilotPaneState() { return useSyncExternalStore(subscribeFull, getFull, getFull); }

export async function setCopilotActive(active: boolean) {
  const next = await copilotSetActive(active);
  receiveCopilotState(next);
}
