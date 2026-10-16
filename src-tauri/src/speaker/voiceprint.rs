use mimi_runtime::clients::translation_client::SourceAudioRange;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeakerLabel {
    Me,
    Other,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeakerBasis {
    AudioRange,
    RecentVoice,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerVerdict {
    pub label: SpeakerLabel,
    pub score: Option<f32>,
    pub voiced_ms: u32,
    pub basis: SpeakerBasis,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceprintStatus {
    pub voiceprint: VoiceprintState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at_ms: Option<u64>,
    pub model: SpeakerModelState,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceprintState {
    Missing,
    Ready,
    Recording,
    Invalid,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeakerModelState {
    Missing,
    Downloading,
    Ready,
    Failed,
}

pub struct SpeakerService;

impl SpeakerService {
    pub fn new(_data_dir: Option<PathBuf>) -> Arc<Self> {
        Arc::new(Self)
    }
    pub fn set_active(&self, _active: bool) {}
    pub async fn prewarm(&self) -> Result<(), String> {
        Err("not_implemented".into())
    }
    pub fn begin_capture(&self, _generation: u64, _sample_rate_hz: u32) {}
    pub fn observe_pcm(&self, _generation: u64, _pcm16le: &[u8]) {}
    pub async fn classify(
        &self,
        _generation: u64,
        _range: Option<SourceAudioRange>,
        _confirmed_at: Instant,
    ) -> SpeakerVerdict {
        SpeakerVerdict {
            label: SpeakerLabel::Unknown,
            score: None,
            voiced_ms: 0,
            basis: SpeakerBasis::Unavailable,
        }
    }
    pub fn voiceprint_status(&self) -> VoiceprintStatus {
        VoiceprintStatus {
            voiceprint: VoiceprintState::Missing,
            duration_ms: None,
            updated_at_ms: None,
            model: SpeakerModelState::Missing,
        }
    }
}
