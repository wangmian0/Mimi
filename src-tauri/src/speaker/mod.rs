#![allow(dead_code)] // T0 freezes interfaces before T4 fills their implementations.

pub mod commands;
pub mod embedding;
pub mod recent_pcm;
pub mod voiceprint;

pub use voiceprint::{SpeakerService, SpeakerVerdict};
