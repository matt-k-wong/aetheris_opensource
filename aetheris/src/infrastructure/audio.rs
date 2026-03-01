use crate::simulation::{Vertex, WorldState};
use parking_lot::Mutex;
use rodio::Source;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct SoundSample {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl SoundSample {
    pub fn from_dmx(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        let sample_rate = u16::from_le_bytes([data[2], data[3]]) as u32;
        let sample_count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        if sample_rate == 0 {
            return None;
        }
        let pcm_start = 8;
        let pcm_end = (pcm_start + sample_count).min(data.len());
        let samples: Vec<f32> = data[pcm_start..pcm_end]
            .iter()
            .map(|&b| (b as f32 - 128.0) / 128.0)
            .collect();
        Some(Self {
            samples,
            sample_rate,
        })
    }
}

struct ActiveSound {
    sample: Arc<SoundSample>,
    position: f32,
    volume: f32,
    spatial_gain: f32,
    stereo_pan: f32,
    pitch: f32,
}

struct MixerSource {
    active_sounds: Arc<Mutex<Vec<ActiveSound>>>,
    sample_rate: u32,
    next_sample: Option<f32>,
}

impl Iterator for MixerSource {
    type Item = f32;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(sample) = self.next_sample.take() {
            return Some(sample);
        }
        let mut sounds = self.active_sounds.lock();
        let mut left_mix = 0.0f32;
        let mut right_mix = 0.0f32;
        for sound in sounds.iter_mut() {
            let pos = sound.position as usize;
            if pos < sound.sample.samples.len() {
                let frac = sound.position - pos as f32;
                let sample = if pos + 1 < sound.sample.samples.len() {
                    let s1 = sound.sample.samples[pos];
                    let s2 = sound.sample.samples[pos + 1];
                    s1 + (s2 - s1) * frac
                } else {
                    sound.sample.samples[pos]
                };
                let (lg, rg) = if sound.stereo_pan <= 0.0 {
                    (1.0, 1.0 + sound.stereo_pan)
                } else {
                    (1.0 - sound.stereo_pan, 1.0)
                };
                left_mix += sample * sound.volume * sound.spatial_gain * lg;
                right_mix += sample * sound.volume * sound.spatial_gain * rg;
            }
        }
        for sound in sounds.iter_mut() {
            sound.position +=
                sound.pitch * (sound.sample.sample_rate as f32 / self.sample_rate as f32);
        }
        sounds.retain(|s| (s.position as usize) < s.sample.samples.len());

        let soft_clip = |x: f32| x.tanh();
        self.next_sample = Some(soft_clip(right_mix));
        Some(soft_clip(left_mix))
    }
}

impl Source for MixerSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

pub trait AudioBridge {
    fn play_spatial_sound(&mut self, sound_id: &str, position: Vertex, volume: f32);
    fn update_listener(&mut self, position: Vertex, angle: f32);
    fn update(&mut self, world: &WorldState) -> anyhow::Result<()>;
    fn handle(&self) -> Option<&rodio::OutputStreamHandle>;
}

pub struct SampleAudioEngine {
    _stream: rodio::OutputStream,
    handle: rodio::OutputStreamHandle,
    active_sounds: Arc<Mutex<Vec<ActiveSound>>>,
    sound_cache: HashMap<String, Arc<SoundSample>>,
    listener_pos: Vertex,
    listener_angle: f32,
}

impl SampleAudioEngine {
    pub fn new_with_wad_sounds(sound_data: HashMap<String, Vec<u8>>) -> anyhow::Result<Self> {
        let (stream, handle) = rodio::OutputStream::try_default()?;
        let mut sound_cache = HashMap::new();
        for (name, data) in sound_data {
            if let Some(sample) = SoundSample::from_dmx(&data) {
                sound_cache.insert(name, Arc::new(sample));
            }
        }
        let active_sounds = Arc::new(Mutex::new(Vec::new()));
        let mixer = MixerSource {
            active_sounds: Arc::clone(&active_sounds),
            sample_rate: 44100,
            next_sample: None,
        };
        handle.play_raw(mixer.convert_samples())?;
        Ok(Self {
            _stream: stream,
            handle,
            active_sounds,
            sound_cache,
            listener_pos: Vertex::ZERO,
            listener_angle: 0.0,
        })
    }
}

impl AudioBridge for SampleAudioEngine {
    fn handle(&self) -> Option<&rodio::OutputStreamHandle> {
        Some(&self.handle)
    }
    fn play_spatial_sound(&mut self, sound_id: &str, position: Vertex, volume: f32) {
        let nid = if sound_id.starts_with("DS") {
            sound_id.to_string()
        } else {
            format!("DS{}", sound_id)
        };
        let sample = if let Some(s) = self.sound_cache.get(&nid) {
            Arc::clone(s)
        } else {
            return;
        };
        let rel = position - self.listener_pos;
        let dist = rel.length();
        let gain = ((1.0 - (dist / 1000.0).min(1.0)).powi(2) * volume).max(0.0);
        if gain <= 0.01 {
            return;
        }
        let to_sound = rel.normalize_or_zero();
        let forward = glam::Vec2::new(self.listener_angle.cos(), self.listener_angle.sin());
        let right = glam::Vec2::new(-forward.y, forward.x);
        let pan = to_sound.dot(right).clamp(-1.0, 1.0);
        let mut sounds = self.active_sounds.lock();
        if sounds.len() >= 8 {
            // Find the oldest sound or the quietest one
            let mut weakest_idx = 0;
            let mut weakest_score = f32::MAX;
            for (i, s) in sounds.iter().enumerate() {
                // Score = remaining life * volume
                let life = 1.0 - (s.position / s.sample.samples.len() as f32);
                let score = life * s.volume * s.spatial_gain;
                if score < weakest_score {
                    weakest_score = score;
                    weakest_idx = i;
                }
            }
            sounds.remove(weakest_idx);
        }
        sounds.push(ActiveSound {
            sample,
            position: 0.0,
            volume,
            spatial_gain: gain,
            stereo_pan: pan,
            pitch: 0.95 + rand::random::<f32>() * 0.1,
        });
    }
    fn update_listener(&mut self, position: Vertex, angle: f32) {
        self.listener_pos = position;
        self.listener_angle = angle;
    }
    fn update(&mut self, world: &WorldState) -> anyhow::Result<()> {
        // Scale down the overall SFX mix by 60% so OPL music can be heard clearly
        let global_vol = (world.options.sfx_volume as f32 / 100.0) * 0.4;
        self.update_listener(world.player.position, world.player.angle);
        for event in &world.audio_events {
            let vol = event.volume * global_vol;
            if vol > 0.01 {
                if let Some(pos) = event.position {
                    self.play_spatial_sound(&event.sound_id, pos, vol);
                }
            }
        }
        Ok(())
    }
}

pub struct NullAudioEngine;
impl AudioBridge for NullAudioEngine {
    fn handle(&self) -> Option<&rodio::OutputStreamHandle> {
        None
    }
    fn play_spatial_sound(&mut self, _id: &str, _pos: Vertex, _vol: f32) {}
    fn update_listener(&mut self, _pos: Vertex, _ang: f32) {}
    fn update(&mut self, _world: &WorldState) -> anyhow::Result<()> {
        Ok(())
    }
}
