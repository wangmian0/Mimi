use super::config::{CopilotScenario, CopilotStore};
use super::conversation::CopilotConversation;
use crate::commands::AppState;
use crate::core::audio_input::AudioSource;
use crate::core::models::{SourceLanguage, TargetLanguage};
use crate::pipeline_log;
use crate::settings_store::SettingsStore;
use crate::speaker::voiceprint::{SpeakerLabel, SpeakerModelState, VoiceprintState};
use crate::speaker::{SpeakerService, SpeakerVerdict};
use mimi_runtime::clients::apple_translation_client::AppleTranslationClient;
use mimi_runtime::clients::copilot_client::CopilotClient;
use mimi_runtime::clients::provider_network::ProviderNetwork;
use mimi_runtime::clients::translation_client::SourceAudioRange;
use mimi_runtime::core::protocols::copilot_prompt::{self, CopilotPrompt};
use serde::Serialize;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, watch};

pub const COPILOT_STATE_EVENT: &str = "copilot-state";
const OBSERVATION_CAPACITY: usize = 32;
const LABEL_CAPACITY: usize = 32;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CopilotUnavailableReason {
    Disabled,
    ModelMissing,
    VoiceprintMissing,
    MicrophoneRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
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

struct Observation {
    line: ConfirmedLine,
    generation: u64,
    range: Option<SourceAudioRange>,
    epoch: u64,
}

struct EngineState {
    overlay: CopilotOverlayState,
    conversation: CopilotConversation,
    client: Option<CopilotClient>,
    request_id: u64,
    initial_trigger: bool,
}

pub struct CopilotEngine {
    self_ref: Weak<Self>,
    app: Option<AppHandle>,
    settings: Arc<SettingsStore>,
    store: Arc<CopilotStore>,
    speaker: Arc<SpeakerService>,
    speaker_transition: tokio::sync::Mutex<()>,
    active: AtomicBool,
    epoch: AtomicU64,
    epoch_tx: watch::Sender<u64>,
    dropped: AtomicU64,
    request_attempts: AtomicU64,
    superseded_count: AtomicU64,
    emit_scheduled: AtomicBool,
    state: Mutex<EngineState>,
    observation_tx: mpsc::Sender<Observation>,
    request_task: Mutex<Option<JoinHandle<()>>>,
}

impl CopilotEngine {
    pub fn new(
        app: AppHandle,
        settings: Arc<SettingsStore>,
        store: Arc<CopilotStore>,
        speaker: Arc<SpeakerService>,
    ) -> Arc<Self> {
        let (observation_tx, mut observation_rx) = mpsc::channel(OBSERVATION_CAPACITY);
        let (epoch_tx, _) = watch::channel(0);
        let engine = Arc::new_cyclic(|weak| Self {
            self_ref: weak.clone(),
            app: Some(app),
            settings,
            store,
            speaker,
            speaker_transition: tokio::sync::Mutex::new(()),
            active: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            epoch_tx,
            dropped: AtomicU64::new(0),
            request_attempts: AtomicU64::new(0),
            superseded_count: AtomicU64::new(0),
            emit_scheduled: AtomicBool::new(false),
            state: Mutex::new(EngineState {
                overlay: CopilotOverlayState::default(),
                conversation: CopilotConversation::new(),
                client: None,
                request_id: 0,
                initial_trigger: false,
            }),
            observation_tx,
            request_task: Mutex::new(None),
        });
        let worker = Arc::clone(&engine);
        tauri::async_runtime::spawn(async move {
            while let Some(observation) = observation_rx.recv().await {
                if !worker.is_epoch_current(observation.epoch)
                    || !worker.generation_current(observation.generation)
                {
                    continue;
                }
                let verdict = worker
                    .speaker
                    .classify(
                        observation.generation,
                        observation.range,
                        observation.line.confirmed_at,
                    )
                    .await;
                if worker.is_epoch_current(observation.epoch)
                    && worker.generation_current(observation.generation)
                {
                    worker.observe_confirmed(observation.line, verdict);
                }
            }
        });
        engine.refresh_availability();
        engine
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub fn enqueue_confirmed(
        &self,
        line: ConfirmedLine,
        generation: u64,
        range: Option<SourceAudioRange>,
    ) {
        if !self.is_active() {
            return;
        }
        let observation = Observation {
            line,
            generation,
            range,
            epoch: self.epoch.load(Ordering::Acquire),
        };
        if self.observation_tx.try_send(observation).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            pipeline_log!(
                "copilot observation dropped count={}",
                self.dropped.load(Ordering::Relaxed)
            );
        }
    }

    pub fn observe_confirmed(&self, line: ConfirmedLine, verdict: SpeakerVerdict) {
        if !self.is_active() || line.source != AudioSource::Microphone {
            return;
        }
        let speaker = match verdict.label {
            SpeakerLabel::Me => CopilotSpeaker::Me,
            SpeakerLabel::Other => CopilotSpeaker::Other,
            SpeakerLabel::Unknown => {
                pipeline_log!("copilot skipped_reason=1 voiced_ms={}", verdict.voiced_ms);
                return;
            }
        };
        let (request, client, epoch, request_id) = {
            let mut state = self.state.lock().unwrap();
            state.overlay.speakers.push(CopilotSpeakerLabel {
                audio_source: line.source,
                created_at: line.created_at_ms,
                speaker,
            });
            if state.overlay.speakers.len() > LABEL_CAPACITY {
                state.overlay.speakers.remove(0);
            }
            state.conversation.append(speaker, &line.text);
            if matches!(speaker, CopilotSpeaker::Other) {
                state.initial_trigger = true;
            }
            let request = if matches!(speaker, CopilotSpeaker::Other) {
                Some((
                    state.conversation.prefix().to_owned(),
                    state.conversation.transcript().to_owned(),
                ))
            } else {
                None
            };
            if request.is_some() {
                state.request_id = state.request_id.wrapping_add(1);
            }
            let id = state.request_id;
            let client = state.client.clone();
            state.overlay.revision = state.overlay.revision.wrapping_add(1);
            (request, client, self.epoch.load(Ordering::Acquire), id)
        };
        self.schedule_emit();
        if let (Some((prefix, transcript)), Some(client)) = (request, client) {
            self.launch_request(
                epoch,
                request_id,
                client,
                prefix,
                transcript,
                Some(line.confirmed_at),
            );
        }
    }

    pub async fn config_changed(&self) {
        self.deactivate().await;
        self.refresh_availability();
    }

    pub fn reset_conversation(&self) {
        let mut state = self.state.lock().unwrap();
        let warming = self.is_active() && matches!(state.overlay.phase, CopilotPhase::Warming);
        let desired_phase = if warming {
            CopilotPhase::Warming
        } else if self.is_active() {
            CopilotPhase::Waiting
        } else {
            CopilotPhase::Idle
        };
        let changed = !state.conversation.is_empty()
            || !state.overlay.speakers.is_empty()
            || state.overlay.suggestion.is_some()
            || state.overlay.phase != desired_phase
            || state.overlay.error_label.is_some();
        state.conversation.clear();
        state.initial_trigger = false;
        state.overlay.speakers.clear();
        state.overlay.suggestion = None;
        state.overlay.phase = desired_phase;
        state.overlay.error_label = None;
        if changed {
            state.overlay.revision = state.overlay.revision.wrapping_add(1);
        }
        drop(state);
        if !warming {
            let epoch = self.epoch.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
            self.epoch_tx.send_replace(epoch);
            self.abort_request();
        }
        if changed {
            self.schedule_emit();
        }
    }

    pub async fn deactivate(&self) {
        self.active.store(false, Ordering::Release);
        self.reset_conversation();
        let changed = {
            let mut state = self.state.lock().unwrap();
            let changed = state.client.is_some()
                || state.overlay.active
                || state.overlay.phase != CopilotPhase::Idle;
            state.client = None;
            state.overlay.active = false;
            state.overlay.phase = CopilotPhase::Idle;
            if changed {
                state.overlay.revision = state.overlay.revision.wrapping_add(1);
            }
            changed
        };
        if changed {
            self.schedule_emit();
        }
        let _transition = self.speaker_transition.lock().await;
        self.speaker.set_active(false).await;
    }

    pub async fn set_active(&self, active: bool) -> Result<CopilotOverlayState, String> {
        if !active {
            self.deactivate().await;
            return Ok(self.snapshot());
        }
        self.deactivate().await;
        if self.settings.is_ui_test() {
            let epoch = self.epoch.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
            self.epoch_tx.send_replace(epoch);
            self.active.store(true, Ordering::Release);
            let mut state = self.state.lock().unwrap();
            state.overlay.available = true;
            state.overlay.unavailable_reason = None;
            state.overlay.active = true;
            state.overlay.phase = CopilotPhase::Ready;
            state.overlay.suggestion = Some(CopilotSuggestion {
                id: 1,
                english: "Could you please clarify the next step?".into(),
                chinese: Some("您能说明下一步吗？".into()),
                is_final: true,
            });
            state.overlay.revision = state.overlay.revision.wrapping_add(1);
            let snapshot = state.overlay.clone();
            drop(state);
            self.schedule_emit();
            return Ok(snapshot);
        }
        let config = self.store.snapshot();
        if !config.enabled {
            self.set_unavailable(CopilotUnavailableReason::Disabled);
            return Err("copilot_disabled".into());
        }
        let Some(model) = config.model else {
            self.set_unavailable(CopilotUnavailableReason::ModelMissing);
            return Err("copilot_model_missing".into());
        };
        let Some(scenario) = self.store.active_scenario() else {
            self.set_unavailable(CopilotUnavailableReason::ModelMissing);
            return Err("copilot_scenario_missing".into());
        };
        if !self.settings.is_ui_test() {
            let Some(app_state) = self
                .app
                .as_ref()
                .and_then(|app| app.try_state::<AppState>())
            else {
                return Err("copilot_session_unavailable".into());
            };
            if !app_state.session.has_active_session()
                || !self
                    .settings
                    .preferences()
                    .audio_input
                    .sources()
                    .contains(&AudioSource::Microphone)
            {
                self.set_unavailable(CopilotUnavailableReason::MicrophoneRequired);
                return Err("copilot_microphone_required".into());
            }
        }
        let status = self.speaker.voiceprint_status();
        if !self.settings.is_ui_test()
            && (!matches!(status.voiceprint, VoiceprintState::Ready)
                || !matches!(status.model, SpeakerModelState::Ready))
        {
            self.set_unavailable(CopilotUnavailableReason::VoiceprintMissing);
            return Err("copilot_voiceprint_missing".into());
        }
        let previous = self
            .app
            .as_ref()
            .and_then(|app| app.try_state::<AppState>())
            .map(|app_state| {
                app_state
                    .session
                    .current_state_event()
                    .subtitles
                    .history
                    .into_iter()
                    .rev()
                    .take(6)
                    .map(|pair| pair.source)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let Some((epoch, prefix)) = self.begin_activation(&scenario, &previous).await else {
            return Ok(self.snapshot());
        };
        let target = match self
            .settings
            .chat_completions_target(&model.profile_id, model.route)
        {
            Ok(target) => target,
            Err(_) => {
                self.fail_start(epoch, "copilot_model_unavailable".into())
                    .await;
                return Err("copilot_model_unavailable".into());
            }
        };
        let network = match ProviderNetwork::resolve(&target.network_proxy) {
            Ok(network) => network,
            Err(_) => {
                self.fail_start(epoch, "copilot_network_invalid".into())
                    .await;
                return Err("copilot_network_invalid".into());
            }
        };
        let client =
            match CopilotClient::new(&target.endpoint, &target.api_key, &target.model, network) {
                Ok(client) => client,
                Err(error) => {
                    let label = error.diagnostic_label();
                    self.fail_start(epoch, label.clone()).await;
                    return Err(label);
                }
            };
        let apple = match AppleTranslationClient::new(
            SourceLanguage::English,
            TargetLanguage::SimplifiedChinese,
        ) {
            Ok(apple) => apple,
            Err(error) => {
                let label = error.diagnostic_label();
                self.fail_start(epoch, label.clone()).await;
                return Err(label);
            }
        };
        self.warm_and_finish(epoch, client, prefix, async {
            let (speaker_ready, apple_ready) = tokio::join!(
                tokio::time::timeout(Duration::from_secs(20), self.speaker.prewarm()),
                apple.check_ready()
            );
            (
                matches!(speaker_ready, Ok(Ok(()))),
                apple_ready.err().map(|error| error.diagnostic_label()),
            )
        })
        .await
    }

    async fn warm_and_finish(
        &self,
        epoch: u64,
        client: CopilotClient,
        prefix: String,
        readiness: impl Future<Output = (bool, Option<String>)>,
    ) -> Result<CopilotOverlayState, String> {
        let mut cancellation = self.epoch_tx.subscribe();
        if *cancellation.borrow() != epoch {
            return Ok(self.snapshot());
        }
        if !copilot_prompt::prefix_fits(&prefix) {
            let label = "copilot_scenario_too_large".to_owned();
            self.fail_start(epoch, label.clone()).await;
            return Err(label);
        }
        let request_count = self.request_attempts.fetch_add(1, Ordering::Relaxed) + 1;
        pipeline_log!(
            "copilot request_count={} superseded_count={} kind=0",
            request_count,
            self.superseded_count.load(Ordering::Relaxed)
        );
        let (warm, speaker_ready, apple_error) = tokio::select! {
            result = async { tokio::join!(client.warm(&prefix), readiness) } => (result.0, result.1.0, result.1.1),
            _ = cancellation.changed() => {
                self.record_superseded();
                return Ok(self.snapshot());
            },
        };
        if !self.is_epoch_current(epoch) {
            return Ok(self.snapshot());
        }
        let warm = match warm {
            Ok(warm) => warm,
            Err(error) => {
                self.fail_start(epoch, error.diagnostic_label()).await;
                return Err("copilot_warmup_failed".into());
            }
        };
        if !speaker_ready {
            self.fail_start(epoch, "copilot_speaker_unavailable".into())
                .await;
            return Err("copilot_speaker_unavailable".into());
        }
        pipeline_log!(
            "copilot warmup total_ms={} cache_hit_tokens={} cache_miss_tokens={}",
            warm.total_ms,
            warm.usage
                .and_then(|usage| usage.cache_hit_tokens)
                .unwrap_or(0),
            warm.usage
                .and_then(|usage| usage.cache_miss_tokens)
                .unwrap_or(0)
        );
        Ok(self.finish_warm(epoch, client, apple_error))
    }

    async fn begin_activation(
        &self,
        scenario: &CopilotScenario,
        previous: &[String],
    ) -> Option<(u64, String)> {
        let epoch = self.epoch.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
        self.epoch_tx.send_replace(epoch);
        let mut conversation = CopilotConversation::new();
        conversation.configure(scenario);
        conversation.backfill(previous);
        let prefix = conversation.prefix().to_owned();
        let initial_trigger = !conversation.is_empty();
        let _transition = self.speaker_transition.lock().await;
        if self.epoch.load(Ordering::Acquire) != epoch {
            return None;
        }
        self.speaker.set_active(true).await;
        if self.epoch.load(Ordering::Acquire) != epoch {
            return None;
        }
        {
            let mut state = self.state.lock().unwrap();
            state.conversation = conversation;
            state.initial_trigger = initial_trigger;
            state.overlay.active = true;
            state.overlay.available = true;
            state.overlay.unavailable_reason = None;
            state.overlay.phase = CopilotPhase::Warming;
            state.overlay.revision = state.overlay.revision.wrapping_add(1);
        }
        self.active.store(true, Ordering::Release);
        self.schedule_emit();
        Some((epoch, prefix))
    }

    fn finish_warm(
        &self,
        epoch: u64,
        client: CopilotClient,
        apple_error: Option<String>,
    ) -> CopilotOverlayState {
        let (initial, id) = {
            let mut state = self.state.lock().unwrap();
            if !self.is_epoch_current(epoch) {
                return state.overlay.clone();
            }
            state.client = Some(client.clone());
            state.overlay.phase = CopilotPhase::Waiting;
            state.overlay.error_label = apple_error;
            let initial = state.initial_trigger.then(|| {
                (
                    state.conversation.prefix().to_owned(),
                    state.conversation.transcript().to_owned(),
                )
            });
            state.initial_trigger = false;
            if initial.is_some() {
                state.request_id = state.request_id.wrapping_add(1);
            }
            state.overlay.revision = state.overlay.revision.wrapping_add(1);
            (initial, state.request_id)
        };
        self.schedule_emit();
        if let Some((prefix, transcript)) = initial {
            self.launch_request(epoch, id, client, prefix, transcript, None);
        }
        self.snapshot()
    }

    pub fn snapshot(&self) -> CopilotOverlayState {
        self.state.lock().unwrap().overlay.clone()
    }

    fn generation_current(&self, generation: u64) -> bool {
        self.app.as_ref().is_none_or(|app| {
            app.try_state::<AppState>()
                .is_some_and(|state| state.session.is_generation_current(generation))
        })
    }

    fn is_epoch_current(&self, epoch: u64) -> bool {
        self.is_active() && self.epoch.load(Ordering::Acquire) == epoch
    }

    fn request_current(&self, epoch: u64, id: u64) -> bool {
        self.is_epoch_current(epoch) && self.state.lock().unwrap().request_id == id
    }

    fn abort_request(&self) {
        if let Some(task) = self.request_task.lock().unwrap().take() {
            if !task.inner().is_finished() {
                self.record_superseded();
            }
            task.abort();
        }
    }

    fn record_superseded(&self) {
        let superseded_count = self.superseded_count.fetch_add(1, Ordering::Relaxed) + 1;
        pipeline_log!(
            "copilot request_count={} superseded_count={}",
            self.request_attempts.load(Ordering::Relaxed),
            superseded_count
        );
    }

    fn launch_request(
        &self,
        epoch: u64,
        id: u64,
        client: CopilotClient,
        prefix: String,
        transcript: String,
        confirmed_at: Option<Instant>,
    ) {
        {
            let mut state = self.state.lock().unwrap();
            if !self.is_epoch_current(epoch) || state.request_id != id {
                return;
            }
            state.overlay.phase = CopilotPhase::Streaming;
            state.overlay.suggestion = Some(CopilotSuggestion {
                id,
                english: String::new(),
                chinese: None,
                is_final: false,
            });
            state.overlay.error_label = None;
            state.overlay.revision = state.overlay.revision.wrapping_add(1);
        }
        self.schedule_emit();
        let mut task_slot = self.request_task.lock().unwrap();
        if !self.request_current(epoch, id) {
            return;
        }
        if let Some(old) = task_slot.take() {
            if !old.inner().is_finished() {
                self.record_superseded();
            }
            old.abort();
        }
        let request_count = self.request_attempts.fetch_add(1, Ordering::Relaxed) + 1;
        pipeline_log!(
            "copilot started id={} lines={} prefix_bytes={} transcript_bytes={} request_count={} superseded_count={}",
            id,
            transcript.lines().count(),
            prefix.len(),
            transcript.len(),
            request_count,
            self.superseded_count.load(Ordering::Relaxed)
        );
        let engine = self
            .self_ref
            .upgrade()
            .expect("copilot engine owned by app");
        let task = tauri::async_runtime::spawn(async move {
            let result = client
                .stream(
                    CopilotPrompt {
                        prefix: &prefix,
                        transcript: &transcript,
                    },
                    |delta| {
                        engine.apply_delta_if_current(epoch, id, delta);
                    },
                )
                .await;
            if !engine.request_current(epoch, id) {
                pipeline_log!("copilot superseded id={}", id);
                return;
            }
            match result {
                Ok(completion) => {
                    let confirm_to_done = confirmed_at
                        .map(|at| at.elapsed().as_millis() as u64)
                        .unwrap_or(0);
                    let confirm_to_first = confirm_to_done
                        .saturating_sub(completion.total_ms)
                        .saturating_add(completion.first_token_ms.unwrap_or(0));
                    pipeline_log!("copilot done id={} first_token_ms={} total_ms={} confirm_to_first_ms={} confirm_to_done_ms={} prompt_tokens={} completion_tokens={} cache_hit_tokens={} cache_miss_tokens={}", id, completion.first_token_ms.unwrap_or(0), completion.total_ms, confirm_to_first, confirm_to_done, completion.usage.map(|usage| usage.prompt_tokens).unwrap_or(0), completion.usage.map(|usage| usage.completion_tokens).unwrap_or(0), completion.usage.and_then(|usage| usage.cache_hit_tokens).unwrap_or(0), completion.usage.and_then(|usage| usage.cache_miss_tokens).unwrap_or(0));
                    if !engine.apply_final_if_current(epoch, id, &completion.text) {
                        return;
                    }
                    let apple_start = Instant::now();
                    let translated = if engine.app.is_none() {
                        Err("apple_translation_unavailable".into())
                    } else {
                        let apple = AppleTranslationClient::new(
                            SourceLanguage::English,
                            TargetLanguage::SimplifiedChinese,
                        );
                        match apple {
                            Ok(apple) => tokio::time::timeout(
                                Duration::from_secs(3),
                                apple.translate(&completion.text, None),
                            )
                            .await
                            .map_err(|_| "apple_translation_timeout".to_string())
                            .and_then(|result| result.map_err(|error| error.diagnostic_label())),
                            Err(error) => Err(error.diagnostic_label()),
                        }
                    };
                    pipeline_log!(
                        "copilot apple id={} duration_ms={} success={}",
                        id,
                        apple_start.elapsed().as_millis(),
                        u8::from(translated.is_ok())
                    );
                    engine.apply_chinese_if_current(epoch, id, translated);
                }
                Err(error) => {
                    let mut state = engine.state.lock().unwrap();
                    if !engine.is_epoch_current(epoch) || state.request_id != id {
                        return;
                    }
                    state.overlay.phase = CopilotPhase::Error;
                    state.overlay.error_label = Some(error.diagnostic_label());
                    state.overlay.revision = state.overlay.revision.wrapping_add(1);
                    drop(state);
                    engine.schedule_emit();
                }
            }
        });
        *task_slot = Some(task);
    }

    fn apply_delta_if_current(&self, epoch: u64, id: u64, delta: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        if !self.is_epoch_current(epoch) || state.request_id != id {
            return false;
        }
        if let Some(suggestion) = state.overlay.suggestion.as_mut() {
            suggestion.english.push_str(delta);
        }
        state.overlay.revision = state.overlay.revision.wrapping_add(1);
        drop(state);
        self.schedule_emit();
        true
    }

    fn apply_final_if_current(&self, epoch: u64, id: u64, english: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        if !self.is_epoch_current(epoch) || state.request_id != id {
            return false;
        }
        if let Some(suggestion) = state.overlay.suggestion.as_mut() {
            suggestion.english = english.to_owned();
            suggestion.is_final = true;
        }
        state.overlay.phase = CopilotPhase::Ready;
        state.overlay.revision = state.overlay.revision.wrapping_add(1);
        drop(state);
        self.schedule_emit();
        true
    }

    fn apply_chinese_if_current(
        &self,
        epoch: u64,
        id: u64,
        translated: Result<String, String>,
    ) -> bool {
        let mut state = self.state.lock().unwrap();
        if !self.is_epoch_current(epoch) || state.request_id != id {
            return false;
        }
        match translated {
            Ok(chinese) => {
                if let Some(suggestion) = state.overlay.suggestion.as_mut() {
                    suggestion.chinese = Some(chinese);
                }
            }
            Err(label) => state.overlay.error_label = Some(label),
        }
        state.overlay.revision = state.overlay.revision.wrapping_add(1);
        drop(state);
        self.schedule_emit();
        true
    }

    async fn fail_start(&self, epoch: u64, label: String) {
        let _transition = self.speaker_transition.lock().await;
        if !self.is_epoch_current(epoch) {
            return;
        }
        self.active.store(false, Ordering::Release);
        self.reset_conversation();
        {
            let mut state = self.state.lock().unwrap();
            state.overlay.active = false;
            state.overlay.phase = CopilotPhase::Error;
            state.overlay.error_label = Some(label);
            state.overlay.revision = state.overlay.revision.wrapping_add(1);
        }
        self.schedule_emit();
        self.speaker.set_active(false).await;
    }

    fn set_unavailable(&self, reason: CopilotUnavailableReason) {
        let mut state = self.state.lock().unwrap();
        if !state.overlay.available && state.overlay.unavailable_reason == Some(reason) {
            return;
        }
        state.overlay.available = false;
        state.overlay.unavailable_reason = Some(reason);
        state.overlay.revision = state.overlay.revision.wrapping_add(1);
        drop(state);
        self.schedule_emit();
    }

    pub fn refresh_availability(&self) {
        let config = self.store.snapshot();
        let reason = if !config.enabled {
            Some(CopilotUnavailableReason::Disabled)
        } else if config.model.is_none() || self.store.active_scenario().is_none() {
            Some(CopilotUnavailableReason::ModelMissing)
        } else if !self.settings.is_ui_test()
            && !self
                .settings
                .preferences()
                .audio_input
                .sources()
                .contains(&AudioSource::Microphone)
        {
            Some(CopilotUnavailableReason::MicrophoneRequired)
        } else if !self.settings.is_ui_test()
            && (!matches!(
                self.speaker.voiceprint_status().voiceprint,
                VoiceprintState::Ready
            ) || !matches!(
                self.speaker.voiceprint_status().model,
                SpeakerModelState::Ready
            ))
        {
            Some(CopilotUnavailableReason::VoiceprintMissing)
        } else {
            None
        };
        let mut state = self.state.lock().unwrap();
        if state.overlay.available == reason.is_none() && state.overlay.unavailable_reason == reason
        {
            return;
        }
        state.overlay.available = reason.is_none();
        state.overlay.unavailable_reason = reason;
        state.overlay.revision = state.overlay.revision.wrapping_add(1);
        drop(state);
        self.schedule_emit();
    }

    fn schedule_emit(&self) {
        if self.app.is_none() {
            return;
        }
        if self.emit_scheduled.swap(true, Ordering::AcqRel) {
            return;
        }
        let engine = self
            .self_ref
            .upgrade()
            .expect("copilot engine owned by app");
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            engine.emit_scheduled.store(false, Ordering::Release);
            if let Some(app) = &engine.app {
                let _ = app.emit(COPILOT_STATE_EVENT, engine.snapshot());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::network_proxy::{ProxyConfig, ProxyMode};
    use crate::settings_store::DEVELOPMENT_APPLICATION_IDENTIFIER;
    use crate::speaker::voiceprint::SpeakerBasis;
    use serde_json::Value;
    use std::sync::atomic::AtomicUsize;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::Notify;

    fn test_engine() -> Arc<CopilotEngine> {
        let directory = tempfile::tempdir().unwrap();
        let settings = Arc::new(SettingsStore::load(
            directory.path().to_path_buf(),
            true,
            DEVELOPMENT_APPLICATION_IDENTIFIER,
        ));
        let store = Arc::new(CopilotStore::load(directory.path().to_path_buf(), true));
        let speaker = SpeakerService::new(None);
        let (observation_tx, _observations) = mpsc::channel(OBSERVATION_CAPACITY);
        let (epoch_tx, _) = watch::channel(0);
        Arc::new_cyclic(|weak| CopilotEngine {
            self_ref: weak.clone(),
            app: None,
            settings,
            store,
            speaker,
            speaker_transition: tokio::sync::Mutex::new(()),
            active: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            epoch_tx,
            dropped: AtomicU64::new(0),
            request_attempts: AtomicU64::new(0),
            superseded_count: AtomicU64::new(0),
            emit_scheduled: AtomicBool::new(false),
            state: Mutex::new(EngineState {
                overlay: CopilotOverlayState::default(),
                conversation: CopilotConversation::new(),
                client: None,
                request_id: 0,
                initial_trigger: false,
            }),
            observation_tx,
            request_task: Mutex::new(None),
        })
    }

    fn scenario() -> CopilotScenario {
        CopilotScenario {
            id: "s".into(),
            name: "case".into(),
            prompt: "help".into(),
            background: "background".into(),
            strategy: "brief".into(),
        }
    }

    #[tokio::test]
    async fn oversized_scenario_reports_label_without_sending_warmup() {
        let (client, count, _, server) = mock_server(None).await;
        let engine = test_engine();
        let mut large = scenario();
        large.prompt = "x".repeat(copilot_prompt::MAX_PROMPT_BYTES);
        let (epoch, prefix) = engine.begin_activation(&large, &[]).await.unwrap();
        let result = engine
            .warm_and_finish(epoch, client, prefix, async { (true, None) })
            .await;
        assert_eq!(result.err().as_deref(), Some("copilot_scenario_too_large"));
        assert_eq!(
            engine.snapshot().error_label.as_deref(),
            Some("copilot_scenario_too_large")
        );
        assert_eq!(count.load(Ordering::SeqCst), 0);
        server.abort();
    }

    fn other(text: &str, created_at_ms: u64) -> ConfirmedLine {
        ConfirmedLine {
            source: AudioSource::Microphone,
            created_at_ms,
            text: text.into(),
            confirmed_at: Instant::now(),
        }
    }

    fn verdict() -> SpeakerVerdict {
        SpeakerVerdict {
            label: SpeakerLabel::Other,
            score: Some(0.1),
            voiced_ms: 1500,
            basis: SpeakerBasis::AudioRange,
        }
    }

    async fn read_body(socket: &mut tokio::net::TcpStream) -> Value {
        let mut bytes = Vec::new();
        loop {
            let mut buffer = [0; 4096];
            let read = socket.read(&mut buffer).await.unwrap();
            assert!(read > 0);
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..end]);
                let length = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap();
                if bytes.len() >= end + 4 + length {
                    return serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
                }
            }
        }
    }

    async fn mock_server(
        warm_gate: Option<Arc<Notify>>,
    ) -> (
        CopilotClient,
        Arc<AtomicUsize>,
        Arc<Mutex<Vec<Value>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let server_count = Arc::clone(&count);
        let server_bodies = Arc::clone(&bodies);
        let server = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let count = Arc::clone(&server_count);
                let bodies = Arc::clone(&server_bodies);
                let gate = warm_gate.clone();
                tokio::spawn(async move {
                    let body = read_body(&mut socket).await;
                    let warm = body["max_tokens"] == 1;
                    let old = body["messages"][1]["content"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("Other: A")
                        && !body["messages"][1]["content"]
                            .as_str()
                            .unwrap_or_default()
                            .contains("Other: B");
                    bodies.lock().unwrap().push(body);
                    count.fetch_add(1, Ordering::SeqCst);
                    if warm {
                        if let Some(gate) = gate {
                            gate.notified().await;
                        }
                    }
                    if old {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                    let answer = if warm {
                        "x"
                    } else if old {
                        "old reply"
                    } else {
                        "new reply"
                    };
                    let payload = format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{answer}\"}}}}]}}\r\n\r\ndata: [DONE]\r\n\r\n");
                    let header = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", payload.len());
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(payload.as_bytes()).await;
                });
            }
        });
        let network = ProviderNetwork::resolve(&ProxyConfig {
            mode: ProxyMode::Direct,
            url: None,
        })
        .unwrap();
        let client =
            CopilotClient::new(&format!("http://{address}/v1"), "", "test-model", network).unwrap();
        (client, count, bodies, server)
    }

    async fn wait_count(count: &AtomicUsize, expected: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while count.load(Ordering::SeqCst) < expected {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }

    async fn wait_english(engine: &CopilotEngine, expected: &str) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if engine
                    .snapshot()
                    .suggestion
                    .as_ref()
                    .is_some_and(|suggestion| suggestion.english == expected && suggestion.is_final)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn newer_other_cancels_old_stream_and_late_outputs_cannot_replace_it() {
        let (client, count, _, server) = mock_server(None).await;
        let engine = test_engine();
        let (epoch, _) = engine.begin_activation(&scenario(), &[]).await.unwrap();
        engine.finish_warm(epoch, client, None);
        engine.observe_confirmed(other("A", 1), verdict());
        let old_id = engine.snapshot().suggestion.unwrap().id;
        wait_count(&count, 1).await;
        engine.observe_confirmed(other("B", 2), verdict());
        let new_id = engine.snapshot().suggestion.unwrap().id;
        assert!(new_id > old_id);
        wait_english(&engine, "new reply").await;
        assert!(!engine.apply_delta_if_current(epoch, old_id, "late token"));
        assert!(!engine.apply_final_if_current(epoch, old_id, "late final"));
        assert!(!engine.apply_chinese_if_current(epoch, old_id, Ok("迟到中文".into())));
        tokio::time::sleep(Duration::from_millis(300)).await;
        let current = engine.snapshot().suggestion.unwrap();
        assert_eq!(current.id, new_id);
        assert_eq!(current.english, "new reply");
        assert_ne!(current.chinese.as_deref(), Some("迟到中文"));
        engine.deactivate().await;
        engine.observe_confirmed(other("C", 3), verdict());
        engine.config_changed().await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(count.load(Ordering::SeqCst), 2);
        assert_eq!(engine.request_attempts.load(Ordering::Relaxed), 2);
        assert_eq!(engine.superseded_count.load(Ordering::Relaxed), 1);
        server.abort();
    }

    #[tokio::test]
    async fn backfill_starts_once_after_warm_and_empty_history_only_warms() {
        let (client, count, bodies, server) = mock_server(None).await;
        let engine = test_engine();
        let history = (0..6)
            .map(|index| format!("prior {index}"))
            .collect::<Vec<_>>();
        let (epoch, prefix) = engine
            .begin_activation(&scenario(), &history)
            .await
            .unwrap();
        engine
            .warm_and_finish(epoch, client, prefix, async { (true, None) })
            .await
            .unwrap();
        wait_count(&count, 2).await;
        wait_english(&engine, "new reply").await;
        assert_eq!(engine.request_attempts.load(Ordering::Relaxed), 2);
        {
            let requests = bodies.lock().unwrap();
            assert_eq!(
                requests
                    .iter()
                    .filter(|body| body["max_tokens"] == 1)
                    .count(),
                1
            );
            assert_eq!(
                requests
                    .iter()
                    .filter(|body| body["max_tokens"] == 96)
                    .count(),
                1
            );
            let suggestion = requests
                .iter()
                .find(|body| body["max_tokens"] == 96)
                .unwrap();
            let transcript = suggestion["messages"][1]["content"].as_str().unwrap();
            assert!(transcript.starts_with("开启提词前的对话，未区分说话人\n"));
            assert_eq!(
                transcript
                    .lines()
                    .filter(|line| line.starts_with("prior "))
                    .count(),
                6
            );
            assert!(!transcript.contains("Other:"));
        }
        server.abort();

        let (client, count, _, server) = mock_server(None).await;
        let engine = test_engine();
        let (epoch, prefix) = engine.begin_activation(&scenario(), &[]).await.unwrap();
        engine
            .warm_and_finish(epoch, client, prefix, async { (true, None) })
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(engine.request_attempts.load(Ordering::Relaxed), 1);
        assert!(engine.snapshot().suggestion.is_none());
        server.abort();
    }

    #[tokio::test]
    async fn closing_during_warm_cancels_initial_and_new_other_joins_single_initial_request() {
        let gate = Arc::new(Notify::new());
        let (client, count, _, server) = mock_server(Some(Arc::clone(&gate))).await;
        let engine = test_engine();
        let (epoch, prefix) = engine
            .begin_activation(&scenario(), &["prior".into()])
            .await
            .unwrap();
        let warming = tokio::spawn({
            let engine = Arc::clone(&engine);
            async move {
                engine
                    .warm_and_finish(epoch, client, prefix, async { (true, None) })
                    .await
            }
        });
        wait_count(&count, 1).await;
        engine.deactivate().await;
        gate.notify_one();
        assert!(!warming.await.unwrap().unwrap().active);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(engine.request_attempts.load(Ordering::Relaxed), 1);
        assert_eq!(engine.superseded_count.load(Ordering::Relaxed), 1);
        server.abort();

        let gate = Arc::new(Notify::new());
        let (client, count, bodies, server) = mock_server(Some(Arc::clone(&gate))).await;
        let engine = test_engine();
        let (epoch, prefix) = engine
            .begin_activation(&scenario(), &["prior".into()])
            .await
            .unwrap();
        let warming = tokio::spawn({
            let engine = Arc::clone(&engine);
            async move {
                engine
                    .warm_and_finish(epoch, client, prefix, async { (true, None) })
                    .await
            }
        });
        wait_count(&count, 1).await;
        engine.observe_confirmed(other("new caller", 2), verdict());
        assert_eq!(count.load(Ordering::SeqCst), 1);
        gate.notify_one();
        warming.await.unwrap().unwrap();
        wait_count(&count, 2).await;
        wait_english(&engine, "new reply").await;
        assert_eq!(engine.request_attempts.load(Ordering::Relaxed), 2);
        let requests = bodies.lock().unwrap();
        let suggestion = requests
            .iter()
            .find(|body| body["max_tokens"] == 96)
            .unwrap();
        assert!(suggestion["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("prior\nOther: new caller\n"));
        assert_eq!(
            requests
                .iter()
                .filter(|body| body["max_tokens"] == 96)
                .count(),
            1
        );
        server.abort();
    }

    #[test]
    fn snapshots_and_unchanged_availability_do_not_publish_new_revisions() {
        let engine = test_engine();
        engine.refresh_availability();
        let revision = engine.snapshot().revision;
        assert_eq!(engine.snapshot().revision, revision);
        engine.refresh_availability();
        assert_eq!(engine.snapshot().revision, revision);
    }
}
