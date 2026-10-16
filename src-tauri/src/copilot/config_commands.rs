use super::config::{
    CopilotConfig, CopilotModelOption, CopilotModelRef, CopilotScenarioPatch, CopilotStore,
    COPILOT_CONFIG_CHANGED_EVENT,
};
use super::engine::CopilotEngine;
use crate::settings_store::SettingsStore;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

async fn committed(
    app: &AppHandle,
    engine: &CopilotEngine,
    config: CopilotConfig,
) -> Result<CopilotConfig, String> {
    engine.config_changed().await;
    if app.emit(COPILOT_CONFIG_CHANGED_EVENT, &config).is_err() {
        tracing::warn!("copilot event unavailable label=config_changed");
    }
    Ok(config)
}

#[tauri::command]
pub async fn copilot_config_get(
    store: State<'_, Arc<CopilotStore>>,
) -> Result<CopilotConfig, String> {
    if store.is_write_blocked() {
        return Err("copilot_config_unavailable".into());
    }
    Ok(store.snapshot())
}

#[tauri::command]
pub async fn copilot_config_set_enabled(
    app: AppHandle,
    store: State<'_, Arc<CopilotStore>>,
    engine: State<'_, Arc<CopilotEngine>>,
    enabled: bool,
) -> Result<CopilotConfig, String> {
    committed(&app, &engine, store.set_enabled(enabled)?).await
}

#[tauri::command]
pub async fn copilot_model_options(
    settings: State<'_, Arc<SettingsStore>>,
) -> Result<Vec<CopilotModelOption>, String> {
    settings.chat_completions_options()
}

#[tauri::command]
pub async fn copilot_select_model(
    app: AppHandle,
    store: State<'_, Arc<CopilotStore>>,
    engine: State<'_, Arc<CopilotEngine>>,
    model: Option<CopilotModelRef>,
) -> Result<CopilotConfig, String> {
    committed(&app, &engine, store.select_model(model)?).await
}

#[tauri::command]
pub async fn copilot_scenario_create(
    app: AppHandle,
    store: State<'_, Arc<CopilotStore>>,
    engine: State<'_, Arc<CopilotEngine>>,
    name: String,
) -> Result<CopilotConfig, String> {
    committed(&app, &engine, store.create_scenario(&name)?).await
}

#[tauri::command]
pub async fn copilot_scenario_update(
    app: AppHandle,
    store: State<'_, Arc<CopilotStore>>,
    engine: State<'_, Arc<CopilotEngine>>,
    id: String,
    patch: CopilotScenarioPatch,
) -> Result<CopilotConfig, String> {
    committed(&app, &engine, store.update_scenario(&id, patch)?).await
}

#[tauri::command]
pub async fn copilot_scenario_delete(
    app: AppHandle,
    store: State<'_, Arc<CopilotStore>>,
    engine: State<'_, Arc<CopilotEngine>>,
    id: String,
) -> Result<CopilotConfig, String> {
    committed(&app, &engine, store.delete_scenario(&id)?).await
}

#[tauri::command]
pub async fn copilot_scenario_select(
    app: AppHandle,
    store: State<'_, Arc<CopilotStore>>,
    engine: State<'_, Arc<CopilotEngine>>,
    id: String,
) -> Result<CopilotConfig, String> {
    committed(&app, &engine, store.select_scenario(&id)?).await
}
