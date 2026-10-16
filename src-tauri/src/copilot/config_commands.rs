#![allow(unused_variables)] // T0 command signatures are wired before T1 implements them.

use super::config::{
    CopilotConfig, CopilotModelOption, CopilotModelRef, CopilotScenarioPatch, CopilotStore,
};
use super::engine::CopilotEngine;
use crate::settings_store::SettingsStore;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn copilot_config_get(
    store: State<'_, Arc<CopilotStore>>,
) -> Result<CopilotConfig, String> {
    Ok(store.snapshot())
}

#[tauri::command]
pub async fn copilot_config_set_enabled(
    _app: AppHandle,
    _store: State<'_, Arc<CopilotStore>>,
    _engine: State<'_, Arc<CopilotEngine>>,
    enabled: bool,
) -> Result<CopilotConfig, String> {
    Err("not_implemented".into())
}

#[tauri::command]
pub async fn copilot_model_options(
    _settings: State<'_, Arc<SettingsStore>>,
) -> Result<Vec<CopilotModelOption>, String> {
    Err("not_implemented".into())
}

#[tauri::command]
pub async fn copilot_select_model(
    _app: AppHandle,
    _store: State<'_, Arc<CopilotStore>>,
    _engine: State<'_, Arc<CopilotEngine>>,
    model: Option<CopilotModelRef>,
) -> Result<CopilotConfig, String> {
    Err("not_implemented".into())
}

#[tauri::command]
pub async fn copilot_scenario_create(
    _app: AppHandle,
    _store: State<'_, Arc<CopilotStore>>,
    _engine: State<'_, Arc<CopilotEngine>>,
    name: String,
) -> Result<CopilotConfig, String> {
    Err("not_implemented".into())
}

#[tauri::command]
pub async fn copilot_scenario_update(
    _app: AppHandle,
    _store: State<'_, Arc<CopilotStore>>,
    _engine: State<'_, Arc<CopilotEngine>>,
    id: String,
    patch: CopilotScenarioPatch,
) -> Result<CopilotConfig, String> {
    Err("not_implemented".into())
}

#[tauri::command]
pub async fn copilot_scenario_delete(
    _app: AppHandle,
    _store: State<'_, Arc<CopilotStore>>,
    _engine: State<'_, Arc<CopilotEngine>>,
    id: String,
) -> Result<CopilotConfig, String> {
    Err("not_implemented".into())
}

#[tauri::command]
pub async fn copilot_scenario_select(
    _app: AppHandle,
    _store: State<'_, Arc<CopilotStore>>,
    _engine: State<'_, Arc<CopilotEngine>>,
    id: String,
) -> Result<CopilotConfig, String> {
    Err("not_implemented".into())
}
