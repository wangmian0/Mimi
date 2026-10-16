use mimi_runtime::clients::translation_client::SourceAudioRange;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const MAX_SECONDS: usize = 30;
const VAD_RMS: f32 = 0.005;

/// PCM exists only while the copilot is on. Sample positions keep advancing
/// while it is off so Apple Speech's time-indexed ranges remain aligned.
pub struct RecentPcm {
    generation: u64,
    rate: u32,
    end_sample: u64,
    pcm: VecDeque<i16>,
    marks: VecDeque<(u64, Instant)>,
}

impl RecentPcm {
    pub fn new() -> Self {
        Self {
            generation: 0,
            rate: 16_000,
            end_sample: 0,
            pcm: VecDeque::new(),
            marks: VecDeque::new(),
        }
    }

    pub fn begin(&mut self, generation: u64, rate: u32) {
        self.clear();
        self.generation = generation;
        self.rate = rate;
        self.end_sample = 0;
    }

    pub fn clear(&mut self) {
        self.pcm.clear();
        self.marks.clear();
    }

    pub fn push(
        &mut self,
        generation: u64,
        start_sample: u64,
        bytes: &[u8],
        active: bool,
        at: Instant,
    ) {
        if generation != self.generation || bytes.is_empty() || !bytes.len().is_multiple_of(2) {
            return;
        }
        if !active {
            self.clear();
            self.end_sample = start_sample.saturating_add((bytes.len() / 2) as u64);
            return;
        }
        if start_sample != self.end_sample {
            // A chunk skipped by the non-blocking tap leaves a short hole; pad it
            // with silence so later recogniser ranges stay sample-aligned.
            let gap = start_sample.saturating_sub(self.end_sample);
            if start_sample > self.end_sample && gap <= self.rate as u64 && !self.pcm.is_empty() {
                self.pcm.extend(std::iter::repeat_n(0i16, gap as usize));
            } else {
                self.clear();
            }
        }
        self.end_sample = start_sample.saturating_add((bytes.len() / 2) as u64);
        self.marks.push_back((self.end_sample, at));
        self.pcm.extend(
            bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| i16::from_le_bytes([c[0], c[1]])),
        );
        let max = self.rate as usize * MAX_SECONDS;
        while self.pcm.len() > max {
            self.pcm.pop_front();
        }
        let base = self.end_sample.saturating_sub(self.pcm.len() as u64);
        while self.marks.front().is_some_and(|(end, _)| *end <= base) {
            self.marks.pop_front();
        }
    }

    pub fn bounds(&self) -> (u64, u64) {
        (
            self.end_sample.saturating_sub(self.pcm.len() as u64),
            self.end_sample,
        )
    }

    pub fn exact(&self, generation: u64, range: SourceAudioRange) -> Option<Vec<i16>> {
        if generation != self.generation
            || !range.start_ms.is_finite()
            || !range.end_ms.is_finite()
            || range.start_ms < 0.0
            || range.end_ms <= range.start_ms
        {
            return None;
        }
        let start = (range.start_ms * self.rate as f64 / 1000.0).round() as u64;
        let end = (range.end_ms * self.rate as f64 / 1000.0).round() as u64;
        self.samples(start, end)
    }

    fn samples(&self, start: u64, end: u64) -> Option<Vec<i16>> {
        let base = self.end_sample.saturating_sub(self.pcm.len() as u64);
        if start < base || end > self.end_sample || start >= end {
            return None;
        }
        Some(
            self.pcm
                .iter()
                .skip((start - base) as usize)
                .take((end - start) as usize)
                .copied()
                .collect(),
        )
    }

    /// Latest voiced island, with up to 400 ms of pauses, from the last four
    /// seconds. This is deliberately weaker than an ASR range.
    pub fn recent_voice(&self, generation: u64, confirmed_at: Instant) -> Option<Vec<i16>> {
        if generation != self.generation {
            return None;
        }
        let (end, at) = self
            .marks
            .iter()
            .rev()
            .find(|(_, at)| *at <= confirmed_at)?;
        if confirmed_at.saturating_duration_since(*at) > Duration::from_secs(4) {
            return None;
        }
        let start = end.saturating_sub(self.rate as u64 * 4);
        let data = self.samples(
            start.max(self.end_sample.saturating_sub(self.pcm.len() as u64)),
            *end,
        )?;
        let frame = (self.rate / 100).max(1) as usize;
        let mut last = None;
        let mut start = None;
        let mut gap = 0usize;
        for (i, chunk) in data.chunks(frame).enumerate() {
            let rms = (chunk
                .iter()
                .map(|s| (*s as f32 / 32768.0).powi(2))
                .sum::<f32>()
                / chunk.len() as f32)
                .sqrt();
            if rms >= VAD_RMS {
                if start.is_none() {
                    start = Some(i);
                }
                last = Some(i + 1);
                gap = 0;
            } else if start.is_some() {
                gap += 1;
                if gap > 40 {
                    start = None;
                }
            }
        }
        let (start, end) = (start?, last?);
        let from = start * frame;
        Some(data[from..(end * frame).min(data.len())].to_vec())
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.pcm.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skipped_chunk_is_padded_instead_of_clearing_history() {
        let mut pcm = RecentPcm::new();
        pcm.begin(1, 16_000);
        pcm.push(1, 0, &[1u8; 3_200], true, Instant::now());
        pcm.push(1, 3_200, &[1u8; 3_200], true, Instant::now());
        assert_eq!(pcm.bounds(), (0, 4_800));
        let padded = pcm
            .exact(
                1,
                SourceAudioRange {
                    start_ms: 100.0,
                    end_ms: 200.0,
                },
            )
            .unwrap();
        assert!(padded.iter().all(|sample| *sample == 0));
    }

    #[test]
    fn bounded_generation_and_off() {
        let mut pcm = RecentPcm::new();
        pcm.begin(4, 16_000);
        let chunk = vec![1u8; 64_000];
        for i in 0..17 {
            pcm.push(4, i * 32_000, &chunk, true, Instant::now());
        }
        assert_eq!(pcm.len(), 480_000);
        assert!(pcm
            .exact(
                4,
                SourceAudioRange {
                    start_ms: 0.0,
                    end_ms: 1000.0
                }
            )
            .is_none());
        assert_eq!(
            pcm.exact(
                4,
                SourceAudioRange {
                    start_ms: 32_000.0,
                    end_ms: 33_000.0
                }
            )
            .unwrap()
            .len(),
            16_000
        );
        pcm.push(4, 544_000, &chunk, false, Instant::now());
        assert_eq!(pcm.len(), 0);
        pcm.push(4, 576_000, &chunk, true, Instant::now());
        assert_eq!(
            pcm.exact(
                4,
                SourceAudioRange {
                    start_ms: 36_000.0,
                    end_ms: 37_000.0
                }
            )
            .unwrap()
            .len(),
            16_000
        );
        pcm.begin(5, 24_000);
        assert!(pcm
            .exact(
                4,
                SourceAudioRange {
                    start_ms: 0.0,
                    end_ms: 1.0
                }
            )
            .is_none());
    }

    #[test]
    fn recent_voice_requires_fresh_voiced_audio() {
        let mut pcm = RecentPcm::new();
        pcm.begin(7, 16_000);
        let speech: Vec<u8> = std::iter::repeat_n(12_000i16, 16_000)
            .flat_map(i16::to_le_bytes)
            .collect();
        let now = Instant::now();
        pcm.push(7, 0, &speech, true, now);
        assert!(pcm.recent_voice(7, now).is_some());
        pcm.push(
            7,
            16_000,
            &vec![0; 32_000],
            true,
            now + Duration::from_secs(1),
        );
        assert!(pcm.recent_voice(7, now).is_some());
        assert!(pcm.recent_voice(7, now + Duration::from_secs(6)).is_none());
        pcm.clear();
        assert!(pcm.recent_voice(7, now).is_none());
    }
}
