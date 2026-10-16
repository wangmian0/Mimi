import { effectiveUiLanguage, type ResolvedUiLanguage } from "../../lib/i18n";

const en = {
  title: "AI Copilot", help: "Prepare short English replies during a microphone call.",
  enabled: "Enable AI Copilot", enabledHelp: "The overlay has its own temporary switch for each call.",
  model: "Suggestion model", modelHelp: "Uses a saved OpenAI compatible or ChatMock destination from Models & Services.",
  modelMissing: "No saved chat model", modelRemoved: "Selected model is unavailable", modelError: "Could not load saved models.",
  addModel: "Open Models & Services", scenario: "Scenario", scenarioMissing: "No scenarios yet",
  create: "New scenario", createTitle: "Scenario name", createPlaceholder: "e.g. Customer support call",
  createConfirm: "Create", delete: "Delete scenario", deleteConfirm: "Delete this scenario?",
  prompt: "Prompt", promptHelp: "Instructions for the English suggestion.",
  background: "My background", backgroundHelp: "Facts about you or this call.",
  strategy: "Response approach", strategyHelp: "How you want to handle the conversation.",
  retry: "Retry", saveFailed: "Could not save this text. Your draft is kept.",
  operationFailed: "Could not save AI Copilot settings.", nameRequired: "Enter a scenario name.",
  voiceprint: "Voiceprint", voiceprintHelp: "Only used locally to distinguish your voice from the other speaker.",
  voiceMissing: "No voiceprint", voiceReady: "Voiceprint ready", voiceInvalid: "Voiceprint needs replacing",
  modelReady: "Speaker model ready", modelMissingSpeaker: "Speaker model not prepared", modelDownloading: "Preparing speaker model", modelFailed: "Speaker model unavailable",
  import: "Import WAV", record: "Record voiceprint", stop: "Stop recording", cancel: "Cancel recording",
  recording: "Recording", recordHelp: "At the same place where you make calls, put your phone on speaker but mute it. Read aloud at your usual volume for at least 20 seconds.",
  sessionActive: "Stop the current session before recording a voiceprint.",
  voiceError: "Voiceprint operation failed.", loading: "Loading AI Copilot settings…",
};

const zh: Record<keyof typeof en, string> = {
  title: "AI 提词", help: "麦克风通话时准备简短的英文应答。",
  enabled: "启用 AI 提词", enabledHelp: "字幕窗每次通话另有临时提词开关。",
  model: "提词模型", modelHelp: "使用“模型与服务”中已保存的 OpenAI 兼容或 ChatMock 目的地。",
  modelMissing: "没有已保存的对话模型", modelRemoved: "所选模型已不可用", modelError: "无法读取已保存模型。",
  addModel: "打开模型与服务", scenario: "场景", scenarioMissing: "还没有场景",
  create: "新建场景", createTitle: "场景名称", createPlaceholder: "例如：客服通话",
  createConfirm: "创建", delete: "删除场景", deleteConfirm: "删除这个场景？",
  prompt: "提示词", promptHelp: "英文建议应遵循的指令。",
  background: "我的背景", backgroundHelp: "与你或这通电话有关的事实。",
  strategy: "应对思路", strategyHelp: "希望如何处理这段对话。",
  retry: "重试", saveFailed: "保存失败，草稿已保留。",
  operationFailed: "无法保存 AI 提词设置。", nameRequired: "请输入场景名称。",
  voiceprint: "声纹", voiceprintHelp: "仅在本机用于区分你的声音与对方。",
  voiceMissing: "尚未录入声纹", voiceReady: "声纹已就绪", voiceInvalid: "声纹需要重新录入",
  modelReady: "说话人模型已就绪", modelMissingSpeaker: "尚未准备说话人模型", modelDownloading: "正在准备说话人模型", modelFailed: "说话人模型不可用",
  import: "导入 WAV", record: "录制声纹", stop: "停止录制", cancel: "取消录制",
  recording: "录制中", recordHelp: "在打电话的同一位置，手机开免提但静音，用平时音量念 20 秒以上。",
  sessionActive: "请先停止当前会话，再录制声纹。",
  voiceError: "声纹操作失败。", loading: "正在读取 AI 提词设置…",
};

const copies = { zh, en, "zh-TW": en, ja: en, ko: en, de: en, fr: en, th: en } satisfies Record<ResolvedUiLanguage, Record<keyof typeof en, string>>;
export function copilotSettingsCopy() { return copies[effectiveUiLanguage()]; }
