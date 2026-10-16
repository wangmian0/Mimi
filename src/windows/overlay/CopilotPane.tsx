import { copilotCopy, copilotErrorCopy } from "./copilotCopy";
import { copilotPaneDensity } from "./copilotModel";
import { useCopilotPaneState } from "./useCopilotState";
import "./copilot-pane.css";

export function CopilotPane({ height, immersive = false }: { height: number; immersive?: boolean }) {
  const state = useCopilotPaneState();
  const copy = copilotCopy();
  const tight = copilotPaneDensity(height) === "tight";
  if (!state.active) return null;
  const english = state.suggestion?.english || null;
  const chinese = state.suggestion?.chinese || null;
  const errorText = state.errorLabel ? copilotErrorCopy(state.errorLabel) : state.phase === "error" ? copy.error : null;
  const phaseText = state.phase === "warming" ? copy.warming
    : state.phase === "streaming" ? copy.streaming : copy.waiting;
  return <section className={`copilot-pane${immersive ? " copilot-pane--immersive" : ""}${tight ? " copilot-pane--tight" : ""}`}
    aria-label={copy.title} data-testid="copilot-pane" data-phase={state.phase}>
    {english ? <p className="copilot-pane__english" lang="en">{english}</p>
      : <p className="copilot-pane__status" role="status">{errorText ?? phaseText}</p>}
    {chinese && <p className="copilot-pane__chinese" lang="zh">{chinese}</p>}
    {errorText && english && <p className="copilot-pane__status" role="status">{errorText}</p>}
  </section>;
}
