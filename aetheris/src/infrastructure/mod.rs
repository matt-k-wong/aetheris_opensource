pub mod audio;
pub mod menu;
pub mod music;
pub mod music_parser;
pub mod savegame;

use crate::simulation::GameAction;
use crate::simulation::WorldState;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::time::{Duration, Instant};
use winit::dpi::LogicalSize;
use winit::event::VirtualKeyCode;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

pub struct Telemetry {
    pub last_export: Instant,
    pub interval: Duration,
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            last_export: Instant::now(),
            interval: Duration::from_secs(60),
        }
    }

    pub fn snapshot(&mut self, world: &WorldState) {
        if self.last_export.elapsed() < self.interval {
            return;
        }
        self.last_export = Instant::now();

        if let Ok(json) = serde_json::to_string_pretty(world) {
            if let Ok(mut file) = File::create("telemetry.json") {
                let _ = file.write_all(json.as_bytes());
                log::info!("Telemetry: Exported snapshot to telemetry.json");
            } else {
                log::warn!("Telemetry: Failed to create telemetry.json");
            }
        }
    }
}

pub struct PerformanceProfiler {
    pub frames: u64,
    pub stage_times: std::collections::HashMap<String, Duration>,
}

impl PerformanceProfiler {
    pub fn new() -> Self {
        Self {
            frames: 0,
            stage_times: std::collections::HashMap::new(),
        }
    }

    pub fn record(&mut self, stage: &str, duration: Duration) {
        let entry = self
            .stage_times
            .entry(stage.to_string())
            .or_insert(Duration::ZERO);
        *entry += duration;
    }

    pub fn print_histogram(&mut self) {
        self.frames += 1;
        if self.frames < 300 {
            return;
        } // Every 300 frames

        println!("\n--- Performance Histogram (Averages over last 300 frames) ---");
        let mut sorted_stages: Vec<_> = self.stage_times.iter().collect();
        sorted_stages.sort_by(|a, b| b.1.cmp(a.1));

        for (stage, total_duration) in sorted_stages {
            let avg_us = total_duration.as_micros() / self.frames as u128;
            let bar_len = (avg_us / 50).min(50) as usize;
            let bar = "█".repeat(bar_len);
            println!("{:<15} | {:>6}µs {}", stage, avg_us, bar);
        }
        println!("--------------------------------------------------------\n");

        // Reset for next window
        self.stage_times.clear();
        self.frames = 0;
    }
}

pub struct InputManager {
    pub pressed_keys: HashSet<VirtualKeyCode>,
}

impl InputManager {
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
        }
    }

    pub fn get_active_actions(&self) -> HashSet<GameAction> {
        let mut actions = HashSet::new();
        let strafe_mod = self.pressed_keys.contains(&VirtualKeyCode::LAlt)
            || self.pressed_keys.contains(&VirtualKeyCode::Z);

        if self.pressed_keys.contains(&VirtualKeyCode::W)
            || self.pressed_keys.contains(&VirtualKeyCode::Up)
        {
            actions.insert(GameAction::MoveForward);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::S)
            || self.pressed_keys.contains(&VirtualKeyCode::Down)
        {
            actions.insert(GameAction::MoveBackward);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::A) {
            actions.insert(GameAction::StrafeLeft);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::D) {
            actions.insert(GameAction::StrafeRight);
        }

        if self.pressed_keys.contains(&VirtualKeyCode::Left) {
            if strafe_mod {
                actions.insert(GameAction::StrafeLeft);
            } else {
                actions.insert(GameAction::TurnLeft);
            }
        }
        if self.pressed_keys.contains(&VirtualKeyCode::Right) {
            if strafe_mod {
                actions.insert(GameAction::StrafeRight);
            } else {
                actions.insert(GameAction::TurnRight);
            }
        }

        if self.pressed_keys.contains(&VirtualKeyCode::F)
            || self.pressed_keys.contains(&VirtualKeyCode::E)
        {
            actions.insert(GameAction::Use);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::P) {
            actions.insert(GameAction::Pause);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::Equals)
            || self.pressed_keys.contains(&VirtualKeyCode::Plus)
        {
            actions.insert(GameAction::ZoomIn);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::Minus) {
            actions.insert(GameAction::ZoomOut);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::Space)
            || self.pressed_keys.contains(&VirtualKeyCode::LControl)
            || self.pressed_keys.contains(&VirtualKeyCode::RControl)
        {
            actions.insert(GameAction::Fire);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::Key1) {
            actions.insert(GameAction::Weapon1);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::Key2) {
            actions.insert(GameAction::Weapon2);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::Key3) {
            actions.insert(GameAction::Weapon3);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::Key4) {
            actions.insert(GameAction::Weapon4);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::Key5) {
            actions.insert(GameAction::Weapon5);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::Key6) {
            actions.insert(GameAction::Weapon6);
        }
        if self.pressed_keys.contains(&VirtualKeyCode::Key7) {
            actions.insert(GameAction::Weapon7);
        }
        actions
    }
}

pub fn create_window() -> (EventLoop<()>, winit::window::Window) {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Aetheris Engine")
        .with_inner_size(LogicalSize::new(640.0, 400.0)) // Classic Doom aspect ratio
        .build(&event_loop)
        .expect("Failed to create window");

    (event_loop, window)
}
