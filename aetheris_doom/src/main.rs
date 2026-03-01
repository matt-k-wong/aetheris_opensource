mod bridge;
mod doom;
use doom::{DoomThingExt, DoomWorldExt};

use aetheris::assets::AssetWarehouse;
use aetheris::assets::wad::WadLoader;
use aetheris::infrastructure::InputManager;
use aetheris::infrastructure::audio::AudioBridge;
use aetheris::presentation::VisualBridge;
use aetheris::presentation::classic_engine::ClassicSoftwareEngine;
use aetheris::simulation::WorldCommand;
use winit::event::{ElementState, Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

struct DoomEntity<'a> {
    thing: &'a aetheris::simulation::Thing,
    world: &'a aetheris::simulation::WorldState,
}
impl<'a> aetheris::presentation::AetherisEntity for DoomEntity<'a> {
    fn position(&self) -> glam::Vec2 {
        self.thing.position
    }
    fn z(&self) -> f32 {
        self.thing.z
    }
    fn get_sprites(&self, viewer_pos: glam::Vec2, frame_count: u64) -> Vec<String> {
        crate::bridge::PresentationMapper::get_animated_sprite(
            self.thing,
            viewer_pos,
            frame_count,
            self.world,
        )
    }
    fn should_draw(&self) -> bool {
        !self.thing.picked_up && self.thing.kind != 1
    }
    fn is_spectral(&self) -> bool {
        self.thing.kind == 58 // Demon Invisibility effect
    }
}

struct DoomPlayer<'a> {
    player: &'a aetheris::simulation::Player,
}
impl<'a> aetheris::presentation::AetherisPlayer for DoomPlayer<'a> {
    fn position(&self) -> glam::Vec2 {
        self.player.position
    }
    fn z(&self) -> f32 {
        self.player.z
    }
    fn angle(&self) -> f32 {
        self.player.angle
    }
    fn fov(&self) -> f32 {
        self.player.fov
    }
    fn damage_flash(&self) -> f32 {
        self.player.damage_flash
    }
    fn bonus_flash(&self) -> f32 {
        self.player.bonus_flash
    }
    fn invuln_timer(&self) -> u32 {
        self.player.invuln_timer
    }
    fn radsuit_timer(&self) -> u32 {
        self.player.radsuit_timer
    }
}

#[derive(PartialEq)]
pub enum EngineState {
    MainMenu,
    Playing,
    Intermission,
    Paused,
}

pub async fn run_game(
    event_loop: EventLoop<()>,
    window: Window,
    warehouse: Box<dyn AssetWarehouse>,
) -> anyhow::Result<()> {
    let window_size = window.inner_size();
    let mut input = InputManager::new();
    #[allow(dead_code, unused_assignments, unused_variables)]
    let mut _engine_state = EngineState::Playing; // Start in game for now

    // Determine backend and test modes
    let args: Vec<String> = std::env::args().collect();
    let use_modern = args.iter().any(|arg| arg == "--modern");
    let is_golden_test = args.iter().any(|arg| arg == "--golden-test");
    let is_update_goldens = args.iter().any(|arg| arg == "--update-goldens");

    let mut target_wad = "freedoom1.wad".to_string();
    if let Some(idx) = args.iter().position(|a| a == "--wad") {
        if idx + 1 < args.len() {
            target_wad = args[idx + 1].clone();
        }
    }

    // Initialize the simulation (The World) from WAD via Warehouse
    let wad_data = match warehouse.load_raw(&target_wad).await {
        Ok(data) => {
            log::info!("Loaded {}", target_wad);
            data
        }
        Err(_) => {
            if target_wad == "freedoom1.wad" {
                log::info!("freedoom1.wad not found, trying DOOM1.WAD");
                warehouse.load_raw("DOOM1.WAD").await?
            } else {
                anyhow::bail!("Failed to load specified WAD: {}", target_wad);
            }
        }
    };
    let loader = WadLoader::new(wad_data)?;

    // Try to load DEHACKED patch if present
    let _dehacked_patch = if let Ok(deh_data) = warehouse.load_raw("DOOM1.DEH").await {
        log::info!("Loading DEHACKED patch...");
        match aetheris::assets::dehacked::DehackedPatch::parse(&String::from_utf8_lossy(&deh_data))
        {
            Ok(patch) => {
                log::info!(
                    "DEHACKED patch loaded with {} thing patches, {} weapon patches",
                    patch.things.len(),
                    patch.weapons.len()
                );
                Some(patch)
            }
            Err(e) => {
                log::warn!("Failed to parse DEHACKED patch: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Level Management
    let mut current_map_index = 1;
    let mut current_episode = 1;
    let mut _map_name = format!("E{}M{}", current_episode, current_map_index);

    let mut world = loader.load_map(&_map_name)?;
    loader.load_textures(&mut world)?;

    // Spawn a test monster 200 units right in front of the player
    let p_pos = world.player.position;
    let p_angle = world.player.angle;
    let test_pos = glam::Vec2::new(
        p_pos.x + 200.0 * p_angle.cos(),
        p_pos.y + 200.0 * p_angle.sin(),
    );
    let test_monster = aetheris::simulation::Thing {
        position: test_pos,
        z: 0.0,
        angle: p_angle + std::f32::consts::PI,
        kind: 3004, // Zombieman
        flags: 0,
        health: 20.0,
        picked_up: false,
        state_idx: crate::doom::get_start_state(3004),
        ai_timer: 0,
        target_thing_idx: None,
        attack_cooldown: 0,
    };
    world.things.push(test_monster);

    // Determine backend and test modes
    // (args parsed above)

    // Initialize the renderer (The View) - Open Source Version (Software Render Only)
    if use_modern {
        log::warn!(
            "--modern flag ignored: aetheris_pro WGPU renderer is not included in the open-source release."
        );
    }
    let mut renderer: Box<dyn VisualBridge> = Box::new(ClassicSoftwareEngine::new(
        &window,
        window_size.width,
        window_size.height,
    )?);

    renderer.on_map_loaded(&world);

    if is_golden_test || is_update_goldens {
        for i in 0..8 {
            world.player.owned_weapons[i] = true;
        }
        for i in 0..4 {
            world.player.ammo[i] = 500;
        }
    }

    // Spawn MonsterThinkers
    // Helper to respawn thinkers - preserves AI state from thing.ai_timer
    let spawn_thinkers = |w: &mut aetheris::simulation::WorldState| {
        w.thinkers.clear();
        for (idx, thing) in w.things.iter().enumerate() {
            if thing.is_monster() || thing.is_barrel() {
                let state_idx = thing.state_idx;
                // Use saved ai_timer if valid, otherwise use state duration
                let tics = if thing.ai_timer > 0 {
                    thing.ai_timer as i32
                } else {
                    crate::doom::STATES[state_idx].duration
                };
                w.thinkers.push(Box::new(crate::doom::MonsterThinker::new(
                    idx,
                    state_idx,
                    tics,
                    thing.target_thing_idx,
                    thing.attack_cooldown,
                )));
            }
        }
    };
    spawn_thinkers(&mut world);

    // Timing
    const TICK_RATE: f32 = 35.0;
    const TICK_DURATION: std::time::Duration =
        std::time::Duration::from_nanos((1_000_000_000.0 / TICK_RATE) as u64);
    let mut last_tick_time = std::time::Instant::now();
    let mut accumulator = std::time::Duration::ZERO;

    let mut cheat_buffer = String::new();

    // Audio
    let sound_data = loader.load_sounds();
    let mut audio: Box<dyn AudioBridge> =
        match aetheris::infrastructure::audio::SampleAudioEngine::new_with_wad_sounds(sound_data) {
            Ok(engine) => {
                log::info!("✅ Audio initialized successfully");
                Box::new(engine)
            }
            Err(e) => {
                log::error!("❌ Audio initialization failed: {:?}", e);
                log::error!("   Falling back to NullAudioEngine (no sound)");
                log::error!("   Possible causes:");
                log::error!("   1. No audio output device available");
                log::error!("   2. WAD file contains invalid/missing sound lumps");
                log::error!("   3. Rodio cannot initialize on this platform");
                log::error!("   4. Sound format parsing failed (DMX header issue)");
                Box::new(aetheris::infrastructure::audio::NullAudioEngine)
            }
        };

    // Music
    let mut music = if let Some(handle) = audio.handle() {
        match aetheris::infrastructure::music::MusicEngine::new(handle) {
            Ok(mut m) => {
                if let Err(e) = m.play_map_music(&loader, &_map_name) {
                    println!("Music: [ERROR] Failed to start track: {:?}", e);
                }
                m.set_volume(200); // Pump the music volume up
                Some(m)
            }
            Err(e) => {
                println!("Music: [ERROR] Failed to initialize MusicEngine: {:?}", e);
                None
            }
        }
    } else {
        println!("Music: [ERROR] Audio handle missing!");
        None
    };

    let mut profiler = aetheris::infrastructure::PerformanceProfiler::new();
    let mut telemetry = aetheris::infrastructure::Telemetry::new();

    // Intermission State
    let mut intermission_timer = 0.0;

    // Main Menu State
    let _menu_selection = 0; // 0: New Game, 1: Quit
    let mut exiting = false;
    let mut _menu_cooldown = 0;

    event_loop.run(move |event, _, control_flow| {
        if exiting { return; }

        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                log::info!("Exit requested via window close.");
                exiting = true;
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent { event: WindowEvent::Resized(new_size), .. } => {
                renderer.handle_resize(new_size.width, new_size.height, false);
            }
            Event::WindowEvent { event: WindowEvent::Focused(false), .. } => {
                input.pressed_keys.clear();
            }
            Event::WindowEvent { event: WindowEvent::KeyboardInput { input: keyboard_input, .. }, .. } => {
                let state = keyboard_input.state;
                let mut mapped_key = keyboard_input.virtual_keycode;
                #[cfg(target_os = "macos")]
                {
                    if keyboard_input.scancode == 53 {
                        mapped_key = Some(VirtualKeyCode::Escape);
                    } else if mapped_key.is_none() {
                        match keyboard_input.scancode {
                            36 => mapped_key = Some(VirtualKeyCode::Return),
                            49 => mapped_key = Some(VirtualKeyCode::Space),
                            _ => {}
                        }
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    if mapped_key.is_none() {
                        match keyboard_input.scancode {
                            1 => mapped_key = Some(VirtualKeyCode::Escape), // Generic fallback
                            _ => {}
                        }
                    }
                }

                if let Some(key) = mapped_key {
                    log::info!("RAW KEY INPUT: {:?} {:?}", key, state);
                if state == ElementState::Pressed {
                    if !input.pressed_keys.insert(key) { return; }
                    if key == VirtualKeyCode::F12 {
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let filename = format!("screenshot_{}.png", timestamp);
                        renderer.take_screenshot(&filename).ok();
                    }

                    // Cheat Code Detection
                    let key_char = match key {
                        VirtualKeyCode::A => Some('a'), VirtualKeyCode::B => Some('b'), VirtualKeyCode::C => Some('c'),
                        VirtualKeyCode::D => Some('d'), VirtualKeyCode::E => Some('e'), VirtualKeyCode::F => Some('f'),
                        VirtualKeyCode::G => Some('g'), VirtualKeyCode::H => Some('h'), VirtualKeyCode::I => Some('i'),
                        VirtualKeyCode::J => Some('j'), VirtualKeyCode::K => Some('k'), VirtualKeyCode::L => Some('l'),
                        VirtualKeyCode::M => Some('m'), VirtualKeyCode::N => Some('n'), VirtualKeyCode::O => Some('o'),
                        VirtualKeyCode::P => Some('p'), VirtualKeyCode::Q => Some('q'), VirtualKeyCode::R => Some('r'),
                        VirtualKeyCode::S => Some('s'), VirtualKeyCode::T => Some('t'), VirtualKeyCode::U => Some('u'),
                        VirtualKeyCode::V => Some('v'), VirtualKeyCode::W => Some('w'), VirtualKeyCode::X => Some('x'),
                        VirtualKeyCode::Y => Some('y'), VirtualKeyCode::Z => Some('z'),
                        VirtualKeyCode::Key1 => { world.player.current_weapon = aetheris::simulation::WeaponType::Fist; None },
                        VirtualKeyCode::Key2 => { world.player.current_weapon = aetheris::simulation::WeaponType::Pistol; None },
                        VirtualKeyCode::Key3 => { world.player.current_weapon = aetheris::simulation::WeaponType::Shotgun; None },
                        VirtualKeyCode::Key4 => { world.player.current_weapon = aetheris::simulation::WeaponType::Chaingun; None },
                        VirtualKeyCode::Key5 => { world.player.current_weapon = aetheris::simulation::WeaponType::RocketLauncher; None },
                        VirtualKeyCode::Key6 => { world.player.current_weapon = aetheris::simulation::WeaponType::PlasmaRifle; None },
                        VirtualKeyCode::Key7 => { world.player.current_weapon = aetheris::simulation::WeaponType::BFG9000; None },
                        VirtualKeyCode::Key8 => { world.player.current_weapon = aetheris::simulation::WeaponType::Chainsaw; None },
                        _ => None,
                    };
                    if let Some(c) = key_char {
                        cheat_buffer.push(c);
                        if cheat_buffer.len() > 10 { cheat_buffer.remove(0); }

                        if cheat_buffer.ends_with("iddqd") {
                            if world.player.invuln_timer > 0 {
                                log::info!("Degreelessness Mode OFF");
                                world.apply_commands(vec![
                                    WorldCommand::UpdatePlayer {
                                        health: None, armor: Some(0.0),
                                        position: None, angle: None, velocity: None, z: None,
                                        weapon_state: None, fire_cooldown: None, noise_radius: None,
                                        current_weapon: None, damage_flash: None, bonus_flash: None,
                                        bob_phase: None,
                                    },
                                    WorldCommand::ShowMessage { text: "Degreelessness Mode Off".to_string(), duration_secs: 3.0, color: [255, 0, 0] },
                                ]);
                                world.player.invuln_timer = 0;
                            } else {
                                log::info!("Degreelessness Mode ON");
                                world.apply_commands(vec![
                                    WorldCommand::UpdatePlayer {
                                        health: Some(100.0), armor: Some(100.0),
                                        position: None, angle: None, velocity: None, z: None,
                                        weapon_state: None, fire_cooldown: None, noise_radius: None,
                                        current_weapon: None, damage_flash: None, bonus_flash: None,
                                        bob_phase: None,
                                    },
                                    WorldCommand::ShowMessage { text: "Degreelessness Mode On".to_string(), duration_secs: 3.0, color: [255, 255, 0] },
                                ]);
                                world.player.invuln_timer = 999999;
                            }
                            cheat_buffer.clear();
                        } else if cheat_buffer.ends_with("idkfa") {
                            log::info!("Very Happy Ammo!");
                            world.apply_commands(vec![
                                WorldCommand::ShowMessage { text: "Very Happy Ammo!".to_string(), duration_secs: 3.0, color: [0, 255, 0] },
                            ]);
                            for i in 0..8 { world.player.owned_weapons[i] = true; }
                            for i in 0..3 { world.player.keys[i] = true; }
                            for i in 0..4 { world.player.ammo[i] = 500; }
                            cheat_buffer.clear();
                        }
                    }

                    match world.menu_state {
                        aetheris::simulation::MenuState::Main => {
                            let main_options = ["New Game", "Load Game", "Options", "Quit Game"];
                            if key == VirtualKeyCode::Escape {
                                world.menu_state = aetheris::simulation::MenuState::None;
                                _engine_state = EngineState::Playing;
                            }
                            if key == VirtualKeyCode::Up || key == VirtualKeyCode::W {
                                if world.menu_selection > 0 { world.menu_selection -= 1; _menu_cooldown = 10; }
                            }
                            if key == VirtualKeyCode::Down || key == VirtualKeyCode::S {
                                if world.menu_selection < main_options.len() - 1 { world.menu_selection += 1; _menu_cooldown = 10; }
                            }
                            if key == VirtualKeyCode::Return || key == VirtualKeyCode::Space {
                                match world.menu_selection {
                                    0 => { world.menu_state = aetheris::simulation::MenuState::EpisodeSelect; world.menu_selection = 0; },
                                    1 => { world.menu_state = aetheris::simulation::MenuState::LoadGame; world.menu_selection = 0; },
                                    2 => world.menu_state = aetheris::simulation::MenuState::Options,
                                    3 => { exiting = true; *control_flow = ControlFlow::Exit; }
                                    _ => {}
                                }
                            }
                        }
                        aetheris::simulation::MenuState::EpisodeSelect => {
                            if key == VirtualKeyCode::Escape {
                                world.menu_state = aetheris::simulation::MenuState::Main;
                                world.menu_selection = 0;
                            }
                            if key == VirtualKeyCode::Up || key == VirtualKeyCode::W {
                                if world.menu_selection > 0 { world.menu_selection -= 1; _menu_cooldown = 10; }
                            }
                            if key == VirtualKeyCode::Down || key == VirtualKeyCode::S {
                                if world.menu_selection < 3 { world.menu_selection += 1; _menu_cooldown = 10; }
                            }
                            if key == VirtualKeyCode::Return || key == VirtualKeyCode::Space {
                                world.menu_state = aetheris::simulation::MenuState::DifficultySelect;
                                world.menu_selection = 2; // Default to 'Hurt Me Plenty'
                            }
                        }
                        aetheris::simulation::MenuState::DifficultySelect => {
                            if key == VirtualKeyCode::Escape {
                                world.menu_state = aetheris::simulation::MenuState::EpisodeSelect;
                                world.menu_selection = 0;
                            }
                            if key == VirtualKeyCode::Up || key == VirtualKeyCode::W {
                                if world.menu_selection > 0 { world.menu_selection -= 1; _menu_cooldown = 10; }
                            }
                            if key == VirtualKeyCode::Down || key == VirtualKeyCode::S {
                                if world.menu_selection < 4 { world.menu_selection += 1; _menu_cooldown = 10; }
                            }
                            if key == VirtualKeyCode::Return || key == VirtualKeyCode::Space {
                                // Reset game for new game
                                current_map_index = 1;
                                current_episode = 1; // Assuming shareware
                                _map_name = format!("E{}M{}", current_episode, current_map_index);

                                // Reload the world for fresh game
                                match loader.load_map(&_map_name) {
                                    Ok(mut new_world) => {
                                        if loader.load_textures(&mut new_world).is_ok() {
                                            new_world.thinkers.clear();
                                            world = new_world;
                                            spawn_thinkers(&mut world);
                                            renderer.on_map_loaded(&world);

                                            // Update music
                                            if let Some(m) = &mut music {
                                                let _ = m.play_map_music(&loader, &_map_name);
                                            }

                                            log::info!("New game started: {} at difficulty {}", _map_name, world.menu_selection);
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("Failed to load map for new game: {}", e);
                                        // Continue with current world if loading fails
                                    }
                                }

                                _engine_state = EngineState::Playing;
                                world.menu_state = aetheris::simulation::MenuState::None;
                                intermission_timer = 0.0;
                            }
                        }
                        aetheris::simulation::MenuState::Options => {
                            if key == VirtualKeyCode::Escape {
                                world.menu_state = aetheris::simulation::MenuState::None;
                                _engine_state = EngineState::Playing;
                            }
                        }
                        aetheris::simulation::MenuState::LoadGame | aetheris::simulation::MenuState::SaveGame => {
                            if key == VirtualKeyCode::Escape {
                                world.menu_state = aetheris::simulation::MenuState::None;
                                _engine_state = EngineState::Playing;
                            }
                            if key == VirtualKeyCode::Up || key == VirtualKeyCode::W {
                                if world.menu_selection > 0 { world.menu_selection -= 1; _menu_cooldown = 10; }
                            }
                            if key == VirtualKeyCode::Down || key == VirtualKeyCode::S {
                                if world.menu_selection < 5 { world.menu_selection += 1; _menu_cooldown = 10; }
                            }
                            if key == VirtualKeyCode::Return || key == VirtualKeyCode::Space {
                                let slot = world.menu_selection + 1;
                                let filename = format!("save{}.json", slot);
                                if world.menu_state == aetheris::simulation::MenuState::LoadGame {
                                    match aetheris::infrastructure::savegame::io::load_with_checksum(&filename) {
                                        Ok(json) => {
                                            if let Ok(mut loaded_world) = serde_json::from_str::<aetheris::simulation::WorldState>(&json) {
                                                loaded_world.textures = world.textures.clone();
                                                loaded_world.thinkers.clear();
                                                loaded_world.player.fire_cooldown = 0;
                                                world = loaded_world;
                                                spawn_thinkers(&mut world);
                                                renderer.on_map_loaded(&world);
                                                world.menu_state = aetheris::simulation::MenuState::None;
                                                _engine_state = EngineState::Playing;
                                                intermission_timer = 0.0;
                                            }
                                        },
                                        Err(e) => log::error!("Load failed: {}", e)
                                    }
                                } else {
                                    match aetheris::infrastructure::savegame::io::save_with_checksum(&filename, &world) {
                                        Ok(_) => { log::info!("Game Saved to {}", filename); world.menu_state = aetheris::simulation::MenuState::Main; },
                                        Err(e) => log::error!("Save failed: {}", e)
                                    }
                                }
                            }
                        }
                        aetheris::simulation::MenuState::None => {
                            if key == VirtualKeyCode::Escape {
                                log::info!("Escape pressed from None state! Transitioning to Main Menu.");
                                world.menu_state = aetheris::simulation::MenuState::Main;
                                world.menu_selection = 0;
                            }
                        }
                    }

                    if key == VirtualKeyCode::Tab {
                        world.is_automap = !world.is_automap;
                    }
                    if key == VirtualKeyCode::F5 { let _ = aetheris::infrastructure::savegame::io::quick_save(&world); }
                    if key == VirtualKeyCode::F9 {
                        if let Ok(json) = aetheris::infrastructure::savegame::io::quick_load() {
                            if let Ok(mut loaded_world) = serde_json::from_str::<aetheris::simulation::WorldState>(&json) {
                                loaded_world.textures = world.textures.clone();
                                loaded_world.thinkers.clear();
                                world = loaded_world;
                                spawn_thinkers(&mut world);
                                renderer.on_map_loaded(&world);
                                intermission_timer = 0.0;
                            }
                        }
                    }

                    if world.is_intermission && intermission_timer > 1.0 &&
                       (key == VirtualKeyCode::Space || key == VirtualKeyCode::Return ||
                        key == VirtualKeyCode::NumpadEnter || key == VirtualKeyCode::LControl ||
                        key == VirtualKeyCode::RControl) {
                        log::info!("Intermission: User requested next level via keyboard.");
                        world.is_intermission = false;
                        current_map_index += 1;
                        if current_map_index > 9 { current_map_index = 1; current_episode += 1; }
                        let next_map_name = format!("E{}M{}", current_episode, current_map_index);
                        _map_name = next_map_name.clone();
                        log::info!("Intermission: Loading next map {}...", _map_name);

                        match loader.load_map(&next_map_name) {
                            Ok(mut new_world) => {
                                log::info!("Intermission: Map loaded, initializing textures...");
                                if let Err(e) = loader.load_textures(&mut new_world) {
                                    log::error!("Intermission: Failed to load textures for {}: {}", _map_name, e);
                                }
                                new_world.thinkers.clear();
                                world = new_world;
                                spawn_thinkers(&mut world);
                                renderer.on_map_loaded(&world);
                                if let Some(m) = &mut music {
                                    let _ = m.play_map_music(&loader, &_map_name);
                                }
                                intermission_timer = 0.0;
                                log::info!("Intermission: Successfully transitioned to {}", _map_name);
                            }
                            Err(e) => {
                                log::error!("Intermission: CRITICAL ERROR - Failed to load map {}: {}", _map_name, e);
                                world.is_intermission = false;
                            }
                        }
                    }
                } else {
                    input.pressed_keys.remove(&key);
                }
                } // End if let Some(key)
            }
            Event::MainEventsCleared => {
                if exiting { return; }
                *control_flow = ControlFlow::Poll;
                let now = std::time::Instant::now();
                let frame_time = now - last_tick_time;
                last_tick_time = now;

                let actions = input.get_active_actions();
                renderer.handle_input(&actions);

                let current_fps = 1.0 / frame_time.as_secs_f32().max(0.001);
                world.fps = world.fps * 0.9 + current_fps * 0.1;

                if world.menu_state != aetheris::simulation::MenuState::None {
                    window.request_redraw();
                    return;
                }

                if world.is_intermission {
                    intermission_timer += frame_time.as_secs_f32();
                }

                // Update Music volume
                if let Some(m) = &mut music {
                    m.set_volume(world.options.music_volume);
                    m.update(world.frame_count);
                }

                accumulator += frame_time;
                accumulator = accumulator.min(TICK_DURATION * 5);
                while accumulator >= TICK_DURATION {
                    let start_sim = std::time::Instant::now();
                    let tick_actions = input.get_active_actions();

                    if tick_actions.contains(&aetheris::simulation::GameAction::Pause) {
                        world.is_paused = !world.is_paused;
                        input.pressed_keys.remove(&VirtualKeyCode::P);
                    }

                    if !world.is_paused {
                        // Allow update() even during intermission so it can increment intermission_tic
                        world.update(&tick_actions);

                        if world.is_win {
                            world.is_intermission = true;
                            intermission_timer = 0.0;
                            world.is_win = false;
                        }

                        if !world.is_intermission {
                            audio.update_listener(world.player.position, world.player.angle);
                            let _ = audio.update(&world);
                        }
                        world.audio_events.clear();
                    }

                    accumulator -= TICK_DURATION;
                    profiler.record("simulation_tick", start_sim.elapsed());
                }
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                if is_golden_test || is_update_goldens {
                    let test_cases = [
                        (60, "golden_1", glam::Vec2::new(1056.0, -3616.0), 0.0, aetheris::simulation::WeaponType::Pistol),
                        (90, "golden_2", glam::Vec2::new(1056.0, -3616.0), 0.0, aetheris::simulation::WeaponType::Shotgun),
                        (120, "golden_3", glam::Vec2::new(3072.0, -4736.0), 1.57, aetheris::simulation::WeaponType::Chaingun),
                        (150, "golden_4", glam::Vec2::new(3800.0, -3200.0), -0.78, aetheris::simulation::WeaponType::Fist),
                    ];
                    static mut LAST_GOLDEN: i32 = -1;
                    for (frame, name, pos, angle, weapon) in test_cases {
                        unsafe {
                            if world.frame_count >= frame && LAST_GOLDEN < frame as i32 {
                                LAST_GOLDEN = frame as i32;
                                log::info!("Golden Case: {} (Frame {})", name, world.frame_count);
                                world.player.position = pos;
                                world.player.angle = angle;
                                if let Some(sid) = world.find_sector_at(pos) {
                                    world.player.z = world.sectors[sid].floor_height;
                                }
                                world.player.current_weapon = weapon;
                                world.player.bob_phase = 0.0;

                                // FORCE RENDER THE SCENE FLUSH BEFORE SCREENCAP!
                                let doom_player = DoomPlayer { player: &world.player };
                                let wrapped_things: Vec<DoomEntity> = world.things.iter().map(|t| DoomEntity { thing: t, world: &world }).collect();
                                let entities: Vec<&dyn aetheris::presentation::AetherisEntity> = wrapped_things.iter().map(|wt| wt as &dyn aetheris::presentation::AetherisEntity).collect();

                                let mut dummy_profiler = aetheris::infrastructure::PerformanceProfiler::new();
                                let _ = renderer.render_scene(&world, &entities, &doom_player, &mut dummy_profiler);
                                let _ = renderer.render_hud(&world);

                                let actual_path = format!("temp_{}.png", name);
                                let golden_path = format!("tests/goldens/{}.png", name);
                                let diff_path = format!("diff_{}.png", name);
                                let _ = renderer.take_screenshot(&actual_path);
                                if is_golden_test {
                                    let engine = aetheris::presentation::visual_test::VisualRegressionEngine::new(5);
                                    if std::path::Path::new(&golden_path).exists() {
                                        match engine.compare_images(std::path::Path::new(&actual_path), std::path::Path::new(&golden_path), std::path::Path::new(&diff_path)) {
                                            Ok(score) => {
                                                if score > 0.001 { log::error!("GOLDEN TEST FAILED: {} (score: {:.4})", name, score); }
                                                else { log::info!("GOLDEN TEST PASSED: {} (score: {:.4})", name, score); }
                                            }
                                            Err(e) => log::error!("Comparison error: {:?}", e),
                                        }
                                    }
                                } else if is_update_goldens {
                                    let _ = std::fs::copy(&actual_path, &golden_path);
                                }
                            }
                        }
                    }
                    if world.frame_count > 160 { log::info!("Golden tests complete."); *control_flow = ControlFlow::Exit; }
                }

                let doom_player = DoomPlayer { player: &world.player };
                let wrapped_things: Vec<DoomEntity> = world.things.iter().map(|t| DoomEntity { thing: t, world: &world }).collect();
                let entities: Vec<&dyn aetheris::presentation::AetherisEntity> = wrapped_things.iter().map(|wt| wt as &dyn aetheris::presentation::AetherisEntity).collect();

                let _ = renderer.render_scene(&world, &entities, &doom_player, &mut profiler);
                let _ = renderer.render_hud(&world);
                let _ = renderer.render_automap(&world);
                // Removed temporary turning code
                let _ = renderer.present();
                profiler.print_histogram();
                telemetry.snapshot(&world);

                // The game loop will now run naturally
            }
            Event::LoopDestroyed => {
                log::info!("Exiting.");
                #[cfg(not(target_arch = "wasm32"))]
                std::process::exit(0);
            }
            _ => {}
        }
    });
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn wasm_main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("Failed to init logger");
    let (event_loop, window) = infrastructure::create_window();
    use winit::platform::web::WindowExtWebSys;
    web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| doc.body())
        .and_then(|body| {
            let canvas = window.canvas();
            canvas.set_id("doom-canvas");
            body.append_child(&canvas).ok()
        })
        .expect("Failed to append canvas");
    let warehouse = Box::new(aetheris::assets::WebWarehouse);
    let _ = run_game(event_loop, window, warehouse).await;
}
fn main() -> anyhow::Result<()> {
    env_logger::init();
    let (event_loop, window) = aetheris::infrastructure::create_window();
    let warehouse = Box::new(aetheris::assets::FileSystemWarehouse);
    pollster::block_on(run_game(event_loop, window, warehouse))
}
