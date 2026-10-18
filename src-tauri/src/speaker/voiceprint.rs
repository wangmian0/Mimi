use super::embedding::{self, SpeakerClassifier};
use super::recent_pcm::RecentPcm;
use futures_util::StreamExt;
use mimi_runtime::clients::translation_client::SourceAudioRange;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const QUIET_OTHER_RMS: f32 = 0.01; // -40 dBFS
const SILENT_RMS: f32 = 0.0001; // -80 dBFS: digital silence, not a quiet voice

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

#[derive(serde::Serialize, serde::Deserialize)]
struct SavedVoiceprint {
    model: String,
    duration_ms: u32,
    updated_at_ms: u64,
    vector: Vec<f32>,
}

pub struct SpeakerService {
    pub(super) directory: Option<PathBuf>,
    active: Arc<AtomicBool>,
    pub(super) recording_active: AtomicBool,
    downloading: AtomicBool,
    model_failed: AtomicBool,
    epoch: Arc<AtomicU64>,
    in_flight: Arc<tokio::sync::RwLock<()>>,
    pub(super) enrollment_epoch: Arc<AtomicU64>,
    pub(super) enrollment_commit: Arc<Mutex<()>>,
    pub(super) enrollment_change: tokio::sync::watch::Sender<u64>,
    generation: AtomicU64,
    rate: AtomicU32,
    next_sample: AtomicU64,
    pcm: Mutex<RecentPcm>,
    classifier: Mutex<Option<SpeakerClassifier>>,
    #[cfg(target_os = "macos")]
    model: Mutex<Option<Arc<embedding::SpeakerEmbedding>>>,
    pub(super) recording: tokio::sync::Mutex<Option<super::recording::VoiceRecording>>,
}

impl SpeakerService {
    pub fn new(data_dir: Option<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            directory: data_dir.map(|path| path.join("speaker")),
            active: Arc::new(AtomicBool::new(false)),
            recording_active: AtomicBool::new(false),
            downloading: AtomicBool::new(false),
            model_failed: AtomicBool::new(false),
            epoch: Arc::new(AtomicU64::new(0)),
            in_flight: Arc::new(tokio::sync::RwLock::new(())),
            enrollment_epoch: Arc::new(AtomicU64::new(0)),
            enrollment_commit: Arc::new(Mutex::new(())),
            enrollment_change: tokio::sync::watch::channel(0).0,
            generation: AtomicU64::new(u64::MAX),
            rate: AtomicU32::new(16_000),
            next_sample: AtomicU64::new(0),
            pcm: Mutex::new(RecentPcm::new()),
            classifier: Mutex::new(None),
            #[cfg(target_os = "macos")]
            model: Mutex::new(None),
            recording: tokio::sync::Mutex::new(None),
        })
    }
    pub async fn set_active(&self, active: bool) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        self.active.store(active, Ordering::SeqCst);
        if let Ok(mut pcm) = self.pcm.lock() {
            pcm.clear();
        }
        // Invalidate first, then drain workers before resetting learned state.
        let _released = if active {
            None
        } else {
            Some(self.in_flight.write().await)
        };
        if let Ok(mut classifier) = self.classifier.lock() {
            if let Some(classifier) = classifier.as_mut() {
                classifier.reset();
            }
        }
    }
    pub fn is_recording(&self) -> bool {
        self.recording_active.load(Ordering::SeqCst)
    }
    pub async fn prewarm(&self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let directory = self
                .directory
                .clone()
                .ok_or("speaker_storage_unavailable")?;
            let model_path = directory.join("models").join(embedding::MODEL_NAME);
            let voiceprint = read_voiceprint(&directory)?;
            if voiceprint.is_none() {
                return Err("speaker_voiceprint_missing".into());
            }
            if !verified_file(&model_path, embedding::MODEL_SHA256)
                .map_err(|_| "speaker_model_unavailable")?
            {
                return Err("speaker_model_unavailable".into());
            }
            let model = tauri::async_runtime::spawn_blocking(move || {
                embedding::SpeakerEmbedding::load(&model_path)
            })
            .await
            .map_err(|_| "speaker_model_unavailable")??;
            *self.model.lock().unwrap() = Some(Arc::new(model));
            *self.classifier.lock().unwrap() = voiceprint.map(|v| SpeakerClassifier::new(v.vector));
            Ok(())
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("speaker_platform_unavailable".into())
        }
    }
    pub fn begin_capture(&self, generation: u64, sample_rate_hz: u32) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        self.generation.store(generation, Ordering::SeqCst);
        self.rate.store(sample_rate_hz, Ordering::SeqCst);
        self.next_sample.store(0, Ordering::SeqCst);
        if let Ok(mut pcm) = self.pcm.lock() {
            pcm.begin(generation, sample_rate_hz);
        }
        if let Ok(mut classifier) = self.classifier.lock() {
            if let Some(classifier) = classifier.as_mut() {
                classifier.reset();
            }
        }
    }
    pub fn observe_pcm(&self, generation: u64, pcm16le: &[u8]) {
        if generation != self.generation.load(Ordering::SeqCst) || !pcm16le.len().is_multiple_of(2)
        {
            return;
        }
        let start = self
            .next_sample
            .fetch_add((pcm16le.len() / 2) as u64, Ordering::SeqCst);
        // Audio delivery must never wait for model work or classification.
        if let Ok(mut pcm) = self.pcm.try_lock() {
            pcm.push(
                generation,
                start,
                pcm16le,
                self.active.load(Ordering::SeqCst),
                Instant::now(),
            );
        }
    }
    pub async fn classify(
        &self,
        generation: u64,
        range: Option<SourceAudioRange>,
        confirmed_at: Instant,
    ) -> SpeakerVerdict {
        let unknown = || SpeakerVerdict {
            label: SpeakerLabel::Unknown,
            score: None,
            voiced_ms: 0,
            basis: SpeakerBasis::Unavailable,
        };
        let in_flight = Arc::clone(&self.in_flight).read_owned().await;
        if !self.active.load(Ordering::SeqCst)
            || generation != self.generation.load(Ordering::SeqCst)
        {
            tracing::info!(reason = "inactive_or_stale", "speaker_unknown");
            return unknown();
        }
        let epoch = self.epoch.load(Ordering::SeqCst);
        #[cfg(target_os = "macos")]
        {
            let Some(model) = self.model.lock().unwrap().clone() else {
                tracing::info!(reason = "model_missing", "speaker_unknown");
                return unknown();
            };
            let (pcm, rate, basis, padded) = {
                let Ok(recent) = self.pcm.lock() else {
                    return unknown();
                };
                let (ring_start, ring_end) = recent.bounds();
                tracing::info!(
                    range_start_ms = range.map(|r| r.start_ms as i64).unwrap_or(-1),
                    range_end_ms = range.map(|r| r.end_ms as i64).unwrap_or(-1),
                    ring_start,
                    ring_end,
                    rate = recent.rate(),
                    "speaker_lookup"
                );
                if let Some((exact, padded)) = range.and_then(|range| {
                    Some((
                        recent.exact(generation, range)?,
                        recent.padded(generation, range),
                    ))
                }) {
                    (exact, recent.rate(), SpeakerBasis::AudioRange, padded)
                } else if let Some(fallback) = recent.recent_voice(generation, confirmed_at) {
                    (fallback, recent.rate(), SpeakerBasis::RecentVoice, false)
                } else {
                    tracing::info!(reason = "no_audio", "speaker_unknown");
                    return unknown();
                }
            };
            let peak_rms = pcm
                .chunks((rate / 100).max(1) as usize)
                .map(|chunk| {
                    (chunk
                        .iter()
                        .map(|s| (*s as f32 / 32768.0).powi(2))
                        .sum::<f32>()
                        / chunk.len().max(1) as f32)
                        .sqrt()
                })
                .fold(0.0f32, f32::max);
            let (voiced, voiced_ms) = embedding::voiced_pcm(&pcm, rate);
            tracing::info!(
                basis = ?basis,
                samples = pcm.len(),
                voiced_ms,
                peak_rms_dbfs = (20.0 * peak_rms.max(1e-6).log10()) as i32,
                "speaker_audio"
            );
            drop(pcm);
            if voiced_ms < 1_000 {
                return SpeakerVerdict {
                    label: quiet_speech_label(peak_rms, padded),
                    voiced_ms,
                    basis,
                    ..unknown()
                };
            }
            let cancellation = Arc::clone(&self.epoch);
            let active = Arc::clone(&self.active);
            let windows = match tauri::async_runtime::spawn_blocking(move || {
                let _in_flight = in_flight;
                let windows = model.embed_windows(&voiced, rate, || {
                    !active.load(Ordering::SeqCst) || cancellation.load(Ordering::SeqCst) != epoch
                });
                drop(voiced);
                windows
            })
            .await
            {
                Ok(windows) => windows,
                Err(_) => return unknown(),
            };
            if !self.active.load(Ordering::SeqCst)
                || generation != self.generation.load(Ordering::SeqCst)
                || epoch != self.epoch.load(Ordering::SeqCst)
            {
                return unknown();
            }
            let Some(classifier) = self
                .classifier
                .lock()
                .unwrap()
                .as_mut()
                .map(|c| c.classify(&windows, voiced_ms))
            else {
                return unknown();
            };
            tracing::info!(
                windows = windows.len(),
                me_votes = classifier.2[0],
                other_votes = classifier.2[1],
                unknown_votes = classifier.2[2],
                score_bucket = classifier.1.map(|v| (v * 10.0).floor() as i32),
                "speaker_classification"
            );
            SpeakerVerdict {
                label: classifier.0,
                score: classifier.1,
                voiced_ms,
                basis,
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            unknown()
        }
    }
    pub fn voiceprint_status(&self) -> VoiceprintStatus {
        let loaded = self.directory.as_ref().map(|d| read_voiceprint(d));
        let invalid = loaded.as_ref().is_some_and(|r| r.is_err());
        let saved = loaded.and_then(Result::ok).flatten();
        let model = self
            .directory
            .as_ref()
            .map(|d| d.join("models").join(embedding::MODEL_NAME));
        VoiceprintStatus {
            voiceprint: if self.is_recording() {
                VoiceprintState::Recording
            } else if invalid {
                VoiceprintState::Invalid
            } else if saved.is_some() {
                VoiceprintState::Ready
            } else {
                VoiceprintState::Missing
            },
            duration_ms: saved.as_ref().map(|v| v.duration_ms),
            updated_at_ms: saved.as_ref().map(|v| v.updated_at_ms),
            model: if self.downloading.load(Ordering::SeqCst) {
                SpeakerModelState::Downloading
            } else if self.model_failed.load(Ordering::SeqCst) {
                SpeakerModelState::Failed
            } else if model.as_ref().is_some_and(|p| p.exists()) {
                SpeakerModelState::Ready
            } else {
                SpeakerModelState::Missing
            },
        }
    }

    pub async fn import(&self, path: PathBuf) -> Result<VoiceprintStatus, String> {
        let token = self.enrollment_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        self.enrollment_change.send_replace(token);
        self.import_with_token(path, token).await
    }

    async fn until_enrollment_change<T>(
        &self,
        token: u64,
        future: impl std::future::Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        let mut change = self.enrollment_change.subscribe();
        if *change.borrow() != token {
            return Err("speaker_record_cancelled".into());
        }
        tokio::select! {
            result = future => result,
            _ = change.changed() => Err("speaker_record_cancelled".into()),
        }
    }

    pub(super) async fn import_with_token(
        &self,
        path: PathBuf,
        token: u64,
    ) -> Result<VoiceprintStatus, String> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = path;
            return Err("speaker_platform_unavailable".into());
        }
        #[cfg(target_os = "macos")]
        {
            let directory = self
                .directory
                .clone()
                .ok_or("speaker_storage_unavailable")?;
            let wav = self
                .until_enrollment_change(token, async move {
                    tauri::async_runtime::spawn_blocking(move || parse_wav(&path))
                        .await
                        .map_err(|_| "speaker_wav_invalid".to_owned())?
                })
                .await?;
            if self.enrollment_epoch.load(Ordering::SeqCst) != token {
                return Err("speaker_record_cancelled".into());
            }
            self.until_enrollment_change(token, self.prepare_model())
                .await?;
            if self.enrollment_epoch.load(Ordering::SeqCst) != token {
                return Err("speaker_record_cancelled".into());
            }
            let model = self
                .model
                .lock()
                .unwrap()
                .clone()
                .ok_or("speaker_model_unavailable")?;
            let saved = self
                .until_enrollment_change(token, async move {
                    tauri::async_runtime::spawn_blocking(move || enroll(model.as_ref(), wav))
                        .await
                        .map_err(|_| "speaker_enrollment_failed".to_owned())?
                })
                .await?;
            if self.enrollment_epoch.load(Ordering::SeqCst) != token {
                return Err("speaker_record_cancelled".into());
            }
            let write_dir = directory.clone();
            let epoch = Arc::clone(&self.enrollment_epoch);
            let commit = Arc::clone(&self.enrollment_commit);
            tauri::async_runtime::spawn_blocking(move || {
                commit_voiceprint(&write_dir, &saved, token, &epoch, &commit)
            })
            .await
            .map_err(|_| "speaker_storage_failed")??;
            // The model is already loaded; the durable write is the commit.
            if let Some(saved) = read_voiceprint(&directory)? {
                *self.classifier.lock().unwrap() = Some(SpeakerClassifier::new(saved.vector));
            }
            Ok(self.voiceprint_status())
        }
    }

    #[cfg(target_os = "macos")]
    async fn prepare_model(&self) -> Result<(), String> {
        let directory = self
            .directory
            .as_ref()
            .ok_or("speaker_storage_unavailable")?;
        private_dir(directory)?;
        let model_dir = directory.join("models");
        let path = model_dir.join(embedding::MODEL_NAME);
        if verified_file(&path, embedding::MODEL_SHA256).map_err(|_| "speaker_model_unavailable")? {
            self.model_failed.store(false, Ordering::SeqCst);
            let model_path = path.clone();
            let model = tauri::async_runtime::spawn_blocking(move || {
                embedding::SpeakerEmbedding::load(&model_path)
            })
            .await
            .map_err(|_| "speaker_model_unavailable")??;
            *self.model.lock().unwrap() = Some(Arc::new(model));
            return Ok(());
        }
        self.downloading.store(true, Ordering::SeqCst);
        struct Reset<'a>(&'a AtomicBool);
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let _reset = Reset(&self.downloading);
        let result = self.download_model(&model_dir, &path).await;
        self.model_failed.store(result.is_err(), Ordering::SeqCst);
        result?;
        let model_path = directory.join("models").join(embedding::MODEL_NAME);
        let model = tauri::async_runtime::spawn_blocking(move || {
            embedding::SpeakerEmbedding::load(&model_path)
        })
        .await
        .map_err(|_| "speaker_model_unavailable")??;
        *self.model.lock().unwrap() = Some(Arc::new(model));
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn download_model(
        &self,
        model_dir: &std::path::Path,
        path: &std::path::Path,
    ) -> Result<(), String> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let response = reqwest::get(embedding::MODEL_URL)
            .await
            .map_err(|_| "speaker_model_download_failed")?;
        if !response.status().is_success() {
            return Err("speaker_model_download_failed".into());
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "speaker_model_download_failed")?;
            if bytes.len().saturating_add(chunk.len()) > 50_000_000 {
                return Err("speaker_model_download_failed".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        if format!("{:x}", Sha256::digest(&bytes)) != embedding::MODEL_SHA256 {
            return Err("speaker_model_checksum_failed".into());
        }
        let model_dir = model_dir.to_path_buf();
        let path = path.to_path_buf();
        tauri::async_runtime::spawn_blocking(move || atomic_write(&model_dir, &path, &bytes))
            .await
            .map_err(|_| "speaker_storage_failed")??;
        Ok(())
    }
}

fn verified_file(path: &std::path::Path, hash: &str) -> std::io::Result<bool> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    if file.metadata()?.len() > 50_000_000 {
        return Ok(false);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 16_384];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()) == hash)
}

pub(super) fn private_dir(path: &std::path::Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|_| "speaker_storage_failed")?;
    let metadata = fs::symlink_metadata(path).map_err(|_| "speaker_storage_failed")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("speaker_storage_failed".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        unsafe extern "C" {
            fn getuid() -> u32;
        }
        if metadata.uid() != unsafe { getuid() } {
            return Err("speaker_storage_failed".into());
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| "speaker_storage_failed")?;
    }
    #[cfg(windows)]
    private_windows_acl(path, true).map_err(|_| "speaker_storage_failed")?;
    Ok(())
}

fn atomic_write(
    directory: &std::path::Path,
    path: &std::path::Path,
    bytes: &[u8],
) -> Result<(), String> {
    private_dir(directory)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(directory).map_err(|_| "speaker_storage_failed")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| "speaker_storage_failed")?;
    }
    #[cfg(windows)]
    private_windows_acl(temporary.path(), false).map_err(|_| "speaker_storage_failed")?;
    temporary
        .write_all(bytes)
        .map_err(|_| "speaker_storage_failed")?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| "speaker_storage_failed")?;
    temporary
        .persist(path)
        .map_err(|_| "speaker_storage_failed")?;
    fs::File::open(directory)
        .and_then(|dir| dir.sync_all())
        .map_err(|_| "speaker_storage_failed")?;
    Ok(())
}

#[cfg(windows)]
pub(super) fn private_windows_acl(path: &std::path::Path, directory: bool) -> std::io::Result<()> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt};
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text: *const u16,
            revision: u32,
            descriptor: *mut *mut c_void,
            size: *mut u32,
        ) -> i32;
        fn SetFileSecurityW(path: *const u16, information: u32, descriptor: *mut c_void) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }
    let text = if directory {
        "D:P(A;OICI;FA;;;OW)"
    } else {
        "D:P(A;;FA;;;OW)"
    };
    let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut descriptor = std::ptr::null_mut();
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let result = SetFileSecurityW(path.as_ptr(), 0x80000004, descriptor);
        let error = if result == 0 {
            Some(std::io::Error::last_os_error())
        } else {
            None
        };
        LocalFree(descriptor);
        error.map_or(Ok(()), Err)
    }
}

fn read_voiceprint(directory: &std::path::Path) -> Result<Option<SavedVoiceprint>, String> {
    let path = directory.join("me.embedding.json");
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("speaker_storage_failed".into()),
    };
    if file.metadata().map_err(|_| "speaker_storage_failed")?.len() > 32_768 {
        return Err("speaker_voiceprint_invalid".into());
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| "speaker_storage_failed")?;
    let saved: SavedVoiceprint =
        serde_json::from_slice(&bytes).map_err(|_| "speaker_voiceprint_invalid")?;
    if saved.model != embedding::MODEL_NAME
        || saved.vector.len() != 192
        || saved.vector.iter().any(|v| !v.is_finite())
        || !(0.99..=1.01).contains(&saved.vector.iter().map(|v| v * v).sum::<f32>().sqrt())
    {
        return Err("speaker_voiceprint_invalid".into());
    }
    Ok(Some(saved))
}

fn save_voiceprint(directory: &std::path::Path, saved: &SavedVoiceprint) -> Result<(), String> {
    let bytes = serde_json::to_vec(saved).map_err(|_| "speaker_storage_failed")?;
    atomic_write(directory, &directory.join("me.embedding.json"), &bytes)
}

fn commit_voiceprint(
    directory: &std::path::Path,
    saved: &SavedVoiceprint,
    token: u64,
    epoch: &AtomicU64,
    lock: &Mutex<()>,
) -> Result<(), String> {
    let _guard = lock.lock().unwrap();
    if epoch.load(Ordering::SeqCst) != token {
        return Err("speaker_record_cancelled".into());
    }
    save_voiceprint(directory, saved)
}

pub fn parse_wav(path: &std::path::Path) -> Result<Vec<i16>, String> {
    let file = fs::File::open(path).map_err(|_| "speaker_wav_invalid")?;
    if file.metadata().map_err(|_| "speaker_wav_invalid")?.len() > 8_000_000 {
        return Err("speaker_wav_invalid".into());
    }
    let mut bytes = Vec::new();
    file.take(8_000_001)
        .read_to_end(&mut bytes)
        .map_err(|_| "speaker_wav_invalid")?;
    parse_wav_bytes(&bytes)
}

fn parse_wav_bytes(bytes: &[u8]) -> Result<Vec<i16>, String> {
    if bytes.len() < 12
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8 != bytes.len()
    {
        return Err("speaker_wav_invalid".into());
    }
    let mut offset = 12;
    let mut format = false;
    let mut seen_format = false;
    let mut data = None;
    while offset + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(8)
            .and_then(|v| v.checked_add(size))
            .ok_or("speaker_wav_invalid")?;
        if end > bytes.len() {
            return Err("speaker_wav_invalid".into());
        }
        match &bytes[offset..offset + 4] {
            b"fmt " => {
                if seen_format {
                    return Err("speaker_wav_invalid".into());
                }
                seen_format = true;
                let chunk = &bytes[offset + 8..end];
                if chunk.len() < 16 {
                    return Err("speaker_wav_invalid".into());
                }
                format = u16::from_le_bytes(chunk[0..2].try_into().unwrap()) == 1
                    && u16::from_le_bytes(chunk[2..4].try_into().unwrap()) == 1
                    && u32::from_le_bytes(chunk[4..8].try_into().unwrap()) == 16_000
                    && u32::from_le_bytes(chunk[8..12].try_into().unwrap()) == 32_000
                    && u16::from_le_bytes(chunk[12..14].try_into().unwrap()) == 2
                    && u16::from_le_bytes(chunk[14..16].try_into().unwrap()) == 16;
            }
            b"data" => {
                if data.is_some() {
                    return Err("speaker_wav_invalid".into());
                }
                data = Some(&bytes[offset + 8..end]);
            }
            _ => {}
        }
        offset = end + (size & 1);
    }
    if offset != bytes.len() || !format {
        return Err("speaker_wav_invalid".into());
    }
    let data = data.ok_or("speaker_wav_invalid")?;
    if !data.len().is_multiple_of(2) || !(320_000..=3_840_000).contains(&data.len()) {
        return Err("speaker_wav_duration_invalid".into());
    }
    Ok(data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect())
}

#[cfg(target_os = "macos")]
fn enroll(model: &embedding::SpeakerEmbedding, wav: Vec<i16>) -> Result<SavedVoiceprint, String> {
    let (voiced, duration_ms) = embedding::voiced_pcm(&wav, 16_000);
    if duration_ms < 10_000 {
        return Err("speaker_wav_voiced_short".into());
    }
    let mut vectors = Vec::new();
    for chunk in voiced.chunks(64_000) {
        if chunk.len() >= 16_000 {
            vectors.push(
                model
                    .embed(chunk, 16_000)
                    .ok_or("speaker_enrollment_failed")?,
            );
        }
    }
    let vector = embedding::centroid(&vectors).ok_or("speaker_enrollment_failed")?;
    let updated_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "speaker_storage_failed")?
        .as_millis() as u64;
    Ok(SavedVoiceprint {
        model: embedding::MODEL_NAME.into(),
        duration_ms,
        updated_at_ms,
        vector,
    })
}

/// Mic-only calls: the owner talks close to the Mac, so recognised speech that
/// never rises above the voice gate came through the phone speaker. Short but
/// loud fragments (the owner's "Okay") stay undecided, as do ranges with
/// padded or digitally silent audio, which carry no evidence of who spoke.
fn quiet_speech_label(peak_rms: f32, padded: bool) -> SpeakerLabel {
    if !padded && (SILENT_RMS..QUIET_OTHER_RMS).contains(&peak_rms) {
        SpeakerLabel::Other
    } else {
        SpeakerLabel::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_recognised_speech_is_the_other_party() {
        assert_eq!(quiet_speech_label(0.003, false), SpeakerLabel::Other);
        assert_eq!(quiet_speech_label(0.04, false), SpeakerLabel::Unknown);
    }

    #[test]
    fn padded_or_silent_audio_is_not_the_other_party() {
        assert_eq!(quiet_speech_label(0.003, true), SpeakerLabel::Unknown);
        assert_eq!(quiet_speech_label(0.0, false), SpeakerLabel::Unknown);
        assert_eq!(quiet_speech_label(0.0, true), SpeakerLabel::Unknown);
    }

    #[tokio::test]
    async fn closing_waits_until_in_flight_pcm_is_released() {
        let service = SpeakerService::new(None);
        service.set_active(true).await;
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let released = Arc::new(AtomicBool::new(false));
        let worker_service = Arc::clone(&service);
        let worker_released = Arc::clone(&released);
        let worker = tokio::spawn(async move {
            let guard = Arc::clone(&worker_service.in_flight).read_owned().await;
            let _ = tokio::task::spawn_blocking(move || {
                let _guard = guard;
                let pcm = vec![0i16; 16_000];
                let _ = started_tx.send(());
                let _ = release_rx.recv();
                drop(pcm);
                worker_released.store(true, Ordering::SeqCst);
            })
            .await;
        });
        started_rx.await.unwrap();
        worker.abort();
        let _ = worker.await;
        let close_service = Arc::clone(&service);
        let mut close = tokio::spawn(async move { close_service.set_active(false).await });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut close)
                .await
                .is_err()
        );
        release_tx.send(()).unwrap();
        (&mut close).await.unwrap();
        assert!(released.load(Ordering::SeqCst));
    }

    #[test]
    fn wav_chunks_and_rejection() {
        let mut wav = crate::session_history::wav_header(16_000, 320_000).to_vec();
        wav.splice(36..36, [b'L', b'I', b'S', b'T', 3, 0, 0, 0, 1, 2, 3, 0]);
        wav.extend(std::iter::repeat_n(0, 320_000));
        let length = wav.len() as u32 - 8;
        wav[4..8].copy_from_slice(&length.to_le_bytes());
        assert_eq!(parse_wav_bytes(&wav).unwrap().len(), 160_000);
        let mut stereo = wav.clone();
        stereo[22..24].copy_from_slice(&2u16.to_le_bytes());
        assert!(parse_wav_bytes(&stereo).is_err());
        let mut short = wav.clone();
        short.truncate(44 + 12 + 100);
        short[52..56].copy_from_slice(&100u32.to_le_bytes());
        let short_size = short.len() as u32 - 8;
        short[4..8].copy_from_slice(&short_size.to_le_bytes());
        assert!(parse_wav_bytes(&short).is_err());
        wav[24] = 0;
        assert!(parse_wav_bytes(&wav).is_err());
    }
    #[test]
    fn private_file_permissions() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("speaker").join("me.embedding.json");
        atomic_write(path.parent().unwrap(), &path, b"{}").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn cancelled_enrollment_cannot_commit_a_late_embedding() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("speaker");
        let saved = SavedVoiceprint {
            model: embedding::MODEL_NAME.into(),
            duration_ms: 20_000,
            updated_at_ms: 1,
            vector: vec![1.0; 192],
        };
        let epoch = AtomicU64::new(2);
        let lock = Mutex::new(());
        assert_eq!(
            commit_voiceprint(&directory, &saved, 1, &epoch, &lock),
            Err("speaker_record_cancelled".into())
        );
        assert!(!directory.join("me.embedding.json").exists());
    }
}
