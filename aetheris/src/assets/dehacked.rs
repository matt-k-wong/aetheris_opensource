//! DEHACKED support for Doom modding
//! DEHACKED is a text format that allows modifying game behavior
//! without changing the engine code.

use std::collections::HashMap;

/// A DEHACKED patch containing modifications to game data
#[derive(Debug, Default)]
pub struct DehackedPatch {
    /// Thing (monster/item) modifications
    pub things: HashMap<u16, ThingPatch>,
    /// Weapon modifications
    pub weapons: HashMap<u16, WeaponPatch>,
    /// Frame/state modifications
    pub frames: HashMap<u16, FramePatch>,
    /// Sprite name replacements
    pub sprites: HashMap<String, String>,
    /// Text string replacements
    pub strings: HashMap<String, String>,
    /// Miscellaneous settings
    pub misc: MiscPatch,
}

/// Modifications to a thing (monster, item, etc.)
#[derive(Debug, Default, Clone)]
pub struct ThingPatch {
    pub id: u16,
    pub name: Option<String>,
    pub hit_points: Option<i32>,
    pub speed: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub mass: Option<i32>,
    pub damage: Option<i32>,
    pub reaction_time: Option<i32>,
    pub pain_chance: Option<i32>,
    pub flags: Option<u32>,
    pub splash_group: Option<u32>,
}

/// Modifications to a weapon
#[derive(Debug, Default, Clone)]
pub struct WeaponPatch {
    pub ammo_type: Option<u16>,
    pub deselect_frame: Option<u16>,
    pub bobbing_frame: Option<u16>,
    pub firing_frame: Option<u16>,
    pub ammo_per_shot: Option<i32>,
}

/// Modifications to a frame/state
#[derive(Debug, Default, Clone)]
pub struct FramePatch {
    pub sprite: Option<u16>,
    pub frame: Option<u16>,
    pub tics: Option<i32>,
    pub action: Option<String>,
    pub next_frame: Option<u16>,
    pub code_pointer: Option<String>,
}

/// Miscellaneous settings
#[derive(Debug, Default, Clone)]
pub struct MiscPatch {
    pub initial_bullets: Option<i32>,
    pub initial_shells: Option<i32>,
    pub initial_rockets: Option<i32>,
    pub initial_cells: Option<i32>,
    pub initial_health: Option<i32>,
    pub initial_armor: Option<i32>,
    pub max_soulsphere: Option<i32>,
    pub soulsphere_health: Option<i32>,
    pub megasphere_health: Option<i32>,
    pub god_mode_health: Option<i32>,
    pub idfa_armor: Option<i32>,
    pub idfa_armor_class: Option<i32>,
    pub idkfa_armor: Option<i32>,
    pub idkfa_armor_class: Option<i32>,
    pub bfg_cells_per_shot: Option<i32>,
    pub monsters_infight: Option<bool>,
}

impl DehackedPatch {
    /// Parse a DEHACKED patch from text content
    pub fn parse(content: &str) -> Result<Self, String> {
        let mut patch = DehackedPatch::default();
        let mut lines = content.lines().peekable();

        while let Some(line) = lines.next() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse section headers
            if line.starts_with("Thing ") {
                let id = line[6..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .parse::<u16>()
                    .map_err(|e| format!("Invalid Thing ID: {} in {:?}", e, line))?;
                let thing_patch = Self::parse_thing(&mut lines, id)?;
                patch.things.insert(id, thing_patch);
            } else if line.starts_with("Weapon ") {
                let id = line[7..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .parse::<u16>()
                    .map_err(|e| format!("Invalid Weapon ID: {} in {:?}", e, line))?;
                let weapon_patch = Self::parse_weapon(&mut lines, id)?;
                patch.weapons.insert(id, weapon_patch);
            } else if line.starts_with("Frame ") {
                let id = line[6..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .parse::<u16>()
                    .map_err(|e| format!("Invalid Frame ID: {} in {:?}", e, line))?;
                let frame_patch = Self::parse_frame(&mut lines, id)?;
                patch.frames.insert(id, frame_patch);
            } else if line.starts_with("[STRINGS]") {
                Self::parse_strings(&mut lines, &mut patch)?;
            } else if line.starts_with("[SPRITES]") {
                Self::parse_sprites(&mut lines, &mut patch)?;
            } else if line.starts_with("[MISC]") {
                Self::parse_misc(&mut lines, &mut patch)?;
            }
        }

        Ok(patch)
    }

    fn parse_thing<'a>(
        lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
        id: u16,
    ) -> Result<ThingPatch, String> {
        let mut patch = ThingPatch {
            id,
            ..Default::default()
        };

        while let Some(line) = lines.peek() {
            let line = line.trim();

            // End of thing section
            if line.is_empty()
                || line.starts_with('[')
                || line.starts_with("Thing ")
                || line.starts_with("Weapon ")
                || line.starts_with("Frame ")
            {
                break;
            }

            lines.next(); // Consume the line

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "Name" => patch.name = Some(value.to_string()),
                    "Hit points" => patch.hit_points = value.parse().ok(),
                    "Speed" => patch.speed = value.parse::<f32>().ok().map(|s| s / 65536.0),
                    "Width" => patch.width = value.parse::<f32>().ok().map(|w| w / 65536.0),
                    "Height" => patch.height = value.parse::<f32>().ok().map(|h| h / 65536.0),
                    "Mass" => patch.mass = value.parse().ok(),
                    "Damage" => patch.damage = value.parse().ok(),
                    "Reaction time" => patch.reaction_time = value.parse().ok(),
                    "Pain chance" => patch.pain_chance = value.parse().ok(),
                    "Flags" => patch.flags = value.parse::<u32>().ok(),
                    "Splash group" => patch.splash_group = value.parse().ok(),
                    _ => {}
                }
            }
        }

        Ok(patch)
    }

    fn parse_weapon<'a>(
        lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
        _id: u16,
    ) -> Result<WeaponPatch, String> {
        let mut patch = WeaponPatch::default();

        while let Some(line) = lines.peek() {
            let line = line.trim();

            if line.is_empty()
                || line.starts_with('[')
                || line.starts_with("Thing ")
                || line.starts_with("Weapon ")
                || line.starts_with("Frame ")
            {
                break;
            }

            lines.next();

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "Ammo type" => patch.ammo_type = value.parse().ok(),
                    "Deselect frame" => patch.deselect_frame = value.parse().ok(),
                    "Bobbing frame" => patch.bobbing_frame = value.parse().ok(),
                    "Firing frame" => patch.firing_frame = value.parse().ok(),
                    "Ammo per shot" => patch.ammo_per_shot = value.parse().ok(),
                    _ => {}
                }
            }
        }

        Ok(patch)
    }

    fn parse_frame<'a>(
        lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
        _id: u16,
    ) -> Result<FramePatch, String> {
        let mut patch = FramePatch::default();

        while let Some(line) = lines.peek() {
            let line = line.trim();

            if line.is_empty()
                || line.starts_with('[')
                || line.starts_with("Thing ")
                || line.starts_with("Weapon ")
                || line.starts_with("Frame ")
            {
                break;
            }

            lines.next();

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "Sprite number" => patch.sprite = value.parse().ok(),
                    "Sprite subnumber" => patch.frame = value.parse().ok(),
                    "Duration" => patch.tics = value.parse().ok(),
                    "Action" => patch.action = Some(value.to_string()),
                    "Next frame" => patch.next_frame = value.parse().ok(),
                    "Code pointer" => patch.code_pointer = Some(value.to_string()),
                    _ => {}
                }
            }
        }

        Ok(patch)
    }

    fn parse_strings<'a>(
        lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
        patch: &mut DehackedPatch,
    ) -> Result<(), String> {
        while let Some(line) = lines.peek() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('[') {
                break;
            }

            lines.next();

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                patch.strings.insert(key, value);
            }
        }
        Ok(())
    }

    fn parse_sprites<'a>(
        lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
        patch: &mut DehackedPatch,
    ) -> Result<(), String> {
        while let Some(line) = lines.peek() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('[') {
                break;
            }

            lines.next();

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                patch.sprites.insert(key, value);
            }
        }
        Ok(())
    }

    fn parse_misc<'a>(
        lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
        patch: &mut DehackedPatch,
    ) -> Result<(), String> {
        while let Some(line) = lines.peek() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('[') {
                break;
            }

            lines.next();

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "Initial bullets" => patch.misc.initial_bullets = value.parse().ok(),
                    "Initial shells" => patch.misc.initial_shells = value.parse().ok(),
                    "Initial rockets" => patch.misc.initial_rockets = value.parse().ok(),
                    "Initial cells" => patch.misc.initial_cells = value.parse().ok(),
                    "Initial health" => patch.misc.initial_health = value.parse().ok(),
                    "Initial armor" => patch.misc.initial_armor = value.parse().ok(),
                    "Max soulsphere" => patch.misc.max_soulsphere = value.parse().ok(),
                    "Soulsphere health" => patch.misc.soulsphere_health = value.parse().ok(),
                    "Megasphere health" => patch.misc.megasphere_health = value.parse().ok(),
                    "God mode health" => patch.misc.god_mode_health = value.parse().ok(),
                    "IDFA armor" => patch.misc.idfa_armor = value.parse().ok(),
                    "IDFA armor class" => patch.misc.idfa_armor_class = value.parse().ok(),
                    "IDKFA armor" => patch.misc.idkfa_armor = value.parse().ok(),
                    "IDKFA armor class" => patch.misc.idkfa_armor_class = value.parse().ok(),
                    "BFG cells per shot" => patch.misc.bfg_cells_per_shot = value.parse().ok(),
                    "Monsters infight" => {
                        patch.misc.monsters_infight =
                            Some(value == "1" || value.eq_ignore_ascii_case("true"))
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Load a DEHACKED patch from a file
    pub fn load(path: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read DEHACKED file: {}", e))?;
        Self::parse(&content)
    }
}

/// Apply a DEHACKED patch to the game state
pub struct DehackedApplier;

impl DehackedApplier {
    /// Apply patches to thing definitions
    pub fn apply_thing_patches(_world: &mut crate::simulation::WorldState, patch: &DehackedPatch) {
        // Store the patches in the world for runtime application
        // This would modify thing behavior as they're spawned
        for (_id, thing_patch) in &patch.things {
            // Apply thing modifications
            // This would need to hook into thing spawning/management
            if let Some(_name) = &thing_patch.name {
                // Could log or store renamed things
            }
            // Other properties would be applied when things are created or updated
        }
    }

    /// Apply weapon patches
    pub fn apply_weapon_patches(_world: &mut crate::simulation::WorldState, patch: &DehackedPatch) {
        for (_id, weapon_patch) in &patch.weapons {
            if let Some(ammo_per_shot) = weapon_patch.ammo_per_shot {
                // Modify weapon ammo consumption
                // This would need to integrate with weapon firing logic
                let _ = ammo_per_shot;
            }
        }
    }

    /// Apply text string patches (for endgame text, etc.)
    pub fn apply_string_patches(_patch: &DehackedPatch) {
        // String replacements would be applied as needed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_thing() {
        let content = r#"
Thing 1 (Player)
Hit points = 100
Speed = 65536
Width = 131072
Height = 1572864
"#;

        let patch = DehackedPatch::parse(content).unwrap();
        let player = patch.things.get(&1).unwrap();

        assert_eq!(player.hit_points, Some(100));
        assert_eq!(player.speed, Some(1.0)); // 65536 / 65536
        assert_eq!(player.width, Some(2.0)); // 131072 / 65536
        assert_eq!(player.height, Some(24.0)); // 1572864 / 65536
    }

    #[test]
    fn test_parse_misc() {
        let content = r#"
[MISC]
Initial bullets = 100
BFG cells per shot = 20
Monsters infight = 1
"#;

        let patch = DehackedPatch::parse(content).unwrap();

        assert_eq!(patch.misc.initial_bullets, Some(100));
        assert_eq!(patch.misc.bfg_cells_per_shot, Some(20));
        assert_eq!(patch.misc.monsters_infight, Some(true));
    }
}
