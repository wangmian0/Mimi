import type { CopilotSpeakerLabel } from "../../lib/copilot";
import type { SubtitleBlock } from "./overlayModel";

export function withSpeakerLabels(blocks: SubtitleBlock[], labels: readonly CopilotSpeakerLabel[]): SubtitleBlock[] {
  if (labels.length === 0) return blocks;
  const byKey = new Map(labels.map(label => [`${label.audioSource}:${label.createdAt}`, label.speaker]));
  let changed = false;
  const result = blocks.map(block => {
    if (block.presentation === "live" || block.createdAt === null || !block.audioSource) return block;
    const speaker = byKey.get(`${block.audioSource}:${block.createdAt}`);
    if (!speaker || block.speaker === speaker) return block;
    changed = true;
    return { ...block, speaker };
  });
  return changed ? result : blocks;
}

export function copilotPaneDensity(height: number) { return height < 240 ? "tight" : "normal"; }
