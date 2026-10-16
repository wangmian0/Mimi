use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;

pub const COPILOT_CONFIG_FILE: &str = "copilot.json";
pub const COPILOT_CONFIG_CHANGED_EVENT: &str = "copilot-config-changed";
pub const MAX_SCENARIOS: usize = 20;
pub const MAX_SCENARIO_TEXT_CHARS: usize = 8_000;
const MAX_CONFIG_BYTES: u64 = 256 * 1024;
const MAX_SCENARIO_NAME_CHARS: usize = 64;
const CONFIG_UNAVAILABLE: &str = "copilot_config_unavailable";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CopilotModelRef {
    pub profile_id: String,
    pub route: CopilotModelRoute,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredConfig {
    schema_version: u32,
    #[serde(flatten)]
    config: CopilotConfig,
}

struct StoreState {
    config: CopilotConfig,
    write_blocked: bool,
}
pub struct CopilotStore {
    path: Option<PathBuf>,
    state: Mutex<StoreState>,
}

impl CopilotStore {
    pub fn load(app_config_dir: PathBuf, is_ui_test: bool) -> Self {
        let path = (!is_ui_test).then(|| app_config_dir.join(COPILOT_CONFIG_FILE));
        let mut state = StoreState {
            config: CopilotConfig::default(),
            write_blocked: false,
        };
        if let Some(path) = &path {
            match fs::File::open(path) {
                Ok(file) => {
                    let mut bytes = Vec::new();
                    let loaded = file
                        .metadata()
                        .ok()
                        .filter(|m| m.len() <= MAX_CONFIG_BYTES)
                        .and_then(|_| file.take(MAX_CONFIG_BYTES + 1).read_to_end(&mut bytes).ok())
                        .filter(|&len| len as u64 <= MAX_CONFIG_BYTES)
                        .and_then(|_| serde_json::from_slice::<StoredConfig>(&bytes).ok())
                        .filter(|s| s.schema_version == 1 && validate(&s.config).is_ok());
                    match loaded {
                        Some(stored) => state.config = stored.config,
                        None => state.write_blocked = true,
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => state.write_blocked = true,
            }
        }
        Self {
            path,
            state: Mutex::new(state),
        }
    }

    pub fn snapshot(&self) -> CopilotConfig {
        self.state.lock().unwrap().config.clone()
    }
    pub fn is_write_blocked(&self) -> bool {
        self.state.lock().unwrap().write_blocked
    }
    pub fn active_scenario(&self) -> Option<CopilotScenario> {
        let state = self.state.lock().unwrap();
        state
            .config
            .scenarios
            .iter()
            .find(|s| Some(&s.id) == state.config.active_scenario_id.as_ref())
            .cloned()
    }
    fn mutate(
        &self,
        change: impl FnOnce(&mut CopilotConfig) -> Result<(), String>,
    ) -> Result<CopilotConfig, String> {
        let mut state = self.state.lock().unwrap();
        if state.write_blocked {
            return Err(CONFIG_UNAVAILABLE.into());
        }
        let mut next = state.config.clone();
        change(&mut next)?;
        validate(&next)?;
        if let Some(path) = &self.path {
            persist(path, &next)?;
        }
        state.config = next.clone();
        Ok(next)
    }
    pub fn set_enabled(&self, enabled: bool) -> Result<CopilotConfig, String> {
        self.mutate(|c| {
            c.enabled = enabled;
            Ok(())
        })
    }
    pub fn select_model(&self, model: Option<CopilotModelRef>) -> Result<CopilotConfig, String> {
        self.mutate(|c| {
            c.model = model;
            Ok(())
        })
    }
    pub fn create_scenario(&self, name: &str) -> Result<CopilotConfig, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_SCENARIO_NAME_CHARS {
            return Err("copilot_scenario_name_invalid".into());
        }
        self.mutate(|c| {
            if c.scenarios.len() >= MAX_SCENARIOS {
                return Err("copilot_scenario_limit".into());
            }
            let id = uuid::Uuid::new_v4().to_string();
            c.scenarios.push(CopilotScenario {
                id: id.clone(),
                name: name.into(),
                ..Default::default()
            });
            c.active_scenario_id = Some(id);
            Ok(())
        })
    }
    pub fn update_scenario(
        &self,
        id: &str,
        patch: CopilotScenarioPatch,
    ) -> Result<CopilotConfig, String> {
        self.mutate(|c| {
            let s = c
                .scenarios
                .iter_mut()
                .find(|s| s.id == id)
                .ok_or("copilot_scenario_missing")?;
            if let Some(name) = patch.name {
                s.name = name.trim().into();
            }
            if let Some(text) = patch.prompt {
                s.prompt = text;
            }
            if let Some(text) = patch.background {
                s.background = text;
            }
            if let Some(text) = patch.strategy {
                s.strategy = text;
            }
            Ok(())
        })
    }
    pub fn delete_scenario(&self, id: &str) -> Result<CopilotConfig, String> {
        self.mutate(|c| {
            let before = c.scenarios.len();
            c.scenarios.retain(|s| s.id != id);
            if c.scenarios.len() == before {
                return Err("copilot_scenario_missing".into());
            }
            if c.active_scenario_id.as_deref() == Some(id) {
                c.active_scenario_id = c.scenarios.first().map(|s| s.id.clone());
            }
            Ok(())
        })
    }
    pub fn select_scenario(&self, id: &str) -> Result<CopilotConfig, String> {
        self.mutate(|c| {
            if !c.scenarios.iter().any(|s| s.id == id) {
                return Err("copilot_scenario_missing".into());
            }
            c.active_scenario_id = Some(id.into());
            Ok(())
        })
    }
}

fn validate(c: &CopilotConfig) -> Result<(), String> {
    if c.scenarios.len() > MAX_SCENARIOS {
        return Err("copilot_scenario_limit".into());
    }
    if c.model
        .as_ref()
        .is_some_and(|m| m.profile_id.trim().is_empty())
    {
        return Err("copilot_model_invalid".into());
    }
    let mut ids = HashSet::new();
    for s in &c.scenarios {
        if s.id.is_empty()
            || !ids.insert(&s.id)
            || s.name.trim().is_empty()
            || s.name.chars().count() > MAX_SCENARIO_NAME_CHARS
            || [&s.prompt, &s.background, &s.strategy]
                .iter()
                .any(|t| t.chars().count() > MAX_SCENARIO_TEXT_CHARS)
        {
            return Err("copilot_scenario_invalid".into());
        }
    }
    if !c.scenarios.is_empty() && c.active_scenario_id.is_none() {
        return Err("copilot_scenario_missing".into());
    }
    if c.active_scenario_id
        .as_ref()
        .is_some_and(|id| !ids.contains(id))
    {
        return Err("copilot_scenario_missing".into());
    }
    Ok(())
}

fn persist(path: &PathBuf, config: &CopilotConfig) -> Result<(), String> {
    let parent = path.parent().ok_or(CONFIG_UNAVAILABLE)?;
    fs::create_dir_all(parent).map_err(|_| CONFIG_UNAVAILABLE.to_string())?;
    let bytes = serde_json::to_vec(&StoredConfig {
        schema_version: 1,
        config: config.clone(),
    })
    .map_err(|_| CONFIG_UNAVAILABLE.to_string())?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err("copilot_config_too_large".into());
    }
    let mut temp =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| CONFIG_UNAVAILABLE.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| CONFIG_UNAVAILABLE.to_string())?;
    }
    temp.write_all(&bytes)
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|_| CONFIG_UNAVAILABLE.to_string())?;
    temp.persist(path)
        .map_err(|_| CONFIG_UNAVAILABLE.to_string())?;
    #[cfg(unix)]
    {
        let _ = fs::File::open(parent).and_then(|dir| dir.sync_all());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persists_and_reloads_privately() {
        let dir = tempfile::tempdir().unwrap();
        let store = CopilotStore::load(dir.path().to_path_buf(), false);
        let created = store.create_scenario("Call").unwrap();
        store
            .update_scenario(
                &created.scenarios[0].id,
                CopilotScenarioPatch {
                    prompt: Some("a\nb".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            CopilotStore::load(dir.path().to_path_buf(), false)
                .active_scenario()
                .unwrap()
                .prompt,
            "a\nb"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(dir.path().join(COPILOT_CONFIG_FILE))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn damaged_and_oversized_files_lock_writes() {
        for data in [
            b"broken".to_vec(),
            vec![b'x'; MAX_CONFIG_BYTES as usize + 1],
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(COPILOT_CONFIG_FILE);
            fs::write(&path, &data).unwrap();
            let store = CopilotStore::load(dir.path().to_path_buf(), false);
            assert!(store.is_write_blocked());
            assert!(store.set_enabled(true).is_err());
            assert_eq!(fs::read(path).unwrap(), data);
        }
    }
    #[test]
    fn failed_validation_preserves_snapshot() {
        let store = CopilotStore::load(PathBuf::new(), true);
        let created = store.create_scenario("Call").unwrap();
        assert!(store
            .update_scenario(
                &created.scenarios[0].id,
                CopilotScenarioPatch {
                    strategy: Some("x".repeat(MAX_SCENARIO_TEXT_CHARS + 1)),
                    ..Default::default()
                }
            )
            .is_err());
        assert!(store.active_scenario().unwrap().strategy.is_empty());
    }

    #[test]
    fn twenty_scenarios_are_allowed_and_the_next_is_rejected() {
        let store = CopilotStore::load(PathBuf::new(), true);
        for index in 0..MAX_SCENARIOS {
            store.create_scenario(&format!("Call {index}")).unwrap();
        }
        assert!(store.create_scenario("one more").is_err());
        assert_eq!(store.snapshot().scenarios.len(), MAX_SCENARIOS);
    }

    #[test]
    fn full_valid_scenarios_cannot_write_a_file_that_reload_would_reject() {
        let dir = tempfile::tempdir().unwrap();
        let store = CopilotStore::load(dir.path().to_path_buf(), false);
        for index in 0..11 {
            let created = store.create_scenario(&format!("Call {index}")).unwrap();
            let id = created.active_scenario_id.unwrap();
            let patch = CopilotScenarioPatch {
                prompt: Some("x".repeat(MAX_SCENARIO_TEXT_CHARS)),
                background: Some("x".repeat(MAX_SCENARIO_TEXT_CHARS)),
                strategy: Some("x".repeat(MAX_SCENARIO_TEXT_CHARS)),
                ..Default::default()
            };
            let before = store.snapshot();
            if store.update_scenario(&id, patch).is_err() {
                assert_eq!(
                    store.snapshot().scenarios[index].prompt,
                    before.scenarios[index].prompt
                );
                let reopened = CopilotStore::load(dir.path().to_path_buf(), false);
                assert!(reopened.set_enabled(true).is_ok());
                return;
            }
        }
        panic!("test fixture did not exceed the config read limit");
    }
}
