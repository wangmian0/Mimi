// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { CopilotOverlayState } from "../../lib/copilot";

const mocks = vi.hoisted(() => ({ get: vi.fn(), listen: vi.fn() }));
vi.mock("../../lib/copilot", async importOriginal => ({
  ...await importOriginal<typeof import("../../lib/copilot")>(),
  copilotGetState: mocks.get,
  listenCopilotState: mocks.listen,
}));
import { useCopilotState } from "./useCopilotState";

const makeState = (revision: number, active: boolean): CopilotOverlayState => ({
  revision, available: true, unavailableReason: null, active, phase: active ? "waiting" : "idle",
  speakers: [], suggestion: null, errorLabel: null,
});

it("keeps a newer event ahead of a delayed read and releases a late listener after unmount", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div"); document.body.append(host);
  const root = createRoot(host);
  let emit!: (state: CopilotOverlayState) => void;
  let resolveRead!: (state: CopilotOverlayState) => void;
  const remove = vi.fn();
  mocks.listen.mockImplementation((handler) => { emit = handler; return Promise.resolve(remove); });
  mocks.get.mockImplementation(() => new Promise(resolve => { resolveRead = resolve; }));
  function Probe() { const state = useCopilotState(); return <span>{String(state.active)}</span>; }
  await act(async () => root.render(<Probe />));
  await act(async () => emit(makeState(3, true)));
  await act(async () => resolveRead(makeState(2, false)));
  expect(host.textContent).toBe("true");
  await act(async () => root.unmount());
  expect(remove).toHaveBeenCalledOnce();
  await act(async () => emit(makeState(4, false)));

  const nextRoot = createRoot(host);
  let resolveListen!: (remove: () => void) => void;
  const lateRemove = vi.fn();
  mocks.listen.mockImplementation(() => new Promise(resolve => { resolveListen = resolve; }));
  await act(async () => nextRoot.render(<Probe />));
  await act(async () => nextRoot.unmount());
  await act(async () => resolveListen(lateRemove));
  expect(lateRemove).toHaveBeenCalledOnce();
  host.remove(); vi.unstubAllGlobals();
});
