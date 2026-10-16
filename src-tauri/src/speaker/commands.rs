use super::voiceprint::{SpeakerService, VoiceprintStatus};
use crate::commands::AppState;
use crate::copilot::engine::CopilotEngine;
use crate::settings_store::SettingsStore;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn speaker_voiceprint_status(
    service: State<'_, Arc<SpeakerService>>,
    settings: State<'_, Arc<SettingsStore>>,
) -> Result<VoiceprintStatus, String> {
    if settings.is_ui_test() {
        return Ok(SpeakerService::new(None).voiceprint_status());
    }
    Ok(service.voiceprint_status())
}

#[tauri::command]
pub async fn speaker_voiceprint_import(
    app: AppHandle,
    service: State<'_, Arc<SpeakerService>>,
    settings: State<'_, Arc<SettingsStore>>,
    copilot: State<'_, Arc<CopilotEngine>>,
    path: String,
) -> Result<VoiceprintStatus, String> {
    if settings.is_ui_test() {
        return Err("speaker_ui_test_unavailable".into());
    }
    if service.is_recording() {
        return Err("speaker_recording_active".into());
    }
    let path = if path.is_empty() {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        app.dialog()
            .file()
            .add_filter("WAV audio", &["wav"])
            .pick_file(move |selection| {
                let _ = sender.send(selection);
            });
        let Some(selection) = receiver.await.map_err(|_| "speaker_picker_failed")? else {
            return Ok(service.voiceprint_status());
        };
        selection.into_path().map_err(|_| "speaker_wav_invalid")?
    } else {
        std::path::PathBuf::from(path)
    };
    let status = service.import(path).await?;
    copilot.refresh_availability();
    Ok(status)
}

#[tauri::command]
pub async fn speaker_voiceprint_record_start(
    app: AppHandle,
    service: State<'_, Arc<SpeakerService>>,
    settings: State<'_, Arc<SettingsStore>>,
    state: State<'_, AppState>,
    max_seconds: Option<u32>,
) -> Result<(), String> {
    if settings.is_ui_test() {
        return Err("speaker_ui_test_unavailable".into());
    }
    if state.session.is_active() {
        return Err("session_active".into());
    }
    service.inner().record_start(app, max_seconds).await
}

#[tauri::command]
pub async fn speaker_voiceprint_record_stop(
    _app: AppHandle,
    service: State<'_, Arc<SpeakerService>>,
    settings: State<'_, Arc<SettingsStore>>,
    state: State<'_, AppState>,
    copilot: State<'_, Arc<CopilotEngine>>,
) -> Result<VoiceprintStatus, String> {
    if settings.is_ui_test() {
        return Err("speaker_ui_test_unavailable".into());
    }
    if state.session.is_active() {
        service.record_cancel().await?;
        return Err("session_active".into());
    }
    let status = service.record_stop().await?;
    copilot.refresh_availability();
    Ok(status)
}

#[tauri::command]
pub async fn speaker_voiceprint_record_cancel(
    _app: AppHandle,
    service: State<'_, Arc<SpeakerService>>,
    settings: State<'_, Arc<SettingsStore>>,
) -> Result<(), String> {
    if settings.is_ui_test() {
        return Err("speaker_ui_test_unavailable".into());
    }
    service.record_cancel().await
}
