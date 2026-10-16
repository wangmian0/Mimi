#![allow(unused_variables)] // T0 command signatures are wired before T4 implements them.

use super::voiceprint::{SpeakerService, VoiceprintStatus};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn speaker_voiceprint_status(
    service: State<'_, Arc<SpeakerService>>,
) -> Result<VoiceprintStatus, String> {
    Ok(service.voiceprint_status())
}

#[tauri::command]
pub async fn speaker_voiceprint_import(
    _app: AppHandle,
    _service: State<'_, Arc<SpeakerService>>,
    path: String,
) -> Result<VoiceprintStatus, String> {
    Err("not_implemented".into())
}

#[tauri::command]
pub async fn speaker_voiceprint_record_start(
    _app: AppHandle,
    _service: State<'_, Arc<SpeakerService>>,
    max_seconds: Option<u32>,
) -> Result<(), String> {
    Err("not_implemented".into())
}

#[tauri::command]
pub async fn speaker_voiceprint_record_stop(
    _app: AppHandle,
    _service: State<'_, Arc<SpeakerService>>,
) -> Result<VoiceprintStatus, String> {
    Err("not_implemented".into())
}

#[tauri::command]
pub async fn speaker_voiceprint_record_cancel(
    _app: AppHandle,
    _service: State<'_, Arc<SpeakerService>>,
) -> Result<(), String> {
    Err("not_implemented".into())
}
