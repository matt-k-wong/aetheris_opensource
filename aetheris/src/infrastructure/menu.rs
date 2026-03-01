//! Menu System for Doom
//!
//! Handles all menu navigation, input processing, and state transitions.
//! Extracted from main.rs for reusability (Goal #5)

use crate::simulation::{MenuState, WorldState};
use winit::event::VirtualKeyCode;

/// Menu controller handles navigation and selection
pub struct MenuController {
    pub current_state: MenuState,
    pub selection: usize,
}

impl MenuController {
    pub fn new() -> Self {
        Self {
            current_state: MenuState::Main,
            selection: 0,
        }
    }

    /// Handle keyboard input for menu navigation
    /// Returns true if the game should exit
    pub fn handle_input(&mut self, key: VirtualKeyCode, world: &mut WorldState) -> MenuAction {
        match self.current_state {
            MenuState::Main => self.handle_main_menu(key, world),
            MenuState::EpisodeSelect => self.handle_episode_select(key, world),
            MenuState::Options => self.handle_options(key, world),
            MenuState::DifficultySelect => MenuAction::Continue,
            MenuState::LoadGame => self.handle_load_game(key, world),
            MenuState::SaveGame => self.handle_save_game(key, world),
            MenuState::None => self.handle_in_game(key, world),
        }
    }

    fn handle_main_menu(&mut self, key: VirtualKeyCode, _world: &mut WorldState) -> MenuAction {
        const MAIN_OPTIONS: [&str; 4] = ["New Game", "Load Game", "Options", "Quit Game"];

        match key {
            VirtualKeyCode::Up | VirtualKeyCode::W => {
                if self.selection > 0 {
                    self.selection -= 1;
                }
            }
            VirtualKeyCode::Down | VirtualKeyCode::S => {
                if self.selection < MAIN_OPTIONS.len() - 1 {
                    self.selection += 1;
                }
            }
            VirtualKeyCode::Return | VirtualKeyCode::Space => match self.selection {
                0 => {
                    self.current_state = MenuState::EpisodeSelect;
                    return MenuAction::Continue;
                }
                1 => {
                    self.current_state = MenuState::LoadGame;
                    self.selection = 0;
                    return MenuAction::Continue;
                }
                2 => {
                    self.current_state = MenuState::Options;
                    return MenuAction::Continue;
                }
                3 => return MenuAction::Exit,
                _ => {}
            },
            _ => {}
        }
        MenuAction::Continue
    }

    fn handle_episode_select(
        &mut self,
        key: VirtualKeyCode,
        _world: &mut WorldState,
    ) -> MenuAction {
        match key {
            VirtualKeyCode::Escape => {
                self.current_state = MenuState::Main;
            }
            VirtualKeyCode::Return | VirtualKeyCode::Space => {
                // Start Episode 1 (shareware only has episode 1)
                self.current_state = MenuState::None;
                return MenuAction::StartGame { episode: 1, map: 1 };
            }
            _ => {}
        }
        MenuAction::Continue
    }

    fn handle_options(&mut self, key: VirtualKeyCode, world: &mut WorldState) -> MenuAction {
        const OPT_COUNT: usize = 6; // Screen, Gamma, Sfx, Music, FPS, Back

        match key {
            VirtualKeyCode::Escape => {
                self.current_state = MenuState::Main;
                self.selection = 2; // Return to "Options"
            }
            VirtualKeyCode::Up | VirtualKeyCode::W => {
                if self.selection > 0 {
                    self.selection -= 1;
                }
            }
            VirtualKeyCode::Down | VirtualKeyCode::S => {
                if self.selection < OPT_COUNT - 1 {
                    self.selection += 1;
                }
            }
            VirtualKeyCode::Left | VirtualKeyCode::A => {
                self.change_option(world, -1);
            }
            VirtualKeyCode::Right
            | VirtualKeyCode::D
            | VirtualKeyCode::Return
            | VirtualKeyCode::Space => {
                if self.selection == 5 {
                    // Back
                    self.current_state = MenuState::Main;
                    self.selection = 2;
                } else {
                    self.change_option(world, 1);
                }
            }
            _ => {}
        }
        MenuAction::Continue
    }

    fn change_option(&mut self, world: &mut WorldState, dir: i32) {
        match self.selection {
            0 => {
                // Screen Size
                let mut size = world.options.screen_size as i32 + dir;
                if size < 3 {
                    size = 3;
                }
                if size > 11 {
                    size = 11;
                }
                world.options.screen_size = size as u32;
            }
            1 => {
                // Gamma
                let mut g = world.options.gamma as i32 + dir;
                if g < 0 {
                    g = 0;
                }
                if g > 4 {
                    g = 4;
                }
                world.options.gamma = g as u32;
            }
            2 => {
                // SFX Volume
                let mut v = world.options.sfx_volume as i32 + (dir * 10);
                if v < 0 {
                    v = 0;
                }
                if v > 100 {
                    v = 100;
                }
                world.options.sfx_volume = v as u32;
            }
            3 => {
                // Music Volume
                let mut v = world.options.music_volume as i32 + (dir * 10);
                if v < 0 {
                    v = 0;
                }
                if v > 100 {
                    v = 100;
                }
                world.options.music_volume = v as u32;
            }
            4 => {
                // Show FPS
                world.options.show_fps = !world.options.show_fps;
            }
            _ => {}
        }
    }

    fn handle_load_game(&mut self, key: VirtualKeyCode, _world: &mut WorldState) -> MenuAction {
        match key {
            VirtualKeyCode::Escape => {
                self.current_state = MenuState::Main;
                self.selection = 1;
            }
            VirtualKeyCode::Up | VirtualKeyCode::W => {
                if self.selection > 0 {
                    self.selection -= 1;
                }
            }
            VirtualKeyCode::Down | VirtualKeyCode::S => {
                if self.selection < 5 {
                    self.selection += 1;
                }
            }
            VirtualKeyCode::Return | VirtualKeyCode::Space => {
                let slot = self.selection + 1;
                return MenuAction::LoadGame { slot };
            }
            _ => {}
        }
        MenuAction::Continue
    }

    fn handle_save_game(&mut self, key: VirtualKeyCode, _world: &mut WorldState) -> MenuAction {
        match key {
            VirtualKeyCode::Escape => {
                self.current_state = MenuState::Main;
                self.selection = 0;
            }
            VirtualKeyCode::Up | VirtualKeyCode::W => {
                if self.selection > 0 {
                    self.selection -= 1;
                }
            }
            VirtualKeyCode::Down | VirtualKeyCode::S => {
                if self.selection < 5 {
                    self.selection += 1;
                }
            }
            VirtualKeyCode::Return | VirtualKeyCode::Space => {
                let slot = self.selection + 1;
                return MenuAction::SaveGame { slot };
            }
            _ => {}
        }
        MenuAction::Continue
    }

    fn handle_in_game(&mut self, key: VirtualKeyCode, _world: &mut WorldState) -> MenuAction {
        if key == VirtualKeyCode::Escape {
            self.current_state = MenuState::Main;
            self.selection = 0;
        }
        MenuAction::Continue
    }

    /// Sync the menu controller state with the world state
    pub fn sync_to_world(&self, world: &mut WorldState) {
        world.menu_state = self.current_state;
        world.menu_selection = self.selection;
    }

    /// Sync from world state (e.g., after loading a game)
    pub fn sync_from_world(&mut self, world: &WorldState) {
        self.current_state = world.menu_state;
        self.selection = world.menu_selection;
    }
}

/// Actions that the menu system can trigger
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuAction {
    /// Continue normal operation
    Continue,
    /// Exit the application
    Exit,
    /// Start a new game
    StartGame { episode: u32, map: u32 },
    /// Load a saved game
    LoadGame { slot: usize },
    /// Save the current game
    SaveGame { slot: usize },
}

/// Helper to get menu option labels
pub fn get_menu_options(state: MenuState) -> &'static [&'static str] {
    match state {
        MenuState::Main => &["New Game", "Load Game", "Options", "Quit Game"],
        MenuState::Options => &[
            "Screen Size",
            "Gamma",
            "Sound Vol",
            "Music Vol",
            "Show FPS",
            "Back",
        ],
        _ => &[],
    }
}

/// Check if a save slot has a saved game
pub fn save_slot_exists(slot: usize) -> bool {
    let filename = format!("save{}.json", slot);
    std::path::Path::new(&filename).exists()
}

/// Get label for save slot
pub fn get_save_slot_label(slot: usize) -> String {
    if save_slot_exists(slot) {
        format!("Slot {}: Saved Game", slot)
    } else {
        format!("Slot {}: <Empty>", slot)
    }
}
