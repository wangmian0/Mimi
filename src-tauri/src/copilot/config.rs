use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const COPILOT_CONFIG_FILE: &str = "copilot.json";
pub const COPILOT_CONFIG_CHANGED_EVENT: &str = "copilot-config-changed";
pub const MAX_SCENARIOS: usize = 20;
pub const MAX_SCENARIO_TEXT_CHARS: usize = 8_000;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotScenario {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub background: String,
    pub strategy: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CopilotModelRoute {
    OpenAICompatible,
    ChatMock,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotModelRef {
    pub profile_id: String,
    pub route: CopilotModelRoute,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotConfig {
    pub enabled: bool,
    pub model: Option<CopilotModelRef>,
    pub active_scenario_id: Option<String>,
    pub scenarios: Vec<CopilotScenario>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CopilotScenarioPatch {
    pub name: Option<String>,
    pub prompt: Option<String>,
    pub background: Option<String>,
    pub strategy: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotModelOption {
    pub profile_id: String,
    pub profile_name: String,
    pub route: CopilotModelRoute,
    pub route_name: Option<String>,
    pub model: String,
    pub has_api_key: bool,
}

pub struct CopilotStore;

impl CopilotStore {
    pub fn load(_app_config_dir: PathBuf, _is_ui_test: bool) -> Self {
        Self
    }

    pub fn snapshot(&self) -> CopilotConfig {
        CopilotConfig::default()
    }

    pub fn active_scenario(&self) -> Option<CopilotScenario> {
        None
    }

    pub fn set_enabled(&self, _enabled: bool) -> Result<CopilotConfig, String> {
        Err("not_implemented".into())
    }

    pub fn select_model(&self, _model: Option<CopilotModelRef>) -> Result<CopilotConfig, String> {
        Err("not_implemented".into())
    }

    pub fn create_scenario(&self, _name: &str) -> Result<CopilotConfig, String> {
        Err("not_implemented".into())
    }

    pub fn update_scenario(
        &self,
        _id: &str,
        _patch: CopilotScenarioPatch,
    ) -> Result<CopilotConfig, String> {
        Err("not_implemented".into())
    }

    pub fn delete_scenario(&self, _id: &str) -> Result<CopilotConfig, String> {
        Err("not_implemented".into())
    }

    pub fn select_scenario(&self, _id: &str) -> Result<CopilotConfig, String> {
        Err("not_implemented".into())
    }
}
