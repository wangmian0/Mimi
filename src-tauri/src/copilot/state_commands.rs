use super::engine::{CopilotEngine, CopilotOverlayState};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn copilot_get_state(
    engine: State<'_, Arc<CopilotEngine>>,
) -> Result<CopilotOverlayState, String> {
    Ok(engine.snapshot())
}

#[tauri::command]
pub async fn copilot_set_active(
    engine: State<'_, Arc<CopilotEngine>>,
    active: bool,
) -> Result<CopilotOverlayState, String> {
    engine.set_active(active).await
}
