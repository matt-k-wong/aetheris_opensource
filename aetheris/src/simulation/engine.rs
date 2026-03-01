// Generic engine types and physics - not Doom-specific
use glam::Vec2;
use serde::{Deserialize, Serialize};

pub type Vertex = Vec2;

// Physics Constants
pub const PLAYER_RADIUS: f32 = 16.0;
pub const MONSTER_RADIUS: f32 = 20.0;
pub const MONSTER_SPEED: f32 = 8.0;
pub const TURN_SPEED: f32 = 0.05;
pub const STEP_HEIGHT: f32 = 24.0; // Doom's MAXSTEPHEIGHT
pub const NOISE_RADIUS_FIRE: f32 = 2000.0;
pub const FOG_DISTANCE: f32 = 2500.0;
pub const PICKUP_RADIUS: f32 = 32.0;
pub const MELEE_RANGE: f32 = 64.0;
pub const PROJECTILE_RADIUS: f32 = 5.0;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum GameAction {
    MoveForward,
    MoveBackward,
    TurnLeft,
    TurnRight,
    StrafeLeft,
    StrafeRight,
    Use,
    Fire,
    Weapon1,
    Weapon2,
    Weapon3,
    Weapon4,
    Weapon5,
    Weapon6,
    Weapon7,
    Pause,
    ZoomIn,
    ZoomOut,
    ThrustUp,
    ThrustDown,
    PitchUp,
    PitchDown,
    RollLeft,
    RollRight,
    ToggleFlightMode,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum WeaponType {
    Fist,
    Pistol,
    Shotgun,
    Chaingun,
    RocketLauncher,
    PlasmaRifle,
    BFG9000,
    Chainsaw,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Seg {
    pub start_idx: usize,
    pub end_idx: usize,
    pub linedef_idx: usize,
    pub side: usize,
    pub offset: f32,
}
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Sidedef {
    pub texture_middle: Option<String>,
    pub texture_upper: Option<String>,
    pub texture_lower: Option<String>,
    pub sector_idx: usize,
    pub x_offset: f32,
    pub y_offset: f32,
}

#[derive(Serialize, Deserialize)]
pub struct LineDefinition {
    pub start_idx: usize,
    pub end_idx: usize,
    pub front: Option<Sidedef>,
    pub back: Option<Sidedef>,
    pub sector_front: Option<usize>,
    pub sector_back: Option<usize>, // Keeping these for compat/convenience
    pub special_type: u16,
    pub sector_tag: u16,
    pub flags: u16, // Pegging and other line flags
    pub activated: bool,
}
impl LineDefinition {
    pub fn is_portal(&self) -> bool {
        self.sector_front.is_some() && self.sector_back.is_some()
    }

    /// Check if upper texture is pegged to ceiling (normal) or floor (unpegged)
    /// Flag 0x0008: Upper unpegged - texture starts at floor, goes down
    pub fn upper_pegged_to_ceiling(&self) -> bool {
        (self.flags & 0x0008) == 0 // If NOT set, pegged to ceiling
    }

    /// Check if lower texture is pegged to floor (normal) or ceiling (unpegged)
    /// Flag 0x0010: Lower unpegged - texture starts at ceiling, goes up
    pub fn lower_pegged_to_floor(&self) -> bool {
        (self.flags & 0x0010) == 0 // If NOT set, pegged to floor
    }

    /// Check if middle texture is double-pegged (pegged to both floor and ceiling)
    /// Flag 0x0020: Double pegged
    pub fn middle_double_pegged(&self) -> bool {
        (self.flags & 0x0020) != 0
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum SectorAction {
    None,
    FloorMove {
        target_height: f32,
        speed: f32,
    },
    CeilingMove {
        target_height: f32,
        speed: f32,
    },
    Door {
        state: DoorState,
        wait_timer: f32,
        speed: f32,
        open_height: f32,
        close_height: f32,
    },
    Lift {
        state: LiftState,
        wait_timer: f32,
        speed: f32,
        low_height: f32,
        high_height: f32,
    },
    Light {
        effect: LightEffect,
        timer: f32,
        base_light: f32,
        alt_light: f32,
    },
    Crusher {
        state: CrusherState,
        speed: f32,
        low_height: f32,
        high_height: f32,
        damage: f32,
    },
    MuzzleFlash {
        timer: f32,
        original_light: f32,
    },
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum CrusherState {
    GoingDown,
    GoingUp,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum LightEffect {
    Flicker,
    StrobeFast,
    StrobeSlow,
    Glow,
    BlinkOff,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum DoorState {
    Closed,
    Opening,
    Waiting,
    Closing,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum LiftState {
    Floor,
    GoingDown,
    Waiting,
    GoingUp,
}

#[derive(Serialize, Deserialize)]
pub struct Sector {
    pub floor_height: f32,
    pub ceiling_height: f32,
    pub light_level: f32,
    pub texture_floor: String,
    pub texture_ceiling: String,
    pub tag: i16,
    pub action: SectorAction,
    pub special_type: u16,
    pub secret_found: bool,
}

impl Sector {
    pub fn calculate_update(
        &self,
        dt: f32,
        sector_idx: usize,
        world_frame: u64,
    ) -> Vec<WorldCommand> {
        let mut cmds = Vec::new();
        let mut current_floor = self.floor_height;
        let mut current_ceiling = self.ceiling_height;
        let mut current_light = self.light_level;
        let mut new_action = self.action.clone();

        let mut changed = false;

        match &mut new_action {
            SectorAction::None => {}
            SectorAction::Light {
                effect,
                timer,
                base_light,
                alt_light,
            } => {
                *timer -= dt;
                if *timer <= 0.0 {
                    changed = true;
                    match effect {
                        LightEffect::StrobeFast => {
                            current_light = if self.light_level == *base_light {
                                *alt_light
                            } else {
                                *base_light
                            };
                            *timer = if current_light == *base_light {
                                0.5
                            } else {
                                0.1
                            };
                        }
                        LightEffect::StrobeSlow => {
                            current_light = if self.light_level == *base_light {
                                *alt_light
                            } else {
                                *base_light
                            };
                            *timer = if current_light == *base_light {
                                1.0
                            } else {
                                0.2
                            };
                        }
                        LightEffect::Flicker => {
                            current_light = if rand::random::<f32>() > 0.5 {
                                *base_light
                            } else {
                                *alt_light
                            };
                            *timer = (rand::random::<f32>() * 0.1) + 0.05;
                        }
                        LightEffect::Glow => {
                            current_light =
                                *base_light + (*alt_light - *base_light) * (rand::random::<f32>());
                            *timer = 0.1;
                        }
                        LightEffect::BlinkOff => {
                            current_light = if self.light_level == *base_light {
                                *alt_light
                            } else {
                                *base_light
                            };
                            *timer = if current_light == *base_light {
                                1.0 + rand::random::<f32>() * 2.0
                            } else {
                                0.2
                            };
                        }
                    }
                }
            }
            SectorAction::MuzzleFlash {
                timer,
                original_light,
            } => {
                *timer -= dt;
                if *timer <= 0.0 {
                    current_light = *original_light;
                    new_action = SectorAction::None;
                    changed = true;
                }
            }
            SectorAction::Door {
                state,
                wait_timer,
                speed,
                open_height,
                close_height,
            } => {
                changed = true;
                match state {
                    DoorState::Opening => {
                        current_ceiling += *speed;
                        if current_ceiling >= *open_height {
                            current_ceiling = *open_height;
                            *state = DoorState::Waiting;
                            *wait_timer = 4.0;
                        }
                    }
                    DoorState::Waiting => {
                        *wait_timer -= dt;
                        if *wait_timer <= 0.0 {
                            *state = DoorState::Closing;
                        }
                    }
                    DoorState::Closing => {
                        current_ceiling -= *speed;
                        if current_ceiling <= *close_height {
                            current_ceiling = *close_height;
                            *state = DoorState::Closed;
                        }
                    }
                    DoorState::Closed => {
                        new_action = SectorAction::None;
                    }
                }
            }
            SectorAction::Lift {
                state,
                wait_timer,
                speed,
                low_height,
                high_height,
            } => {
                changed = true;
                match state {
                    LiftState::GoingDown => {
                        current_floor -= *speed;
                        if current_floor <= *low_height {
                            current_floor = *low_height;
                            *state = LiftState::Waiting;
                            *wait_timer = 3.0;
                        }
                    }
                    LiftState::Waiting => {
                        *wait_timer -= dt;
                        if *wait_timer <= 0.0 {
                            *state = LiftState::GoingUp;
                        }
                    }
                    LiftState::GoingUp => {
                        current_floor += *speed;
                        if current_floor >= *high_height {
                            current_floor = *high_height;
                            *state = LiftState::Floor;
                        }
                    }
                    LiftState::Floor => {
                        new_action = SectorAction::None;
                    }
                }
            }
            SectorAction::FloorMove {
                target_height,
                speed,
            } => {
                changed = true;
                if current_floor < *target_height {
                    current_floor += *speed;
                    if current_floor >= *target_height {
                        current_floor = *target_height;
                        new_action = SectorAction::None;
                    }
                } else {
                    current_floor -= *speed;
                    if current_floor <= *target_height {
                        current_floor = *target_height;
                        new_action = SectorAction::None;
                    }
                }
            }
            SectorAction::CeilingMove {
                target_height,
                speed,
            } => {
                changed = true;
                if current_ceiling < *target_height {
                    current_ceiling += *speed;
                    if current_ceiling >= *target_height {
                        current_ceiling = *target_height;
                        new_action = SectorAction::None;
                    }
                } else {
                    current_ceiling -= *speed;
                    if current_ceiling <= *target_height {
                        current_ceiling = *target_height;
                        new_action = SectorAction::None;
                    }
                }
            }
            SectorAction::Crusher {
                state,
                speed,
                low_height,
                high_height,
                damage,
            } => {
                changed = true;
                match state {
                    CrusherState::GoingDown => {
                        current_ceiling -= *speed;
                        if current_ceiling <= *low_height {
                            current_ceiling = *low_height;
                            let limit = self.floor_height;
                            if current_ceiling <= limit {
                                current_ceiling = limit;
                                *state = CrusherState::GoingUp;
                            }
                        }
                        // Damage logic: if ceiling is low enough (below player/monster height), inflict damage
                        // In authentic Doom, this triggers every tick the movement is blocked.
                        // We'll apply it whenever the ceiling is in the "danger zone".
                        if current_ceiling < current_floor + 56.0 {
                            cmds.push(WorldCommand::DamageThingsInSector {
                                sector_idx,
                                amount: *damage * 2.0,
                            });
                        }
                    }
                    CrusherState::GoingUp => {
                        current_ceiling += *speed;
                        if current_ceiling >= *high_height {
                            current_ceiling = *high_height;
                            *state = CrusherState::GoingDown;
                        }
                    }
                }
            }
        }

        // Animated Flats (Liquids)
        let mut final_floor_tex = self.texture_floor.clone();
        let final_ceiling_tex = self.texture_ceiling.clone();
        if self.texture_floor.starts_with("NUKAGE") {
            let frame = (world_frame / 8) % 3 + 1;
            final_floor_tex = format!("NUKAGE{}", frame);
            changed = true;
        } else if self.texture_floor.starts_with("LAVA") {
            let frame = (world_frame / 8) % 4 + 1;
            final_floor_tex = format!("LAVA{}", frame);
            changed = true;
        } else if self.texture_floor.starts_with("BLOOD") {
            let frame = (world_frame / 8) % 3 + 1;
            final_floor_tex = format!("BLOOD{}", frame);
            changed = true;
        }

        if changed {
            cmds.push(WorldCommand::SetSectorState {
                sector_idx,
                floor: current_floor,
                ceiling: current_ceiling,
                light: current_light,
                action: new_action,
                texture_floor: Some(final_floor_tex),
                texture_ceiling: Some(final_ceiling_tex),
            });
        }
        cmds
    }
}

#[derive(Serialize, Deserialize)]
pub struct BspNode {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub bbox_right: [f32; 4],
    pub bbox_left: [f32; 4],
    pub child_right: u16,
    pub child_left: u16,
}

#[derive(Serialize, Deserialize)]
pub struct Subsector {
    pub seg_count: usize,
    pub first_seg_idx: usize,
}
pub fn weapon_ammo_type(weapon: WeaponType) -> Option<usize> {
    match weapon {
        WeaponType::Fist | WeaponType::Chainsaw => None,
        WeaponType::Pistol | WeaponType::Chaingun => Some(0),
        WeaponType::Shotgun => Some(1),
        WeaponType::RocketLauncher => Some(2),
        WeaponType::PlasmaRifle | WeaponType::BFG9000 => Some(3),
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum WeaponState {
    Ready,
    Raising,
    Lowering,
    Firing(u32),
    Flash(u32),
}

#[derive(Serialize, Deserialize)]
pub struct Player {
    pub position: Vertex,
    pub velocity: Vec2,
    pub z: f32,
    pub angle: f32,
    pub fov: f32,
    pub health: f32,
    pub armor: f32,
    pub ammo: [u32; 8],
    pub current_weapon: WeaponType,
    pub weapon_state: WeaponState,
    pub fire_cooldown: u32,
    pub damage_flash: f32,
    pub bonus_flash: f32,
    pub noise_radius: f32,
    pub bob_phase: f32,
    pub owned_weapons: [bool; 8],
    pub keys: [bool; 3],
    pub invuln_timer: u32,
    pub berserk_timer: u32,
    pub radsuit_timer: u32,
    pub lightamp_timer: u32,
    pub invis_timer: u32,
    pub last_damage_angle: Option<f32>, // For face direction
    pub pitch: f32,
    pub roll: f32,
    pub throttle: f32,
    pub flight_mode: bool,
}

impl Player {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            position: Vec2::new(x, y),
            velocity: Vec2::ZERO,
            z: 0.0,
            angle: 0.0,
            fov: 90.0,
            health: 100.0,
            armor: 0.0,
            ammo: [50, 0, 0, 0, 0, 0, 0, 0],
            current_weapon: WeaponType::Pistol,
            weapon_state: WeaponState::Ready,
            fire_cooldown: 0,
            noise_radius: 0.0,
            damage_flash: 0.0,
            bonus_flash: 0.0,
            bob_phase: 0.0,
            owned_weapons: [true, true, false, false, false, false, false, false],
            keys: [false, false, false],
            invuln_timer: 0,
            berserk_timer: 0,
            radsuit_timer: 0,
            lightamp_timer: 0,
            invis_timer: 0,
            last_damage_angle: None,
            pitch: 0.0,
            roll: 0.0,
            throttle: 0.0,
            flight_mode: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Texture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub left_offset: i32,
    pub top_offset: i32,
    #[serde(skip)]
    pub pixels: Vec<u8>,
    #[serde(skip)]
    pub pixels_indexed: Vec<i16>,
}

pub enum WorldCommand {
    SpawnThinker(Box<dyn Thinker>),
    SpawnAudioEvent(AudioEvent),
    ShowMessage {
        text: String,
        duration_secs: f32,
        color: [u8; 3],
    },
    ModifySector {
        sector_idx: usize,
        floor_delta: f32,
        ceiling_delta: f32,
    },
    SetSectorState {
        sector_idx: usize,
        floor: f32,
        ceiling: f32,
        light: f32,
        action: SectorAction,
        texture_floor: Option<String>,
        texture_ceiling: Option<String>,
    },
    ModifyThing {
        thing_idx: usize,
        pos_delta: Vec2,
        z_delta: f32,
        angle: f32,
    },
    SetThingHealth {
        thing_idx: usize,
        health: f32,
    },
    UpdatePlayer {
        position: Option<Vertex>,
        angle: Option<f32>,
        velocity: Option<Vec2>,
        z: Option<f32>,
        health: Option<f32>,
        armor: Option<f32>,
        weapon_state: Option<WeaponState>,
        fire_cooldown: Option<u32>,
        noise_radius: Option<f32>,
        current_weapon: Option<WeaponType>,
        damage_flash: Option<f32>,
        bonus_flash: Option<f32>,
        bob_phase: Option<f32>,
    },
    UpdatePlayerAmmo {
        weapon: WeaponType,
        amount: i32,
        set: bool,
    },
    PickupItem {
        thing_idx: usize,
    },
    DamageThing {
        thing_idx: usize,
        amount: f32,
        inflictor_idx: Option<usize>,
    },
    DamagePlayer {
        amount: f32,
        angle: Option<f32>,
    }, // angle for face direction
    DamageThingsInSector {
        sector_idx: usize,
        amount: f32,
    },
    FireHitscan {
        origin: Vertex,
        angle: f32,
        damage: f32,
        attacker_idx: Option<usize>,
    },
    SplashDamage {
        center: Vertex,
        damage: f32,
        radius: f32,
        owner_is_player: bool,
    },
    SpawnThing {
        kind: u16,
        position: Vertex,
        z: f32,
        angle: f32,
    },
    SpawnProjectile {
        kind: u16,
        position: Vertex,
        z: f32,
        velocity: Vec2,
        z_velocity: f32,
        damage: f32,
        owner_is_player: bool,
        owner_thing_idx: Option<usize>,
    },
    InflictPain {
        thing_idx: usize,
        inflictor_idx: Option<usize>,
    },
    Win,
    RespawnPlayer,
    SyncAiState {
        thing_idx: usize,
        state_idx: usize,
        timer: u32,
        target: Option<usize>,
        cooldown: u32,
    }, // Sync thinker state to thing for save/load
}

pub trait Thinker: Send + Sync {
    fn update(&mut self, world: &WorldState) -> (bool, Vec<WorldCommand>);
    fn on_pain(
        &mut self,
        target_idx: usize,
        target_kind: u16,
        inflictor_idx: Option<usize>,
        inflictor_kind: Option<u16>,
    );
    fn on_wake(&mut self, thing_idx: usize);
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Thing {
    pub position: Vertex,
    pub z: f32,
    pub angle: f32,
    pub kind: u16,
    pub flags: u16,
    pub health: f32,
    pub picked_up: bool,
    pub state_idx: usize,
    pub ai_timer: u32,
    pub target_thing_idx: Option<usize>,
    pub attack_cooldown: u32,
}

#[derive(Serialize, Deserialize)]
pub struct AudioEvent {
    pub sound_id: String,
    pub position: Option<Vertex>,
    pub volume: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HudMessage {
    pub text: String,
    pub timer: f32, // Seconds remaining
    pub color: [u8; 3],
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum MenuState {
    None,
    Main,
    EpisodeSelect,
    DifficultySelect,
    Options,
    LoadGame,
    SaveGame,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct GameOptions {
    pub screen_size: u32,  // 0-10, affects 3D view size
    pub gamma: u32,        // 0-4
    pub sfx_volume: u32,   // 0-100
    pub music_volume: u32, // 0-100
    pub show_fps: bool,
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            screen_size: 10,
            gamma: 0,
            sfx_volume: 100,
            music_volume: 100,
            show_fps: false,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct WorldState {
    pub frame_count: u64,
    pub vertices: Vec<Vertex>,
    pub linedefs: Vec<LineDefinition>,
    pub sectors: Vec<Sector>,
    pub nodes: Vec<BspNode>,
    pub subsectors: Vec<Subsector>,
    pub segs: Vec<Seg>,
    pub things: Vec<Thing>,
    pub player: Player,
    pub player_start_pos: Vertex,
    #[serde(skip)]
    pub textures: std::collections::HashMap<String, Texture>,
    #[serde(skip)]
    pub audio_events: Vec<AudioEvent>,
    #[serde(skip)]
    pub thinkers: Vec<Box<dyn Thinker + Send + Sync>>,
    pub is_win: bool,
    pub is_intermission: bool,
    pub intermission_tic: u32,
    pub is_paused: bool,
    pub is_automap: bool,
    pub is_automap_follow: bool,
    pub menu_state: MenuState,
    pub menu_selection: usize,
    pub monsters_killed: u32,
    pub total_monsters: u32,
    pub items_collected: u32,
    pub total_items: u32,
    pub secrets_found: u32,
    pub total_secrets: u32,
    pub adjacent_sectors: Vec<Vec<usize>>,
    pub hud_messages: Vec<HudMessage>, // Top-of-screen messages
    pub colormap: Vec<u8>,
    pub palettes: Vec<Vec<u8>>,
    pub current_palette_idx: usize,
    pub options: GameOptions,
    #[serde(skip)]
    pub fps: f32,
}

impl WorldState {
    pub fn new() -> Self {
        Self {
            frame_count: 0,
            vertices: Vec::new(),
            linedefs: Vec::new(),
            sectors: Vec::new(),
            nodes: Vec::new(),
            subsectors: Vec::new(),
            segs: Vec::new(),
            things: Vec::new(),
            player: Player::new(0.0, 0.0),
            player_start_pos: Vertex::ZERO,
            textures: std::collections::HashMap::new(),
            audio_events: Vec::new(),
            thinkers: Vec::new(),
            is_win: false,
            is_intermission: false,
            intermission_tic: 0,
            is_paused: false,
            is_automap: false,
            is_automap_follow: true,
            menu_state: MenuState::None,
            menu_selection: 0,
            monsters_killed: 0,
            total_monsters: 0,
            items_collected: 0,
            total_items: 0,
            secrets_found: 0,
            total_secrets: 0,
            adjacent_sectors: Vec::new(),
            hud_messages: Vec::new(),
            colormap: Vec::new(),
            palettes: Vec::new(),
            current_palette_idx: 0,
            options: GameOptions::default(),
            fps: 0.0,
        }
    }

    pub fn has_line_of_sight(&self, origin: Vertex, target: Vertex) -> bool {
        for line in &self.linedefs {
            if line.is_portal() {
                if let (Some(_fs), Some(bs)) = (line.sector_front, line.sector_back) {
                    let back = &self.sectors[bs];
                    if back.ceiling_height <= back.floor_height + 1.0 {
                        let p1 = self.vertices[line.start_idx];
                        let p2 = self.vertices[line.end_idx];
                        if Self::intersect(origin, target, p1, p2).is_some() {
                            return false;
                        }
                    }
                }
                continue;
            }
            let p1 = self.vertices[line.start_idx];
            let p2 = self.vertices[line.end_idx];
            if Self::intersect(origin, target, p1, p2).is_some() {
                return false;
            }
        }
        true
    }

    pub fn find_sector_at(&self, pos: Vertex) -> Option<usize> {
        if self.sectors.is_empty()
            || self.nodes.is_empty()
            || self.subsectors.is_empty()
            || self.segs.is_empty()
        {
            return None;
        }

        let subsector_idx = self.find_subsector(pos.x, pos.y);

        if let Some(subsector) = self.subsectors.get(subsector_idx) {
            // Try all segs in this subsector to find a valid sector
            for seg_idx in subsector.first_seg_idx..subsector.first_seg_idx + subsector.seg_count {
                if let Some(seg) = self.segs.get(seg_idx) {
                    if let Some(linedef) = self.linedefs.get(seg.linedef_idx) {
                        // seg.side indicates which side of the linedef this seg is on
                        // side 0 = front sidedef's sector, side 1 = back sidedef's sector
                        let sector = if seg.side == 0 {
                            linedef.sector_front
                        } else {
                            linedef.sector_back
                        };

                        // Bounds check before returning
                        if let Some(sid) = sector {
                            if sid < self.sectors.len() {
                                return Some(sid);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub fn find_subsector(&self, x: f32, y: f32) -> usize {
        if self.nodes.is_empty() {
            return 0;
        }
        let mut idx = (self.nodes.len() - 1) as u16;
        let mut d = 0;

        while idx & 0x8000 == 0 && d < 100 {
            let n = match self.nodes.get(idx as usize) {
                Some(node) => node,
                None => {
                    log::error!("find_subsector: Invalid node index {}", idx);
                    return 0;
                }
            };

            let side = (x - n.x) * n.dy - (y - n.y) * n.dx;
            // side <= 0 means LEFT side (Back) -> child_left
            // side > 0 means RIGHT side (Front) -> child_right
            // (Based on testing: previous logic was inverted and caused 100% wrong sector)
            let child = if side <= 0.0 {
                n.child_left
            } else {
                n.child_right
            };
            idx = child;
            d += 1;
        }

        (idx & 0x7FFF) as usize
    }

    pub fn intersect(p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2) -> Option<Vec2> {
        let den = (p1.x - p2.x) * (p3.y - p4.y) - (p1.y - p2.y) * (p3.x - p4.x);
        if den.abs() < 0.0001 {
            return None;
        }
        let t = ((p1.x - p3.x) * (p3.y - p4.y) - (p1.y - p3.y) * (p3.x - p4.x)) / den;
        let u = -((p1.x - p2.x) * (p1.y - p3.y) - (p1.y - p2.y) * (p1.x - p3.x)) / den;
        if t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0 {
            Some(p1 + t * (p2 - p1))
        } else {
            None
        }
    }

    pub fn closest_point_on_segment(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
        let ab = b - a;
        let l_sq = ab.length_squared();
        if l_sq < 0.0001 {
            return a;
        }
        let t = ((p - a).dot(ab) / l_sq).clamp(0.0, 1.0);
        a + ab * t
    }

    pub fn add_test_room(&mut self) {
        let start = self.vertices.len();
        self.vertices.extend([
            Vertex::new(-100., -100.),
            Vertex::new(100., -100.),
            Vertex::new(100., 100.),
            Vertex::new(-100., 100.),
        ]);
        let sid = self.sectors.len();
        self.sectors.push(Sector {
            floor_height: 0.,
            ceiling_height: 128.,
            light_level: 1.,
            texture_floor: "FLR".into(),
            texture_ceiling: "CEIL".into(),
            tag: 0,
            action: SectorAction::None,
            special_type: 0,
            secret_found: false,
        });
        for i in 0..4 {
            let front = Some(Sidedef {
                texture_middle: None,
                texture_upper: None,
                texture_lower: None,
                sector_idx: sid,
                x_offset: 0.0,
                y_offset: 0.0,
            });
            self.linedefs.push(LineDefinition {
                start_idx: start + i,
                end_idx: start + (i + 1) % 4,
                sector_front: Some(sid),
                sector_back: None,
                front,
                back: None,
                special_type: 0,
                sector_tag: 0,
                flags: 0,
                activated: false,
            });
        }
    }

    /// Get animated flat name based on game time
    /// Doom animates flats by cycling through NUKAGE1-3, FWATER1-4, SLIME1-4, etc.
    pub fn get_animated_flat_name(&self, base_name: &str) -> String {
        // Animation cycles every 8 tics (about 0.23 seconds at 35Hz)
        let anim_frame = (self.frame_count / 8) as usize;

        match base_name {
            // Nukage (lava) - 3 frames
            "NUKAGE1" | "NUKAGE2" | "NUKAGE3" => {
                let frames = ["NUKAGE1", "NUKAGE2", "NUKAGE3"];
                frames[anim_frame % 3].to_string()
            }
            // Water - 4 frames
            "FWATER1" | "FWATER2" | "FWATER3" | "FWATER4" => {
                let frames = ["FWATER1", "FWATER2", "FWATER3", "FWATER4"];
                frames[anim_frame % 4].to_string()
            }
            // Slime - 4 frames
            "SLIME1" | "SLIME2" | "SLIME3" | "SLIME4" => {
                let frames = ["SLIME1", "SLIME2", "SLIME3", "SLIME4"];
                frames[anim_frame % 4].to_string()
            }
            // Blood - 2 frames
            "BLOOD1" | "BLOOD2" | "BLOOD3" => {
                let frames = ["BLOOD1", "BLOOD2", "BLOOD3"];
                frames[anim_frame % 3].to_string()
            }
            // Lava (shareware) - 3 frames
            "LAVA1" | "LAVA2" | "LAVA3" | "LAVA4" => {
                let frames = ["LAVA1", "LAVA2", "LAVA3", "LAVA4"];
                frames[anim_frame % 4].to_string()
            }
            // Default: no animation
            _ => base_name.to_string(),
        }
    }

    /// Get animated wall texture name
    /// Doom animates certain wall textures like switches and computer screens
    pub fn get_animated_wall_name(&self, base_name: &str) -> String {
        let anim_frame = (self.frame_count / 8) as usize;

        match base_name {
            // Computer screens - 2 frames
            "COMPSTA1" | "COMPSTA2" => {
                if anim_frame % 2 == 0 {
                    "COMPSTA1".to_string()
                } else {
                    "COMPSTA2".to_string()
                }
            }
            // Red computer - 2 frames
            "COMP2A" | "COMP2B" => {
                if anim_frame % 2 == 0 {
                    "COMP2A".to_string()
                } else {
                    "COMP2B".to_string()
                }
            }
            // Blue computer - 2 frames
            "COMP2C" | "COMP2D" => {
                if anim_frame % 2 == 0 {
                    "COMP2C".to_string()
                } else {
                    "COMP2D".to_string()
                }
            }
            // Firestick/tech lamps - 2 frames (glowing)
            "LITE3" | "LITE4" => {
                if anim_frame % 2 == 0 {
                    "LITE3".to_string()
                } else {
                    "LITE4".to_string()
                }
            }
            // Default: no animation
            _ => base_name.to_string(),
        }
    }

    /// Get switch texture name based on state
    /// SW1* = off state, SW2* = on state
    pub fn get_switch_name(&self, base_name: &str, activated: bool) -> String {
        // If base is SW1*, return SW2* when activated
        // If base is SW2*, return SW1* when activated
        if base_name.starts_with("SW1") {
            if activated {
                base_name.replace("SW1", "SW2")
            } else {
                base_name.to_string()
            }
        } else if base_name.starts_with("SW2") {
            if activated {
                base_name.replace("SW2", "SW1")
            } else {
                base_name.to_string()
            }
        } else {
            base_name.to_string()
        }
    }
}
