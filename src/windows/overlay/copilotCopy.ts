import { effectiveUiLanguage, type ResolvedUiLanguage } from "../../lib/i18n";

const zh = {
  enable: "开启提词", disable: "关闭提词", title: "AI 提词", me: "我", other: "对方",
  warming: "正在准备建议…", waiting: "等待对方说话…", streaming: "正在生成建议…",
  error: "暂时无法生成建议", unavailable: "提词暂不可用", modelMissing: "请先选择提词模型",
  voiceprintMissing: "请先录入声纹", microphoneRequired: "请先选择麦克风",
  modelUnavailable: "提词模型不可用，请检查模型设置", networkInvalid: "提词网络设置不可用",
  speakerUnavailable: "声纹暂不可用", providerUnavailable: "提词服务暂不可用",
  translationUnavailable: "本地中文翻译暂不可用",
};
const en: typeof zh = {
  enable: "Enable suggestions", disable: "Disable suggestions", title: "AI suggestions", me: "Me", other: "Other",
  warming: "Preparing suggestions…", waiting: "Waiting for the other speaker…", streaming: "Writing a suggestion…",
  error: "Suggestions are temporarily unavailable", unavailable: "Suggestions unavailable", modelMissing: "Select a suggestion model first",
  voiceprintMissing: "Add a voiceprint first", microphoneRequired: "Select a microphone first",
  modelUnavailable: "Suggestion model unavailable. Check model settings", networkInvalid: "Suggestion network settings unavailable",
  speakerUnavailable: "Voiceprint temporarily unavailable", providerUnavailable: "Suggestion service temporarily unavailable",
  translationUnavailable: "Local Chinese translation unavailable",
};
const copy = { zh, en, ja: en, de: en, fr: en, ko: en, th: en, "zh-TW": en } satisfies Record<ResolvedUiLanguage, typeof zh>;
export function copilotCopy() { return copy[effectiveUiLanguage()]; }
export function copilotUnavailableLabel(reason: "disabled" | "modelMissing" | "voiceprintMissing" | "microphoneRequired" | null) {
  const words = copilotCopy();
  return reason === "modelMissing" ? words.modelMissing : reason === "voiceprintMissing" ? words.voiceprintMissing
    : reason === "microphoneRequired" ? words.microphoneRequired : words.unavailable;
}

/** The backend sends diagnostic labels, never user-facing copy. Do not echo an unknown value. */
export function copilotErrorCopy(label: string) {
  const words = copilotCopy();
  if (label === "copilot_model_unavailable") return words.modelUnavailable;
  if (label === "copilot_network_invalid") return words.networkInvalid;
  if (label === "copilot_speaker_unavailable") return words.speakerUnavailable;
  if (label.startsWith("apple_translation_")) return words.translationUnavailable;
  if (label.startsWith("openai_compatible.")) return words.providerUnavailable;
  return words.error;
}
