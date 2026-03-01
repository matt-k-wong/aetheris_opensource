use aetheris::simulation::*;
use glam::{Mat2, Vec2};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterAction {
    Look,
    Chase,
    FaceTarget,
    PosAttack,
    SPosAttack,
    TroopAttack,
    SargAttack,
    HeadAttack,
    BruisAttack,
    SkelMissile,
    FatAttack,
    VileChase,
    VileAttack,
    PainAttack,
    Pain,
    Scream,
    Fall,
    Explode,
    Raise,
    SkullAttack,
}

#[derive(Clone, Copy)]
pub struct MobjState {
    pub sprite: &'static str,
    pub frame: char,
    pub duration: i32,
    pub action: Option<MonsterAction>,
    pub next_state: usize,
}

#[derive(Clone, Copy)]
pub struct ThingDef {
    pub health: f32,
    pub speed: f32,
    pub radius: f32,
    pub height: f32,
    pub damage: i32,
    pub reaction_time: i32,
    pub pain_chance: u8,
    pub mass: i32,
}

pub struct PuffThinker {
    pub position: glam::Vec2,
    pub timer: i32,
}
impl Thinker for PuffThinker {
    fn update(&mut self, _: &WorldState) -> (bool, Vec<WorldCommand>) {
        self.timer -= 1;
        (self.timer > 0, vec![])
    }
    fn on_pain(&mut self, _: usize, _: u16, _: Option<usize>, _: Option<u16>) {}
    fn on_wake(&mut self, _: usize) {}
}

pub struct ProjectileThinker {
    pub thing_idx: usize,
    pub position: Vertex,
    pub z: f32,
    pub velocity: Vec2,
    pub z_velocity: f32,
    pub damage: f32,
    pub owner_is_player: bool,
    pub owner_thing_idx: Option<usize>,
}

const PROJECTILE_RADIUS: f32 = 10.0;

impl Thinker for ProjectileThinker {
    fn update(&mut self, world: &WorldState) -> (bool, Vec<WorldCommand>) {
        let speed = self.velocity.length();
        let steps = (speed / PROJECTILE_RADIUS).ceil().max(1.0) as u32;
        let step_vec = self.velocity / steps as f32;
        let z_step = self.z_velocity / steps as f32;

        let mut current_pos = self.position;
        let mut current_z = self.z;
        let mut cmds = Vec::new();

        for _ in 0..steps {
            let next = current_pos + step_vec;
            let next_z = current_z + z_step;
            cmds.push(WorldCommand::ModifyThing {
                thing_idx: self.thing_idx,
                pos_delta: step_vec,
                z_delta: z_step,
                angle: 0.0,
            });

            for line in &world.linedefs {
                if line.is_portal() {
                    continue;
                }
                let start = match world.vertices.get(line.start_idx) {
                    Some(v) => *v,
                    None => continue,
                };
                let end = match world.vertices.get(line.end_idx) {
                    Some(v) => *v,
                    None => continue,
                };

                let closest = WorldState::closest_point_on_segment(next, start, end);
                if (next - closest).length() < PROJECTILE_RADIUS {
                    cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                        sound_id: "DSBAREXP".into(),
                        position: Some(closest),
                        volume: 1.0,
                    }));
                    cmds.push(WorldCommand::SetThingHealth {
                        thing_idx: self.thing_idx,
                        health: 0.0,
                    });

                    if self.damage >= 20.0 {
                        cmds.push(WorldCommand::SplashDamage {
                            center: closest,
                            damage: 128.0,
                            radius: 128.0,
                            owner_is_player: self.owner_is_player,
                        });
                        cmds.push(WorldCommand::SpawnThing {
                            kind: 9999,
                            position: closest,
                            z: next_z,
                            angle: 0.0,
                        });

                        if self.damage >= 100.0 {
                            let bfg_origin = self
                                .owner_thing_idx
                                .map(|idx| world.things.get(idx).map(|t| t.position))
                                .flatten()
                                .unwrap_or(closest);
                            for i in 0..40 {
                                let angle_offset =
                                    (i as f32 / 40.0 - 0.5) * std::f32::consts::PI * 0.5;
                                let tracer_angle =
                                    (closest - bfg_origin).y.atan2((closest - bfg_origin).x)
                                        + angle_offset
                                        + (rand::random::<f32>() - 0.5) * 0.2;
                                cmds.push(WorldCommand::FireHitscan {
                                    origin: closest,
                                    angle: tracer_angle,
                                    damage: 15.0,
                                    attacker_idx: self.owner_thing_idx,
                                });
                            }
                        }
                    } else {
                        cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                            sound_id: "DSOWHIT".into(),
                            position: Some(closest),
                            volume: 0.5,
                        }));
                        cmds.push(WorldCommand::SpawnThing {
                            kind: 9998,
                            position: closest,
                            z: next_z,
                            angle: 0.0,
                        });
                    }
                    return (false, cmds);
                }
            }
            if self.owner_is_player {
                for (i, t) in world.things.iter().enumerate() {
                    if (t.is_monster() || t.is_barrel())
                        && !t.picked_up
                        && t.health > 0.0
                        && (next - t.position).length() < 20.0
                        && (next_z - t.z).abs() < 40.0
                    {
                        cmds.push(WorldCommand::DamageThing {
                            thing_idx: i,
                            amount: self.damage,
                            inflictor_idx: self.owner_thing_idx,
                        });
                        cmds.push(WorldCommand::InflictPain {
                            thing_idx: i,
                            inflictor_idx: self.owner_thing_idx,
                        });
                        cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                            sound_id: "DSPOPAIN".into(),
                            position: Some(t.position),
                            volume: 1.0,
                        }));
                        cmds.push(WorldCommand::SetThingHealth {
                            thing_idx: self.thing_idx,
                            health: 0.0,
                        });

                        if self.damage >= 20.0 {
                            cmds.push(WorldCommand::SplashDamage {
                                center: t.position,
                                damage: 128.0,
                                radius: 128.0,
                                owner_is_player: true,
                            });
                            let b_kind = if t.kind == 3003 || t.kind == 3005 {
                                9997
                            } else {
                                9999
                            };
                            cmds.push(WorldCommand::SpawnThing {
                                kind: b_kind,
                                position: t.position,
                                z: t.z + 20.0,
                                angle: 0.0,
                            });
                        }
                        return (false, cmds);
                    }
                }
            } else {
                for (i, t) in world.things.iter().enumerate() {
                    if i == self.thing_idx {
                        continue;
                    }
                    if let Some(owner) = self.owner_thing_idx {
                        if i == owner {
                            continue;
                        }
                    }

                    if (t.is_monster() || t.is_barrel())
                        && !t.picked_up
                        && t.health > 0.0
                        && (next - t.position).length() < 20.0
                        && (next_z - t.z).abs() < 40.0
                    {
                        cmds.push(WorldCommand::DamageThing {
                            thing_idx: i,
                            amount: self.damage,
                            inflictor_idx: self.owner_thing_idx,
                        });
                        cmds.push(WorldCommand::InflictPain {
                            thing_idx: i,
                            inflictor_idx: self.owner_thing_idx,
                        });
                        cmds.push(WorldCommand::SetThingHealth {
                            thing_idx: self.thing_idx,
                            health: 0.0,
                        });
                        return (false, cmds);
                    }
                }

                if (next - world.player.position).length() < 20.0
                    && (next_z - world.player.z).abs() < 40.0
                {
                    cmds.push(WorldCommand::DamagePlayer {
                        amount: self.damage,
                        angle: Some(
                            (world.player.position - next)
                                .y
                                .atan2((world.player.position - next).x),
                        ),
                    });
                    cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                        sound_id: "DSPLPAIN".into(),
                        position: Some(world.player.position),
                        volume: 1.0,
                    }));
                    cmds.push(WorldCommand::SetThingHealth {
                        thing_idx: self.thing_idx,
                        health: 0.0,
                    });
                    return (false, cmds);
                }
            }
            current_pos = next;
            current_z = next_z;
        }
        self.position = current_pos;
        self.z = current_z;
        (true, cmds)
    }

    fn on_pain(
        &mut self,
        _target_idx: usize,
        _target_kind: u16,
        _inflictor_idx: Option<usize>,
        _inflictor_kind: Option<u16>,
    ) {
    }
    fn on_wake(&mut self, _thing_idx: usize) {}
}

use aetheris::simulation::*;
use std::collections::HashSet;

// Monster Type Constants
pub const MONSTER_IMP: u16 = 3001;
pub const MONSTER_DEMON: u16 = 3002;
pub const MONSTER_BARON: u16 = 3003;
pub const MONSTER_ZOMBIEMAN: u16 = 3004;
pub const MONSTER_CACODEMON: u16 = 3005;
pub const MONSTER_LOST_SOUL: u16 = 3006;
pub const MONSTER_SERGEANT: u16 = 9;

// Weapon/Item Type Constants
pub const ITEM_SHOTGUN: u16 = 2001;
pub const ITEM_CHAINGUN: u16 = 2002;
pub const ITEM_ROCKET_LAUNCHER: u16 = 2003;
pub const ITEM_PLASMA_RIFLE: u16 = 2004;
pub const ITEM_CHAINSAW: u16 = 2005;
pub const ITEM_BFG9000: u16 = 2006;
pub const ITEM_CLIP: u16 = 2007;
pub const ITEM_SHELLS: u16 = 2008;
pub const ITEM_ROCKETS: u16 = 2010;
pub const ITEM_STIMPACK: u16 = 2011;
pub const ITEM_MEDIKIT: u16 = 2012;
pub const ITEM_SOULSPHERE: u16 = 2013;
pub const ITEM_HEALTH_BONUS: u16 = 2014;
pub const ITEM_ARMOR_BONUS: u16 = 2015;
pub const ITEM_GREEN_ARMOR: u16 = 2018;
pub const ITEM_BLUE_ARMOR: u16 = 2019;
pub const ITEM_INVULN: u16 = 2022;
pub const ITEM_BERSERK: u16 = 2023;
pub const ITEM_INVIS: u16 = 2024;
pub const ITEM_RADSUIT: u16 = 2025;
pub const ITEM_MAP: u16 = 2026;

// Key Type Constants
pub const KEY_BLUE: u16 = 5;
pub const KEY_YELLOW: u16 = 6;
pub const KEY_RED: u16 = 13;
pub const KEY_BLUE_SKULL: u16 = 40;
pub const KEY_YELLOW_SKULL: u16 = 39;
pub const KEY_RED_SKULL: u16 = 38;

// Effect/Projectile Type Constants
pub const EFFECT_BLOOD: u16 = 9999;
pub const EFFECT_BLOOD_GREEN: u16 = 9997;
pub const EFFECT_PUFF: u16 = 9998;

// Doom-style RNG and Pain Chance
// Doom uses a 256-byte lookup table with a prng index that advances each call
static mut PRND_INDEX: usize = 0;

#[rustfmt::skip]
const PRND_TABLE: [u8; 256] = [
    0,   8, 109, 220, 222, 241, 149, 107,  75, 248, 254, 140,  16,  66,  74,  21,
    211,  47,  80, 242, 154,  27, 205, 128, 161,  89,  77,  36,  95, 110,  85,  46,
    114, 163, 182, 232, 198,   6, 128,  91,  76, 179,  88,  24, 104,  63, 148, 161,
    194,  16,  33, 101,  32,  89, 157,  87,  38,  55,  78,  22,  42, 143, 160,  18,
    38,  38, 125, 198,  13,  53,  86, 127, 156,  40,  74, 202,  79,  27,  87,  83,
    33, 183,  93,  40,  78,  84,  73, 208, 218,  89, 227,  68,  57,  11,  24,  86,
    2,  60,  95,  10, 183,  90,  62,  18,  51,  50,  72, 168,  91,  54,  61,  89,
    79, 168, 120, 156,  82,  34,  33,  20,  47,  79,  14,  46,  18,  90,  62,  26,
    20,  77,  21,  73,  82, 117,  86,  33,  40, 192, 205,  88,  78,  51,  12, 254,
    236, 223,  76,  52, 194,  28, 229,  40, 152,  24,  77, 239,  51,  25,  77,  59,
    30, 162,  36, 223,  68,  44,  20, 133, 106, 137,  76,  41,  84,  26,  44, 146,
    73, 103,  84, 144, 107,  75, 101,  60, 154, 105,  33,  13,  12, 255, 190, 255,
    28, 219,  14,  19,  22,  11,  91,  17,  24, 204, 139,  71, 141, 108, 146, 214,
    121,  64, 168, 148, 176, 248, 181, 197,  55, 233,  43,  60, 233, 242,  77, 205,
    203,  83,  28,  11,  83,  93,  76,  32,  11, 129,  66,  64,  71, 135, 167,  40,
    229,  89, 201, 110,  21,  12,  97,  93, 102, 128, 153, 223, 183,  55,  36, 134,
];

/// Doom's P_Random - returns 0-255 using lookup table
pub fn p_random() -> u8 {
    unsafe {
        let val = PRND_TABLE[PRND_INDEX];
        PRND_INDEX = (PRND_INDEX + 1) % 256;
        val
    }
}

/// Reset the RNG to a known state for testing
pub fn reset_rng() {
    unsafe {
        PRND_INDEX = 0;
    }
}

/// Reset the RNG to a specific state for deterministic testing
pub fn reset_rng_to(index: usize) {
    unsafe {
        PRND_INDEX = index % 256;
    }
}

/// Pain chance values (0-255) for each monster type
/// Higher = more likely to enter pain state when hit
// Helper removed in favor of Thing::pain_chance
/*
pub fn pain_chance_for_kind(kind: u16) -> u8 {
    match kind {
        3001 => 128, // Imp: 50% chance
        3002 => 180, // Demon: ~70% chance
        3003 => 50,  // Baron: ~20% chance
        3004 => 200, // Zombieman: ~78% chance
        3005 => 128, // Cacodemon: 50% chance
        3006 => 255, // Lost Soul: always (100%)
        9 => 170,    // Sergeant: ~66% chance
        _ => 100,    // Default: ~39% chance
    }
}
*/

// Doom-specific Thing methods
pub trait DoomThingExt {
    fn is_monster(&self) -> bool;
    fn is_flying(&self) -> bool;
    fn is_pickup(&self) -> bool;
    fn is_barrel(&self) -> bool;
    fn is_effect(&self) -> bool;
    fn initial_health(k: u16, world: &WorldState) -> f32;
    fn pain_chance(k: u16, world: &WorldState) -> u8;
    fn sprite_name<'a>(&self, world: &'a WorldState) -> &'a str;
    fn frame_char(&self, world: &WorldState) -> char;
}

impl DoomThingExt for Thing {
    fn is_monster(&self) -> bool {
        matches!(self.kind,
            7 | 9 | 16 |               // Spiderdemon, Shotgun Guy, Cyberdemon
            3001..=3006 |               // Imp, Demon, Baron, Zombieman, Cacodemon, Lost Soul
            64..=69 | 71 | 84           // Archvile, Chaingunner, Revenant, Mancubus, Arachnotron, Hell Knight, Pain Elemental, WolfSS
        )
    }
    fn is_flying(&self) -> bool {
        matches!(self.kind, 3005 | 3006) // Cacodemon, Lost Soul
    }
    fn is_pickup(&self) -> bool {
        matches!(self.kind, 2001..=2008 | 2010..=2015 | 2018..=2019 | 2022..=2026 | 2045..=2049 | 5..=6 | 13 | 17 | 38..=40)
    }
    fn is_barrel(&self) -> bool {
        self.kind == 2035
    }
    fn is_effect(&self) -> bool {
        matches!(self.kind, 9997 | 9998 | 9999)
    }
    fn initial_health(_k: u16, _world: &WorldState) -> f32 {
        DEFAULT_THING_DEFS
            .iter()
            .find(|&&(k, _)| k == _k)
            .map(|&(_, d)| d.health)
            .unwrap_or(100.0)
    }
    fn pain_chance(_k: u16, _world: &WorldState) -> u8 {
        DEFAULT_THING_DEFS
            .iter()
            .find(|&&(k, _)| k == _k)
            .map(|&(_, d)| d.pain_chance)
            .unwrap_or(0)
    }
    fn sprite_name<'a>(&self, _world: &'a WorldState) -> &'a str {
        if self.state_idx < STATES.len() {
            STATES[self.state_idx].sprite
        } else {
            "TROO"
        }
    }
    fn frame_char(&self, _world: &WorldState) -> char {
        if self.state_idx < STATES.len() {
            STATES[self.state_idx].frame
        } else {
            'A'
        }
    }
}

// Doom Monster AI
// MobjState and MonsterAction moved to engine.rs

pub struct MonsterThinker {
    pub thing_idx: usize,
    pub state_idx: usize,
    pub tics: i32,
    pub target_thing_idx: Option<usize>,
    pub attack_cooldown: u32,     // Cooldown between attacks to prevent spam
    pub just_entered_state: bool, // True when state was just set, action should fire
}

pub const S_NULL: usize = 0;
// Zombieman States
pub const S_POSS_STND: usize = 1;
pub const S_POSS_RUN: usize = 3;
pub const S_POSS_ATK: usize = 11;
pub const S_POSS_PAIN: usize = 15;
pub const S_POSS_DIE: usize = 17;
// Imp States
pub const S_TROO_STND: usize = 22;
pub const S_TROO_RUN: usize = 24;
pub const S_TROO_ATK: usize = 32;
pub const S_TROO_PAIN: usize = 38;
pub const S_TROO_DIE: usize = 40;
// Lost Soul States
pub const S_SKULL_STND: usize = 44;
pub const S_SKULL_RUN: usize = 46;
pub const S_SKULL_ATK: usize = 48;
pub const S_SKULL_PAIN: usize = 49;
pub const S_SKULL_DIE: usize = 50;

// Barrel States
pub const S_BAR1: usize = 56;
pub const S_BEXP: usize = 58;

// Doom 2 Monster States
pub const S_CPOS_STND: usize = 63;
pub const S_CPOS_RUN: usize = 65;
pub const S_CPOS_ATK: usize = 71;
pub const S_CPOS_PAIN: usize = 74;
pub const S_CPOS_DIE: usize = 75;

pub const S_SKEL_STND: usize = 80;
pub const S_SKEL_RUN: usize = 82;
pub const S_SKEL_ATK: usize = 88;
pub const S_SKEL_PAIN: usize = 91;
pub const S_SKEL_DIE: usize = 92;

pub const S_FATT_STND: usize = 98;
pub const S_FATT_RUN: usize = 100;
pub const S_FATT_ATK: usize = 106;
pub const S_FATT_PAIN: usize = 109;
pub const S_FATT_DIE: usize = 110;

pub const S_BSPI_STND: usize = 116;
pub const S_BSPI_RUN: usize = 118;
pub const S_BSPI_ATK: usize = 124;
pub const S_BSPI_PAIN: usize = 127;
pub const S_BSPI_DIE: usize = 128;

pub const S_BOS2_STND: usize = 134;
pub const S_BOS2_RUN: usize = 136;
pub const S_BOS2_ATK: usize = 142;
pub const S_BOS2_PAIN: usize = 145;
pub const S_BOS2_DIE: usize = 146;

pub const S_PAIN_STND: usize = 152;
pub const S_PAIN_RUN: usize = 154;
pub const S_PAIN_ATK: usize = 160;
pub const S_PAIN_PAIN: usize = 163;
pub const S_PAIN_DIE: usize = 164;

pub const S_VILE_STND: usize = 170;
pub const S_VILE_RUN: usize = 172;
pub const S_VILE_ATK: usize = 178;
pub const S_VILE_PAIN: usize = 181;
pub const S_VILE_DIE: usize = 182;

pub const S_SPID_STND: usize = 188;
pub const S_SPID_RUN: usize = 190;
pub const S_SPID_ATK: usize = 196;
pub const S_SPID_PAIN: usize = 199;
pub const S_SPID_DIE: usize = 200;

pub const S_CYBR_STND: usize = 206;
pub const S_CYBR_RUN: usize = 208;
pub const S_CYBR_ATK: usize = 214;
pub const S_CYBR_PAIN: usize = 217;
pub const S_CYBR_DIE: usize = 218;

pub const S_SSWV_STND: usize = 224;
pub const S_SSWV_RUN: usize = 226;
pub const S_SSWV_ATK: usize = 232;
pub const S_SSWV_PAIN: usize = 235;
pub const S_SSWV_DIE: usize = 236;

// Shotgun Guy (SPOS) — separate sprites from Zombieman
pub const S_SPOS_STND: usize = 240;
pub const S_SPOS_RUN: usize = 242;
pub const S_SPOS_ATK: usize = 250;
pub const S_SPOS_PAIN: usize = 254;
pub const S_SPOS_DIE: usize = 256;

// Demon/Pinky (SARG)
pub const S_SARG_STND: usize = 261;
pub const S_SARG_RUN: usize = 263;
pub const S_SARG_ATK: usize = 271;
pub const S_SARG_PAIN: usize = 274;
pub const S_SARG_DIE: usize = 276;

// Cacodemon (HEAD)
pub const S_HEAD_STND: usize = 282;
pub const S_HEAD_RUN: usize = 284;
pub const S_HEAD_ATK: usize = 290;
pub const S_HEAD_PAIN: usize = 294;
pub const S_HEAD_DIE: usize = 296;

// Baron of Hell (BOSS)
pub const S_BOSS_STND: usize = 302;
pub const S_BOSS_RUN: usize = 304;
pub const S_BOSS_ATK: usize = 308;
pub const S_BOSS_PAIN: usize = 312;
pub const S_BOSS_DIE: usize = 314;

pub const STATES: &[MobjState] = DEFAULT_STATES;

pub const DEFAULT_STATES: &[MobjState] = &[
    /* 0 S_NULL */
    MobjState {
        sprite: "TNT1",
        frame: 'A',
        duration: -1,
        action: None,
        next_state: 0,
    },
    /* 1 S_POSS_STND */
    MobjState {
        sprite: "POSS",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 2,
    },
    /* 2 */
    MobjState {
        sprite: "POSS",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 1,
    },
    /* 3 S_POSS_RUN */
    MobjState {
        sprite: "POSS",
        frame: 'A',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 4,
    },
    /* 4 */
    MobjState {
        sprite: "POSS",
        frame: 'A',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 5,
    },
    /* 5 */
    MobjState {
        sprite: "POSS",
        frame: 'B',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 6,
    },
    /* 6 */
    MobjState {
        sprite: "POSS",
        frame: 'B',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 7,
    },
    /* 7 */
    MobjState {
        sprite: "POSS",
        frame: 'C',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 8,
    },
    /* 8 */
    MobjState {
        sprite: "POSS",
        frame: 'C',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 9,
    },
    /* 9 */
    MobjState {
        sprite: "POSS",
        frame: 'D',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 10,
    },
    /* 10 */
    MobjState {
        sprite: "POSS",
        frame: 'D',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 3,
    },
    /* 11 S_POSS_ATK */
    MobjState {
        sprite: "POSS",
        frame: 'E',
        duration: 10,
        action: Some(MonsterAction::FaceTarget),
        next_state: 12,
    },
    /* 12 */
    MobjState {
        sprite: "POSS",
        frame: 'F',
        duration: 8,
        action: Some(MonsterAction::PosAttack),
        next_state: 13,
    },
    /* 13 */
    MobjState {
        sprite: "POSS",
        frame: 'E',
        duration: 8,
        action: None,
        next_state: 3,
    }, // Back to chase
    /* 14 */
    MobjState {
        sprite: "POSS",
        frame: 'G',
        duration: 3,
        action: None,
        next_state: 15,
    },
    /* 15 S_POSS_PAIN */
    MobjState {
        sprite: "POSS",
        frame: 'G',
        duration: 3,
        action: Some(MonsterAction::Pain),
        next_state: 3,
    },
    /* 16 */
    MobjState {
        sprite: "POSS",
        frame: 'H',
        duration: 5,
        action: None,
        next_state: 17,
    },
    /* 17 S_POSS_DIE */
    MobjState {
        sprite: "POSS",
        frame: 'I',
        duration: 5,
        action: Some(MonsterAction::Scream),
        next_state: 18,
    },
    /* 18 */
    MobjState {
        sprite: "POSS",
        frame: 'J',
        duration: 5,
        action: Some(MonsterAction::Fall),
        next_state: 19,
    },
    /* 19 */
    MobjState {
        sprite: "POSS",
        frame: 'K',
        duration: 5,
        action: None,
        next_state: 20,
    },
    /* 20 */
    MobjState {
        sprite: "POSS",
        frame: 'L',
        duration: -1,
        action: None,
        next_state: 20,
    },
    /* 21 */
    MobjState {
        sprite: "POSS",
        frame: 'M',
        duration: 5,
        action: None,
        next_state: 22,
    }, // Extra check?
    /* 22 S_TROO_STND */
    MobjState {
        sprite: "TROO",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 23,
    },
    /* 23 */
    MobjState {
        sprite: "TROO",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 22,
    },
    /* 24 S_TROO_RUN */
    MobjState {
        sprite: "TROO",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 25,
    },
    /* 25 */
    MobjState {
        sprite: "TROO",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 26,
    },
    /* 26 */
    MobjState {
        sprite: "TROO",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 27,
    },
    /* 27 */
    MobjState {
        sprite: "TROO",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 28,
    },
    /* 28 */
    MobjState {
        sprite: "TROO",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 29,
    },
    /* 29 */
    MobjState {
        sprite: "TROO",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 30,
    },
    /* 30 */
    MobjState {
        sprite: "TROO",
        frame: 'D',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 31,
    },
    /* 31 */
    MobjState {
        sprite: "TROO",
        frame: 'D',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 24,
    },
    /* 32 S_TROO_ATK */
    MobjState {
        sprite: "TROO",
        frame: 'E',
        duration: 8,
        action: Some(MonsterAction::FaceTarget),
        next_state: 33,
    },
    /* 33 */
    MobjState {
        sprite: "TROO",
        frame: 'F',
        duration: 8,
        action: Some(MonsterAction::FaceTarget),
        next_state: 34,
    },
    /* 34 */
    MobjState {
        sprite: "TROO",
        frame: 'G',
        duration: 6,
        action: Some(MonsterAction::TroopAttack),
        next_state: 35,
    },
    /* 35 */
    MobjState {
        sprite: "TROO",
        frame: 'H',
        duration: 2,
        action: None,
        next_state: 36,
    }, // Extra frame?
    /* 36 */
    MobjState {
        sprite: "TROO",
        frame: 'H',
        duration: 2,
        action: None,
        next_state: 24,
    }, // Back to chase
    /* 37 */
    MobjState {
        sprite: "TROO",
        frame: 'H',
        duration: 2,
        action: None,
        next_state: 38,
    },
    /* 38 S_TROO_PAIN */
    MobjState {
        sprite: "TROO",
        frame: 'H',
        duration: 2,
        action: Some(MonsterAction::Pain),
        next_state: 24,
    },
    /* 39 */
    MobjState {
        sprite: "TROO",
        frame: 'I',
        duration: 8,
        action: None,
        next_state: 40,
    },
    /* 40 S_TROO_DIE */
    MobjState {
        sprite: "TROO",
        frame: 'J',
        duration: 8,
        action: Some(MonsterAction::Scream),
        next_state: 41,
    },
    /* 41 */
    MobjState {
        sprite: "TROO",
        frame: 'K',
        duration: 6,
        action: Some(MonsterAction::Fall),
        next_state: 42,
    },
    /* 42 */
    MobjState {
        sprite: "TROO",
        frame: 'L',
        duration: 6,
        action: None,
        next_state: 43,
    },
    /* 43 */
    MobjState {
        sprite: "TROO",
        frame: 'M',
        duration: -1,
        action: None,
        next_state: 43,
    },
    /* 44 S_SKULL_STND */
    MobjState {
        sprite: "SKUL",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 45,
    },
    /* 45 */
    MobjState {
        sprite: "SKUL",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 44,
    },
    /* 46 S_SKULL_RUN */
    MobjState {
        sprite: "SKUL",
        frame: 'A',
        duration: 6,
        action: Some(MonsterAction::Chase),
        next_state: 47,
    },
    /* 47 */
    MobjState {
        sprite: "SKUL",
        frame: 'B',
        duration: 6,
        action: Some(MonsterAction::Chase),
        next_state: 46,
    },
    /* 48 S_SKULL_ATK */
    MobjState {
        sprite: "SKUL",
        frame: 'C',
        duration: 20,
        action: Some(MonsterAction::SkullAttack),
        next_state: 46,
    },
    /* 49 S_SKULL_PAIN */
    MobjState {
        sprite: "SKUL",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Pain),
        next_state: 46,
    },
    /* 50 S_SKULL_DIE */
    MobjState {
        sprite: "SKUL",
        frame: 'F',
        duration: 6,
        action: Some(MonsterAction::Scream),
        next_state: 51,
    },
    /* 51 */
    MobjState {
        sprite: "SKUL",
        frame: 'G',
        duration: 6,
        action: None,
        next_state: 52,
    },
    /* 52 */
    MobjState {
        sprite: "SKUL",
        frame: 'H',
        duration: 6,
        action: Some(MonsterAction::Fall),
        next_state: 53,
    },
    /* 53 */
    MobjState {
        sprite: "SKUL",
        frame: 'I',
        duration: 6,
        action: None,
        next_state: 54,
    },
    /* 54 */
    MobjState {
        sprite: "SKUL",
        frame: 'J',
        duration: 6,
        action: None,
        next_state: 55,
    },
    /* 55 */
    MobjState {
        sprite: "SKUL",
        frame: 'K',
        duration: -1,
        action: None,
        next_state: 55,
    },
    /* 56 S_BAR1 */
    MobjState {
        sprite: "BAR1",
        frame: 'A',
        duration: 10,
        action: None,
        next_state: 57,
    },
    /* 57 */
    MobjState {
        sprite: "BAR1",
        frame: 'B',
        duration: 10,
        action: None,
        next_state: 56,
    },
    /* 58 S_BEXP */
    MobjState {
        sprite: "BEXP",
        frame: 'A',
        duration: 5,
        action: Some(MonsterAction::Explode),
        next_state: 59,
    },
    /* 59 */
    MobjState {
        sprite: "BEXP",
        frame: 'B',
        duration: 5,
        action: None,
        next_state: 60,
    },
    /* 60 */
    MobjState {
        sprite: "BEXP",
        frame: 'C',
        duration: 5,
        action: None,
        next_state: 61,
    },
    /* 61 */
    MobjState {
        sprite: "BEXP",
        frame: 'D',
        duration: 10,
        action: None,
        next_state: 62,
    },
    /* 62 */
    MobjState {
        sprite: "TNT1",
        frame: 'A',
        duration: -1,
        action: None,
        next_state: 62,
    },
    // Chaingunner (CPOS)
    /* 63 */
    MobjState {
        sprite: "CPOS",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 64,
    },
    /* 64 */
    MobjState {
        sprite: "CPOS",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 63,
    },
    /* 65 */
    MobjState {
        sprite: "CPOS",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 66,
    },
    /* 66 */
    MobjState {
        sprite: "CPOS",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 67,
    },
    /* 67 */
    MobjState {
        sprite: "CPOS",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 68,
    },
    /* 68 */
    MobjState {
        sprite: "CPOS",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 69,
    },
    /* 69 */
    MobjState {
        sprite: "CPOS",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 70,
    },
    /* 70 */
    MobjState {
        sprite: "CPOS",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 65,
    },
    /* 71 */
    MobjState {
        sprite: "CPOS",
        frame: 'E',
        duration: 10,
        action: Some(MonsterAction::FaceTarget),
        next_state: 72,
    },
    /* 72 */
    MobjState {
        sprite: "CPOS",
        frame: 'F',
        duration: 4,
        action: Some(MonsterAction::PosAttack),
        next_state: 73,
    },
    /* 73 */
    MobjState {
        sprite: "CPOS",
        frame: 'F',
        duration: 4,
        action: Some(MonsterAction::PosAttack),
        next_state: 65,
    },
    /* 74 */
    MobjState {
        sprite: "CPOS",
        frame: 'G',
        duration: 3,
        action: Some(MonsterAction::Pain),
        next_state: 65,
    },
    /* 75 */
    MobjState {
        sprite: "CPOS",
        frame: 'H',
        duration: 5,
        action: None,
        next_state: 76,
    },
    /* 76 */
    MobjState {
        sprite: "CPOS",
        frame: 'I',
        duration: 5,
        action: Some(MonsterAction::Scream),
        next_state: 77,
    },
    /* 77 */
    MobjState {
        sprite: "CPOS",
        frame: 'J',
        duration: 5,
        action: Some(MonsterAction::Fall),
        next_state: 78,
    },
    /* 78 */
    MobjState {
        sprite: "CPOS",
        frame: 'K',
        duration: 5,
        action: None,
        next_state: 79,
    },
    /* 79 */
    MobjState {
        sprite: "CPOS",
        frame: 'L',
        duration: -1,
        action: None,
        next_state: 79,
    },
    // Revenant (SKEL)
    /* 80 */
    MobjState {
        sprite: "SKEL",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 81,
    },
    /* 81 */
    MobjState {
        sprite: "SKEL",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 80,
    },
    /* 82 */
    MobjState {
        sprite: "SKEL",
        frame: 'A',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 83,
    },
    /* 83 */
    MobjState {
        sprite: "SKEL",
        frame: 'B',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 84,
    },
    /* 84 */
    MobjState {
        sprite: "SKEL",
        frame: 'C',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 85,
    },
    /* 85 */
    MobjState {
        sprite: "SKEL",
        frame: 'D',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 86,
    },
    /* 86 */
    MobjState {
        sprite: "SKEL",
        frame: 'E',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 87,
    },
    /* 87 */
    MobjState {
        sprite: "SKEL",
        frame: 'F',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 82,
    },
    /* 88 */
    MobjState {
        sprite: "SKEL",
        frame: 'J',
        duration: 10,
        action: Some(MonsterAction::FaceTarget),
        next_state: 89,
    },
    /* 89 */
    MobjState {
        sprite: "SKEL",
        frame: 'K',
        duration: 10,
        action: Some(MonsterAction::TroopAttack),
        next_state: 90,
    },
    /* 90 */
    MobjState {
        sprite: "SKEL",
        frame: 'K',
        duration: 10,
        action: None,
        next_state: 82,
    },
    /* 91 */
    MobjState {
        sprite: "SKEL",
        frame: 'L',
        duration: 5,
        action: Some(MonsterAction::Pain),
        next_state: 82,
    },
    /* 92 */
    MobjState {
        sprite: "SKEL",
        frame: 'L',
        duration: 5,
        action: None,
        next_state: 93,
    },
    /* 93 */
    MobjState {
        sprite: "SKEL",
        frame: 'M',
        duration: 5,
        action: Some(MonsterAction::Scream),
        next_state: 94,
    },
    /* 94 */
    MobjState {
        sprite: "SKEL",
        frame: 'N',
        duration: 5,
        action: Some(MonsterAction::Fall),
        next_state: 95,
    },
    /* 95 */
    MobjState {
        sprite: "SKEL",
        frame: 'O',
        duration: 5,
        action: None,
        next_state: 96,
    },
    /* 96 */
    MobjState {
        sprite: "SKEL",
        frame: 'P',
        duration: -1,
        action: None,
        next_state: 96,
    },
    /* 97 */
    MobjState {
        sprite: "SKEL",
        frame: 'Q',
        duration: -1,
        action: None,
        next_state: 97,
    },
    // Mancubus (FATT)
    /* 98 */
    MobjState {
        sprite: "FATT",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 99,
    },
    /* 99 */
    MobjState {
        sprite: "FATT",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 98,
    },
    /* 100 */
    MobjState {
        sprite: "FATT",
        frame: 'A',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 101,
    },
    /* 101 */
    MobjState {
        sprite: "FATT",
        frame: 'B',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 102,
    },
    /* 102 */
    MobjState {
        sprite: "FATT",
        frame: 'C',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 103,
    },
    /* 103 */
    MobjState {
        sprite: "FATT",
        frame: 'D',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 104,
    },
    /* 104 */
    MobjState {
        sprite: "FATT",
        frame: 'E',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 105,
    },
    /* 105 */
    MobjState {
        sprite: "FATT",
        frame: 'F',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 100,
    },
    /* 106 */
    MobjState {
        sprite: "FATT",
        frame: 'G',
        duration: 20,
        action: Some(MonsterAction::FaceTarget),
        next_state: 107,
    },
    /* 107 */
    MobjState {
        sprite: "FATT",
        frame: 'H',
        duration: 10,
        action: Some(MonsterAction::TroopAttack),
        next_state: 108,
    },
    /* 108 */
    MobjState {
        sprite: "FATT",
        frame: 'I',
        duration: 5,
        action: None,
        next_state: 100,
    },
    /* 109 */
    MobjState {
        sprite: "FATT",
        frame: 'J',
        duration: 3,
        action: Some(MonsterAction::Pain),
        next_state: 100,
    },
    /* 110 */
    MobjState {
        sprite: "FATT",
        frame: 'K',
        duration: 5,
        action: None,
        next_state: 111,
    },
    /* 111 */
    MobjState {
        sprite: "FATT",
        frame: 'L',
        duration: 5,
        action: Some(MonsterAction::Scream),
        next_state: 112,
    },
    /* 112 */
    MobjState {
        sprite: "FATT",
        frame: 'M',
        duration: 5,
        action: Some(MonsterAction::Fall),
        next_state: 113,
    },
    /* 113 */
    MobjState {
        sprite: "FATT",
        frame: 'N',
        duration: 5,
        action: None,
        next_state: 114,
    },
    /* 114 */
    MobjState {
        sprite: "FATT",
        frame: 'O',
        duration: -1,
        action: None,
        next_state: 114,
    },
    /* 115 */
    MobjState {
        sprite: "FATT",
        frame: 'P',
        duration: -1,
        action: None,
        next_state: 115,
    },
    // Arachnotron (BSPI)
    /* 116 */
    MobjState {
        sprite: "BSPI",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 117,
    },
    /* 117 */
    MobjState {
        sprite: "BSPI",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 116,
    },
    /* 118 */
    MobjState {
        sprite: "BSPI",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 119,
    },
    /* 119 */
    MobjState {
        sprite: "BSPI",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 120,
    },
    /* 120 */
    MobjState {
        sprite: "BSPI",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 121,
    },
    /* 121 */
    MobjState {
        sprite: "BSPI",
        frame: 'D',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 122,
    },
    /* 122 */
    MobjState {
        sprite: "BSPI",
        frame: 'E',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 123,
    },
    /* 123 */
    MobjState {
        sprite: "BSPI",
        frame: 'F',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 118,
    },
    /* 124 */
    MobjState {
        sprite: "BSPI",
        frame: 'A',
        duration: 20,
        action: Some(MonsterAction::FaceTarget),
        next_state: 125,
    },
    /* 125 */
    MobjState {
        sprite: "BSPI",
        frame: 'G',
        duration: 4,
        action: Some(MonsterAction::TroopAttack),
        next_state: 126,
    },
    /* 126 */
    MobjState {
        sprite: "BSPI",
        frame: 'H',
        duration: 4,
        action: None,
        next_state: 118,
    },
    /* 127 */
    MobjState {
        sprite: "BSPI",
        frame: 'I',
        duration: 3,
        action: Some(MonsterAction::Pain),
        next_state: 118,
    },
    /* 128 */
    MobjState {
        sprite: "BSPI",
        frame: 'J',
        duration: 5,
        action: None,
        next_state: 129,
    },
    /* 129 */
    MobjState {
        sprite: "BSPI",
        frame: 'K',
        duration: 5,
        action: Some(MonsterAction::Scream),
        next_state: 130,
    },
    /* 130 */
    MobjState {
        sprite: "BSPI",
        frame: 'L',
        duration: 5,
        action: Some(MonsterAction::Fall),
        next_state: 131,
    },
    /* 131 */
    MobjState {
        sprite: "BSPI",
        frame: 'M',
        duration: 5,
        action: None,
        next_state: 132,
    },
    /* 132 */
    MobjState {
        sprite: "BSPI",
        frame: 'N',
        duration: -1,
        action: None,
        next_state: 132,
    },
    /* 133 */
    MobjState {
        sprite: "BSPI",
        frame: 'O',
        duration: -1,
        action: None,
        next_state: 133,
    },
    // Hell Knight (BOS2)
    /* 134 */
    MobjState {
        sprite: "BOS2",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 135,
    },
    /* 135 */
    MobjState {
        sprite: "BOS2",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 134,
    },
    /* 136 */
    MobjState {
        sprite: "BOS2",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 137,
    },
    /* 137 */
    MobjState {
        sprite: "BOS2",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 138,
    },
    /* 138 */
    MobjState {
        sprite: "BOS2",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 139,
    },
    /* 139 */
    MobjState {
        sprite: "BOS2",
        frame: 'D',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 136,
    },
    /* 140 */
    MobjState {
        sprite: "BOS2",
        frame: 'A',
        duration: 0,
        action: None,
        next_state: 136,
    },
    /* 141 */
    MobjState {
        sprite: "BOS2",
        frame: 'A',
        duration: 0,
        action: None,
        next_state: 136,
    },
    /* 142 */
    MobjState {
        sprite: "BOS2",
        frame: 'E',
        duration: 8,
        action: Some(MonsterAction::FaceTarget),
        next_state: 143,
    },
    /* 143 */
    MobjState {
        sprite: "BOS2",
        frame: 'F',
        duration: 8,
        action: Some(MonsterAction::FaceTarget),
        next_state: 144,
    },
    /* 144 */
    MobjState {
        sprite: "BOS2",
        frame: 'G',
        duration: 8,
        action: Some(MonsterAction::TroopAttack),
        next_state: 136,
    },
    /* 145 */
    MobjState {
        sprite: "BOS2",
        frame: 'H',
        duration: 2,
        action: Some(MonsterAction::Pain),
        next_state: 136,
    },
    /* 146 */
    MobjState {
        sprite: "BOS2",
        frame: 'I',
        duration: 8,
        action: None,
        next_state: 147,
    },
    /* 147 */
    MobjState {
        sprite: "BOS2",
        frame: 'J',
        duration: 8,
        action: Some(MonsterAction::Scream),
        next_state: 148,
    },
    /* 148 */
    MobjState {
        sprite: "BOS2",
        frame: 'K',
        duration: 8,
        action: Some(MonsterAction::Fall),
        next_state: 149,
    },
    /* 149 */
    MobjState {
        sprite: "BOS2",
        frame: 'L',
        duration: 8,
        action: None,
        next_state: 150,
    },
    /* 150 */
    MobjState {
        sprite: "BOS2",
        frame: 'M',
        duration: -1,
        action: None,
        next_state: 150,
    },
    /* 151 */
    MobjState {
        sprite: "BOS2",
        frame: 'N',
        duration: -1,
        action: None,
        next_state: 151,
    },
    // Pain Elemental (PAIN)
    /* 152 */
    MobjState {
        sprite: "PAIN",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 153,
    },
    /* 153 */
    MobjState {
        sprite: "PAIN",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 152,
    },
    /* 154 */
    MobjState {
        sprite: "PAIN",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 155,
    },
    /* 155 */
    MobjState {
        sprite: "PAIN",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 156,
    },
    /* 156 */
    MobjState {
        sprite: "PAIN",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 154,
    },
    /* 157 */
    MobjState {
        sprite: "PAIN",
        frame: 'A',
        duration: 0,
        action: None,
        next_state: 154,
    },
    /* 158 */
    MobjState {
        sprite: "PAIN",
        frame: 'A',
        duration: 0,
        action: None,
        next_state: 154,
    },
    /* 159 */
    MobjState {
        sprite: "PAIN",
        frame: 'A',
        duration: 0,
        action: None,
        next_state: 154,
    },
    /* 160 */
    MobjState {
        sprite: "PAIN",
        frame: 'D',
        duration: 20,
        action: Some(MonsterAction::FaceTarget),
        next_state: 161,
    },
    /* 161 */
    MobjState {
        sprite: "PAIN",
        frame: 'E',
        duration: 10,
        action: Some(MonsterAction::SkullAttack),
        next_state: 162,
    },
    /* 162 */
    MobjState {
        sprite: "PAIN",
        frame: 'F',
        duration: 10,
        action: None,
        next_state: 154,
    },
    /* 163 */
    MobjState {
        sprite: "PAIN",
        frame: 'G',
        duration: 6,
        action: Some(MonsterAction::Pain),
        next_state: 154,
    },
    /* 164 */
    MobjState {
        sprite: "PAIN",
        frame: 'H',
        duration: 8,
        action: Some(MonsterAction::Scream),
        next_state: 165,
    },
    /* 165 */
    MobjState {
        sprite: "PAIN",
        frame: 'I',
        duration: 8,
        action: Some(MonsterAction::Fall),
        next_state: 166,
    },
    /* 166 */
    MobjState {
        sprite: "PAIN",
        frame: 'J',
        duration: 8,
        action: None,
        next_state: 167,
    },
    /* 167 */
    MobjState {
        sprite: "PAIN",
        frame: 'K',
        duration: -1,
        action: None,
        next_state: 167,
    },
    /* 168 */
    MobjState {
        sprite: "PAIN",
        frame: 'L',
        duration: -1,
        action: None,
        next_state: 168,
    },
    /* 169 */
    MobjState {
        sprite: "PAIN",
        frame: 'M',
        duration: -1,
        action: None,
        next_state: 169,
    },
    // Archvile (VILE)
    /* 170 */
    MobjState {
        sprite: "VILE",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 171,
    },
    /* 171 */
    MobjState {
        sprite: "VILE",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 170,
    },
    /* 172 */
    MobjState {
        sprite: "VILE",
        frame: 'A',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 173,
    },
    /* 173 */
    MobjState {
        sprite: "VILE",
        frame: 'B',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 174,
    },
    /* 174 */
    MobjState {
        sprite: "VILE",
        frame: 'C',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 175,
    },
    /* 175 */
    MobjState {
        sprite: "VILE",
        frame: 'D',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 176,
    },
    /* 176 */
    MobjState {
        sprite: "VILE",
        frame: 'E',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 177,
    },
    /* 177 */
    MobjState {
        sprite: "VILE",
        frame: 'F',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 172,
    },
    /* 178 */
    MobjState {
        sprite: "VILE",
        frame: 'G',
        duration: 20,
        action: Some(MonsterAction::FaceTarget),
        next_state: 179,
    },
    /* 179 */
    MobjState {
        sprite: "VILE",
        frame: 'H',
        duration: 10,
        action: Some(MonsterAction::TroopAttack),
        next_state: 180,
    },
    /* 180 */
    MobjState {
        sprite: "VILE",
        frame: 'I',
        duration: 10,
        action: None,
        next_state: 172,
    },
    /* 181 */
    MobjState {
        sprite: "VILE",
        frame: 'Q',
        duration: 5,
        action: Some(MonsterAction::Pain),
        next_state: 172,
    },
    /* 182 */
    MobjState {
        sprite: "VILE",
        frame: 'Q',
        duration: 7,
        action: Some(MonsterAction::Scream),
        next_state: 183,
    },
    /* 183 */
    MobjState {
        sprite: "VILE",
        frame: 'R',
        duration: 7,
        action: None,
        next_state: 184,
    },
    /* 184 */
    MobjState {
        sprite: "VILE",
        frame: 'S',
        duration: 7,
        action: Some(MonsterAction::Fall),
        next_state: 185,
    },
    /* 185 */
    MobjState {
        sprite: "VILE",
        frame: 'T',
        duration: 7,
        action: None,
        next_state: 186,
    },
    /* 186 */
    MobjState {
        sprite: "VILE",
        frame: 'U',
        duration: 7,
        action: None,
        next_state: 187,
    },
    /* 187 */
    MobjState {
        sprite: "VILE",
        frame: 'V',
        duration: -1,
        action: None,
        next_state: 187,
    },
    // Spider Mastermind (SPID)
    /* 188 */
    MobjState {
        sprite: "SPID",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 189,
    },
    /* 189 */
    MobjState {
        sprite: "SPID",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 188,
    },
    /* 190 */
    MobjState {
        sprite: "SPID",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 191,
    },
    /* 191 */
    MobjState {
        sprite: "SPID",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 192,
    },
    /* 192 */
    MobjState {
        sprite: "SPID",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 193,
    },
    /* 193 */
    MobjState {
        sprite: "SPID",
        frame: 'D',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 194,
    },
    /* 194 */
    MobjState {
        sprite: "SPID",
        frame: 'E',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 195,
    },
    /* 195 */
    MobjState {
        sprite: "SPID",
        frame: 'F',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 190,
    },
    /* 196 */
    MobjState {
        sprite: "SPID",
        frame: 'G',
        duration: 20,
        action: Some(MonsterAction::FaceTarget),
        next_state: 197,
    },
    /* 197 */
    MobjState {
        sprite: "SPID",
        frame: 'H',
        duration: 4,
        action: Some(MonsterAction::PosAttack),
        next_state: 198,
    },
    /* 198 */
    MobjState {
        sprite: "SPID",
        frame: 'H',
        duration: 4,
        action: Some(MonsterAction::PosAttack),
        next_state: 190,
    },
    /* 199 */
    MobjState {
        sprite: "SPID",
        frame: 'I',
        duration: 3,
        action: Some(MonsterAction::Pain),
        next_state: 190,
    },
    /* 200 */
    MobjState {
        sprite: "SPID",
        frame: 'J',
        duration: 20,
        action: Some(MonsterAction::Scream),
        next_state: 201,
    },
    /* 201 */
    MobjState {
        sprite: "SPID",
        frame: 'K',
        duration: 10,
        action: Some(MonsterAction::Fall),
        next_state: 202,
    },
    /* 202 */
    MobjState {
        sprite: "SPID",
        frame: 'L',
        duration: 10,
        action: None,
        next_state: 203,
    },
    /* 203 */
    MobjState {
        sprite: "SPID",
        frame: 'M',
        duration: 10,
        action: None,
        next_state: 204,
    },
    /* 204 */
    MobjState {
        sprite: "SPID",
        frame: 'N',
        duration: 10,
        action: None,
        next_state: 205,
    },
    /* 205 */
    MobjState {
        sprite: "SPID",
        frame: 'O',
        duration: -1,
        action: None,
        next_state: 205,
    },
    // Cyberdemon (CYBR)
    /* 206 */
    MobjState {
        sprite: "CYBR",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 207,
    },
    /* 207 */
    MobjState {
        sprite: "CYBR",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 206,
    },
    /* 208 */
    MobjState {
        sprite: "CYBR",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 209,
    },
    /* 209 */
    MobjState {
        sprite: "CYBR",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 210,
    },
    /* 210 */
    MobjState {
        sprite: "CYBR",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 211,
    },
    /* 211 */
    MobjState {
        sprite: "CYBR",
        frame: 'D',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 208,
    },
    /* 212 */
    MobjState {
        sprite: "CYBR",
        frame: 'A',
        duration: 0,
        action: None,
        next_state: 208,
    },
    /* 213 */
    MobjState {
        sprite: "CYBR",
        frame: 'A',
        duration: 0,
        action: None,
        next_state: 208,
    },
    /* 214 */
    MobjState {
        sprite: "CYBR",
        frame: 'E',
        duration: 6,
        action: Some(MonsterAction::FaceTarget),
        next_state: 215,
    },
    /* 215 */
    MobjState {
        sprite: "CYBR",
        frame: 'F',
        duration: 12,
        action: Some(MonsterAction::TroopAttack),
        next_state: 216,
    },
    /* 216 */
    MobjState {
        sprite: "CYBR",
        frame: 'E',
        duration: 12,
        action: None,
        next_state: 208,
    },
    /* 217 */
    MobjState {
        sprite: "CYBR",
        frame: 'G',
        duration: 10,
        action: Some(MonsterAction::Pain),
        next_state: 208,
    },
    /* 218 */
    MobjState {
        sprite: "CYBR",
        frame: 'H',
        duration: 10,
        action: Some(MonsterAction::Scream),
        next_state: 219,
    },
    /* 219 */
    MobjState {
        sprite: "CYBR",
        frame: 'I',
        duration: 10,
        action: None,
        next_state: 220,
    },
    /* 220 */
    MobjState {
        sprite: "CYBR",
        frame: 'J',
        duration: 10,
        action: None,
        next_state: 221,
    },
    /* 221 */
    MobjState {
        sprite: "CYBR",
        frame: 'K',
        duration: 10,
        action: None,
        next_state: 222,
    },
    /* 222 */
    MobjState {
        sprite: "CYBR",
        frame: 'L',
        duration: 10,
        action: Some(MonsterAction::Fall),
        next_state: 223,
    },
    /* 223 */
    MobjState {
        sprite: "CYBR",
        frame: 'M',
        duration: -1,
        action: None,
        next_state: 223,
    },
    // WolfSS (SSWV)
    /* 224 */
    MobjState {
        sprite: "SSWV",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 225,
    },
    /* 225 */
    MobjState {
        sprite: "SSWV",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 224,
    },
    /* 226 */
    MobjState {
        sprite: "SSWV",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 227,
    },
    /* 227 */
    MobjState {
        sprite: "SSWV",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 228,
    },
    /* 228 */
    MobjState {
        sprite: "SSWV",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 229,
    },
    /* 229 */
    MobjState {
        sprite: "SSWV",
        frame: 'D',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 226,
    },
    /* 230 */
    MobjState {
        sprite: "SSWV",
        frame: 'A',
        duration: 0,
        action: None,
        next_state: 226,
    },
    /* 231 */
    MobjState {
        sprite: "SSWV",
        frame: 'A',
        duration: 0,
        action: None,
        next_state: 226,
    },
    /* 232 */
    MobjState {
        sprite: "SSWV",
        frame: 'E',
        duration: 10,
        action: Some(MonsterAction::FaceTarget),
        next_state: 233,
    },
    /* 233 */
    MobjState {
        sprite: "SSWV",
        frame: 'F',
        duration: 10,
        action: Some(MonsterAction::PosAttack),
        next_state: 234,
    },
    /* 234 */
    MobjState {
        sprite: "SSWV",
        frame: 'G',
        duration: 10,
        action: Some(MonsterAction::PosAttack),
        next_state: 226,
    },
    /* 235 */
    MobjState {
        sprite: "SSWV",
        frame: 'H',
        duration: 3,
        action: Some(MonsterAction::Pain),
        next_state: 226,
    },
    /* 236 */
    MobjState {
        sprite: "SSWV",
        frame: 'I',
        duration: 5,
        action: Some(MonsterAction::Scream),
        next_state: 237,
    },
    /* 237 */
    MobjState {
        sprite: "SSWV",
        frame: 'J',
        duration: 5,
        action: Some(MonsterAction::Fall),
        next_state: 238,
    },
    /* 238 */
    MobjState {
        sprite: "SSWV",
        frame: 'K',
        duration: 5,
        action: None,
        next_state: 239,
    },
    /* 239 */
    MobjState {
        sprite: "SSWV",
        frame: 'L',
        duration: -1,
        action: None,
        next_state: 239,
    },
    // Shotgun Guy (SPOS) — uses separate sprites from Zombieman
    /* 240 S_SPOS_STND */
    MobjState {
        sprite: "SPOS",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 241,
    },
    /* 241 */
    MobjState {
        sprite: "SPOS",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 240,
    },
    /* 242 S_SPOS_RUN */
    MobjState {
        sprite: "SPOS",
        frame: 'A',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 243,
    },
    /* 243 */
    MobjState {
        sprite: "SPOS",
        frame: 'A',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 244,
    },
    /* 244 */
    MobjState {
        sprite: "SPOS",
        frame: 'B',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 245,
    },
    /* 245 */
    MobjState {
        sprite: "SPOS",
        frame: 'B',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 246,
    },
    /* 246 */
    MobjState {
        sprite: "SPOS",
        frame: 'C',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 247,
    },
    /* 247 */
    MobjState {
        sprite: "SPOS",
        frame: 'C',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 248,
    },
    /* 248 */
    MobjState {
        sprite: "SPOS",
        frame: 'D',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 249,
    },
    /* 249 */
    MobjState {
        sprite: "SPOS",
        frame: 'D',
        duration: 4,
        action: Some(MonsterAction::Chase),
        next_state: 242,
    },
    /* 250 S_SPOS_ATK */
    MobjState {
        sprite: "SPOS",
        frame: 'E',
        duration: 10,
        action: Some(MonsterAction::FaceTarget),
        next_state: 251,
    },
    /* 251 */
    MobjState {
        sprite: "SPOS",
        frame: 'F',
        duration: 10,
        action: Some(MonsterAction::SPosAttack),
        next_state: 252,
    },
    /* 252 */
    MobjState {
        sprite: "SPOS",
        frame: 'E',
        duration: 10,
        action: None,
        next_state: 253,
    },
    /* 253 */
    MobjState {
        sprite: "SPOS",
        frame: 'E',
        duration: 0,
        action: None,
        next_state: 242,
    },
    /* 254 S_SPOS_PAIN */
    MobjState {
        sprite: "SPOS",
        frame: 'G',
        duration: 3,
        action: None,
        next_state: 255,
    },
    /* 255 */
    MobjState {
        sprite: "SPOS",
        frame: 'G',
        duration: 3,
        action: Some(MonsterAction::Pain),
        next_state: 242,
    },
    /* 256 S_SPOS_DIE */
    MobjState {
        sprite: "SPOS",
        frame: 'H',
        duration: 5,
        action: None,
        next_state: 257,
    },
    /* 257 */
    MobjState {
        sprite: "SPOS",
        frame: 'I',
        duration: 5,
        action: Some(MonsterAction::Scream),
        next_state: 258,
    },
    /* 258 */
    MobjState {
        sprite: "SPOS",
        frame: 'J',
        duration: 5,
        action: Some(MonsterAction::Fall),
        next_state: 259,
    },
    /* 259 */
    MobjState {
        sprite: "SPOS",
        frame: 'K',
        duration: 5,
        action: None,
        next_state: 260,
    },
    /* 260 */
    MobjState {
        sprite: "SPOS",
        frame: 'L',
        duration: -1,
        action: None,
        next_state: 260,
    },
    // Demon / Pinky (SARG)
    /* 261 S_SARG_STND */
    MobjState {
        sprite: "SARG",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 262,
    },
    /* 262 */
    MobjState {
        sprite: "SARG",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 261,
    },
    /* 263 S_SARG_RUN */
    MobjState {
        sprite: "SARG",
        frame: 'A',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 264,
    },
    /* 264 */
    MobjState {
        sprite: "SARG",
        frame: 'A',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 265,
    },
    /* 265 */
    MobjState {
        sprite: "SARG",
        frame: 'B',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 266,
    },
    /* 266 */
    MobjState {
        sprite: "SARG",
        frame: 'B',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 267,
    },
    /* 267 */
    MobjState {
        sprite: "SARG",
        frame: 'C',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 268,
    },
    /* 268 */
    MobjState {
        sprite: "SARG",
        frame: 'C',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 269,
    },
    /* 269 */
    MobjState {
        sprite: "SARG",
        frame: 'D',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 270,
    },
    /* 270 */
    MobjState {
        sprite: "SARG",
        frame: 'D',
        duration: 2,
        action: Some(MonsterAction::Chase),
        next_state: 263,
    },
    /* 271 S_SARG_ATK */
    MobjState {
        sprite: "SARG",
        frame: 'E',
        duration: 8,
        action: Some(MonsterAction::FaceTarget),
        next_state: 272,
    },
    /* 272 */
    MobjState {
        sprite: "SARG",
        frame: 'F',
        duration: 8,
        action: Some(MonsterAction::FaceTarget),
        next_state: 273,
    },
    /* 273 */
    MobjState {
        sprite: "SARG",
        frame: 'G',
        duration: 8,
        action: Some(MonsterAction::SargAttack),
        next_state: 263,
    },
    /* 274 S_SARG_PAIN */
    MobjState {
        sprite: "SARG",
        frame: 'H',
        duration: 2,
        action: None,
        next_state: 275,
    },
    /* 275 */
    MobjState {
        sprite: "SARG",
        frame: 'H',
        duration: 2,
        action: Some(MonsterAction::Pain),
        next_state: 263,
    },
    /* 276 S_SARG_DIE */
    MobjState {
        sprite: "SARG",
        frame: 'I',
        duration: 8,
        action: None,
        next_state: 277,
    },
    /* 277 */
    MobjState {
        sprite: "SARG",
        frame: 'J',
        duration: 8,
        action: Some(MonsterAction::Scream),
        next_state: 278,
    },
    /* 278 */
    MobjState {
        sprite: "SARG",
        frame: 'K',
        duration: 4,
        action: None,
        next_state: 279,
    },
    /* 279 */
    MobjState {
        sprite: "SARG",
        frame: 'L',
        duration: 4,
        action: Some(MonsterAction::Fall),
        next_state: 280,
    },
    /* 280 */
    MobjState {
        sprite: "SARG",
        frame: 'M',
        duration: 4,
        action: None,
        next_state: 281,
    },
    /* 281 */
    MobjState {
        sprite: "SARG",
        frame: 'N',
        duration: -1,
        action: None,
        next_state: 281,
    },
    // Cacodemon (HEAD)
    /* 282 S_HEAD_STND */
    MobjState {
        sprite: "HEAD",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 283,
    },
    /* 283 */
    MobjState {
        sprite: "HEAD",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 282,
    },
    /* 284 S_HEAD_RUN */
    MobjState {
        sprite: "HEAD",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 285,
    },
    /* 285 */
    MobjState {
        sprite: "HEAD",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 286,
    },
    /* 286 */
    MobjState {
        sprite: "HEAD",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 287,
    },
    /* 287 */
    MobjState {
        sprite: "HEAD",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 288,
    },
    /* 288 */
    MobjState {
        sprite: "HEAD",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 289,
    },
    /* 289 */
    MobjState {
        sprite: "HEAD",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 284,
    },
    /* 290 S_HEAD_ATK */
    MobjState {
        sprite: "HEAD",
        frame: 'D',
        duration: 5,
        action: Some(MonsterAction::FaceTarget),
        next_state: 291,
    },
    /* 291 */
    MobjState {
        sprite: "HEAD",
        frame: 'E',
        duration: 5,
        action: Some(MonsterAction::FaceTarget),
        next_state: 292,
    },
    /* 292 */
    MobjState {
        sprite: "HEAD",
        frame: 'F',
        duration: 5,
        action: Some(MonsterAction::TroopAttack),
        next_state: 293,
    },
    /* 293 */
    MobjState {
        sprite: "HEAD",
        frame: 'F',
        duration: 0,
        action: None,
        next_state: 284,
    },
    /* 294 S_HEAD_PAIN */
    MobjState {
        sprite: "HEAD",
        frame: 'G',
        duration: 3,
        action: None,
        next_state: 295,
    },
    /* 295 */
    MobjState {
        sprite: "HEAD",
        frame: 'G',
        duration: 3,
        action: Some(MonsterAction::Pain),
        next_state: 284,
    },
    /* 296 S_HEAD_DIE */
    MobjState {
        sprite: "HEAD",
        frame: 'H',
        duration: 8,
        action: None,
        next_state: 297,
    },
    /* 297 */
    MobjState {
        sprite: "HEAD",
        frame: 'I',
        duration: 8,
        action: Some(MonsterAction::Scream),
        next_state: 298,
    },
    /* 298 */
    MobjState {
        sprite: "HEAD",
        frame: 'J',
        duration: 8,
        action: None,
        next_state: 299,
    },
    /* 299 */
    MobjState {
        sprite: "HEAD",
        frame: 'K',
        duration: 8,
        action: Some(MonsterAction::Fall),
        next_state: 300,
    },
    /* 300 */
    MobjState {
        sprite: "HEAD",
        frame: 'L',
        duration: 8,
        action: None,
        next_state: 301,
    },
    /* 301 */
    MobjState {
        sprite: "HEAD",
        frame: 'M',
        duration: -1,
        action: None,
        next_state: 301,
    },
    // Baron of Hell (BOSS)
    /* 302 S_BOSS_STND */
    MobjState {
        sprite: "BOSS",
        frame: 'A',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 303,
    },
    /* 303 */
    MobjState {
        sprite: "BOSS",
        frame: 'B',
        duration: 10,
        action: Some(MonsterAction::Look),
        next_state: 302,
    },
    /* 304 S_BOSS_RUN */
    MobjState {
        sprite: "BOSS",
        frame: 'A',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 305,
    },
    /* 305 */
    MobjState {
        sprite: "BOSS",
        frame: 'B',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 306,
    },
    /* 306 */
    MobjState {
        sprite: "BOSS",
        frame: 'C',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 307,
    },
    /* 307 */
    MobjState {
        sprite: "BOSS",
        frame: 'D',
        duration: 3,
        action: Some(MonsterAction::Chase),
        next_state: 304,
    },
    /* 308 S_BOSS_ATK */
    MobjState {
        sprite: "BOSS",
        frame: 'E',
        duration: 8,
        action: Some(MonsterAction::FaceTarget),
        next_state: 309,
    },
    /* 309 */
    MobjState {
        sprite: "BOSS",
        frame: 'F',
        duration: 8,
        action: Some(MonsterAction::FaceTarget),
        next_state: 310,
    },
    /* 310 */
    MobjState {
        sprite: "BOSS",
        frame: 'G',
        duration: 8,
        action: Some(MonsterAction::TroopAttack),
        next_state: 311,
    },
    /* 311 */
    MobjState {
        sprite: "BOSS",
        frame: 'G',
        duration: 0,
        action: None,
        next_state: 304,
    },
    /* 312 S_BOSS_PAIN */
    MobjState {
        sprite: "BOSS",
        frame: 'H',
        duration: 2,
        action: None,
        next_state: 313,
    },
    /* 313 */
    MobjState {
        sprite: "BOSS",
        frame: 'H',
        duration: 2,
        action: Some(MonsterAction::Pain),
        next_state: 304,
    },
    /* 314 S_BOSS_DIE */
    MobjState {
        sprite: "BOSS",
        frame: 'I',
        duration: 8,
        action: None,
        next_state: 315,
    },
    /* 315 */
    MobjState {
        sprite: "BOSS",
        frame: 'J',
        duration: 8,
        action: Some(MonsterAction::Scream),
        next_state: 316,
    },
    /* 316 */
    MobjState {
        sprite: "BOSS",
        frame: 'K',
        duration: 8,
        action: None,
        next_state: 317,
    },
    /* 317 */
    MobjState {
        sprite: "BOSS",
        frame: 'L',
        duration: 8,
        action: Some(MonsterAction::Fall),
        next_state: 318,
    },
    /* 318 */
    MobjState {
        sprite: "BOSS",
        frame: 'M',
        duration: 8,
        action: None,
        next_state: 319,
    },
    /* 319 */
    MobjState {
        sprite: "BOSS",
        frame: 'N',
        duration: -1,
        action: None,
        next_state: 319,
    },
];

pub fn get_start_state(kind: u16) -> usize {
    match kind {
        3004 => S_POSS_STND,  // Zombieman
        9 => S_SPOS_STND,     // Shotgun Guy (own SPOS sprites)
        3001 => S_TROO_STND,  // Imp
        3002 => S_SARG_STND,  // Demon (Pinky)
        3003 => S_BOSS_STND,  // Baron of Hell
        3005 => S_HEAD_STND,  // Cacodemon
        3006 => S_SKULL_STND, // Lost Soul
        2035 => S_BAR1,       // Barrel
        64 => S_VILE_STND,    // Archvile
        65 => S_CPOS_STND,    // Chaingunner
        66 => S_SKEL_STND,    // Revenant
        67 => S_FATT_STND,    // Mancubus
        68 => S_BSPI_STND,    // Arachnotron
        69 => S_BOS2_STND,    // Hell Knight
        71 => S_PAIN_STND,    // Pain Elemental
        7 => S_SPID_STND,     // Spider Mastermind
        16 => S_CYBR_STND,    // Cyberdemon
        84 => S_SSWV_STND,    // WolfSS
        _ => S_POSS_STND,     // Default to Zombieman (POSS)
    }
}
impl MonsterThinker {
    pub fn new(
        thing_idx: usize,
        state_idx: usize,
        tics: i32,
        target: Option<usize>,
        cooldown: u32,
    ) -> Self {
        Self {
            thing_idx,
            state_idx,
            tics,
            target_thing_idx: target,
            attack_cooldown: cooldown,
            just_entered_state: true, // Fire action on first tick
        }
    }

    fn is_in_death_sequence(&self, die_state: usize, states: &[MobjState]) -> bool {
        // Walk the death sequence from die_state until we hit a terminal state (duration == -1)
        // Check if our current state_idx is any of those states
        let mut s = die_state;
        let mut visited = std::collections::HashSet::new();
        loop {
            if !visited.insert(s) {
                break;
            } // cycle guard
            if self.state_idx == s {
                return true;
            }
            if s >= states.len() {
                break;
            }
            if states[s].duration == -1 {
                break;
            } // terminal
            s = states[s].next_state;
        }
        false
    }

    fn set_state(&mut self, state_idx: usize, states: &[MobjState]) {
        self.state_idx = state_idx;
        self.just_entered_state = true;
        if state_idx < states.len() {
            self.tics = states[state_idx].duration;
        } else {
            self.tics = -1;
        }
    }

    fn execute_action(
        &mut self,
        action: MonsterAction,
        world: &WorldState,
        cmds: &mut Vec<WorldCommand>,
    ) {
        let monster = &world.things[self.thing_idx];

        let (target_pos, target_z) = if let Some(t_idx) = self.target_thing_idx {
            if let Some(t) = world.things.get(t_idx) {
                (t.position, t.z)
            } else {
                (world.player.position, world.player.z)
            }
        } else {
            (world.player.position, world.player.z)
        };
        let target_dist = (target_pos - monster.position).length();

        match action {
            MonsterAction::Look => {
                // Monsters wake up if:
                // 1. Player is within noise radius AND has line of sight, OR
                // 2. Player is within sight radius (increased to 3000 for better aggro) AND has line of sight
                let within_noise = target_dist < world.player.noise_radius;
                let within_sight_range = target_dist < 3000.0;

                if (within_noise || within_sight_range)
                    && world.has_line_of_sight(monster.position, target_pos)
                {
                    // Alert!
                    let run_state = match monster.kind {
                        3004 => S_POSS_RUN,
                        9 => S_SPOS_RUN,
                        3001 => S_TROO_RUN,
                        3002 => S_SARG_RUN,
                        3003 => S_BOSS_RUN,
                        3005 => S_HEAD_RUN,
                        3006 => S_SKULL_RUN,
                        65 => S_CPOS_RUN,
                        66 => S_SKEL_RUN,
                        67 => S_FATT_RUN,
                        68 => S_BSPI_RUN,
                        69 => S_BOS2_RUN,
                        71 => S_PAIN_RUN,
                        64 => S_VILE_RUN,
                        7 => S_SPID_RUN,
                        16 => S_CYBR_RUN,
                        84 => S_SSWV_RUN,
                        _ => S_POSS_RUN,
                    };
                    self.set_state(run_state, STATES);
                    cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                        sound_id: "DSSIGHT".into(),
                        position: Some(monster.position),
                        volume: 1.0,
                    }));
                }
            }
            MonsterAction::Chase => {
                let speed = DEFAULT_THING_DEFS
                    .iter()
                    .find(|&&(k, _)| k == monster.kind)
                    .map(|&(_, d)| d.speed)
                    .unwrap_or(MONSTER_SPEED);
                let dir = (target_pos - monster.position).normalize_or_zero();
                let mut move_vec = dir * speed;

                // Try to move directly
                if !self.try_move(world, move_vec) {
                    // Blocked! Try to slide along the walls that blocked us.
                    let left = Mat2::from_angle(0.785).mul_vec2(move_vec); // 45 deg
                    let right = Mat2::from_angle(-0.785).mul_vec2(move_vec); // -45 deg

                    if !self.try_move(world, left) {
                        if !self.try_move(world, right) {
                            move_vec = Vec2::ZERO;
                        } else {
                            move_vec = right;
                        }
                    } else {
                        move_vec = left;
                    }
                }

                let mut z_move = 0.0;

                // Vertical movement for flying monsters
                if monster.is_flying() {
                    let target_z_eye = target_z + 28.0;
                    if (monster.z - target_z_eye).abs() > 8.0 {
                        z_move = if monster.z < target_z_eye { 2.0 } else { -2.0 };
                    }
                }

                if move_vec.length() > 0.0 {
                    cmds.push(WorldCommand::ModifyThing {
                        thing_idx: self.thing_idx,
                        pos_delta: move_vec,
                        z_delta: z_move,
                        angle: dir.y.atan2(dir.x),
                    });
                }

                // Attack chance - with line-of-sight check and cooldown
                if self.attack_cooldown > 0 {
                    self.attack_cooldown -= 1;
                }
                // INCREASED ATTACK CHANCE: p_random() < 32 (~12% per frame)
                if self.attack_cooldown == 0
                    && target_dist < 2048.0
                    && p_random() < 32
                    && world.has_line_of_sight(monster.position, target_pos)
                {
                    let atk_state = match monster.kind {
                        3004 => S_POSS_ATK,
                        9 => S_SPOS_ATK,
                        3001 => S_TROO_ATK,
                        3002 => S_SARG_ATK,
                        3003 => S_BOSS_ATK,
                        3005 => S_HEAD_ATK,
                        3006 => S_SKULL_ATK,
                        65 => S_CPOS_ATK,
                        66 => S_SKEL_ATK,
                        67 => S_FATT_ATK,
                        68 => S_BSPI_ATK,
                        69 => S_BOS2_ATK,
                        71 => S_PAIN_ATK,
                        64 => S_VILE_ATK,
                        7 => S_SPID_ATK,
                        16 => S_CYBR_ATK,
                        84 => S_SSWV_ATK,
                        _ => S_POSS_ATK,
                    };
                    self.set_state(atk_state, STATES);
                    self.attack_cooldown = 20; // Slightly shorter cooldown
                }
            }
            MonsterAction::SkullAttack => {
                // Lost Soul charge logic
                let speed = DEFAULT_THING_DEFS
                    .iter()
                    .find(|&&(k, _)| k == monster.kind)
                    .map(|&(_, d)| d.speed)
                    .unwrap_or(MONSTER_SPEED);
                let dir = (target_pos - monster.position).normalize_or_zero();
                let z_dir = ((target_z + 20.0) - monster.z).clamp(-1.0, 1.0);
                let move_vec = dir * (speed * 4.0);
                let z_move = z_dir * (speed * 2.0);

                cmds.push(WorldCommand::ModifyThing {
                    thing_idx: self.thing_idx,
                    pos_delta: move_vec,
                    z_delta: z_move,
                    angle: dir.y.atan2(dir.x),
                });

                if target_dist < 48.0 {
                    cmds.push(WorldCommand::DamagePlayer {
                        amount: 10.0,
                        angle: Some(dir.y.atan2(dir.x)),
                    });
                    cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                        sound_id: "DSATK".into(),
                        position: Some(monster.position),
                        volume: 1.0,
                    }));
                    // After hit, stop charging (return to chase state after current state duration)
                }
            }
            MonsterAction::FaceTarget => {
                let dir = (target_pos - monster.position).normalize_or_zero();
                cmds.push(WorldCommand::ModifyThing {
                    thing_idx: self.thing_idx,
                    pos_delta: Vec2::ZERO,
                    z_delta: 0.0,
                    angle: dir.y.atan2(dir.x),
                });
            }
            MonsterAction::PosAttack => {
                let dir = (target_pos - monster.position).normalize_or_zero();
                let angle = dir.y.atan2(dir.x);
                let spread = (p_random() as f32 - 128.0) / 256.0 * 0.1;
                cmds.push(WorldCommand::FireHitscan {
                    origin: monster.position,
                    angle: angle + spread,
                    damage: 10.0,
                    attacker_idx: Some(self.thing_idx),
                });
                cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                    sound_id: "DSPISTOL".into(),
                    position: Some(monster.position),
                    volume: 1.0,
                }));
            }
            MonsterAction::TroopAttack => {
                let dir = (target_pos - monster.position).normalize_or_zero();
                if target_dist < 72.0 {
                    cmds.push(WorldCommand::DamagePlayer {
                        amount: 10.0,
                        angle: Some(dir.y.atan2(dir.x)),
                    });
                    cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                        sound_id: "DSCLAW".into(),
                        position: Some(monster.position),
                        volume: 1.0,
                    }));
                } else {
                    let z_speed = ((target_z + 32.0) - (monster.z + 32.0)) / (target_dist / 20.0);
                    cmds.push(WorldCommand::SpawnProjectile {
                        kind: 10031, // Imp Fireball
                        position: monster.position,
                        z: monster.z + 32.0,
                        velocity: dir * 20.0,
                        z_velocity: z_speed,
                        damage: 10.0,
                        owner_is_player: false,
                        owner_thing_idx: Some(self.thing_idx),
                    });
                }
            }
            MonsterAction::SargAttack => {
                let dir = (target_pos - monster.position).normalize_or_zero();
                if target_dist < 72.0 {
                    let damage = ((p_random() % 10) + 1) as f32 * 4.0;
                    cmds.push(WorldCommand::DamagePlayer {
                        amount: damage,
                        angle: Some(dir.y.atan2(dir.x)),
                    });
                    cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                        sound_id: "DSBGSITE".into(),
                        position: Some(monster.position),
                        volume: 1.0,
                    }));
                }
            }
            MonsterAction::Scream => {
                cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                    sound_id: "DSPDIE".into(),
                    position: Some(monster.position),
                    volume: 1.0,
                }));
            }
            MonsterAction::Fall => {
                // In Doom, Fall makes the monster non-blocking
            }
            MonsterAction::Explode => {
                // Barrel explosion
                let barrel_pos = monster.position;

                cmds.push(WorldCommand::SplashDamage {
                    center: barrel_pos,
                    damage: 128.0,
                    radius: 128.0,
                    owner_is_player: false,
                });

                // Spawn explosion effect
                cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                    sound_id: "DSBAREXP".into(),
                    position: Some(barrel_pos),
                    volume: 1.0,
                }));
            }
            _ => {}
        }
    }
    fn try_move(&self, world: &WorldState, move_vec: Vec2) -> bool {
        let monster = &world.things[self.thing_idx];
        let trial = monster.position + move_vec;
        let radius = DEFAULT_THING_DEFS
            .iter()
            .find(|&&(k, _)| k == monster.kind)
            .map(|&(_, d)| d.radius)
            .unwrap_or(20.0);

        // 1. Collision avoidance against other monsters/player
        // Check against player
        if (trial - world.player.position).length() < radius + PLAYER_RADIUS {
            // Allow sliding off the player if we aren't getting closer
            if (trial - world.player.position).length()
                < (monster.position - world.player.position).length() - 0.01
            {
                return false;
            }
        }

        // Check against other monsters
        for (i, other) in world.things.iter().enumerate() {
            if i == self.thing_idx {
                continue;
            }
            if other.health <= 0.0 || other.picked_up || !other.is_monster() {
                continue;
            }

            let other_radius = DEFAULT_THING_DEFS
                .iter()
                .find(|&&(k, _)| k == other.kind)
                .map(|&(_, d)| d.radius)
                .unwrap_or(20.0);
            if (trial - other.position).length() < radius + other_radius {
                if (trial - other.position).length()
                    < (monster.position - other.position).length() - 0.01
                {
                    return false;
                }
            }
        }

        // 2. Collision avoidance against walls
        for line in &world.linedefs {
            let s = world.vertices[line.start_idx];
            let e = world.vertices[line.end_idx];
            let closest = WorldState::closest_point_on_segment(trial, s, e);
            let dist = (trial - closest).length();

            if dist < radius {
                let closest_old = WorldState::closest_point_on_segment(monster.position, s, e);
                let dist_old = (monster.position - closest_old).length();

                // Only block if we are actually moving geometrically closer to the wall segment
                if dist < dist_old - 0.01 {
                    let mut should_block = true;
                    if line.is_portal() {
                        if let (Some(fs), Some(bs)) = (line.sector_front, line.sector_back) {
                            let front = &world.sectors[fs];
                            let back = &world.sectors[bs];
                            let lowest_ceiling = front.ceiling_height.min(back.ceiling_height);
                            let highest_floor = front.floor_height.max(back.floor_height);
                            let gap = lowest_ceiling - highest_floor;
                            let step_up = highest_floor - monster.z;

                            if gap >= 56.0 && step_up <= STEP_HEIGHT {
                                should_block = false;
                            }
                        }
                    }
                    if should_block {
                        return false;
                    }
                }
            }
        }
        true
    }
}
impl Thinker for MonsterThinker {
    fn on_pain(
        &mut self,
        target_idx: usize,
        target_kind: u16,
        inflictor_idx: Option<usize>,
        _inflictor_kind: Option<u16>,
    ) {
        if self.thing_idx == target_idx {
            // Barrels explode immediately when damaged
            if target_kind == 2035 {
                self.set_state(S_BEXP, STATES);
                return;
            }

            let chance = DEFAULT_THING_DEFS
                .iter()
                .find(|&&(k, _)| k == target_kind)
                .map(|&(_, d)| d.pain_chance)
                .unwrap_or(0);
            if p_random() < chance {
                let pain_state = match target_kind {
                    3004 => S_POSS_PAIN,
                    9 => S_SPOS_PAIN,
                    3001 => S_TROO_PAIN,
                    3002 => S_SARG_PAIN,
                    3003 => S_BOSS_PAIN,
                    3005 => S_HEAD_PAIN,
                    3006 => S_SKULL_PAIN,
                    65 => S_CPOS_PAIN,
                    66 => S_SKEL_PAIN,
                    67 => S_FATT_PAIN,
                    68 => S_BSPI_PAIN,
                    69 => S_BOS2_PAIN,
                    71 => S_PAIN_PAIN,
                    64 => S_VILE_PAIN,
                    7 => S_SPID_PAIN,
                    16 => S_CYBR_PAIN,
                    84 => S_SSWV_PAIN,
                    _ => S_POSS_PAIN,
                };
                self.set_state(pain_state, STATES);

                if let Some(inflictor) = inflictor_idx {
                    if inflictor != self.thing_idx {
                        self.target_thing_idx = Some(inflictor);
                    }
                }
            }
        }
    }

    fn on_wake(&mut self, thing_idx: usize) {
        if self.thing_idx == thing_idx {
            // Monster was woken by noise - switch to chase state if not already active
            // This implements Doom's "monsters hear through doors" behavior
            // Only wake up if in a non-active state (Look/Pain/Idle states)
            // Check if monster is in a Look/idle state (any monster type)
            let current_state = self.state_idx;
            let idle_to_run: Option<usize> = match current_state {
                s if s == S_POSS_STND || s == S_POSS_STND + 1 => Some(S_POSS_RUN),
                s if s == S_SPOS_STND || s == S_SPOS_STND + 1 => Some(S_SPOS_RUN),
                s if s == S_TROO_STND || s == S_TROO_STND + 1 => Some(S_TROO_RUN),
                s if s == S_SARG_STND || s == S_SARG_STND + 1 => Some(S_SARG_RUN),
                s if s == S_HEAD_STND || s == S_HEAD_STND + 1 => Some(S_HEAD_RUN),
                s if s == S_BOSS_STND || s == S_BOSS_STND + 1 => Some(S_BOSS_RUN),
                s if s == S_SKULL_STND || s == S_SKULL_STND + 1 => Some(S_SKULL_RUN),
                s if s == S_CPOS_STND || s == S_CPOS_STND + 1 => Some(S_CPOS_RUN),
                s if s == S_SKEL_STND || s == S_SKEL_STND + 1 => Some(S_SKEL_RUN),
                s if s == S_FATT_STND || s == S_FATT_STND + 1 => Some(S_FATT_RUN),
                s if s == S_BSPI_STND || s == S_BSPI_STND + 1 => Some(S_BSPI_RUN),
                s if s == S_BOS2_STND || s == S_BOS2_STND + 1 => Some(S_BOS2_RUN),
                s if s == S_PAIN_STND || s == S_PAIN_STND + 1 => Some(S_PAIN_RUN),
                s if s == S_VILE_STND || s == S_VILE_STND + 1 => Some(S_VILE_RUN),
                s if s == S_SPID_STND || s == S_SPID_STND + 1 => Some(S_SPID_RUN),
                s if s == S_CYBR_STND || s == S_CYBR_STND + 1 => Some(S_CYBR_RUN),
                s if s == S_SSWV_STND || s == S_SSWV_STND + 1 => Some(S_SSWV_RUN),
                _ => None,
            };
            if let Some(run_state) = idle_to_run {
                self.set_state(run_state, STATES);
            }
        }
    }

    fn update(&mut self, world: &WorldState) -> (bool, Vec<WorldCommand>) {
        let monster = match world.things.get(self.thing_idx) {
            Some(m) => m,
            None => return (false, vec![]),
        };

        if monster.health <= 0.0 {
            let die_state = match monster.kind {
                3004 => S_POSS_DIE,
                9 => S_SPOS_DIE,
                3001 => S_TROO_DIE,
                3002 => S_SARG_DIE,
                3003 => S_BOSS_DIE,
                3005 => S_HEAD_DIE,
                3006 => S_SKULL_DIE,
                65 => S_CPOS_DIE,
                66 => S_SKEL_DIE,
                67 => S_FATT_DIE,
                68 => S_BSPI_DIE,
                69 => S_BOS2_DIE,
                71 => S_PAIN_DIE,
                64 => S_VILE_DIE,
                7 => S_SPID_DIE,
                16 => S_CYBR_DIE,
                84 => S_SSWV_DIE,
                2035 => S_BEXP,
                _ => S_POSS_DIE,
            };
            // Check if NOT already in a death sequence
            if !self.is_in_death_sequence(die_state, STATES) {
                self.set_state(die_state, STATES);
            }
        }

        let mut commands = Vec::new();

        if self.tics > 0 {
            self.tics -= 1;
        }

        if self.tics == 0 {
            if self.state_idx < STATES.len() {
                let next = STATES[self.state_idx].next_state;
                self.set_state(next, STATES);
            }
        }

        // Fire action only on state entry (matching vanilla Doom behavior).
        // In Doom, P_SetMobjState calls the action function once when the state is set.
        if self.just_entered_state {
            self.just_entered_state = false;
            if self.state_idx < STATES.len() {
                if let Some(action) = STATES[self.state_idx].action {
                    self.execute_action(action, world, &mut commands);
                }
            }
        }

        // Sync tics back to thing.ai_timer for save/load preservation
        // This ensures the AI state is saved with the thing
        commands.push(WorldCommand::SyncAiState {
            thing_idx: self.thing_idx,
            state_idx: self.state_idx,
            timer: self.tics.max(0) as u32,
            target: self.target_thing_idx,
            cooldown: self.attack_cooldown,
        });

        // Gravity and step logic for ALL ground monsters
        if !monster.is_flying() {
            if let Some(s_idx) = world.find_sector_at(monster.position) {
                let floor_z = world.sectors[s_idx].floor_height;
                let mut z_snap = 0.0;
                if monster.z > floor_z {
                    // Fall down
                    z_snap = -8.0;
                    if monster.z + z_snap < floor_z {
                        z_snap = floor_z - monster.z;
                    }
                } else if monster.z < floor_z {
                    // Step up climbing
                    z_snap = floor_z - monster.z;
                }

                if z_snap != 0.0 {
                    commands.push(WorldCommand::ModifyThing {
                        thing_idx: self.thing_idx,
                        pos_delta: Vec2::ZERO,
                        z_delta: z_snap,
                        angle: monster.angle,
                    });
                }
            }
        }

        // Keep the thinker alive unless in a terminal death state (duration == -1)
        let keep = if monster.health <= 0.0 {
            // Still animating death sequence — keep until final frame
            self.state_idx < STATES.len() && STATES[self.state_idx].duration != -1
        } else {
            true
        };
        (keep, commands)
    }
}

// Doom-specific WorldState methods: update loop, apply_commands, linedef activation
pub trait DoomWorldExt {
    fn is_walk_trigger(special: u16) -> bool;
    fn spread_noise(&mut self, start_sid: usize, hops: u32);
    fn spawn_effect_thing(&mut self, thing: Thing) -> usize;
    fn fire_hitscan(
        &mut self,
        origin: aetheris::simulation::Vertex,
        angle: f32,
        damage: f32,
        attacker_idx: Option<usize>,
    );
    fn update(&mut self, actions: &std::collections::HashSet<aetheris::simulation::GameAction>);
    fn apply_commands(&mut self, cmds: Vec<aetheris::simulation::WorldCommand>);
    fn activate_linedef_manual(
        &mut self,
        line_idx: usize,
        override_back: Option<usize>,
        cmds: &mut Vec<aetheris::simulation::WorldCommand>,
    );
    fn activate_linedef(
        &mut self,
        special: u16,
        tag: u16,
        sector_back: Option<usize>,
        cmds: &mut Vec<aetheris::simulation::WorldCommand>,
    );
    fn find_lowest_adjacent_ceiling(&self, sector_idx: usize) -> f32;
    fn trigger_door(&mut self, sector_idx: usize, speed: f32, wait: f32) -> bool;
    fn do_door_tagged(&mut self, tag: u16, speed: f32, wait: f32) -> bool;
    fn do_lift_tagged(&mut self, tag: u16);
    fn do_crusher_tagged(&mut self, tag: u16, speed: f32, damage: f32);
    fn do_stairs_tagged(&mut self, tag: u16, step_height: f32);
    fn update_environmental_damage(&mut self);
}

impl DoomWorldExt for WorldState {
    /// Returns true if the linedef special type is a walk-trigger (W1/WR).
    /// Use-triggers (S1/SR), gun-triggers (G1/GR), and manual doors (D/DR) are NOT walk-triggered.
    fn is_walk_trigger(special: u16) -> bool {
        matches!(
            special,
            // W1 types
            2 | 3 | 4 | 5 | 16 | 38 | 39 | 44 | 52 | 56 | 58 | 59 |
            // WR types
            72 | 73 | 74 | 75 | 76 | 77 | 79 | 80 | 86 | 87 | 88 | 90 | 91 |
            97 | 105 | 106 | 107 | 120 | 126 | 128 | 129
        )
    }

    fn spawn_effect_thing(&mut self, thing: Thing) -> usize {
        for (i, t) in self.things.iter_mut().enumerate() {
            if t.picked_up
                && (t.kind == 9997
                    || t.kind == 9998
                    || t.kind == 9999
                    || matches!(t.kind, 127 | 128 | 129 | 10031))
            {
                *t = thing;
                return i;
            }
        }
        let idx = self.things.len();
        self.things.push(thing);
        idx
    }

    fn fire_hitscan(
        &mut self,
        origin: aetheris::simulation::Vertex,
        angle: f32,
        damage: f32,
        attacker_idx: Option<usize>,
    ) {
        let dir = glam::Vec2::new(angle.cos(), angle.sin());
        let max_dist = 2000.0;
        let end = origin + dir * max_dist;

        let mut best_dist = max_dist;
        let mut hit_thing_idx = None;
        let mut hit_player = false;
        let mut hit_pos = end;
        for line in &self.linedefs {
            if line.is_portal() {
                continue;
            }
            let p3 = self.vertices[line.start_idx];
            let p4 = self.vertices[line.end_idx];
            if let Some(hit) = WorldState::intersect(origin, end, p3, p4) {
                let d = (hit - origin).length();
                if d < best_dist {
                    best_dist = d;
                    hit_pos = hit;
                    hit_thing_idx = None;
                    hit_player = false;
                }
            }
        }

        for (i, thing) in self.things.iter().enumerate() {
            if (!thing.is_monster() && !thing.is_barrel()) || thing.health <= 0.0 || thing.picked_up
            {
                continue;
            }
            if attacker_idx == Some(i) {
                continue;
            }
            let v = thing.position - origin;
            let t = v.dot(dir);
            if t < 0.0 || t > best_dist {
                continue;
            }

            let closest = origin + dir * t;
            let dist_sq = (thing.position - closest).length_squared();
            if dist_sq < (20.0 * 20.0) {
                best_dist = t;
                hit_pos = closest;
                hit_thing_idx = Some(i);
                hit_player = false;
            }
        }

        if attacker_idx.is_some() && self.player.health > 0.0 {
            let v = self.player.position - origin;
            let t = v.dot(dir);
            if t >= 0.0 && t < best_dist {
                let closest = origin + dir * t;
                let dist_sq = (self.player.position - closest).length_squared();
                if dist_sq < (20.0 * 20.0) {
                    best_dist = t;
                    hit_pos = closest;
                    hit_thing_idx = None;
                    hit_player = true;
                }
            }
        }

        if let Some(idx) = hit_thing_idx {
            let t_kind = self.things[idx].kind;
            let i_kind = attacker_idx.and_then(|id| self.things.get(id).map(|th| th.kind));

            if let Some(t) = self.things.get_mut(idx) {
                t.health -= damage;

                for i in 0..self.thinkers.len() {
                    let mut thinker = self.thinkers.remove(i);
                    thinker.on_pain(idx, t_kind, attacker_idx, i_kind);
                    self.thinkers.insert(i, thinker);
                }

                let b_kind = if t_kind == 3003 || t_kind == 3005 {
                    9997
                } else {
                    9999
                };
                let puff = Thing {
                    position: hit_pos,
                    angle: 0.0,
                    kind: b_kind,
                    flags: 0,
                    health: 10.0,
                    picked_up: false,
                    state_idx: 0,
                    ai_timer: 0,
                    target_thing_idx: None,
                    attack_cooldown: 0,
                    z: 0.0,
                };
                self.spawn_effect_thing(puff);
                self.thinkers.push(Box::new(PuffThinker {
                    position: hit_pos,
                    timer: 15,
                }));
            }
            if let Some(t) = self.things.get(idx) {
                self.audio_events.push(AudioEvent {
                    sound_id: "DSPOPAIN".into(),
                    position: Some(t.position),
                    volume: 1.0,
                });
            }
        } else if hit_player {
            if self.player.invuln_timer == 0 {
                let absorbed = (damage * 0.333).min(self.player.armor);
                self.player.armor -= absorbed;
                self.player.health -= damage - absorbed;
                if self.player.damage_flash < 0.1 {
                    self.player.damage_flash = 0.5;
                }
                self.player.last_damage_angle = Some(angle);
            }
            let puff = Thing {
                position: hit_pos,
                angle: 0.0,
                kind: 9999, // Blood
                flags: 0,
                health: 10.0,
                picked_up: false,
                state_idx: 0,
                ai_timer: 0,
                target_thing_idx: None,
                attack_cooldown: 0,
                z: 0.0,
            };
            self.spawn_effect_thing(puff);
            self.thinkers.push(Box::new(PuffThinker {
                position: hit_pos,
                timer: 15,
            }));
            self.audio_events.push(AudioEvent {
                sound_id: "DSPLPAIN".into(),
                position: Some(self.player.position),
                volume: 1.0,
            });
        } else if best_dist < max_dist {
            self.audio_events.push(AudioEvent {
                sound_id: "DSNOWHIT".into(),
                position: Some(hit_pos),
                volume: 0.5,
            });
            let puff = Thing {
                position: hit_pos,
                angle: 0.0,
                kind: 9998,
                flags: 0,
                health: 5.0,
                picked_up: false,
                state_idx: 0,
                ai_timer: 0,
                target_thing_idx: None,
                attack_cooldown: 0,
                z: 0.0,
            };
            self.spawn_effect_thing(puff);
        }
    }

    fn spread_noise(&mut self, start_sid: usize, hops: u32) {
        let mut queue = vec![(start_sid, hops)];
        let mut visited = std::collections::HashSet::new();

        while let Some((sid, h)) = queue.pop() {
            if !visited.insert(sid) {
                continue;
            }

            for t_idx in 0..self.things.len() {
                let thing = &self.things[t_idx];
                if thing.is_monster() && thing.health > 0.0 && !thing.picked_up {
                    let sidx = self.find_subsector(thing.position.x, thing.position.y);
                    if let Some(ss) = self.subsectors.get(sidx) {
                        if let Some(seg) = self.segs.get(ss.first_seg_idx) {
                            if let Some(tsid) = self.linedefs[seg.linedef_idx].sector_front {
                                if tsid == sid {
                                    for i in 0..self.thinkers.len() {
                                        let mut thinker = self.thinkers.remove(i);
                                        thinker.on_wake(t_idx);
                                        self.thinkers.insert(i, thinker);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if h > 0 {
                if let Some(neighbors) = self.adjacent_sectors.get(sid) {
                    for &neighbor in neighbors {
                        queue.push((neighbor, h - 1));
                    }
                }
            }
        }
    }

    fn update(&mut self, actions: &HashSet<GameAction>) {
        self.frame_count += 1;

        if self.is_intermission {
            self.intermission_tic += 1;
        }

        if self.player.health <= 0.0 {
            if actions.contains(&GameAction::Fire) {
                self.apply_commands(vec![WorldCommand::RespawnPlayer]);
            }
            return;
        }

        if self.menu_state != MenuState::None && self.frame_count % 35 == 0 {
            log::info!("MENU ACTIVE: Press ESC/ENTER to start map or use ARROWS/WASD to navigate.");
        }

        if self.is_intermission {
            return;
        }

        let mut cmds = Vec::new();

        let old_pos = self.player.position;

        let mut next_angle = self.player.angle;
        if actions.contains(&GameAction::TurnLeft) {
            next_angle += TURN_SPEED;
        }
        if actions.contains(&GameAction::TurnRight) {
            next_angle -= TURN_SPEED;
        }
        next_angle = next_angle.rem_euclid(2.0 * std::f32::consts::PI);

        let dir = Vec2::new(next_angle.cos(), next_angle.sin());
        let strafe = Vec2::new(-dir.y, dir.x);
        let mut wish = Vec2::ZERO;
        if actions.contains(&GameAction::MoveForward) {
            wish += dir;
        }
        if actions.contains(&GameAction::MoveBackward) {
            wish -= dir;
        }
        if actions.contains(&GameAction::StrafeLeft) {
            wish += strafe;
        }
        if actions.contains(&GameAction::StrafeRight) {
            wish -= strafe;
        }

        // Improved movement with better acceleration and friction
        let accel = if wish.length() > 0.1 { 2.5 } else { 0.0 }; // Acceleration when moving
        let friction = 0.85; // Better friction for more control

        let mut next_velocity = self.player.velocity + wish.normalize_or_zero() * accel;
        next_velocity *= friction;

        // Stop completely if velocity is very small
        if next_velocity.length() < 0.01 {
            next_velocity = Vec2::ZERO;
        }

        let mut next_pos = self.player.position + next_velocity;
        let mut next_z = self.player.z;
        let mut next_bob = self.player.bob_phase;

        if next_velocity.length() > 0.1 {
            next_bob += 0.15;
        } else {
            next_bob = 0.0;
        }

        // Collision Detection & Wall Sliding
        let mut lines_to_activate = Vec::new();

        // Use multiple iterations for robust corner collision resolution
        for iter in 0..2 {
            for (line_idx, line) in self.linedefs.iter().enumerate() {
                let (s, e) = (self.vertices[line.start_idx], self.vertices[line.end_idx]);

                // Only check intersection on first pass for efficiency
                let cross = Self::intersect(old_pos, next_pos, s, e);
                let is_crossing = iter == 0 && cross.is_some() && self.player.health > 0.0;

                if is_crossing && line.special_type != 0 && Self::is_walk_trigger(line.special_type)
                {
                    lines_to_activate.push(line_idx);
                }

                let closest = Self::closest_point_on_segment(next_pos, s, e);
                let d_v = next_pos - closest;
                let d = d_v.length();

                if d < PLAYER_RADIUS || is_crossing {
                    let mut should_block = true;

                    // Check portal (door/window) passage
                    if line.is_portal() {
                        if let (Some(fs), Some(bs)) = (line.sector_front, line.sector_back) {
                            let front = &self.sectors[fs];
                            let back = &self.sectors[bs];
                            let lowest_ceiling = front.ceiling_height.min(back.ceiling_height);
                            let highest_floor = front.floor_height.max(back.floor_height);
                            let gap = lowest_ceiling - highest_floor;
                            let step_up = highest_floor - next_z;

                            if gap >= 56.0 && step_up <= STEP_HEIGHT {
                                should_block = false;
                            }
                        }
                    }

                    if should_block {
                        let mut push_dir = if d < 0.001 {
                            let ld = (e - s).normalize_or_zero();
                            Vec2::new(-ld.y, ld.x)
                        } else {
                            d_v / d
                        };

                        let penetration = if push_dir.dot(old_pos - closest) < 0.0 {
                            // We crossed the wall (or the center did)
                            push_dir = -push_dir;
                            PLAYER_RADIUS + d
                        } else {
                            // Still on the correct side
                            PLAYER_RADIUS - d
                        };

                        if penetration > 0.0 {
                            next_pos += push_dir * (penetration + 0.01);

                            // Slide: Remove velocity component moving into the wall
                            let dot = next_velocity.dot(push_dir);
                            if dot < 0.0 {
                                next_velocity -= push_dir * dot;
                            }
                        }
                    }
                }
            }

            // Thing Collision (Monsters, Barrels, Solid Decor)
            for thing in &self.things {
                if thing.picked_up || thing.health <= 0.0 {
                    continue;
                }
                let is_solid = thing.is_monster()
                    || thing.is_barrel()
                    || thing.is_effect()
                    || matches!(thing.kind, 16 | 64..=69 | 71 | 84);
                if !is_solid {
                    continue;
                }

                let d_v = next_pos - thing.position;
                let d = d_v.length();
                // Assumed uniform 20.0 radius for solid DOOM things + 16.0 for player
                let min_dist = PLAYER_RADIUS + 20.0;

                if d < min_dist {
                    let mut push_dir = if d < 0.001 {
                        Vec2::new(1.0, 0.0) // Arbitrary push if exactly stacked
                    } else {
                        d_v / d
                    };

                    let penetration = min_dist - d;
                    next_pos += push_dir * (penetration + 0.01);

                    // Slide against the thing
                    let dot = next_velocity.dot(push_dir);
                    if dot < 0.0 {
                        next_velocity -= push_dir * dot;
                    }
                }
            }
        }

        // Activate linedefs after the collision loop
        for line_idx in lines_to_activate {
            let back = self.linedefs[line_idx].sector_back;
            self.activate_linedef_manual(line_idx, back, &mut cmds);
        }

        // Z-Physics (Gravity & Step Up) & Environmental Effects
        if !self.nodes.is_empty() {
            if let Some(sid) = self.find_sector_at(next_pos) {
                let sector = &mut self.sectors[sid];

                let target_z = sector.floor_height;
                let floor_diff = target_z - next_z;

                // Step UP: Floor is higher but climbable (within STEP_HEIGHT)
                if floor_diff > 0.0 && floor_diff <= STEP_HEIGHT {
                    next_z = target_z;
                // Step DOWN or level: Floor is at or below player - apply gravity
                } else if floor_diff <= 0.0 {
                    if next_z > target_z + 0.1 {
                        next_z -= 2.0;
                        if next_z < target_z {
                            next_z = target_z;
                        }
                    } else {
                        next_z = target_z;
                    }

                    // Damaging Floors and Secrets moved to unified handlers
                    // BLOCKED: Floor is too high to climb
                } else {
                    next_pos = old_pos;
                    next_velocity = Vec2::ZERO;
                }
            }
        }

        // Weapon & State Logic
        let mut next_cooldown = if self.player.fire_cooldown > 0 {
            self.player.fire_cooldown - 1
        } else {
            0
        };
        let mut next_noise = (self.player.noise_radius * 0.9).max(0.0);
        let next_damage_flash = (self.player.damage_flash - 0.05).max(0.0);
        let next_bonus_flash = (self.player.bonus_flash - 0.05).max(0.0);

        // Update Powerup Timers
        if self.player.invuln_timer > 0 {
            self.player.invuln_timer -= 1;
        }
        if self.player.radsuit_timer > 0 {
            self.player.radsuit_timer -= 1;
        }
        if self.player.lightamp_timer > 0 {
            self.player.lightamp_timer -= 1;
        }
        if self.player.invis_timer > 0 {
            self.player.invis_timer -= 1;
        }

        let mut next_weapon = self.player.current_weapon;
        if actions.contains(&GameAction::Weapon1) {
            next_weapon = WeaponType::Fist;
        }
        if actions.contains(&GameAction::Weapon2) {
            next_weapon = WeaponType::Pistol;
        }
        if actions.contains(&GameAction::Weapon3) {
            next_weapon = WeaponType::Shotgun;
        }
        if actions.contains(&GameAction::Weapon4) {
            next_weapon = WeaponType::Chaingun;
        }
        if actions.contains(&GameAction::Weapon5) {
            next_weapon = WeaponType::RocketLauncher;
        }
        if actions.contains(&GameAction::Weapon6) {
            next_weapon = WeaponType::PlasmaRifle;
        }
        if actions.contains(&GameAction::Weapon7) {
            next_weapon = WeaponType::BFG9000;
        }

        let mut next_weapon_state = self.player.weapon_state;
        let mut final_weapon = self.player.current_weapon;

        match next_weapon_state {
            WeaponState::Ready => {
                // Auto weapon swap if current is out of ammo
                let current_ammo_idx = weapon_ammo_type(self.player.current_weapon);
                if let Some(idx) = current_ammo_idx {
                    if self.player.ammo[idx] == 0 {
                        for next_w in [
                            WeaponType::BFG9000,
                            WeaponType::PlasmaRifle,
                            WeaponType::RocketLauncher,
                            WeaponType::Shotgun,
                            WeaponType::Chaingun,
                            WeaponType::Pistol,
                            WeaponType::Chainsaw,
                            WeaponType::Fist,
                        ] {
                            if self.player.owned_weapons[next_w as usize] {
                                let next_ammo_idx = weapon_ammo_type(next_w);
                                if next_ammo_idx.is_none()
                                    || self.player.ammo[next_ammo_idx.unwrap()] > 0
                                {
                                    next_weapon = next_w;
                                    break;
                                }
                            }
                        }
                    }
                }

                if next_weapon != self.player.current_weapon {
                    next_weapon_state = WeaponState::Lowering;
                } else if actions.contains(&GameAction::Fire) && next_cooldown == 0 {
                    let mut fired = false;
                    match final_weapon {
                        WeaponType::Pistol => {
                            if self.player.ammo[0] > 0 {
                                cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                                    sound_id: "DSPISTOL".into(),
                                    position: Some(next_pos),
                                    volume: 1.0,
                                }));
                                let spread = (p_random() as f32 / 255.0 - 0.5) * 0.04;
                                cmds.push(WorldCommand::FireHitscan {
                                    origin: next_pos,
                                    angle: next_angle + spread,
                                    damage: 10.0,
                                    attacker_idx: None,
                                });
                                cmds.push(WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::Pistol,
                                    amount: -1,
                                    set: false,
                                });
                                next_cooldown = 10;
                                next_weapon_state = WeaponState::Firing(4);
                                fired = true;
                            }
                        }
                        WeaponType::Shotgun => {
                            if self.player.ammo[1] > 0 {
                                cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                                    sound_id: "DSSHOTGN".into(),
                                    position: Some(next_pos),
                                    volume: 1.0,
                                }));
                                for _ in 0..7 {
                                    let spread = (p_random() as f32 / 255.0 - 0.5) * 0.15;
                                    cmds.push(WorldCommand::FireHitscan {
                                        origin: next_pos,
                                        angle: next_angle + spread,
                                        damage: 10.0,
                                        attacker_idx: None,
                                    });
                                }
                                cmds.push(WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::Shotgun,
                                    amount: -1,
                                    set: false,
                                });
                                next_cooldown = 35;
                                next_weapon_state = WeaponState::Firing(8);
                                fired = true;
                            }
                        }
                        WeaponType::Chaingun => {
                            if self.player.ammo[0] > 0 {
                                cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                                    sound_id: "DSPISTOL".into(),
                                    position: Some(next_pos),
                                    volume: 0.8,
                                }));
                                let spread = (p_random() as f32 / 255.0 - 0.5) * 0.12;
                                cmds.push(WorldCommand::FireHitscan {
                                    origin: next_pos,
                                    angle: next_angle + spread,
                                    damage: 8.0,
                                    attacker_idx: None,
                                });
                                cmds.push(WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::Pistol,
                                    amount: -1,
                                    set: false,
                                });
                                next_cooldown = 4;
                                next_weapon_state = WeaponState::Firing(2);
                                fired = true;
                            }
                        }
                        WeaponType::RocketLauncher => {
                            if self.player.ammo[2] > 0 {
                                cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                                    sound_id: "DSRLAUNC".into(),
                                    position: Some(next_pos),
                                    volume: 1.0,
                                }));
                                let r_dir = Vec2::new(next_angle.cos(), next_angle.sin());
                                cmds.push(WorldCommand::SpawnProjectile {
                                    kind: 127,
                                    position: next_pos + r_dir * 20.0,
                                    z: next_z + 28.0,
                                    velocity: r_dir * 20.0,
                                    z_velocity: 0.0,
                                    damage: 20.0,
                                    owner_is_player: true,
                                    owner_thing_idx: None,
                                });
                                cmds.push(WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::RocketLauncher,
                                    amount: -1,
                                    set: false,
                                });
                                next_cooldown = 20;
                                next_weapon_state = WeaponState::Firing(4);
                                fired = true;
                            }
                        }
                        WeaponType::PlasmaRifle => {
                            if self.player.ammo[3] > 0 {
                                cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                                    sound_id: "DSPLASMA".into(),
                                    position: Some(next_pos),
                                    volume: 1.0,
                                }));
                                let r_dir = Vec2::new(next_angle.cos(), next_angle.sin());
                                cmds.push(WorldCommand::SpawnProjectile {
                                    kind: 128,
                                    position: next_pos + r_dir * 20.0,
                                    z: next_z + 28.0,
                                    velocity: r_dir * 25.0,
                                    z_velocity: 0.0,
                                    damage: 5.0,
                                    owner_is_player: true,
                                    owner_thing_idx: None,
                                });
                                cmds.push(WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::PlasmaRifle,
                                    amount: -1,
                                    set: false,
                                });
                                next_cooldown = 3;
                                next_weapon_state = WeaponState::Firing(2);
                                fired = true;
                            }
                        }
                        WeaponType::BFG9000 => {
                            if self.player.ammo[3] >= 40 {
                                cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                                    sound_id: "DSBFG".into(),
                                    position: Some(next_pos),
                                    volume: 1.0,
                                }));
                                let r_dir = Vec2::new(next_angle.cos(), next_angle.sin());
                                cmds.push(WorldCommand::SpawnProjectile {
                                    kind: 129,
                                    position: next_pos + r_dir * 20.0,
                                    z: next_z + 28.0,
                                    velocity: r_dir * 15.0,
                                    z_velocity: 0.0,
                                    damage: 100.0,
                                    owner_is_player: true,
                                    owner_thing_idx: None,
                                });
                                cmds.push(WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::BFG9000,
                                    amount: -40,
                                    set: false,
                                });
                                next_cooldown = 40;
                                next_weapon_state = WeaponState::Firing(10);
                                fired = true;
                            }
                        }
                        WeaponType::Chainsaw => {
                            cmds.push(WorldCommand::SpawnAudioEvent(AudioEvent {
                                sound_id: "DSSAWFUL".into(),
                                position: Some(next_pos),
                                volume: 1.0,
                            }));
                            cmds.push(WorldCommand::FireHitscan {
                                origin: next_pos,
                                angle: next_angle,
                                damage: 20.0,
                                attacker_idx: None,
                            });
                            next_cooldown = 4;
                            next_weapon_state = WeaponState::Firing(2);
                            fired = true;
                        }
                        WeaponType::Fist => {
                            let dmg = if self.player.berserk_timer > 0 {
                                200.0
                            } else {
                                20.0
                            };
                            cmds.push(WorldCommand::FireHitscan {
                                origin: next_pos,
                                angle: next_angle,
                                damage: dmg,
                                attacker_idx: None,
                            });
                            next_cooldown = 15;
                            next_weapon_state = WeaponState::Firing(4);
                            fired = true;
                        }
                    }
                    if fired {
                        next_noise = NOISE_RADIUS_FIRE;

                        if let Some(sector_idx) = self.find_sector_at(next_pos) {
                            self.spread_noise(sector_idx, 3);

                            // Trigger Muzzle Flash in the current sector
                            let current_light = self.sectors[sector_idx].light_level;
                            cmds.push(WorldCommand::SetSectorState {
                                sector_idx,
                                floor: self.sectors[sector_idx].floor_height,
                                ceiling: self.sectors[sector_idx].ceiling_height,
                                light: 1.0, // Flash to full brightness
                                action: SectorAction::MuzzleFlash {
                                    timer: 0.05, // 2 tics approx
                                    original_light: current_light,
                                },
                                texture_floor: None,
                                texture_ceiling: None,
                            });
                        }
                    }
                }
            }
            WeaponState::Lowering => {
                if next_cooldown == 0 {
                    next_cooldown = 10;
                    final_weapon = next_weapon;
                    next_weapon_state = WeaponState::Raising;
                }
            }
            WeaponState::Raising => {
                if next_cooldown == 0 {
                    next_weapon_state = WeaponState::Ready;
                }
            }
            WeaponState::Firing(frames_left) => {
                if frames_left > 1 {
                    next_weapon_state = WeaponState::Firing(frames_left - 1);
                } else {
                    next_weapon_state = WeaponState::Ready;
                }
            }
            _ => {}
        }

        cmds.push(WorldCommand::UpdatePlayer {
            position: Some(next_pos),
            angle: Some(next_angle),
            velocity: Some(next_velocity),
            z: Some(next_z),
            health: None,
            armor: None,
            weapon_state: Some(next_weapon_state),
            fire_cooldown: Some(next_cooldown),
            noise_radius: Some(next_noise),
            current_weapon: Some(final_weapon),
            damage_flash: Some(next_damage_flash),
            bonus_flash: Some(next_bonus_flash),
            bob_phase: Some(next_bob),
        });

        // Check for pickups
        for (i, thing) in self.things.iter().enumerate() {
            if thing.is_pickup() && !thing.picked_up {
                let dist = (self.player.position - thing.position).length();
                if dist < PICKUP_RADIUS {
                    cmds.push(WorldCommand::PickupItem { thing_idx: i });
                }
            }
        }

        // Check Linedef Crossings (Walk Triggers)
        // Line crossing triggers are handled in the movement loop above.

        // Use Actions (Manual Triggers)
        // Use Actions (Manual Triggers)
        if actions.contains(&GameAction::Use) {
            // Simple Raycast for Use Line
            let p_pos = self.player.position;
            let reach = 128.0; // Boosted for better accessibility (standard is 64)

            let mut best_line = None;
            let mut best_dist = reach;

            for (idx, line) in self.linedefs.iter().enumerate() {
                if line.special_type == 0 {
                    continue;
                }
                let v1 = self.vertices[line.start_idx];
                let v2 = self.vertices[line.end_idx];

                let line_vec = v2 - v1;
                let line_len_sq = line_vec.length_squared();
                if line_len_sq < 1.0 {
                    continue;
                }

                let line_normal = Vec2::new(-line_vec.y, line_vec.x).normalize();
                let player_to_v1 = v1 - p_pos;
                let dist_to_line = player_to_v1.dot(line_normal).abs();

                if dist_to_line < best_dist {
                    let t = (p_pos - v1).dot(line_vec) / line_len_sq;
                    if t >= -0.1 && t <= 1.1 {
                        best_dist = dist_to_line;
                        best_line = Some(idx);
                    }
                }
            }

            if let Some(idx) = best_line {
                let line = &self.linedefs[idx];
                let mut sector_back = line.sector_back;

                if line.sector_tag == 0 && line.is_portal() {
                    let p_pos = self.player.position;
                    let v1 = self.vertices[line.start_idx];
                    let v2 = self.vertices[line.end_idx];
                    let side = (p_pos.x - v1.x) * (v2.y - v1.y) - (p_pos.y - v1.y) * (v2.x - v1.x);
                    if side > 0.0 {
                        sector_back = line.sector_back;
                    } else {
                        sector_back = line.sector_front;
                    }
                }

                log::info!(
                    "Player used line {} with special {} (target sector_back: {:?})",
                    idx,
                    line.special_type,
                    sector_back
                );
                self.activate_linedef_manual(idx, sector_back, &mut cmds);
            }
        }

        // Apply Commands
        self.apply_commands(cmds);

        // Decay HUD messages
        for msg in &mut self.hud_messages {
            msg.timer -= 1.0 / 35.0; // Decay at 35 FPS
        }
        self.hud_messages.retain(|m| m.timer > 0.0);

        // Decay temporary things (Puffs/Sparks/Projectiles)
        // IMPORTANT: Never remove things from the vector — it invalidates thinker indices.
        // Instead, mark them as picked_up so they're skipped during rendering.

        // Pre-calculate sector IDs to avoid double-borrow during thing update loop
        let sector_ids: Vec<Option<usize>> = self
            .things
            .iter()
            .map(|t| self.find_sector_at(t.position))
            .collect();

        for (i, t) in self.things.iter_mut().enumerate() {
            // Apply Gravity to all non-flying things
            if !t.is_effect() && t.health > -100.0 {
                // Determine floor height at thing position
                if let Some(sid) = sector_ids[i] {
                    if sid < self.sectors.len() {
                        let floor_z = self.sectors[sid].floor_height;
                        if t.z > floor_z + 0.1 {
                            t.z -= 4.0; // Gravity fall rate
                            if t.z < floor_z {
                                t.z = floor_z;
                            }
                        } else if t.z < floor_z - 0.1 {
                            // Snap up (step up) for monsters/items if they are on stairs
                            t.z = floor_z;
                        }
                    }
                }
            }

            if t.is_effect() {
                t.health -= 1.0;
                if t.health <= 0.0 {
                    t.picked_up = true;
                }
            }
            // Mark dead projectiles as picked_up so they don't accumulate
            if matches!(t.kind, 127 | 128 | 129 | 10031) && t.health <= 0.0 {
                t.picked_up = true;
            }
        }

        // Apply Environmental Damage (Slime/Acid) and Sector Effects (Secrets)
        self.update_environmental_damage();

        // Update Sector Actions (Elevators, Doors)
        for i in 0..self.sectors.len() {
            let s_cmds = self.sectors[i].calculate_update(1.0 / 35.0, i, self.frame_count);
            self.apply_commands(s_cmds);
        }

        // Sequential thinker updates — preserves deterministic RNG order
        // (Doom's static PRND_INDEX is not thread-safe, and determinism
        // matters for demo recording. ~20 monsters need no parallelism.)
        let thinkers: Vec<Box<dyn Thinker + Send + Sync>> = std::mem::take(&mut self.thinkers);
        let results: Vec<(Box<dyn Thinker + Send + Sync>, bool, Vec<WorldCommand>)> = thinkers
            .into_iter()
            .map(|mut t| {
                let (keep, cmds) = t.update(self);
                (t, keep, cmds)
            })
            .collect();

        for (t, keep, cmds) in results {
            self.apply_commands(cmds);
            if keep {
                self.thinkers.push(t);
            }
        }

        // Update palette based on flash state (authentic Doom PLAYPAL switching)
        if self.player.damage_flash > 0.3 && self.palettes.len() > 1 {
            self.current_palette_idx = 1; // Red palette (damage)
        } else if self.player.bonus_flash > 0.3 && self.palettes.len() > 4 {
            self.current_palette_idx = 4; // Yellow palette (bonus)
        } else {
            self.current_palette_idx = 0; // Normal palette
        }

        // NOTE: Do NOT clear audio_events here - they are consumed by the audio engine
        // in lib.rs AFTER world.update() returns, then cleared there.
    }

    fn apply_commands(&mut self, cmds: Vec<WorldCommand>) {
        for cmd in cmds {
            match cmd {
                WorldCommand::SpawnThinker(t) => self.thinkers.push(t),
                WorldCommand::SpawnAudioEvent(e) => self.audio_events.push(e),
                WorldCommand::ShowMessage {
                    text,
                    duration_secs,
                    color,
                } => {
                    self.hud_messages.push(HudMessage {
                        text,
                        timer: duration_secs,
                        color,
                    });
                }
                WorldCommand::ModifySector {
                    sector_idx,
                    floor_delta,
                    ceiling_delta,
                } => {
                    let final_floor_delta = floor_delta;
                    let mut final_ceil_delta = ceiling_delta;
                    let mut crush_damage = 0.0;
                    let mut reverse_door = false;

                    // Collision Logic (Immutable Phase)
                    {
                        // We need to check against the *proposed* new heights.
                        // But we can't easily get 'new_ceil' without accessing 's'.
                        // We can lookup 's' inside the loop safely if we don't hold it across 'find_sector_at'.

                        for i in 0..self.things.len() {
                            let (pos, z, height, health) = {
                                let t = &self.things[i];
                                let h = DEFAULT_THING_DEFS
                                    .iter()
                                    .find(|&&(k, _)| k == t.kind)
                                    .map(|&(_, d)| d.height)
                                    .unwrap_or(56.0);
                                (t.position, t.z, h, t.health)
                            };

                            // Check sector
                            if self.find_sector_at(pos) == Some(sector_idx) {
                                // Now safe to borrow sector
                                let s = &self.sectors[sector_idx];
                                let new_ceil = s.ceiling_height + ceiling_delta; // Current + delta

                                // Check Ceiling Collision
                                if z + height > new_ceil {
                                    match &s.action {
                                        SectorAction::Door { state, .. } => {
                                            if *state == DoorState::Closing {
                                                reverse_door = true;
                                                final_ceil_delta = 0.0; // Stop movement
                                            }
                                        }
                                        SectorAction::Crusher { damage, .. } => {
                                            if health > 0.0 {
                                                crush_damage = *damage;
                                                // Clamp to thing top
                                                let clamp_delta = (z + height) - s.ceiling_height;
                                                if clamp_delta > final_ceil_delta {
                                                    // Usually negative when moving down
                                                    final_ceil_delta = clamp_delta;
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }

                    // Mutation Phase
                    if let Some(s) = self.sectors.get_mut(sector_idx) {
                        s.floor_height += final_floor_delta;
                        s.ceiling_height += final_ceil_delta;

                        if reverse_door {
                            if let SectorAction::Door { state, .. } = &mut s.action {
                                *state = DoorState::Opening;
                            }
                        }
                    }

                    // Apply deferred damage if any
                    if crush_damage > 0.0 {
                        // Apply damage to things in sector that are crushing
                        let mut targets = Vec::new();
                        for i in 0..self.things.len() {
                            let (pos, z, height) = {
                                let t = &self.things[i];
                                let h = DEFAULT_THING_DEFS
                                    .iter()
                                    .find(|&&(k, _)| k == t.kind)
                                    .map(|&(_, d)| d.height)
                                    .unwrap_or(56.0);
                                (t.position, t.z, h)
                            };
                            if self.find_sector_at(pos) == Some(sector_idx) {
                                // Need to re-access sector height?
                                // self.sectors is borrowed mutably? No, scope ended?
                                // Wait, self.sectors[sector_idx] usage in loop 's' borrow ended?
                                // Line 1083 '}' ended 'if let Some(s) ...'.
                                // So self.sectors is free.
                                if z + height > (self.sectors[sector_idx].ceiling_height - 0.1) {
                                    targets.push(i);
                                }
                            }
                        }
                        for idx in targets {
                            self.apply_commands(vec![WorldCommand::DamageThing {
                                thing_idx: idx,
                                amount: crush_damage,
                                inflictor_idx: None,
                            }]);
                        }
                    }
                }
                WorldCommand::SetSectorState {
                    sector_idx,
                    floor,
                    ceiling,
                    light,
                    action,
                    texture_floor,
                    texture_ceiling,
                } => {
                    if let Some(s) = self.sectors.get_mut(sector_idx) {
                        s.floor_height = floor;
                        s.ceiling_height = ceiling;
                        s.light_level = light;
                        s.action = action;
                        if let Some(tex) = texture_floor {
                            s.texture_floor = tex;
                        }
                        if let Some(tex) = texture_ceiling {
                            s.texture_ceiling = tex;
                        }
                    }
                }
                WorldCommand::ModifyThing {
                    thing_idx,
                    pos_delta,
                    z_delta,
                    angle,
                } => {
                    if let Some(t) = self.things.get_mut(thing_idx) {
                        t.position += pos_delta;
                        t.z += z_delta;
                        t.angle = angle;
                    }
                }
                WorldCommand::SetThingHealth { thing_idx, health } => {
                    if let Some(t) = self.things.get_mut(thing_idx) {
                        t.health = health;
                    }
                }
                WorldCommand::UpdatePlayer {
                    position,
                    angle,
                    velocity,
                    z,
                    health,
                    armor,
                    weapon_state,
                    fire_cooldown,
                    noise_radius,
                    current_weapon,
                    damage_flash,
                    bonus_flash,
                    bob_phase,
                } => {
                    if let Some(pos) = position {
                        self.player.position = pos;
                    }
                    if let Some(ang) = angle {
                        self.player.angle = ang;
                    }
                    if let Some(vel) = velocity {
                        self.player.velocity = vel;
                    }
                    if let Some(zv) = z {
                        self.player.z = zv;
                    }
                    if let Some(h) = health {
                        self.player.health = h;
                    }
                    if let Some(a) = armor {
                        self.player.armor = a;
                    }
                    if let Some(ws) = weapon_state {
                        self.player.weapon_state = ws;
                    }
                    if let Some(fc) = fire_cooldown {
                        self.player.fire_cooldown = fc;
                    }
                    if let Some(nr) = noise_radius {
                        self.player.noise_radius = nr;
                    }
                    if let Some(cw) = current_weapon {
                        self.player.current_weapon = cw;
                    }
                    if let Some(df) = damage_flash {
                        self.player.damage_flash = df;
                    }
                    if let Some(bf) = bonus_flash {
                        self.player.bonus_flash = bf;
                    }
                    if let Some(bp) = bob_phase {
                        self.player.bob_phase = bp;
                    }
                }
                WorldCommand::UpdatePlayerAmmo {
                    weapon,
                    amount,
                    set,
                } => {
                    let idx = match weapon {
                        WeaponType::Pistol | WeaponType::Chaingun => 0,
                        WeaponType::Shotgun => 1,
                        WeaponType::RocketLauncher => 2,
                        WeaponType::PlasmaRifle | WeaponType::BFG9000 => 3,
                        _ => 0,
                    };
                    if set {
                        self.player.ammo[idx] = amount as u32;
                    } else {
                        self.player.ammo[idx] =
                            (self.player.ammo[idx] as i32 + amount).max(0) as u32;
                    }
                }
                WorldCommand::PickupItem { thing_idx } => {
                    if let Some(thing) = self.things.get(thing_idx) {
                        let kind = thing.kind;
                        let mut success = false;

                        match kind {
                            // Health
                            2011 => {
                                if self.player.health < 100.0 {
                                    self.player.health = (self.player.health + 10.0).min(100.0);
                                    success = true;
                                }
                            }
                            2012 => {
                                if self.player.health < 100.0 {
                                    self.player.health = (self.player.health + 25.0).min(100.0);
                                    success = true;
                                }
                            }
                            2013 => {
                                self.player.health = (self.player.health + 100.0).min(200.0);
                                success = true;
                            }
                            2014 => {
                                self.player.health = (self.player.health + 1.0).min(200.0);
                                success = true;
                            }
                            2045 => {
                                self.player.lightamp_timer = 4200;
                                success = true;
                            }

                            // Powerups
                            2022 => {
                                self.player.invuln_timer = 1050;
                                success = true;
                            }
                            2023 => {
                                self.player.berserk_timer = 40000;
                                self.player.health = 100.0;
                                success = true;
                            }
                            2024 => {
                                self.player.invis_timer = 2100;
                                success = true;
                            }
                            2025 => {
                                self.player.radsuit_timer = 2100;
                                success = true;
                            }
                            2026 => {
                                self.is_automap_follow = true;
                                success = true;
                            }

                            // Armor
                            2018 => {
                                if self.player.armor < 100.0 {
                                    self.player.armor = 100.0;
                                    success = true;
                                }
                            }
                            2019 => {
                                if self.player.armor < 200.0 {
                                    self.player.armor = 200.0;
                                    success = true;
                                }
                            }
                            2015 => {
                                self.player.armor = (self.player.armor + 1.0).min(200.0);
                                success = true;
                            }

                            // Weapons
                            2001 => {
                                self.player.owned_weapons[WeaponType::Shotgun as usize] = true;
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::Shotgun,
                                    amount: 8,
                                    set: false,
                                }]);
                                if (self.player.current_weapon as usize)
                                    < WeaponType::Shotgun as usize
                                {
                                    self.player.current_weapon = WeaponType::Shotgun;
                                    self.player.weapon_state = WeaponState::Raising;
                                    self.player.fire_cooldown = 10;
                                }
                                success = true;
                            }
                            2002 => {
                                self.player.owned_weapons[WeaponType::Chaingun as usize] = true;
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::Pistol,
                                    amount: 20,
                                    set: false,
                                }]);
                                if (self.player.current_weapon as usize)
                                    < WeaponType::Chaingun as usize
                                {
                                    self.player.current_weapon = WeaponType::Chaingun;
                                    self.player.weapon_state = WeaponState::Raising;
                                    self.player.fire_cooldown = 10;
                                }
                                success = true;
                            }
                            2003 => {
                                self.player.owned_weapons[WeaponType::RocketLauncher as usize] =
                                    true;
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::RocketLauncher,
                                    amount: 2,
                                    set: false,
                                }]);
                                if (self.player.current_weapon as usize)
                                    < WeaponType::RocketLauncher as usize
                                {
                                    self.player.current_weapon = WeaponType::RocketLauncher;
                                    self.player.weapon_state = WeaponState::Raising;
                                    self.player.fire_cooldown = 10;
                                }
                                success = true;
                            }
                            2004 => {
                                self.player.owned_weapons[WeaponType::PlasmaRifle as usize] = true;
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::PlasmaRifle,
                                    amount: 40,
                                    set: false,
                                }]);
                                if (self.player.current_weapon as usize)
                                    < WeaponType::PlasmaRifle as usize
                                {
                                    self.player.current_weapon = WeaponType::PlasmaRifle;
                                    self.player.weapon_state = WeaponState::Raising;
                                    self.player.fire_cooldown = 10;
                                }
                                success = true;
                            }
                            2005 => {
                                self.player.owned_weapons[WeaponType::Chainsaw as usize] = true;
                                // Chainsaw is usually preferred over fist/pistol
                                if self.player.current_weapon == WeaponType::Fist
                                    || self.player.current_weapon == WeaponType::Pistol
                                {
                                    self.player.current_weapon = WeaponType::Chainsaw;
                                    self.player.weapon_state = WeaponState::Raising;
                                    self.player.fire_cooldown = 10;
                                }
                                success = true;
                            }
                            2006 => {
                                self.player.owned_weapons[WeaponType::BFG9000 as usize] = true;
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::BFG9000,
                                    amount: 40,
                                    set: false,
                                }]);
                                if (self.player.current_weapon as usize)
                                    < WeaponType::BFG9000 as usize
                                {
                                    self.player.current_weapon = WeaponType::BFG9000;
                                    self.player.weapon_state = WeaponState::Raising;
                                    self.player.fire_cooldown = 10;
                                }
                                success = true;
                            }

                            // Ammo
                            2007 => {
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::Pistol,
                                    amount: 10,
                                    set: false,
                                }]);
                                success = true;
                            }
                            2048 => {
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::Pistol,
                                    amount: 50,
                                    set: false,
                                }]);
                                success = true;
                            }
                            2008 => {
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::Shotgun,
                                    amount: 4,
                                    set: false,
                                }]);
                                success = true;
                            }
                            2049 => {
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::Shotgun,
                                    amount: 20,
                                    set: false,
                                }]);
                                success = true;
                            }
                            2010 => {
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::RocketLauncher,
                                    amount: 1,
                                    set: false,
                                }]);
                                success = true;
                            }
                            2046 => {
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::RocketLauncher,
                                    amount: 5,
                                    set: false,
                                }]);
                                success = true;
                            }
                            2047 => {
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::PlasmaRifle,
                                    amount: 20,
                                    set: false,
                                }]);
                                success = true;
                            }
                            17 => {
                                self.apply_commands(vec![WorldCommand::UpdatePlayerAmmo {
                                    weapon: WeaponType::PlasmaRifle,
                                    amount: 100,
                                    set: false,
                                }]);
                                success = true;
                            }

                            // Keys
                            5 | 40 => {
                                self.player.keys[1] = true;
                                success = true;
                            }
                            6 | 39 => {
                                self.player.keys[2] = true;
                                success = true;
                            }
                            13 | 38 => {
                                self.player.keys[0] = true;
                                success = true;
                            }

                            _ => {
                                success = true;
                            }
                        }

                        if success {
                            if let Some(thing) = self.things.get_mut(thing_idx) {
                                thing.picked_up = true;
                            }
                            self.player.bonus_flash = 0.4;
                            self.audio_events.push(AudioEvent {
                                sound_id: "DSGETPOW".into(),
                                position: None,
                                volume: 1.0,
                            });
                        }
                    }
                }
                WorldCommand::DamageThing {
                    thing_idx,
                    amount,
                    inflictor_idx,
                } => {
                    let t_kind = self.things.get(thing_idx).map(|t| t.kind).unwrap_or(0);
                    let i_kind =
                        inflictor_idx.and_then(|idx| self.things.get(idx).map(|th| th.kind));

                    if let Some(t) = self.things.get_mut(thing_idx) {
                        t.health -= amount;
                        for thinker in &mut self.thinkers {
                            thinker.on_pain(thing_idx, t_kind, inflictor_idx, i_kind);
                        }
                    }
                }
                WorldCommand::DamagePlayer { amount, angle } => {
                    if self.player.invuln_timer == 0 {
                        let absorbed = (amount * 0.333).min(self.player.armor);
                        self.player.armor -= absorbed;
                        self.player.health -= amount - absorbed;
                        if self.player.damage_flash < 0.1 {
                            self.player.damage_flash = 0.5;
                        }
                        self.player.last_damage_angle = angle;
                    }
                }
                WorldCommand::DamageThingsInSector { sector_idx, amount } => {
                    // Check player
                    if let Some(sid) = self.find_sector_at(self.player.position) {
                        if sid == sector_idx {
                            if self.player.invuln_timer == 0 {
                                let absorbed = (amount * 0.333).min(self.player.armor);
                                self.player.armor -= absorbed;
                                self.player.health -= amount - absorbed;
                                if self.player.damage_flash < 0.1 {
                                    self.player.damage_flash = 0.5;
                                }
                            }
                        }
                    }

                    // Check monsters
                    let mut impacts = Vec::new();
                    for (i, t) in self.things.iter().enumerate() {
                        if !t.is_monster() || t.health <= 0.0 || t.picked_up {
                            continue;
                        }
                        if let Some(sid) = self.find_sector_at(t.position) {
                            if sid == sector_idx {
                                impacts.push((i, t.kind));
                            }
                        }
                    }
                    for (idx, kind) in impacts {
                        if let Some(t) = self.things.get_mut(idx) {
                            t.health -= amount;
                        }
                        for thinker in &mut self.thinkers {
                            thinker.on_pain(idx, kind, None, None);
                        }
                    }
                }
                WorldCommand::FireHitscan {
                    origin,
                    angle,
                    damage,
                    attacker_idx,
                } => self.fire_hitscan(origin, angle, damage, attacker_idx),
                WorldCommand::SplashDamage {
                    center,
                    damage,
                    radius,
                    owner_is_player,
                } => {
                    // Collect impacted monsters first to avoid borrow conflicts
                    let mut impacts = Vec::new();
                    for (i, thing) in self.things.iter().enumerate() {
                        if thing.picked_up || thing.health <= 0.0 {
                            continue;
                        }
                        if !thing.is_monster() && !thing.is_barrel() && !(owner_is_player && i == 0)
                        {
                            continue;
                        }
                        let dist = (thing.position - center).length();
                        if dist < radius {
                            let dmg = damage * ((radius - dist) / radius);
                            if dmg > 0.0 {
                                impacts.push((i, dmg, thing.kind));
                            }
                        }
                    }

                    let mut sorted_things: Vec<(usize, &Thing)> =
                        self.things.iter().enumerate().collect();
                    sorted_things.sort_by(|a, b| {
                        let d1 = (a.1.position - center).length_squared();
                        let d2 = (b.1.position - center).length_squared();
                        // Defensive: handle NaN positions gracefully
                        match (d1.is_nan(), d2.is_nan()) {
                            (true, true) => std::cmp::Ordering::Equal,
                            (true, false) => std::cmp::Ordering::Greater,
                            (false, true) => std::cmp::Ordering::Less,
                            (false, false) => {
                                d2.partial_cmp(&d1).unwrap_or(std::cmp::Ordering::Equal)
                            }
                        }
                    });

                    for (idx, dmg, kind) in impacts {
                        if let Some(t) = self.things.get_mut(idx) {
                            t.health -= dmg;
                        }
                        for thinker in &mut self.thinkers {
                            thinker.on_pain(idx, kind, None, None);
                        }
                    }

                    let p_dist = (self.player.position - center).length();
                    if p_dist < radius {
                        let p_dmg = damage * ((radius - p_dist) / radius);
                        if p_dmg > 0.0 {
                            if self.player.invuln_timer == 0 {
                                let absorbed = (p_dmg * 0.333).min(self.player.armor);
                                self.player.armor -= absorbed;
                                self.player.health -= p_dmg - absorbed;
                                self.player.damage_flash =
                                    (self.player.damage_flash + 0.5).min(1.0);
                            }
                        }
                    }
                }
                WorldCommand::SpawnThing {
                    kind,
                    position,
                    z,
                    angle,
                } => {
                    let thing = Thing {
                        position,
                        z,
                        angle,
                        kind,
                        flags: 0,
                        health: if kind == 9999 || kind == 9998 || kind == 9997 {
                            4.0
                        } else {
                            50.0
                        },
                        picked_up: false,
                        state_idx: 0,
                        ai_timer: 0,
                        target_thing_idx: None,
                        attack_cooldown: 0,
                    };
                    self.spawn_effect_thing(thing);
                }
                WorldCommand::SpawnProjectile {
                    kind,
                    position,
                    z,
                    velocity,
                    z_velocity,
                    damage,
                    owner_is_player,
                    owner_thing_idx,
                } => {
                    let proj_thing = Thing {
                        position,
                        z,
                        angle: 0.0,
                        kind,
                        flags: 0,
                        health: 100.0,
                        picked_up: false,
                        state_idx: 0,
                        ai_timer: 0,
                        target_thing_idx: None,
                        attack_cooldown: 0,
                    };
                    let idx = self.spawn_effect_thing(proj_thing);
                    self.thinkers.push(Box::new(ProjectileThinker {
                        thing_idx: idx,
                        position,
                        z,
                        velocity,
                        z_velocity,
                        damage,
                        owner_is_player,
                        owner_thing_idx,
                    }));
                }
                WorldCommand::InflictPain {
                    thing_idx,
                    inflictor_idx,
                } => {
                    let t_kind = self.things.get(thing_idx).map(|t| t.kind).unwrap_or(0);
                    let i_kind = inflictor_idx.and_then(|idx| self.things.get(idx).map(|t| t.kind));
                    for t in &mut self.thinkers {
                        t.on_pain(thing_idx, t_kind, inflictor_idx, i_kind);
                    }
                }
                WorldCommand::Win => self.is_win = true,
                WorldCommand::RespawnPlayer => {
                    self.player.health = 100.0;
                    self.player.position = self.player_start_pos;
                    self.player.velocity = Vec2::ZERO;
                    self.player.invuln_timer = 105;
                    if !self.nodes.is_empty() {
                        let sidx =
                            self.find_subsector(self.player.position.x, self.player.position.y);
                        if let Some(ss) = self.subsectors.get(sidx) {
                            if let Some(seg) = self.segs.get(ss.first_seg_idx) {
                                if let Some(sid) = self.linedefs[seg.linedef_idx].sector_front {
                                    self.player.z = self.sectors[sid].floor_height;
                                }
                            }
                        }
                    }
                }
                WorldCommand::SyncAiState {
                    thing_idx,
                    state_idx,
                    timer,
                    target,
                    cooldown,
                } => {
                    if let Some(t) = self.things.get_mut(thing_idx) {
                        t.state_idx = state_idx;
                        t.ai_timer = timer;
                        t.target_thing_idx = target;
                        t.attack_cooldown = cooldown;
                    }
                }
            }
        }
    }

    fn activate_linedef_manual(
        &mut self,
        line_idx: usize,
        override_back: Option<usize>,
        cmds: &mut Vec<WorldCommand>,
    ) {
        if line_idx >= self.linedefs.len() {
            return;
        }
        let (special, tag, activated) = {
            let line = &self.linedefs[line_idx];
            (line.special_type, line.sector_tag, line.activated)
        };

        let is_repeatable = matches!(special,
            1 | 117 | 26 | 27 | 28 | 32 | 33 | 34 |
            72..=80 | 86..=88 | 90 | 91 | 97 | 105..=107 | 120 | 126 | 128 | 129 |
            11 | 51 | 52 | 100 | 127 | 141 | 48
        );

        if activated && !is_repeatable {
            return;
        }

        let mut changed_tex = false;
        {
            let line = &mut self.linedefs[line_idx];
            let check_and_toggle = |tex: &mut Option<String>| {
                if let Some(t) = tex {
                    let up = t.to_uppercase();
                    if up.starts_with("SW1") {
                        *t = t.replace("SW1", "SW2");
                        true
                    } else if up.starts_with("SW2") {
                        *t = t.replace("SW2", "SW1");
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            };
            if let Some(front) = &mut line.front {
                if check_and_toggle(&mut front.texture_middle) {
                    changed_tex = true;
                }
                if check_and_toggle(&mut front.texture_upper) {
                    changed_tex = true;
                }
                if check_and_toggle(&mut front.texture_lower) {
                    changed_tex = true;
                }
            }
        }

        if changed_tex {
            self.audio_events.push(AudioEvent {
                sound_id: "DSSWTCHN".into(),
                position: Some(self.player.position),
                volume: 1.0,
            });
        }

        self.activate_linedef(special, tag, override_back, cmds);
        self.linedefs[line_idx].activated = true;
    }

    fn activate_linedef(
        &mut self,
        special: u16,
        tag: u16,
        sector_back: Option<usize>,
        cmds: &mut Vec<WorldCommand>,
    ) {
        log::info!("Activated Linedef Special: {} Tag: {}", special, tag);

        match special {
            1 | 117 | 31 | 118 | 46 | 103 | 61 | 114 | 115 => {
                let (speed, wait) = match special {
                    118 | 114 | 115 => (16.0, 4.0),
                    _ => (4.0, 4.0),
                };

                // DR Doors: If tag is 0, it affects the sector on the other side of the line.
                // If tag is NOT 0, it affects all sectors with that tag.
                if tag == 0 {
                    if let Some(sid) = sector_back {
                        if self.trigger_door(sid, speed, wait) {
                            let sound = if speed > 4.0 { "DSBDOPN" } else { "DSDOROPN" };
                            self.audio_events.push(AudioEvent {
                                sound_id: sound.into(),
                                position: Some(self.player.position),
                                volume: 1.0,
                            });
                        }
                    }
                } else {
                    if self.do_door_tagged(tag, speed, wait) {
                        let sound = if speed > 4.0 { "DSBDOPN" } else { "DSDOROPN" };
                        self.audio_events.push(AudioEvent {
                            sound_id: sound.into(),
                            position: Some(self.player.position),
                            volume: 1.0,
                        });
                    }
                }
            }
            11 | 51 | 52 => {
                // Exit Level
                log::info!("EXIT LEVEL ACTIVATED!");
                self.is_intermission = true;
                self.intermission_tic = 0;
                self.audio_events.push(AudioEvent {
                    sound_id: "DSPISTOL".into(),
                    position: None,
                    volume: 1.0,
                });
            }
            26 | 32 => {
                if self.player.keys[1] {
                    let (speed, wait) = (2.0, 4.0);
                    if tag == 0 {
                        if let Some(sid) = sector_back {
                            if self.trigger_door(sid, speed, wait) {
                                self.audio_events.push(AudioEvent {
                                    sound_id: "DSDOROPN".into(),
                                    position: Some(self.player.position),
                                    volume: 1.0,
                                });
                            }
                        }
                    } else {
                        if self.do_door_tagged(tag, speed, wait) {
                            self.audio_events.push(AudioEvent {
                                sound_id: "DSDOROPN".into(),
                                position: Some(self.player.position),
                                volume: 1.0,
                            });
                        }
                    }
                } else {
                    self.audio_events.push(AudioEvent {
                        sound_id: "DSOOF".into(),
                        position: Some(self.player.position),
                        volume: 1.0,
                    });
                    log::info!("Blue Key Required!");
                }
            }
            27 | 34 => {
                if self.player.keys[2] {
                    let (speed, wait) = (2.0, 4.0);
                    if tag == 0 {
                        if let Some(sid) = sector_back {
                            if self.trigger_door(sid, speed, wait) {
                                self.audio_events.push(AudioEvent {
                                    sound_id: "DSDOROPN".into(),
                                    position: Some(self.player.position),
                                    volume: 1.0,
                                });
                            }
                        }
                    } else {
                        if self.do_door_tagged(tag, speed, wait) {
                            self.audio_events.push(AudioEvent {
                                sound_id: "DSDOROPN".into(),
                                position: Some(self.player.position),
                                volume: 1.0,
                            });
                        }
                    }
                } else {
                    self.audio_events.push(AudioEvent {
                        sound_id: "DSOOF".into(),
                        position: Some(self.player.position),
                        volume: 1.0,
                    });
                    log::info!("Yellow Key Required!");
                }
            }
            28 | 33 => {
                if self.player.keys[0] {
                    let (speed, wait) = (2.0, 4.0);
                    if tag == 0 {
                        if let Some(sid) = sector_back {
                            if self.trigger_door(sid, speed, wait) {
                                self.audio_events.push(AudioEvent {
                                    sound_id: "DSDOROPN".into(),
                                    position: Some(self.player.position),
                                    volume: 1.0,
                                });
                            }
                        }
                    } else {
                        if self.do_door_tagged(tag, speed, wait) {
                            self.audio_events.push(AudioEvent {
                                sound_id: "DSDOROPN".into(),
                                position: Some(self.player.position),
                                volume: 1.0,
                            });
                        }
                    }
                } else {
                    self.audio_events.push(AudioEvent {
                        sound_id: "DSOOF".into(),
                        position: Some(self.player.position),
                        volume: 1.0,
                    });
                    log::info!("Red Key Required!");
                }
            }
            88 => {
                self.do_lift_tagged(tag);
                self.audio_events.push(AudioEvent {
                    sound_id: "DSPSTART".into(),
                    position: Some(self.player.position),
                    volume: 1.0,
                });
            }
            39 | 97 => {
                // Teleport (W1 / WR)
                let mut best_dest = None;
                for (s_idx, s) in self.sectors.iter().enumerate() {
                    if s.tag == tag as i16 {
                        for t in &self.things {
                            if t.kind == 14 {
                                let ss_idx = self.find_subsector(t.position.x, t.position.y);
                                if let Some(ss) = self.subsectors.get(ss_idx) {
                                    if let Some(seg) = self.segs.get(ss.first_seg_idx) {
                                        if let Some(sid) =
                                            self.linedefs[seg.linedef_idx].sector_front
                                        {
                                            if sid == s_idx {
                                                best_dest = Some(t);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if best_dest.is_some() {
                            break;
                        }
                    }
                }

                if let Some(dest) = best_dest {
                    // TELEFRAG nearby things at destination
                    for i in 0..self.things.len() {
                        let dist = (self.things[i].position - dest.position).length();
                        if dist < 32.0 {
                            cmds.push(WorldCommand::DamageThing {
                                thing_idx: i,
                                amount: 10000.0,
                                inflictor_idx: None,
                            });
                        }
                    }
                    self.player.position = dest.position;
                    self.player.angle = dest.angle;
                    self.player.velocity = Vec2::ZERO;
                    // Snap to destination sector floor
                    if let Some(s_idx) = self.find_sector_at(dest.position) {
                        self.player.z = self.sectors[s_idx].floor_height;
                    }
                    self.audio_events.push(AudioEvent {
                        sound_id: "DSTELEPT".into(),
                        position: Some(self.player.position),
                        volume: 1.0,
                    });
                }
            }
            6 | 25 | 77 | 141 => {
                // Crusher Specials
                let (speed, damage) = match special {
                    6 | 77 => (2.0, 100.0),
                    25 => (0.5, 10.0),
                    141 => (0.5, 1000.0),
                    _ => (2.0, 100.0),
                };
                self.do_crusher_tagged(tag, speed, damage);
                self.audio_events.push(AudioEvent {
                    sound_id: "DSPSTART".into(),
                    position: Some(self.player.position),
                    volume: 1.0,
                });
            }
            8 | 127 | 100 => {
                // Stair Specials
                let step = if special == 8 { 8.0 } else { 16.0 };
                self.do_stairs_tagged(tag, step);
                self.audio_events.push(AudioEvent {
                    sound_id: "DSPSTART".into(),
                    position: Some(self.player.position),
                    volume: 1.0,
                });
            }
            48 => {
                // Scrolling Texture (Left) - Handled globally in some engines, but we can flag it
                log::info!("Sidedef scrolling (Special 48) active for tag {}", tag);
            }
            _ => {
                log::warn!("Unimplemented Special: {}", special);
            }
        }
    }

    fn find_lowest_adjacent_ceiling(&self, sector_idx: usize) -> f32 {
        let mut min_ceil = f32::MAX;
        let mut found = false;
        let floor = self.sectors[sector_idx].floor_height;

        if let Some(adjs) = self.adjacent_sectors.get(sector_idx) {
            for &adj_idx in adjs {
                if adj_idx < self.sectors.len() {
                    let adj_ceil = self.sectors[adj_idx].ceiling_height;
                    // Standard Doom: Only consider adjacent ceilings that are actually above the current floor
                    // to avoid getting stuck by a neighboring closed door.
                    if adj_ceil > floor + 16.0 {
                        // Increased threshold to skip closed doors more reliably
                        if adj_ceil < min_ceil {
                            min_ceil = adj_ceil;
                            found = true;
                        }
                    }
                }
            }
        }

        // Safety: Ensure doors ALWAYS open to at least 88 units above their floor.
        // Standard player height is 56, so 88 gives plenty of room.
        let min_safe_height = floor + 88.0;

        if !found || min_ceil < min_safe_height {
            log::info!(
                "WadLoader: No suitable high adjacent ceiling for sector {}, using safe height {}",
                sector_idx,
                min_safe_height
            );
            min_safe_height
        } else {
            min_ceil
        }
    }

    fn trigger_door(&mut self, sector_idx: usize, speed: f32, wait: f32) -> bool {
        let (floor, ceil, action) = {
            let s = &self.sectors[sector_idx];
            (s.floor_height, s.ceiling_height, s.action.clone())
        };
        log::info!(
            "DEBUG: trigger_door called for sector {} (ceil={}, floor={}, action={:?})",
            sector_idx,
            ceil,
            floor,
            action
        );

        match action {
            SectorAction::None => {
                if ceil <= floor + 4.0 {
                    // Opening
                    let target = self.find_lowest_adjacent_ceiling(sector_idx) - 4.0;
                    log::info!(
                        "DEBUG: Door opening in sector {} to height {}",
                        sector_idx,
                        target
                    );
                    self.sectors[sector_idx].action = SectorAction::Door {
                        state: DoorState::Opening,
                        wait_timer: wait,
                        speed,
                        open_height: target,
                        close_height: floor,
                    };
                    return true;
                } else {
                    // Closing
                    log::info!("DEBUG: Door closing in sector {}", sector_idx);
                    self.sectors[sector_idx].action = SectorAction::Door {
                        state: DoorState::Closing,
                        wait_timer: 0.0,
                        speed,
                        open_height: ceil,
                        close_height: floor,
                    };
                    return true;
                }
            }
            SectorAction::Door {
                state,
                close_height,
                open_height,
                ..
            } => match state {
                DoorState::Waiting => {
                    log::info!("DEBUG: Door closing early in sector {}", sector_idx);
                    self.sectors[sector_idx].action = SectorAction::Door {
                        state: DoorState::Closing,
                        wait_timer: 0.0,
                        speed,
                        open_height,
                        close_height,
                    };
                    return true;
                }
                DoorState::Closing => {
                    log::info!("DEBUG: Door reversing to open in sector {}", sector_idx);
                    self.sectors[sector_idx].action = SectorAction::Door {
                        state: DoorState::Opening,
                        wait_timer: wait,
                        speed,
                        open_height,
                        close_height,
                    };
                    return true;
                }
                _ => {
                    log::info!(
                        "DEBUG: Door in sector {} is already busy in state {:?}",
                        sector_idx,
                        state
                    );
                    false
                }
            },
            _ => {
                log::info!(
                    "DEBUG: Sector {} is busy with non-door action: {:?}",
                    sector_idx,
                    action
                );
                false
            }
        }
    }

    fn do_door_tagged(&mut self, tag: u16, speed: f32, wait: f32) -> bool {
        let mut triggered = false;
        for i in 0..self.sectors.len() {
            if self.sectors[i].tag == tag as i16 {
                if self.trigger_door(i, speed, wait) {
                    triggered = true;
                }
            }
        }
        triggered
    }

    fn do_lift_tagged(&mut self, tag: u16) {
        for s in &mut self.sectors {
            if s.tag == tag as i16 {
                if let SectorAction::None = s.action {
                    let target = s.floor_height - 72.0;
                    s.action = SectorAction::Lift {
                        state: LiftState::GoingDown,
                        wait_timer: 3.0,
                        speed: 3.0,
                        low_height: target,
                        high_height: s.floor_height,
                    };
                }
            }
        }
    }

    fn do_crusher_tagged(&mut self, tag: u16, speed: f32, damage: f32) {
        for s in &mut self.sectors {
            if s.tag == tag as i16 {
                if let SectorAction::None = s.action {
                    s.action = SectorAction::Crusher {
                        state: CrusherState::GoingDown,
                        speed,
                        low_height: s.floor_height + 8.0,
                        high_height: s.ceiling_height,
                        damage,
                    };
                }
            }
        }
    }

    fn do_stairs_tagged(&mut self, tag: u16, step_height: f32) {
        // Vanilla Doom stair building: start from tagged sector(s), then chain
        // adjacent sectors that share the same floor texture.
        let mut start_sectors = Vec::new();
        for i in 0..self.sectors.len() {
            if self.sectors[i].tag == tag as i16 {
                start_sectors.push(i);
            }
        }

        for start_sid in start_sectors {
            let floor_tex = self.sectors[start_sid].texture_floor.clone();
            let mut current_height = self.sectors[start_sid].floor_height + step_height;
            self.sectors[start_sid].action = SectorAction::FloorMove {
                target_height: current_height,
                speed: 2.0,
            };

            let mut current_sid = start_sid;
            let mut visited = std::collections::HashSet::new();
            visited.insert(start_sid);

            // Chain adjacent sectors with same floor texture
            loop {
                let mut next_sid = None;
                for line in &self.linedefs {
                    let (fs, bs) = match (line.sector_front, line.sector_back) {
                        (Some(f), Some(b)) => (f, b),
                        _ => continue,
                    };
                    let neighbor = if fs == current_sid {
                        bs
                    } else if bs == current_sid {
                        fs
                    } else {
                        continue;
                    };
                    if visited.contains(&neighbor) {
                        continue;
                    }
                    if self.sectors[neighbor].texture_floor == floor_tex {
                        next_sid = Some(neighbor);
                        break;
                    }
                }
                match next_sid {
                    Some(sid) => {
                        visited.insert(sid);
                        current_height += step_height;
                        self.sectors[sid].action = SectorAction::FloorMove {
                            target_height: current_height,
                            speed: 2.0,
                        };
                        current_sid = sid;
                    }
                    None => break,
                }
            }
        }
    }

    fn update_environmental_damage(&mut self) {
        // 1. Secret Detection (Every frame)
        if let Some(s_idx) = self.find_sector_at(self.player.position) {
            if s_idx < self.sectors.len() {
                let sector = &mut self.sectors[s_idx];
                if sector.special_type == 9 && !sector.secret_found {
                    sector.secret_found = true;
                    self.secrets_found += 1;
                    log::info!("SECRET FOUND in sector {}!", s_idx);
                    self.hud_messages.push(HudMessage {
                        text: "SECRET FOUND!".into(),
                        timer: 2.0,
                        color: [255, 255, 0],
                    });
                    self.audio_events.push(AudioEvent {
                        sound_id: "DSGETPOW".into(),
                        position: None,
                        volume: 1.0,
                    });
                }
            }
        }

        // 2. Damage (Every 32 frames)
        if self.frame_count % 32 != 0 {
            return;
        }

        let mut damage_targets = Vec::new();

        // Check Player
        if let Some(s_idx) = self.find_sector_at(self.player.position) {
            if s_idx < self.sectors.len() {
                let special = self.sectors[s_idx].special_type;
                let damage = match special {
                    5 => 10,
                    7 => 5,
                    16 => 20,
                    4 => 20,
                    11 => 20,
                    _ => 0,
                };

                if damage > 0 {
                    if self.player.radsuit_timer == 0 {
                        log::info!(
                            "Player taking slime damage: {} (Sector Special {})",
                            damage,
                            special
                        );
                        damage_targets.push((true, 0, damage as f32));
                    }
                }
            }
        }

        // Check Monsters/Barrels
        for (i, t) in self.things.iter().enumerate() {
            if t.health <= 0.0 || t.picked_up || (!t.is_monster() && !t.is_barrel()) {
                continue;
            }
            if let Some(s_idx) = self.find_sector_at(t.position) {
                if s_idx < self.sectors.len() {
                    let special = self.sectors[s_idx].special_type;
                    let damage = match special {
                        5 | 7 | 16 | 4 | 11 => 5,
                        _ => 0,
                    };
                    if damage > 0 {
                        log::info!(
                            "Thing {} taking slime damage: {} (Sector Special {})",
                            i,
                            damage,
                            special
                        );
                        damage_targets.push((false, i, damage as f32));
                    }
                }
            }
        }

        let mut cmds = Vec::new();
        for (is_player, idx, amount) in damage_targets {
            if is_player {
                cmds.push(WorldCommand::DamagePlayer {
                    amount,
                    angle: None,
                });
            } else {
                cmds.push(WorldCommand::DamageThing {
                    thing_idx: idx,
                    amount,
                    inflictor_idx: None,
                });
            }
        }
        self.apply_commands(cmds);
    }
}

pub const DEFAULT_THING_DEFS: &[(u16, ThingDef)] = &[
    // Zombieman
    (
        3004,
        ThingDef {
            health: 20.0,
            speed: 8.0,
            radius: 20.0,
            height: 56.0,
            damage: 0,
            reaction_time: 8,
            pain_chance: 200,
            mass: 100,
        },
    ),
    // Imp
    (
        3001,
        ThingDef {
            health: 60.0,
            speed: 8.0,
            radius: 20.0,
            height: 56.0,
            damage: 3,
            reaction_time: 8,
            pain_chance: 200,
            mass: 100,
        },
    ),
    // Demon
    (
        3002,
        ThingDef {
            health: 150.0,
            speed: 10.0,
            radius: 30.0,
            height: 56.0,
            damage: 4,
            reaction_time: 8,
            pain_chance: 180,
            mass: 400,
        },
    ),
    // Baron
    (
        3003,
        ThingDef {
            health: 1000.0,
            speed: 8.0,
            radius: 24.0,
            height: 64.0,
            damage: 10,
            reaction_time: 8,
            pain_chance: 50,
            mass: 1000,
        },
    ),
    // Cacodemon
    (
        3005,
        ThingDef {
            health: 400.0,
            speed: 8.0,
            radius: 31.0,
            height: 56.0,
            damage: 5,
            reaction_time: 8,
            pain_chance: 128,
            mass: 400,
        },
    ),
    // Lost Soul
    (
        3006,
        ThingDef {
            health: 100.0,
            speed: 8.0,
            radius: 16.0,
            height: 56.0,
            damage: 3,
            reaction_time: 8,
            pain_chance: 255,
            mass: 50,
        },
    ),
    // Barrel
    (
        2035,
        ThingDef {
            health: 20.0,
            speed: 0.0,
            radius: 10.0,
            height: 32.0,
            damage: 0,
            reaction_time: 0,
            pain_chance: 0,
            mass: 100,
        },
    ),
    // Doom 2 Monsters
    // Archvile (64)
    (
        64,
        ThingDef {
            health: 700.0,
            speed: 15.0,
            radius: 20.0,
            height: 56.0,
            damage: 20,
            reaction_time: 8,
            pain_chance: 10,
            mass: 500,
        },
    ),
    // Chaingunner (65)
    (
        65,
        ThingDef {
            health: 70.0,
            speed: 8.0,
            radius: 20.0,
            height: 56.0,
            damage: 3,
            reaction_time: 8,
            pain_chance: 170,
            mass: 100,
        },
    ),
    // Revenant (66)
    (
        66,
        ThingDef {
            health: 300.0,
            speed: 10.0,
            radius: 20.0,
            height: 56.0,
            damage: 10,
            reaction_time: 8,
            pain_chance: 100,
            mass: 500,
        },
    ),
    // Mancubus (67)
    (
        67,
        ThingDef {
            health: 600.0,
            speed: 8.0,
            radius: 48.0,
            height: 64.0,
            damage: 20,
            reaction_time: 8,
            pain_chance: 80,
            mass: 1000,
        },
    ),
    // Arachnotron (68)
    (
        68,
        ThingDef {
            health: 500.0,
            speed: 12.0,
            radius: 64.0,
            height: 64.0,
            damage: 5,
            reaction_time: 8,
            pain_chance: 128,
            mass: 600,
        },
    ),
    // Hell Knight (69)
    (
        69,
        ThingDef {
            health: 500.0,
            speed: 8.0,
            radius: 24.0,
            height: 64.0,
            damage: 10,
            reaction_time: 8,
            pain_chance: 50,
            mass: 1000,
        },
    ),
    // Pain Elemental (71)
    (
        71,
        ThingDef {
            health: 400.0,
            speed: 8.0,
            radius: 31.0,
            height: 56.0,
            damage: 0,
            reaction_time: 8,
            pain_chance: 128,
            mass: 400,
        },
    ),
    // Spider Mastermind (7)
    (
        7,
        ThingDef {
            health: 3000.0,
            speed: 12.0,
            radius: 128.0,
            height: 100.0,
            damage: 3,
            reaction_time: 8,
            pain_chance: 40,
            mass: 1000,
        },
    ), // Large radius
    // Cyberdemon (16)
    (
        16,
        ThingDef {
            health: 4000.0,
            speed: 16.0,
            radius: 40.0,
            height: 110.0,
            damage: 20,
            reaction_time: 8,
            pain_chance: 20,
            mass: 1000,
        },
    ),
    // WolfSS (84)
    (
        84,
        ThingDef {
            health: 50.0,
            speed: 8.0,
            radius: 20.0,
            height: 56.0,
            damage: 3,
            reaction_time: 8,
            pain_chance: 170,
            mass: 100,
        },
    ),
];

pub fn init_world(_world: &mut WorldState) {}
