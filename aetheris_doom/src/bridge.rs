use crate::doom::DoomThingExt;
use aetheris::simulation::Thing;

pub struct PresentationMapper;

impl PresentationMapper {
    /// Get sprite name with animation frame based on game time.
    /// Uses the state machine's sprite+frame when available (for monsters/barrels with thinkers),
    /// falls back to kind-based lookup for static things.
    pub fn get_animated_sprite(
        thing: &Thing,
        player_pos: glam::Vec2,
        _frame_count: u64,
        world: &aetheris::simulation::WorldState,
    ) -> Vec<String> {
        // For things with state machine (monsters, barrels), use the state's sprite+frame directly
        let (sprite, frame) = if (thing.is_monster() || thing.is_barrel())
            && thing.state_idx < crate::doom::STATES.len()
        {
            let state = &crate::doom::STATES[thing.state_idx];
            (state.sprite.to_string(), state.frame)
        } else {
            // Static things: use kind-based sprite lookup
            let base = Self::get_static_sprite(thing);
            // base is like "CLIPA" or "COL1A" — first 4 chars are sprite, 5th is frame
            if base.len() >= 5 {
                let _sprite_name = &base[..4];
                let _frame_char = base.chars().nth(4).unwrap_or('A');
                // Need to return owned data, so use a static str approach
                return Self::build_candidates_static(base, thing, player_pos);
            } else {
                return vec![base.to_string()];
            }
        };

        // Build the sprite name: 4-char sprite + frame letter + rotation
        let base = format!("{}{}", sprite, frame);

        // Non-rotated sprites (effects, items) — just return with "0" suffix
        if !thing.is_monster() && !thing.is_barrel() {
            return vec![format!("{}0", base)];
        }

        // Dead things don't rotate
        if thing.health <= 0.0 {
            return vec![format!("{}0", base)];
        }

        // Calculate rotation for living monsters
        let dx = thing.position.x - player_pos.x;
        let dy = thing.position.y - player_pos.y;
        let angle_to_player = dy.atan2(dx);
        let monster_angle = thing.angle;

        let mut rel =
            (monster_angle - angle_to_player + std::f32::consts::PI) % (2.0 * std::f32::consts::PI);
        if rel < 0.0 {
            rel += 2.0 * std::f32::consts::PI;
        }

        let rot = (rel / (std::f32::consts::PI / 4.0)).round() as i32 % 8;
        let doom_rot = match rot {
            0 => 1,
            1 => 2,
            2 => 3,
            3 => 4,
            4 => 5,
            5 => 6,
            6 => 7,
            7 => 8,
            _ => 1,
        };

        vec![
            format!("{}{}", base, doom_rot),
            format!("{}1", base),
            format!("{}0", base),
        ]
    }

    /// Build candidates for static (non-state-machine) things
    fn build_candidates_static(base: &str, thing: &Thing, _player_pos: glam::Vec2) -> Vec<String> {
        if base.ends_with('0') {
            return vec![base.to_string()];
        }

        // Items and decorations don't need rotation
        if !thing.is_monster() {
            return vec![format!("{}0", base), base.to_string()];
        }

        vec![format!("{}0", base)]
    }

    /// Get sprite name for static things (no state machine) based on kind
    fn get_static_sprite(thing: &Thing) -> &'static str {
        match thing.kind {
            // Player and Monsters (static fallbacks)
            1 | 2 | 3 | 4 | 11 => "PLAYA0",
            10 | 12 => "PLAYN0",
            15 => "PLAYN0",
            18 => "POSSL0",
            19 => "SPOSL0",
            20 => "TROOL0",
            21 => "SARGN0",
            22 => "HEADL0",
            23 => "SKULK0",
            24 => "POL5A0",

            // Weapons
            2001 => "SHOTA0",
            2002 => "MGUNA0",
            2003 => "LAUNA0",
            2004 => "PLASA0",
            2005 => "CSAWA0",
            2006 => "BFUGA0",

            // Ammo
            2007 => "CLIPA0",
            2008 => "SHELA0",
            2010 => "ROCKA0",
            2011 => "STIMA0",
            2012 => "MEDIA0",
            2013 => "SOULA0",
            2046 => "AMMOA0",
            2047 => "SBOXA0",
            2048 => "BROKA0",
            2049 => "CELPA0",
            17 => "CELBA0",

            // Health / Armor / Powerups
            2014 => "BON1A0",
            2015 => "BON2A0",
            2018 => "ARM1A0",
            2019 => "ARM2A0",
            2022 => "PINVA0",
            2023 => "BPAKA0",
            2024 => "PVISA0",
            2025 => "SUITA0",
            2026 => "PMAPA0",
            2045 => "PVISA0",

            // Keys
            5 => "BKEYA0",
            40 => "BSKUA0",
            13 => "RKEYA0",
            38 => "RSKUA0",
            6 => "YKEYA0",
            39 => "YSKUA0",

            // Decorations & Obstacles
            2035 => "BAR1A0",
            8 => "BBRNA0",
            2028 => "COLUA0",
            2029 => "TLMPA0",
            30 => "COL1A0",
            31 => "COL2A0",
            32 => "COL3A0",
            33 => "COL4A0",
            35 => "CANDA0",
            36 => "COL5A0",
            37 => "COL6A0",
            41 => "CEYEA0",
            42 => "FSKUA0",
            43 => "GOR1A0",
            44 => "TBLUA0",
            45 => "TGRNA0",
            46 => "TREDA0",
            47 => "SMBTA0",
            48 => "ELECA0",
            49 => "GOR2A0",
            50 => "GOR3A0",
            51 => "GOR4A0",
            54 => "TRE2A0",
            55 => "SMBRA0",
            56 => "SMGRA0",
            57 => "SMRDA0",
            58 => "SARGA0",
            60 => "PLSSA0",
            70 => "BUR1A0",
            72 => "KEV1A0",
            73 => "HVC1A0",
            74 => "HVC2A0",
            75 => "HVC3A0",
            76 => "HVC4A0",
            77 => "HVC5A0",
            78 => "HVC6A0",
            79 => "HVC7A0",
            80 => "HVC8A0",
            81 => "HVC9A0",
            85 => "TLP2A0",
            86 => "TLP2B0",

            // Projectiles / Effects
            9999 => "BLUDA0",
            9998 => "PUFFA0",
            9997 => "BLUDA0",
            10031 => "BAL1A0", // Imp Fireball

            _ => {
                log::warn!(
                    "[Sprite] Missing sprite for thing kind {}, state {}, using UNKNA0",
                    thing.kind,
                    thing.state_idx
                );
                "UNKNA0"
            }
        }
    }
}
