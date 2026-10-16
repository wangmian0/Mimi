use super::voiceprint::{SpeakerService, VoiceprintStatus};
use crate::audio::send_pipeline::AudioSendPipeline;
use crate::audio::{
    AudioCapture, AudioCaptureFormat, CaptureFailureSender, SystemAudioCaptureFailure,
};
use crate::core::audio_input::AudioSource;
use crate::core::system_audio_target::SystemAudioTarget;
use std::io::{Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub struct VoiceRecording {
    app: AppHandle,
    started: Instant,
    token: u64,
    capture: AudioCapture,
    pipeline: AudioSendPipeline,
    file: Arc<Mutex<tempfile::NamedTempFile>>,
    failure_rx: tokio::sync::mpsc::Receiver<SystemAudioCaptureFailure>,
    timer: tokio::task::JoinHandle<()>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordingEvent {
    state: &'static str,
    elapsed_ms: u32,
    peak_dbfs: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl VoiceRecording {
    fn emit(&self, state: &'static str, error: Option<&str>) {
        let _ = self.app.emit(
            "speaker://voiceprint-recording",
            RecordingEvent {
                state,
                elapsed_ms: self.started.elapsed().as_millis() as u32,
                peak_dbfs: -96,
                error: error.map(str::to_owned),
            },
        );
    }
}

impl SpeakerService {
    pub async fn record_start(
        self: &Arc<Self>,
        app: AppHandle,
        max_seconds: Option<u32>,
    ) -> Result<(), String> {
        if !cfg!(target_os = "macos") {
            return Err("speaker_platform_unavailable".into());
        }
        let max_seconds = max_seconds.unwrap_or(60);
        if !(10..=60).contains(&max_seconds) {
            return Err("speaker_record_duration_invalid".into());
        }
        let directory = self
            .directory
            .as_ref()
            .ok_or("speaker_storage_unavailable")?
            .clone();
        super::voiceprint::private_dir(&directory)?;
        let mut slot = self.recording.lock().await;
        if slot.is_some() || self.is_recording() {
            return Err("speaker_recording_active".into());
        }
        let token = self
            .enrollment_epoch
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        self.enrollment_change.send_replace(token);
        let mut temporary =
            tempfile::NamedTempFile::new_in(&directory).map_err(|_| "speaker_storage_failed")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temporary
                .as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|_| "speaker_storage_failed")?;
        }
        #[cfg(windows)]
        super::voiceprint::private_windows_acl(temporary.path(), false)
            .map_err(|_| "speaker_storage_failed")?;
        temporary
            .write_all(&[0; 44])
            .map_err(|_| "speaker_storage_failed")?;
        let file = Arc::new(Mutex::new(temporary));
        let written = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let started = Instant::now();
        let last_level = Arc::new(Mutex::new(Instant::now()));
        let write_file = Arc::clone(&file);
        let write_count = Arc::clone(&written);
        let event_app = app.clone();
        let pipeline = AudioSendPipeline::spawn(
            move |pcm| {
                let file = Arc::clone(&write_file);
                let count = Arc::clone(&write_count);
                let app = event_app.clone();
                let last = Arc::clone(&last_level);
                async move {
                    let limit = max_seconds as usize * 16_000 * 2;
                    let previous = count.load(std::sync::atomic::Ordering::SeqCst);
                    let available = limit.saturating_sub(previous).min(pcm.len()) & !1;
                    if available > 0 {
                        file.lock()
                            .unwrap()
                            .write_all(&pcm[..available])
                            .map_err(|_| ())?;
                        count.fetch_add(available, std::sync::atomic::Ordering::SeqCst);
                    }
                    let mut last = last.lock().unwrap();
                    if last.elapsed() >= Duration::from_millis(100) {
                        *last = Instant::now();
                        let peak = pcm
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|s| i16::from_le_bytes([s[0], s[1]]).unsigned_abs() as u32)
                            .max()
                            .unwrap_or(0);
                        let db = if peak == 0 {
                            -96
                        } else {
                            (20.0 * (peak as f32 / 32768.0).log10()).round() as i32
                        };
                        let _ = app.emit(
                            "speaker://voiceprint-recording",
                            RecordingEvent {
                                state: "recording",
                                elapsed_ms: started.elapsed().as_millis() as u32,
                                peak_dbfs: db,
                                error: None,
                            },
                        );
                    }
                    Ok::<(), ()>(())
                }
            },
            |_| {},
        );
        let capture = AudioCapture::for_app(&app);
        let ingress = pipeline.ingress().ok_or("speaker_record_start_failed")?;
        let (failure, failure_rx) = CaptureFailureSender::channel();
        capture
            .start(
                ingress,
                failure,
                AudioCaptureFormat::pcm16_mono(16_000)
                    .map_err(|_| "speaker_record_start_failed")?,
                AudioSource::Microphone,
                SystemAudioTarget::default(),
            )
            .await
            .map_err(|_| "speaker_record_start_failed")?;
        let service = Arc::clone(self);
        let timer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(max_seconds as u64)).await;
            let _ = service.record_stop_for(Some(token)).await;
        });
        *slot = Some(VoiceRecording {
            app,
            started,
            token,
            capture,
            pipeline,
            file,
            failure_rx,
            timer,
        });
        self.recording_active
            .store(true, std::sync::atomic::Ordering::SeqCst);
        slot.as_ref().unwrap().emit("recording", None);
        Ok(())
    }

    pub async fn record_stop(&self) -> Result<VoiceprintStatus, String> {
        self.record_stop_for(None).await
    }

    async fn record_stop_for(&self, token: Option<u64>) -> Result<VoiceprintStatus, String> {
        let mut slot = self.recording.lock().await;
        let mut recording = take_for_token(&mut slot, token, |recording| recording.token)
            .ok_or("speaker_recording_missing")?;
        if token.is_none() {
            recording.timer.abort();
        }
        drop(slot);
        let result = async {
            recording
                .capture
                .stop_and_wait()
                .await
                .map_err(|_| "speaker_record_stop_failed")?;
            if !recording.pipeline.finish(Duration::from_secs(2)).await {
                return Err("speaker_record_stop_failed".into());
            }
            if recording.failure_rx.try_recv().is_ok() {
                return Err("speaker_record_capture_failed".into());
            }
            let path = {
                let mut file = recording.file.lock().unwrap();
                let length = file
                    .as_file()
                    .metadata()
                    .map_err(|_| "speaker_storage_failed")?
                    .len()
                    .saturating_sub(44);
                if !(320_000..=1_920_000).contains(&length) {
                    return Err("speaker_wav_duration_invalid".into());
                }
                file.seek(SeekFrom::Start(0))
                    .map_err(|_| "speaker_storage_failed")?;
                file.write_all(&crate::session_history::wav_header(16_000, length as u32))
                    .map_err(|_| "speaker_storage_failed")?;
                file.as_file()
                    .sync_all()
                    .map_err(|_| "speaker_storage_failed")?;
                file.path().to_path_buf()
            };
            self.import_with_token(path, recording.token).await
        }
        .await;
        let current = self
            .enrollment_epoch
            .load(std::sync::atomic::Ordering::SeqCst)
            == recording.token;
        if current {
            self.recording_active
                .store(false, std::sync::atomic::Ordering::SeqCst);
        }
        let result = if current {
            result
        } else {
            Err("speaker_record_cancelled".into())
        };
        let result = result.map(|_| self.voiceprint_status());
        match &result {
            Ok(_) => recording.emit("saved", None),
            Err(label) => recording.emit("failed", Some(label)),
        }
        result
    }

    pub async fn record_cancel(&self) -> Result<(), String> {
        {
            let _commit = self.enrollment_commit.lock().unwrap();
            let next = self
                .enrollment_epoch
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            self.enrollment_change.send_replace(next);
        }
        let recording = { self.recording.lock().await.take() };
        if let Some(recording) = recording {
            recording.timer.abort();
            let stopped = recording.capture.stop_and_wait().await;
            let _ = recording.pipeline.finish(Duration::from_secs(1)).await;
            recording.emit("failed", Some("speaker_record_cancelled"));
            self.recording_active
                .store(false, std::sync::atomic::Ordering::SeqCst);
            stopped.map_err(|_| "speaker_record_stop_failed")?;
        } else {
            self.recording_active
                .store(false, std::sync::atomic::Ordering::SeqCst);
        }
        Ok(())
    }
}

fn take_for_token<T>(
    slot: &mut Option<T>,
    token: Option<u64>,
    token_of: impl FnOnce(&T) -> u64,
) -> Option<T> {
    if slot
        .as_ref()
        .is_some_and(|current| token.is_none_or(|token| token_of(current) == token))
    {
        slot.take()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::take_for_token;

    #[tokio::test]
    async fn cancelled_timer_cannot_stop_next_recording() {
        let mut slot = Some(1);
        let timer_a =
            tokio::spawn(async { tokio::time::sleep(std::time::Duration::from_millis(10)).await });
        assert_eq!(take_for_token(&mut slot, None, |token| *token), Some(1));
        timer_a.abort();
        slot = Some(2);
        let _ = timer_a.await;
        assert_eq!(take_for_token(&mut slot, Some(1), |token| *token), None);
        assert_eq!(slot, Some(2));
    }
}
