// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { CopilotTextField } from "./CopilotTextField";

let host: HTMLDivElement, root: Root;
const save = vi.fn<(value: string) => Promise<unknown>>();
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.useFakeTimers();
  save.mockReset().mockResolvedValue(undefined);
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount()); host.remove();
  vi.useRealTimers(); vi.unstubAllGlobals();
});
async function render(visible = true) {
  await act(async () => root.render(<CopilotTextField id="prompt" label="Prompt" help="Help"
    value="original" visible={visible} onSave={save} />));
}
const input = () => host.querySelector<HTMLTextAreaElement>("textarea")!;
async function change(value: string) {
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(input(), value);
    input().dispatchEvent(new Event("input", { bubbles: true }));
  });
}
async function tick() { await act(async () => vi.advanceTimersByTimeAsync(350)); }
function deferred() {
  let resolve!: (value?: unknown) => void;
  const promise = new Promise<unknown>(yes => { resolve = yes; });
  return { promise, resolve };
}

it("debounces multiline text and flushes when hidden", async () => {
  await render();
  await change("line one\nline two");
  await act(async () => vi.advanceTimersByTimeAsync(349));
  expect(save).not.toHaveBeenCalled();
  await tick();
  expect(save).toHaveBeenCalledExactlyOnceWith("line one\nline two");
  await change("new draft");
  await render(false);
  expect(save).toHaveBeenLastCalledWith("new draft");
});

it("keeps newer typing when an older save finishes", async () => {
  const first = deferred();
  save.mockReturnValueOnce(first.promise);
  await render();
  await change("old draft");
  await tick();
  await change("new draft");
  await act(async () => first.resolve());
  expect(input().value).toBe("new draft");
  await tick();
  expect(save).toHaveBeenLastCalledWith("new draft");
  expect(input().value).toBe("new draft");
});

it("holds a failed draft beside a retry control", async () => {
  save.mockRejectedValueOnce(new Error("private failure"));
  await render();
  await change("draft");
  await tick();
  expect(input().value).toBe("draft");
  expect(host.querySelector('[role="alert"]')?.textContent).toContain("Could not save");
  await act(async () => host.querySelector<HTMLButtonElement>(".settings-link")!.click());
  expect(save).toHaveBeenCalledTimes(2);
});

it("does not save during IME composition", async () => {
  await render();
  await act(async () => input().dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true })));
  await change("組字");
  await tick();
  expect(save).not.toHaveBeenCalled();
  await act(async () => input().dispatchEvent(new CompositionEvent("compositionend", { bubbles: true })));
  await tick();
  expect(save).toHaveBeenCalledExactlyOnceWith("組字");
});
