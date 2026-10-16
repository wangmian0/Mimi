use super::config::CopilotStore;
use crate::core::audio_input::AudioSource;
use crate::settings_store::SettingsStore;
use crate::speaker::{SpeakerService, SpeakerVerdict};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use tauri::AppHandle;

pub const COPILOT_STATE_EVENT: &str = "copilot-state";

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CopilotSpeaker {
    Me,
    Other,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotSpeakerLabel {
    pub audio_source: AudioSource,
    pub created_at: u64,
    pub speaker: CopilotSpeaker,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CopilotUnavailableReason {
    Disabled,
    ModelMissing,
    VoiceprintMissing,
    MicrophoneRequired,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CopilotPhase {
    Idle,
    Warming,
    Waiting,
    Streaming,
    Ready,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotSuggestion {
    pub id: u64,
    pub english: String,
    pub chinese: Option<String>,
    pub is_final: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotOverlayState {
    pub revision: u64,
    pub available: bool,
    pub unavailable_reason: Option<CopilotUnavailableReason>,
    pub active: bool,
    pub phase: CopilotPhase,
    pub speakers: Vec<CopilotSpeakerLabel>,
    pub suggestion: Option<CopilotSuggestion>,
    pub error_label: Option<String>,
}

impl Default for CopilotOverlayState {
    fn default() -> Self {
        Self {
            revision: 0,
            available: false,
            unavailable_reason: Some(CopilotUnavailableReason::Disabled),
            active: false,
            phase: CopilotPhase::Idle,
            speakers: Vec::new(),
            suggestion: None,
            error_label: None,
        }
    }
}

pub struct ConfirmedLine {
    pub source: AudioSource,
    pub created_at_ms: u64,
    pub text: String,
    pub confirmed_at: Instant,
}

pub struct CopilotEngine;

impl CopilotEngine {
    pub fn new(
        _app: AppHandle,
        _settings: Arc<SettingsStore>,
        _store: Arc<CopilotStore>,
        _speaker: Arc<SpeakerService>,
    ) -> Arc<Self> {
        Arc::new(Self)
    }

    pub fn is_active(&self) -> bool {
        false
    }
    pub fn observe_confirmed(&self, _line: ConfirmedLine, _verdict: SpeakerVerdict) {}
    pub fn config_changed(&self) {}
    pub fn reset_conversation(&self) {}
    pub async fn set_active(&self, _active: bool) -> Result<CopilotOverlayState, String> {
        Err("not_implemented".into())
    }
    pub fn snapshot(&self) -> CopilotOverlayState {
        CopilotOverlayState::default()
    }
}
