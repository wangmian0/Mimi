use super::voiceprint::SpeakerLabel;
use std::collections::VecDeque;

pub const MODEL_NAME: &str = "nemo_en_titanet_small.onnx";
pub const MODEL_SHA256: &str = "ad4a1802485d8b34c722d2a9d04249662f2ece5d28a7a039063ca22f515a789e";
pub const MODEL_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/nemo_en_titanet_small.onnx";
const MAX_OTHER_SEGMENTS: usize = 8;

pub fn normalized(mut vector: Vec<f32>) -> Option<Vec<f32>> {
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if !norm.is_finite() || norm <= 1e-8 {
        return None;
    }
    for v in &mut vector {
        *v /= norm;
    }
    Some(vector)
}

pub fn centroid(vectors: &[Vec<f32>]) -> Option<Vec<f32>> {
    let first = vectors.first()?;
    if vectors.iter().any(|v| v.len() != first.len()) {
        return None;
    }
    normalized(
        (0..first.len())
            .map(|i| vectors.iter().map(|v| v[i]).sum::<f32>() / vectors.len() as f32)
            .collect(),
    )
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return -1.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub struct SpeakerClassifier {
    own: Vec<f32>,
    others: VecDeque<Vec<f32>>,
}

impl SpeakerClassifier {
    pub fn new(own: Vec<f32>) -> Self {
        Self {
            own,
            others: VecDeque::new(),
        }
    }
    pub fn reset(&mut self) {
        self.others.clear();
    }
    pub fn classify(
        &mut self,
        windows: &[Vec<f32>],
        voiced_ms: u32,
    ) -> (SpeakerLabel, Option<f32>, [u32; 3]) {
        if voiced_ms < 1_000 || windows.is_empty() {
            return (SpeakerLabel::Unknown, None, [0; 3]);
        }
        let other_center = if self.others.len() >= 2 {
            centroid(&self.others.iter().cloned().collect::<Vec<_>>())
        } else {
            None
        };
        let mut votes = [0u32; 3]; // Me, Other, Unknown.
        let mut own_sum = 0.0;
        for vector in windows {
            let own = cosine(vector, &self.own);
            own_sum += own;
            let vote = if own <= 0.20 {
                1
            } else if let Some(other) = &other_center {
                if own > cosine(vector, other) {
                    0
                } else {
                    1
                }
            } else if own >= 0.30 {
                0
            } else {
                2
            };
            votes[vote] += 1;
        }
        let max = *votes.iter().max().unwrap();
        let label = if votes.iter().filter(|&&v| v == max).count() != 1 {
            SpeakerLabel::Unknown
        } else if votes[0] == max {
            SpeakerLabel::Me
        } else if votes[1] == max {
            SpeakerLabel::Other
        } else {
            SpeakerLabel::Unknown
        };
        let mean = own_sum / windows.len() as f32;
        if label == SpeakerLabel::Other && (self.others.len() < 2 || mean <= 0.20) {
            if let Some(segment) = centroid(windows) {
                if self.others.len() == MAX_OTHER_SEGMENTS {
                    self.others.pop_front();
                }
                self.others.push_back(segment);
            }
        }
        (label, Some(mean), votes)
    }
}

#[cfg(target_os = "macos")]
pub struct SpeakerEmbedding(sherpa_onnx::SpeakerEmbeddingExtractor);

#[cfg(target_os = "macos")]
impl SpeakerEmbedding {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let config = sherpa_onnx::SpeakerEmbeddingExtractorConfig {
            model: Some(path.to_string_lossy().into_owned()),
            num_threads: 1,
            debug: false,
            provider: Some("cpu".into()),
        };
        sherpa_onnx::SpeakerEmbeddingExtractor::create(&config)
            .map(Self)
            .ok_or_else(|| "speaker_model_invalid".into())
    }

    pub fn embed(&self, pcm: &[i16], rate: u32) -> Option<Vec<f32>> {
        let samples: Vec<f32> = pcm.iter().map(|s| *s as f32 / 32768.0).collect();
        let stream = self.0.create_stream()?;
        stream.accept_waveform(rate as i32, &samples);
        stream.input_finished();
        if !self.0.is_ready(&stream) {
            return None;
        }
        normalized(self.0.compute(&stream)?)
    }

    pub fn embed_windows(
        &self,
        pcm: &[i16],
        rate: u32,
        cancelled: impl Fn() -> bool,
    ) -> Vec<Vec<f32>> {
        embed_windows_with(pcm, rate, cancelled, |chunk| self.embed(chunk, rate))
    }
}

fn embed_windows_with(
    pcm: &[i16],
    rate: u32,
    cancelled: impl Fn() -> bool,
    mut embed: impl FnMut(&[i16]) -> Option<Vec<f32>>,
) -> Vec<Vec<f32>> {
    let width = (rate as usize * 3 / 2).min(pcm.len());
    let step = rate as usize * 3 / 4;
    if width < rate as usize {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut start = 0;
    loop {
        if cancelled() {
            break;
        }
        if let Some(embedding) = embed(&pcm[start..start + width]) {
            result.push(embedding);
        }
        if start + width == pcm.len() {
            break;
        }
        start = (start + step).min(pcm.len() - width);
    }
    result
}

/// Keep only voiced 10 ms frames. The returned duration is measured before
/// windowing and is the gate for short utterances.
pub fn voiced_pcm(pcm: &[i16], rate: u32) -> (Vec<i16>, u32) {
    let frame = (rate / 100).max(1) as usize;
    let mut output = Vec::new();
    for chunk in pcm.chunks(frame) {
        let rms = (chunk
            .iter()
            .map(|s| (*s as f32 / 32768.0).powi(2))
            .sum::<f32>()
            / chunk.len() as f32)
            .sqrt();
        if rms >= 0.005 {
            output.extend_from_slice(chunk);
        }
    }
    let duration = (output.len() as u64 * 1000 / rate as u64) as u32;
    (output, duration)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thresholds_ties_and_second_centroid() {
        let own = normalized(vec![1.0, 0.0]).unwrap();
        let mut classifier = SpeakerClassifier::new(own);
        let vector = |x: f32| normalized(vec![x, (1.0 - x * x).sqrt()]).unwrap();
        assert_eq!(
            classifier.classify(&[vector(0.7)], 999).0,
            SpeakerLabel::Unknown
        );
        assert_eq!(
            classifier.classify(&[vector(0.3)], 1000).0,
            SpeakerLabel::Me
        );
        assert_eq!(
            classifier.classify(&[vector(0.25)], 1000).0,
            SpeakerLabel::Unknown
        );
        assert_eq!(
            classifier.classify(&[vector(0.1)], 1000).0,
            SpeakerLabel::Other
        );
        assert_eq!(
            classifier.classify(&[vector(0.4), vector(0.1)], 1500).0,
            SpeakerLabel::Unknown
        );
        assert_eq!(
            classifier.classify(&[vector(0.1)], 1000).0,
            SpeakerLabel::Other
        );
        assert_eq!(
            classifier.classify(&[vector(0.4)], 1000).0,
            SpeakerLabel::Other
        );
        classifier.reset();
        assert_eq!(
            classifier.classify(&[vector(0.4)], 1000).0,
            SpeakerLabel::Me
        );
    }

    #[test]
    fn two_voted_other_segments_establish_centroid_despite_high_mean() {
        let mut classifier = SpeakerClassifier::new(vec![1.0, 0.0]);
        let vector = |x: f32| normalized(vec![x, (1.0 - x * x).sqrt()]).unwrap();
        let mixed = [vector(0.1), vector(0.1), vector(0.9)];
        for _ in 0..2 {
            assert_eq!(classifier.classify(&mixed, 3_000).0, SpeakerLabel::Other);
        }
        assert_eq!(classifier.others.len(), 2);
        assert_eq!(
            classifier.classify(&[vector(0.4)], 1_000).0,
            SpeakerLabel::Other
        );
        assert_eq!(classifier.others.len(), 2);
    }

    #[test]
    fn cancelled_embedding_skips_remaining_windows() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let cancelled = AtomicBool::new(false);
        let mut calls = 0;
        let windows = embed_windows_with(
            &[0; 48],
            16,
            || cancelled.load(Ordering::SeqCst),
            |_| {
                calls += 1;
                cancelled.store(true, Ordering::SeqCst);
                Some(vec![1.0])
            },
        );
        assert_eq!(calls, 1);
        assert_eq!(windows.len(), 1);
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires explicitly supplied local model and audio paths"]
    fn real_model_selftest() {
        let model = std::env::var("MIMI_SPEAKER_SELFTEST_MODEL").unwrap();
        let me = std::env::var("MIMI_SPEAKER_SELFTEST_ME").unwrap();
        let other = std::env::var("MIMI_SPEAKER_SELFTEST_OTHER").unwrap();
        let extractor = SpeakerEmbedding::load(std::path::Path::new(&model)).unwrap();
        let me = super::super::voiceprint::parse_wav(std::path::Path::new(&me)).unwrap();
        let other = super::super::voiceprint::parse_wav(std::path::Path::new(&other)).unwrap();
        let half = me.len() / 2;
        let a = extractor.embed(&me[..half], 16_000).unwrap();
        let b = extractor.embed(&me[half..], 16_000).unwrap();
        let c = extractor
            .embed(&other[..other.len().min(64_000)], 16_000)
            .unwrap();
        let same = cosine(&a, &b);
        let different = cosine(&a, &c);
        println!("same={same:.6} other={different:.6}");
        assert!(same > 0.5);
        assert!(different < 0.25);
    }
}
